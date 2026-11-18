//! `gates layers`: the crate graph matches the diagram in `docs/PLAN.md` section 4 (L18, L25). A crate depends only on
//! crates above it, and everything below the composition crate binds that crate and nothing else, but for the tooling
//! crates' tests: `occam-verify` and `occam-xtask` may name any crate above them in `[dev-dependencies]`
//! (`docs/decisions/2026-11-15-verify-and-xtask-test-against-any-crate.md`). And a feature only one crate may turn on is
//! turned on by no other manifest, since cargo builds a crate once for a whole workspace build, with every feature any
//! member asks of it, so no build of the workspace would notice.

use super::no_args;
use crate::manifest::{self, Request, is_crate_manifest, is_internal};
use crate::plan::{self, PLAN};
use crate::repo::Repo;
use crate::verdict::{Finding, Verdict, verdict};

/// The crates, never published, whose tests may reach any crate above them.
const TOOLING: [&str; 2] = ["occam-verify", "occam-xtask"];

/// The features of the workspace's crates that only one crate may turn on, with that crate: the open of a model without
/// its check is the browser build's alone (`docs/decisions/2026-11-16-mesh-workers-trust-the-editing-worker.md`).
const RESERVED: [(&str, &str); 1] = [("trusted", "occam-wasm")];

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

/// What is wrong with `name` turning a feature of a workspace crate on. A kernel crate, which is not below the composition
/// crate, may pass a reserved feature on from its own feature of the same name, which is how the one crate reaches it.
fn reserved(path: &str, name: &str, request: &Request, below: bool) -> Option<Finding> {
    let (feature, holder) = RESERVED.iter().find(|(feature, _)| *feature == request.feature)?;
    let passed_on = !below && request.by.as_deref() == Some(*feature);
    if name == *holder || passed_on {
        return None;
    }
    let message = format!("{name} turns on {}'s `{feature}`, which only {holder} may", request.of);
    let remedy = format!("leave `{feature}` to {holder}, and do here without what it adds");
    Some(Finding::new("layers.feature", path, message, remedy))
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
            let tested = TOOLING.contains(&manifest.name.as_str()) && manifest.dev_only.contains(dep);
            findings.extend(judge(path, &manifest.name, dep, (own, composition, tier_of(&tiers, dep)), tested));
        }
        let asked = manifest.requests.iter().filter(|request| is_internal(&request.of));
        findings.extend(asked.filter_map(|request| reserved(path, &manifest.name, request, own > composition)));
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

    /// `name`'s manifest with `body` after its `[package]`.
    fn written(name: &str, body: &str) -> (String, String) {
        (format!("crates/{name}/Cargo.toml"), format!("[package]\nname = \"{name}\"\n{body}"))
    }

    const ASKS: &str = "[dependencies]\noccam = { path = \"../occam\", features = [\"trusted\"] }\n";

    #[test]
    fn only_the_browser_build_may_ask_for_trusted() {
        assert_eq!(check(PLAN_TEXT, &[written("occam-wasm", ASKS)]), Ok("1 crates, 1 internal edges".to_string()));
        let found = check(PLAN_TEXT, &[written("occam-cli", ASKS)]).unwrap_err();
        assert_eq!(codes(&Err(found.clone())), ["layers.feature"]);
        assert_eq!(found[0].file, "crates/occam-cli/Cargo.toml");
        assert_eq!(found[0].message, "occam-cli turns on occam's `trusted`, which only occam-wasm may");
        assert!(found[0].remedy.contains("occam-wasm") && found[0].remedy.contains("`trusted`"), "{found:?}");
        let tested = ASKS.replace("[dependencies]", "[dev-dependencies]");
        assert_eq!(codes(&check(PLAN_TEXT, &[written("occam-cli", &tested)])), ["layers.feature"], "its tests");
        let other = ASKS.replace("trusted", "workloads");
        assert!(check(PLAN_TEXT, &[written("occam-cli", &other)]).is_ok(), "any other feature");
    }

    #[test]
    fn below_the_composition_crate_no_feature_passes_trusted_on() {
        let passed = "[dependencies]\noccam = \"1\"\n[features]\ntrusted = [\"occam/trusted\"]\n";
        assert_eq!(codes(&check(PLAN_TEXT, &[written("occam-cli", passed)])), ["layers.feature"]);
        assert_eq!(codes(&check(PLAN_TEXT, &[written("occam-xtask", passed)])), ["layers.feature"]);
        assert!(check(PLAN_TEXT, &[written("occam-wasm", passed)]).is_ok());
    }

    #[test]
    fn a_kernel_crate_passes_trusted_on_only_from_its_own_trusted() {
        let passed = "[dependencies]\noccam-math = \"1\"\n[features]\ntrusted = [\"occam-math/trusted\"]\n";
        assert_eq!(check(PLAN_TEXT, &[written("occam", passed)]), Ok("1 crates, 1 internal edges".to_string()));
        let found = check(PLAN_TEXT, &[written("occam", &passed.replace("\ntrusted = ", "\nfast = "))]).unwrap_err();
        assert_eq!(codes(&Err(found.clone())), ["layers.feature"]);
        assert_eq!(found[0].message, "occam turns on occam-math's `trusted`, which only occam-wasm may");
        let always = format!("{passed}default = [\"trusted\"]\n");
        let found = check(PLAN_TEXT, &[written("occam", &always)]).unwrap_err();
        assert_eq!(codes(&Err(found.clone())), ["layers.feature"], "on by default");
        assert_eq!(found[0].message, "occam turns on occam's `trusted`, which only occam-wasm may");
        let asked = "[dependencies]\noccam-math = { version = \"1\", features = [\"trusted\"] }\n";
        assert_eq!(codes(&check(PLAN_TEXT, &[written("occam", asked)])), ["layers.feature"], "asked for outright");
    }

    #[test]
    fn a_feature_called_trusted_of_a_crate_from_elsewhere_is_not_the_gates() {
        let elsewhere = "[dependencies]\nserde = { version = \"1\", features = [\"trusted\"] }\n";
        assert!(check(PLAN_TEXT, &[written("occam-cli", elsewhere)]).is_ok());
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
