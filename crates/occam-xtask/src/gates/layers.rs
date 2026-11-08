//! `gates layers`: the crate graph matches the diagram in `docs/PLAN.md` section 4 (L18, L25). A crate depends only on
//! crates above it, and everything below the composition crate binds that crate and nothing else.

use super::no_args;
use crate::manifest::{self, is_crate_manifest, is_internal};
use crate::plan::{self, PLAN};
use crate::repo::Repo;
use crate::verdict::{Finding, Verdict, verdict};

pub fn run(repo: &Repo, args: &[String]) -> Verdict {
    no_args(args)?;
    let plan = repo.read(PLAN).map_err(|e| vec![e])?;
    let manifests = repo.texts(is_crate_manifest).map_err(|e| vec![e])?;
    check(&plan, &manifests)
}

fn tier_of(tiers: &[Vec<String>], name: &str) -> Option<usize> {
    tiers.iter().position(|tier| tier.iter().any(|crate_name| crate_name == name))
}

/// What is wrong with `name` depending on `dep`, given their layers and the composition crate's.
fn judge(path: &str, name: &str, dep: &str, own: usize, composition: usize, theirs: Option<usize>) -> Option<Finding> {
    if own > composition && dep != "occam" {
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
            findings.extend(judge(path, &manifest.name, dep, own, composition, tier_of(&tiers, dep)));
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
        let deps = deps.iter().map(|d| format!("{d} = \"1\"\n")).collect::<Vec<_>>().concat();
        (format!("crates/{name}/Cargo.toml"), format!("[package]\nname = \"{name}\"\n[dependencies]\n{deps}"))
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
