//! The parts of `Cargo.toml` and `Cargo.lock` the gates read.

use std::collections::{BTreeMap, BTreeSet};

const SECTIONS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// The sections whose dependencies the crate is built with, not only its tests.
const BUILT: [&str; 2] = ["dependencies", "build-dependencies"];

/// One crate's manifest.
#[derive(Debug, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    /// Every dependency of any kind, by its real crate name.
    pub deps: BTreeSet<String>,
    /// Of those, the ones named only in `[dev-dependencies]`, which only its tests are built with.
    pub dev_only: BTreeSet<String>,
    /// Every feature it turns on, of a dependency or of its own.
    pub requests: Vec<Request>,
}

/// A feature a manifest turns on.
#[derive(Debug, PartialEq, Eq)]
pub struct Request {
    /// The crate whose feature it is, by its real name: a dependency, or the manifest's own crate.
    pub of: String,
    pub feature: String,
    /// The feature of the manifest's own crate that turns it on, or `None` where a dependency's entry asks for it.
    pub by: Option<String>,
}

/// Whether `path` is a workspace crate's manifest, and so not a lint probe's.
pub fn is_crate_manifest(path: &str) -> bool {
    path.strip_prefix("crates/").and_then(|p| p.strip_suffix("/Cargo.toml")).is_some_and(|name| !name.contains('/'))
}

/// Whether `name` is one of this workspace's crates.
pub fn is_internal(name: &str) -> bool {
    name == "occam" || name.starts_with("occam-")
}

/// The real name of the crate a dependency's entry, under `key`, names.
fn real<'a>(key: &'a str, entry: &'a toml::Value) -> &'a str {
    entry.get("package").and_then(toml::Value::as_str).unwrap_or(key)
}

/// The strings of the list under `key`.
fn strings<'a>(entry: &'a toml::Value, key: &str) -> impl Iterator<Item = &'a str> {
    entry.get(key).and_then(toml::Value::as_array).into_iter().flatten().filter_map(toml::Value::as_str)
}

fn names(table: &toml::Table, sections: &[&str], deps: &mut BTreeSet<String>) {
    for section in sections.iter().filter_map(|section| table.get(*section)?.as_table()) {
        for (key, value) in section {
            deps.insert(real(key, value).to_string());
        }
    }
}

/// The features `table`'s dependency entries ask for, and each entry's real name under the key it is written by.
fn asked(table: &toml::Table, keys: &mut BTreeMap<String, String>, requests: &mut Vec<Request>) {
    for section in SECTIONS.iter().filter_map(|section| table.get(*section)?.as_table()) {
        for (key, value) in section {
            let of = real(key, value);
            keys.insert(key.clone(), of.to_string());
            let asked = strings(value, "features");
            requests.extend(asked.map(|feature| Request {
                of: of.to_string(),
                feature: feature.to_string(),
                by: None,
            }));
        }
    }
}

/// The features `name`'s `[features]` turn on: `feature` is its own, `dep/feature` and `dep?/feature` a dependency's,
/// by the key it is written by, and `dep:name` turns on no feature.
fn turned_on(table: &toml::Table, name: &str, keys: &BTreeMap<String, String>, requests: &mut Vec<Request>) {
    let features = table.get("features").and_then(toml::Value::as_table).into_iter().flatten();
    for (by, list) in features {
        for item in list.as_array().into_iter().flatten().filter_map(toml::Value::as_str) {
            let (of, feature) = match item.split_once('/') {
                Some((key, feature)) => {
                    let key = key.trim_end_matches('?');
                    (keys.get(key).map_or(key, String::as_str), feature)
                }
                None if item.starts_with("dep:") => continue,
                None => (name, item),
            };
            requests.push(Request { of: of.to_string(), feature: feature.to_string(), by: Some(by.clone()) });
        }
    }
}

/// Parses a crate's manifest.
pub fn parse(text: &str) -> Result<Manifest, String> {
    let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let name = table.get("package").and_then(|p| p.get("name")).and_then(toml::Value::as_str);
    let name = name.ok_or("no [package] name")?.to_string();
    let (mut deps, mut built) = (BTreeSet::new(), BTreeSet::new());
    let (mut keys, mut requests) = (BTreeMap::new(), Vec::new());
    let targets = table.get("target").and_then(toml::Value::as_table).into_iter().flatten();
    for part in [Some(&table)].into_iter().chain(targets.map(|target| target.1.as_table())).flatten() {
        names(part, &SECTIONS, &mut deps);
        names(part, &BUILT, &mut built);
        asked(part, &mut keys, &mut requests);
    }
    turned_on(&table, &name, &keys, &mut requests);
    let dev_only = deps.difference(&built).cloned().collect();
    Ok(Manifest { name, deps, dev_only, requests })
}

/// Every package `Cargo.lock` fetches from elsewhere.
pub fn locked_external(text: &str) -> Result<BTreeSet<String>, String> {
    let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let packages = table.get("package").and_then(toml::Value::as_array).ok_or("no [[package]] entries")?;
    let external = packages.iter().filter(|p| p.get("source").is_some());
    Ok(external.filter_map(|p| p.get("name")?.as_str().map(str::to_string)).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn crate_manifests_are_one_level_under_crates() {
        assert!(is_crate_manifest("crates/occam-math/Cargo.toml"));
        assert!(!is_crate_manifest("crates/occam-xtask/probes/bans/Cargo.toml"));
        assert!(!is_crate_manifest("Cargo.toml"));
        assert!(!is_crate_manifest("crates/occam-math/Cargo.lock"));
    }

    #[test]
    fn internal_means_occam_or_an_occam_crate() {
        assert!(is_internal("occam"));
        assert!(is_internal("occam-math"));
        assert!(!is_internal("occamy"));
        assert!(!is_internal("serde"));
    }

    #[test]
    fn every_kind_of_dependency_counts() {
        let text = r#"
[package]
name = "occam-a"
[dependencies]
serde = "1"
renamed = { package = "real", version = "1" }
[dev-dependencies]
dev = "1"
[build-dependencies]
build = "1"
[target.'cfg(unix)'.dependencies]
unix = "1"
"#;
        let manifest = parse(text).unwrap();
        assert_eq!(manifest.name, "occam-a");
        let deps: Vec<&str> = manifest.deps.iter().map(String::as_str).collect();
        assert_eq!(deps, ["build", "dev", "real", "serde", "unix"]);
    }

    #[test]
    fn a_dependency_only_dev_dependencies_name_is_dev_only() {
        let text = r#"
[package]
name = "occam-a"
[dependencies]
both = "1"
[dev-dependencies]
both = "1"
dev = "1"
[target.'cfg(unix)'.dev-dependencies]
unix = "1"
[target.'cfg(unix)'.build-dependencies]
built = "1"
[target.'cfg(windows)'.dev-dependencies]
built = "1"
"#;
        let manifest = parse(text).unwrap();
        assert_eq!(manifest.dev_only.iter().map(String::as_str).collect::<Vec<_>>(), ["dev", "unix"]);
    }

    fn requests(text: &str) -> Vec<(String, String, Option<String>)> {
        parse(text).unwrap().requests.into_iter().map(|r| (r.of, r.feature, r.by)).collect()
    }

    fn request(of: &str, feature: &str, by: Option<&str>) -> (String, String, Option<String>) {
        (of.to_string(), feature.to_string(), by.map(str::to_string))
    }

    #[test]
    fn the_features_a_dependencys_entry_asks_for_are_requests() {
        let text = r#"
[package]
name = "occam-a"
[dependencies]
plain = "1"
serde = { version = "1", features = ["derive", "rc"] }
renamed = { package = "real", version = "1", features = ["x"] }
[dev-dependencies]
dev = { version = "1", features = ["d"] }
[target.'cfg(unix)'.build-dependencies]
unix = { version = "1", features = ["u"] }
"#;
        let want = [
            request("real", "x", None),
            request("serde", "derive", None),
            request("serde", "rc", None),
            request("dev", "d", None),
            request("unix", "u", None),
        ];
        assert_eq!(requests(text), want);
    }

    #[test]
    fn the_features_a_feature_turns_on_are_requests_by_it() {
        let text = r#"
[package]
name = "occam-a"
[dependencies]
serde = { version = "1", optional = true }
renamed = { package = "real", version = "1" }
[target.'cfg(unix)'.dependencies]
unix = { package = "nix", version = "1" }
[features]
default = ["fast"]
fast = ["dep:serde", "serde/derive", "renamed?/x", "unix/u", "unknown/y"]
"#;
        let want = [
            request("occam-a", "fast", Some("default")),
            request("serde", "derive", Some("fast")),
            request("real", "x", Some("fast")),
            request("nix", "u", Some("fast")),
            request("unknown", "y", Some("fast")),
        ];
        assert_eq!(requests(text), want);
    }

    #[test]
    fn a_manifest_without_a_name_or_valid_toml_is_refused() {
        assert_eq!(parse("[lib]\n").unwrap_err(), "no [package] name");
        assert!(parse("[").is_err());
    }

    #[test]
    fn only_fetched_packages_are_external() {
        let lock = "[[package]]\nname = \"occam-a\"\n[[package]]\nname = \"serde\"\nsource = \"registry\"\n";
        assert_eq!(locked_external(lock).unwrap().into_iter().collect::<Vec<_>>(), ["serde"]);
        assert_eq!(locked_external("version = 4\n").unwrap_err(), "no [[package]] entries");
        assert!(locked_external("[").is_err());
    }
}
