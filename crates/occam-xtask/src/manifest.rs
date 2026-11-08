//! The parts of `Cargo.toml` and `Cargo.lock` the gates read.

use std::collections::BTreeSet;

const SECTIONS: [&str; 3] = ["dependencies", "dev-dependencies", "build-dependencies"];

/// One crate's manifest.
#[derive(Debug, PartialEq, Eq)]
pub struct Manifest {
    pub name: String,
    /// Every dependency of any kind, by its real crate name.
    pub deps: BTreeSet<String>,
}

/// Whether `path` is a workspace crate's manifest, and so not a lint probe's.
pub fn is_crate_manifest(path: &str) -> bool {
    path.strip_prefix("crates/").and_then(|p| p.strip_suffix("/Cargo.toml")).is_some_and(|name| !name.contains('/'))
}

/// Whether `name` is one of this workspace's crates.
pub fn is_internal(name: &str) -> bool {
    name == "occam" || name.starts_with("occam-")
}

fn names(table: &toml::Table, deps: &mut BTreeSet<String>) {
    for section in SECTIONS.iter().filter_map(|section| table.get(*section)?.as_table()) {
        for (key, value) in section {
            deps.insert(value.get("package").and_then(toml::Value::as_str).unwrap_or(key).to_string());
        }
    }
}

/// Parses a crate's manifest.
pub fn parse(text: &str) -> Result<Manifest, String> {
    let table: toml::Table = text.parse().map_err(|e: toml::de::Error| e.to_string())?;
    let name = table.get("package").and_then(|p| p.get("name")).and_then(toml::Value::as_str);
    let name = name.ok_or("no [package] name")?.to_string();
    let mut deps = BTreeSet::new();
    names(&table, &mut deps);
    for target in table.get("target").and_then(toml::Value::as_table).into_iter().flatten() {
        if let Some(target) = target.1.as_table() {
            names(target, &mut deps);
        }
    }
    Ok(Manifest { name, deps })
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
