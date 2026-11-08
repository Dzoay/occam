//! `gates deps`: every third-party crate is on the allow-list with the user's decision behind it, and `cargo deny`,
//! `cargo shear` and REUSE pass. The kernel is std-only unless a line here says otherwise (AGENTS.md, O5).

use std::collections::BTreeSet;

use super::docs::decision_slugs;
use super::no_args;
use crate::manifest::{self, is_crate_manifest, is_internal};
use crate::repo::Repo;
use crate::verdict::{Finding, Verdict, verdict};

const LIST: &str = "ci/dependencies.txt";
const LOCK: &str = "Cargo.lock";
const TOOLCHAIN: &str = "scripts/toolchain.sh";

/// The version `scripts/toolchain.sh` pins for `reuse`.
fn reuse_version(toolchain: &str) -> Option<&str> {
    toolchain.lines().find_map(|line| line.strip_prefix("REUSE="))
}

pub fn run(repo: &Repo, args: &[String]) -> Verdict {
    no_args(args)?;
    let manifests = repo.texts(is_crate_manifest).map_err(|e| vec![e])?;
    let lock = repo.read(LOCK).map_err(|e| vec![e])?;
    let list = repo.read(LIST).map_err(|e| vec![e])?;
    let paths = repo.paths().map_err(|e| vec![e])?;
    let summary = check(&manifests, &lock, &list, &decision_slugs(&paths))?;
    repo.checked("deps.cargo-deny", env!("CARGO"), &["deny", "check"]).map_err(|e| vec![e])?;
    repo.checked("deps.cargo-shear", env!("CARGO"), &["shear"]).map_err(|e| vec![e])?;
    let toolchain = repo.read(TOOLCHAIN).map_err(|e| vec![e])?;
    let Some(version) = reuse_version(&toolchain) else {
        return Err(vec![Finding::new(
            "deps.no-reuse-version",
            TOOLCHAIN,
            "no REUSE= line",
            "pin reuse in scripts/toolchain.sh",
        )]);
    };
    repo.checked("deps.reuse", "uvx", &[&format!("reuse=={version}"), "lint"]).map_err(|e| vec![e])?;
    Ok(format!("{summary}; cargo deny, cargo shear and reuse pass"))
}

/// A line of the list: a crate, who may use it (`transitive` for anything in the lock), and the decision behind it.
struct Entry<'a> {
    name: &'a str,
    consumer: &'a str,
    decision: &'a str,
}

fn entries<'a>(list: &'a str, findings: &mut Vec<Finding>) -> Vec<Entry<'a>> {
    let mut found = Vec::new();
    for (line, n) in list.lines().zip(1..).filter(|(line, _)| !line.trim().is_empty() && !line.starts_with('#')) {
        let words: Vec<&str> = line.split_whitespace().collect();
        if let [name, consumer, decision] = words.as_slice() {
            found.push(Entry { name, consumer, decision });
        } else {
            findings.push(
                Finding::new(
                    "deps.malformed",
                    LIST,
                    "not `<crate> <consumer|transitive> <decision-slug>`",
                    "correct the line",
                )
                .at(n),
            );
        }
    }
    found
}

/// Every third-party dependency each crate names directly, as (dependency, consumer).
fn direct(
    manifests: &[(String, String)],
    allowed: &[Entry],
    findings: &mut Vec<Finding>,
) -> BTreeSet<(String, String)> {
    let mut pairs = BTreeSet::new();
    for (path, text) in manifests {
        let manifest = match manifest::parse(text) {
            Ok(manifest) => manifest,
            Err(error) => {
                findings.push(Finding::new("deps.unparsable", path, error, "fix the manifest"));
                continue;
            }
        };
        for dep in manifest.deps.iter().filter(|dep| !is_internal(dep)) {
            if !allowed.iter().any(|entry| entry.name == dep && entry.consumer == manifest.name) {
                let message = format!("{} depends on {dep}, which is not on the list for it", manifest.name);
                findings.push(Finding::new(
                    "deps.not-allowed",
                    path,
                    message,
                    format!("ask the user first, then add it to {LIST}"),
                ));
            }
            pairs.insert((dep.clone(), manifest.name.clone()));
        }
    }
    pairs
}

fn check(manifests: &[(String, String)], lock: &str, list: &str, decisions: &BTreeSet<String>) -> Verdict {
    let external = manifest::locked_external(lock)
        .map_err(|error| vec![Finding::new("deps.unparsable", LOCK, error, "regenerate Cargo.lock")])?;
    let mut findings = Vec::new();
    let entries = entries(list, &mut findings);
    let direct = direct(manifests, &entries, &mut findings);
    let listed: BTreeSet<&str> = entries.iter().map(|entry| entry.name).collect();
    for name in external.iter().filter(|name| !listed.contains(name.as_str())) {
        findings.push(Finding::new(
            "deps.not-allowed",
            LOCK,
            format!("{name} is locked but not on the list"),
            format!("ask the user first, then add it to {LIST}"),
        ));
    }
    for entry in &entries {
        let used = if entry.consumer == "transitive" {
            external.contains(entry.name)
        } else {
            direct.contains(&(entry.name.to_string(), entry.consumer.to_string()))
        };
        if !used {
            findings.push(Finding::new(
                "deps.stale",
                LIST,
                format!("{} {} is no longer used", entry.name, entry.consumer),
                "remove the line",
            ));
        }
        if !decisions.contains(entry.decision) {
            findings.push(Finding::new(
                "deps.no-decision",
                LIST,
                format!("no decision named {}", entry.decision),
                "record the user's decision in docs/decisions/",
            ));
        }
    }
    verdict(format!("{} third-party crates, all on the list", external.len()), findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verdict::codes;

    const LOCK_TEXT: &str = "[[package]]\nname = \"occam-xtask\"\n[[package]]\nname = \"serde\"\nsource = \"r\"\n[[package]]\nname = \"itoa\"\nsource = \"r\"\n";
    const GOOD_LIST: &str = "# allowed\n\nserde occam-xtask the-decision\nitoa transitive the-decision\n";

    fn manifests(deps: &[&str]) -> Vec<(String, String)> {
        let deps = deps.iter().map(|d| format!("{d} = \"1\"\n")).collect::<Vec<_>>().concat();
        vec![(
            "crates/occam-xtask/Cargo.toml".to_string(),
            format!("[package]\nname = \"occam-xtask\"\n[dependencies]\n{deps}"),
        )]
    }

    fn decisions() -> BTreeSet<String> {
        ["the-decision".to_string()].into_iter().collect()
    }

    fn run_check(deps: &[&str], lock: &str, list: &str) -> Verdict {
        check(&manifests(deps), lock, list, &decisions())
    }

    #[test]
    fn listed_dependencies_pass() {
        assert_eq!(
            run_check(&["serde", "occam"], LOCK_TEXT, GOOD_LIST),
            Ok("2 third-party crates, all on the list".to_string())
        );
    }

    #[test]
    fn a_direct_dependency_not_on_the_list_fails() {
        let found = run_check(&["serde", "rand"], LOCK_TEXT, GOOD_LIST).unwrap_err();
        assert_eq!(found[0].code, "deps.not-allowed");
        assert_eq!(found[0].file, "crates/occam-xtask/Cargo.toml");
        assert!(found[0].message.contains("rand"), "{found:?}");
        assert!(found[0].remedy.contains("ask the user"), "{found:?}");
    }

    #[test]
    fn a_dependency_listed_for_another_crate_fails() {
        let list = "serde occam-math the-decision\nitoa transitive the-decision\n";
        assert_eq!(codes(&run_check(&["serde"], LOCK_TEXT, list)), ["deps.not-allowed", "deps.stale"]);
    }

    #[test]
    fn a_locked_package_not_on_the_list_fails() {
        let found = run_check(&["serde"], LOCK_TEXT, "serde occam-xtask the-decision\n").unwrap_err();
        assert_eq!(found[0].code, "deps.not-allowed");
        assert_eq!(found[0].file, LOCK);
        assert!(found[0].message.contains("itoa"), "{found:?}");
    }

    #[test]
    fn an_entry_no_longer_used_is_stale() {
        let list = format!("{GOOD_LIST}gone transitive the-decision\n");
        let found = run_check(&["serde"], LOCK_TEXT, &list).unwrap_err();
        assert_eq!(found[0].code, "deps.stale");
        assert_eq!(found[0].file, LIST);
        assert!(found[0].message.contains("gone"), "{found:?}");
    }

    #[test]
    fn a_direct_entry_its_consumer_dropped_is_stale() {
        assert_eq!(codes(&run_check(&["occam"], LOCK_TEXT, GOOD_LIST)), ["deps.stale"]);
    }

    #[test]
    fn an_entry_needs_a_real_decision() {
        let list = "serde occam-xtask no-such\nitoa transitive the-decision\n";
        let found = run_check(&["serde"], LOCK_TEXT, list).unwrap_err();
        assert_eq!(found[0].code, "deps.no-decision");
        assert!(found[0].message.contains("no-such"), "{found:?}");
    }

    #[test]
    fn a_malformed_entry_fails() {
        for bad in ["serde occam-xtask", "serde", "a b c d"] {
            let list = format!("{GOOD_LIST}{bad}\n");
            let found = run_check(&["serde"], LOCK_TEXT, &list).unwrap_err();
            assert_eq!(found[0].code, "deps.malformed", "{bad}");
            assert_eq!(found[0].line, Some(5), "{bad}");
        }
    }

    #[test]
    fn broken_inputs_fail() {
        assert_eq!(codes(&run_check(&["serde"], "[", GOOD_LIST)), ["deps.unparsable"]);
        let broken = vec![("crates/a/Cargo.toml".to_string(), "[".to_string())];
        assert!(codes(&check(&broken, LOCK_TEXT, GOOD_LIST, &decisions())).contains(&"deps.unparsable"));
    }

    #[test]
    fn the_reuse_pin_comes_from_the_toolchain_script() {
        assert_eq!(reuse_version("A=1\nREUSE=6.2.0\n"), Some("6.2.0"));
        assert_eq!(reuse_version("A=1\n"), None);
    }
}
