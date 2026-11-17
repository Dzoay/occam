//! `gates layers`: the crate graph matches the diagram in `docs/PLAN.md` section 4 (L18, L25). A crate depends only on
//! crates above it, and everything below the composition crate binds that crate and nothing else, but for the tooling
//! crates' tests: `occam-verify` and `occam-xtask` may name any crate above them in `[dev-dependencies]`
//! (`docs/decisions/2026-11-15-verify-and-xtask-test-against-any-crate.md`), and `occam-fixtures` binds any crate above
//! it and is named by any crate, in `[dev-dependencies]` only (`docs/decisions/2026-11-17-occam-fixtures.md`).

use super::no_args;
use crate::manifest::{self, is_crate_manifest, is_internal};
use crate::plan::{self, PLAN};
use crate::repo::Repo;
use crate::verdict::{Finding, Verdict, verdict};

/// The crates, never published, whose tests may reach any crate above them.
const TOOLING: [&str; 2] = ["occam-verify", "occam-xtask"];

/// The crate, never published, that tests, benchmarks and probes build with: it binds any crate above it, and any crate
/// may name it, in `[dev-dependencies]` only (`docs/decisions/2026-11-17-occam-fixtures.md`).
const FIXTURES: &str = "occam-fixtures";

/// What is wrong with `name` naming the fixtures crate, given whether only its tests are built with it.
fn fixtures(path: &str, name: &str, tested: bool) -> Option<Finding> {
    let message = format!("{name} is built with {FIXTURES}, which only tests, benchmarks and examples may use");
    (!tested).then(|| Finding::new("layers.fixtures-shipped", path, message, "name it in `[dev-dependencies]` only"))
}

pub fn run(repo: &Repo, args: &[String]) -> Verdict {
    no_args(args)?;
    let plan = repo.read(PLAN).map_err(|e| vec![e])?;
    let manifests = repo.texts(is_crate_manifest).map_err(|e| vec![e])?;
    check(&plan, &manifests)
}

fn tier_of(tiers: &[Vec<String>], name: &str) -> Option<usize> {
    tiers.iter().position(|tier| tier.iter().any(|crate_name| crate_name == name))
}

/// What is wrong with `name` depending on `dep`, given its layer, the composition crate's and theirs, and whether only its
/// tests are built with it.
fn judge(path: &str, name: &str, dep: &str, layers: (usize, usize, Option<usize>), tested: bool) -> Option<Finding> {
    let (own, composition, theirs) = layers;
    if own > composition && dep != "occam" && !tested {
        let message = format!("{name} binds {dep}, but below the composition crate only `occam` may be bound (L25)");
        return Some(Finding::new("layers.bypass", path, message, format!("reach {dep} through `occam`")));
    }
    match theirs {
        Some(tier) if tier < own => None,
        _ => {
            let remedy = "move the shared code up, or agree a change to docs/PLAN.md section 4 with the user";
            Some(Finding::new("layers.upward", path, format!("{name} depends on {dep}, which is not above it"), remedy))
        }
    }
}

fn check(plan: &str, manifests: &[(String, String)]) -> Verdict {
    let tiers = plan::tiers(plan);
    let Some(composition) = tier_of(&tiers, "occam") else {
        let message = "section 4 has no layer diagram naming the composition crate";
        return Err(vec![Finding::new("layers.no-diagram", PLAN, message, "restore the diagram")]);
    };
    let mut findings = Vec::new();
    let mut edges = 0_usize;
    for (path, text) in manifests {
        let manifest = match manifest::parse(text) {
            Ok(manifest) => manifest,
            Err(error) => {
                findings.push(Finding::new("layers.manifest", path, error, "fix the manifest"));
                continue;
            }
        };
        let Some(own) = tier_of(&tiers, &manifest.name) else {
            let message = format!("{} is not in the diagram", manifest.name);
            findings.push(Finding::new(
                "layers.unlisted",
                path,
                message,
                "add it to docs/PLAN.md section 4, with the user's agreement",
            ));
            continue;
        };
        for dep in manifest.deps.iter().filter(|dep| is_internal(dep)) {
            edges = edges.saturating_add(1);
            if dep == FIXTURES && manifest.name != FIXTURES && tier_of(&tiers, FIXTURES).is_some() {
                findings.extend(fixtures(path, &manifest.name, manifest.dev_only.contains(dep)));
                continue;
            }
            let tested = TOOLING.contains(&manifest.name.as_str()) && manifest.dev_only.contains(dep);
            let tested = tested || manifest.name == FIXTURES;
            findings.extend(judge(path, &manifest.name, dep, (own, composition, tier_of(&tiers, dep)), tested));
        }
    }
    verdict(format!("{} crates, {edges} internal edges", manifests.len()), findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verdict::codes;

    const PLAN_TEXT: &str = "## 4. Layers\n\n```text\noccam-report      Report\noccam-math        Real\noccam             composition\noccam-wasm, occam-cli          facades\noccam-xtask                    gates\nweb/                           renderer\n```\n\n## 5. Gates\n";

    fn manifest(name: &str, deps: &[&str]) -> (String, String) {
        in_section(name, "dependencies", deps)
    }

    fn in_section(name: &str, section: &str, deps: &[&str]) -> (String, String) {
        let deps = deps.iter().map(|d| format!("{d} = \"1\"\n")).collect::<Vec<_>>().concat();
        (format!("crates/{name}/Cargo.toml"), format!("[package]\nname = \"{name}\"\n[{section}]\n{deps}"))
    }

    #[test]
    fn a_graph_that_follows_the_diagram_passes() {
        let manifests = [
            manifest("occam-report", &["serde"]),
            manifest("occam-math", &["occam-report"]),
            manifest("occam", &["occam-report", "occam-math"]),
            manifest("occam-cli", &["occam", "serde"]),
            manifest("occam-xtask", &["occam"]),
        ];
        assert_eq!(check(PLAN_TEXT, &manifests), Ok("5 crates, 5 internal edges".to_string()));
    }

    #[test]
    fn depending_on_a_lower_crate_is_upward() {
        let found = check(PLAN_TEXT, &[manifest("occam-report", &["occam-math"])]).unwrap_err();
        assert_eq!(found[0].code, "layers.upward");
        assert_eq!(found[0].file, "crates/occam-report/Cargo.toml");
        assert!(found[0].message.contains("occam-math"), "{found:?}");
    }

    #[test]
    fn depending_on_a_crate_in_the_same_layer_is_upward() {
        assert_eq!(codes(&check(PLAN_TEXT, &[manifest("occam-math", &["occam-math"])])), ["layers.upward"]);
    }

    #[test]
    fn the_composition_crate_may_use_every_kernel_layer() {
        assert!(check(PLAN_TEXT, &[manifest("occam", &["occam-report"])]).is_ok());
    }

    #[test]
    fn below_the_composition_crate_only_it_may_be_bound() {
        assert_eq!(codes(&check(PLAN_TEXT, &[manifest("occam-cli", &["occam-report"])])), ["layers.bypass"]);
        assert_eq!(codes(&check(PLAN_TEXT, &[manifest("occam-xtask", &["occam-cli"])])), ["layers.bypass"]);
    }

    #[test]
    fn the_tooling_crates_may_test_against_any_crate_above_them() {
        let verify = PLAN_TEXT.replace("occam-xtask                    gates", "occam-verify, occam-xtask      gates");
        for name in ["occam-verify", "occam-xtask"] {
            let tests = in_section(name, "dev-dependencies", &["occam-report", "occam-math", "occam-cli"]);
            assert_eq!(check(&verify, &[tests]), Ok("1 crates, 3 internal edges".to_string()), "{name}");
            assert_eq!(codes(&check(&verify, &[manifest(name, &["occam-math"])])), ["layers.bypass"], "{name}");
            let both = format!("{}[dev-dependencies]\noccam-math = \"1\"\n", manifest(name, &["occam-math"]).1);
            let both = (format!("crates/{name}/Cargo.toml"), both);
            assert_eq!(codes(&check(&verify, &[both])), ["layers.bypass"], "{name}: built with it, not only tested");
            let upward = in_section(name, "dev-dependencies", &[name]);
            assert_eq!(codes(&check(&verify, &[upward])), ["layers.upward"], "{name}: only crates above it");
        }
    }

    #[test]
    fn no_other_crate_below_the_composition_crate_may_test_against_a_kernel_crate() {
        let tests = in_section("occam-cli", "dev-dependencies", &["occam-report"]);
        assert_eq!(codes(&check(PLAN_TEXT, &[tests])), ["layers.bypass"]);
    }

    const WITH_FIXTURES: &str = "occam-fixtures, occam-xtask    fixtures, gates";

    fn plan_with_fixtures() -> String {
        PLAN_TEXT.replace("occam-xtask                    gates", WITH_FIXTURES)
    }

    #[test]
    fn any_crate_may_test_with_the_fixtures_crate_and_none_may_be_built_with_it() {
        let plan = plan_with_fixtures();
        for name in ["occam-report", "occam-math", "occam", "occam-cli", "occam-xtask"] {
            let tests = in_section(name, "dev-dependencies", &["occam-fixtures"]);
            assert_eq!(check(&plan, &[tests]), Ok("1 crates, 1 internal edges".to_string()), "{name}");
            for section in ["dependencies", "build-dependencies"] {
                let built = in_section(name, section, &["occam-fixtures"]);
                assert_eq!(codes(&check(&plan, &[built])), ["layers.fixtures-shipped"], "{name} in {section}");
            }
            let both = format!("{}[dev-dependencies]\noccam-fixtures = \"1\"\n", manifest(name, &["occam-fixtures"]).1);
            let both = (format!("crates/{name}/Cargo.toml"), both);
            assert_eq!(codes(&check(&plan, &[both])), ["layers.fixtures-shipped"], "{name}: built with it too");
        }
        let found = check(&plan, &[manifest("occam-math", &["occam-fixtures"])]).unwrap_err();
        assert_eq!(found[0].file, "crates/occam-math/Cargo.toml");
        assert!(found[0].message.contains("occam-math") && found[0].remedy.contains("[dev-dependencies]"), "{found:?}");
    }

    #[test]
    fn the_fixtures_crate_binds_any_crate_above_it_and_no_other() {
        let plan = plan_with_fixtures();
        let kernel = manifest("occam-fixtures", &["occam-report", "occam-math", "occam"]);
        assert_eq!(check(&plan, &[kernel]), Ok("1 crates, 3 internal edges".to_string()));
        for dep in ["occam-xtask", "occam-fixtures", "occam-new"] {
            let upward = manifest("occam-fixtures", &[dep]);
            assert_eq!(codes(&check(&plan, &[upward])), ["layers.upward"], "{dep}");
        }
        assert_eq!(codes(&check(PLAN_TEXT, &[manifest("occam-fixtures", &[])])), ["layers.unlisted"]);
        let unlisted = in_section("occam-math", "dev-dependencies", &["occam-fixtures"]);
        assert_eq!(codes(&check(PLAN_TEXT, &[unlisted])), ["layers.upward"], "a plan that does not list it");
    }

    #[test]
    fn a_dependency_on_an_unlisted_crate_is_upward() {
        assert_eq!(codes(&check(PLAN_TEXT, &[manifest("occam-math", &["occam-new"])])), ["layers.upward"]);
    }

    #[test]
    fn a_crate_missing_from_the_diagram_fails() {
        assert_eq!(codes(&check(PLAN_TEXT, &[manifest("occam-new", &[])])), ["layers.unlisted"]);
    }

    #[test]
    fn a_plan_without_the_diagram_fails_closed() {
        assert_eq!(codes(&check("## 5. Gates\n", &[manifest("occam-math", &[])])), ["layers.no-diagram"]);
    }

    #[test]
    fn a_broken_manifest_fails() {
        let broken = ("crates/a/Cargo.toml".to_string(), "[".to_string());
        assert_eq!(codes(&check(PLAN_TEXT, &[broken])), ["layers.manifest"]);
    }
}
