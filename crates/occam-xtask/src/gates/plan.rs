//! `gates plan`: a step PR carries its `Plan-Step:` footers, and `just next` agrees (L40). Finished text leaves the plan
//! (L42), so the steps a PR deletes are the steps it claims. A milestone's diagram orders its steps
//! (`docs/decisions/2026-11-15-steps-follow-their-milestones-diagram.md`), so it must draw each of them, and nothing else,
//! without a cycle.

use std::collections::BTreeSet;

use crate::plan::{self, PLAN, Planned, footers};
use crate::repo::Repo;
use crate::verdict::{Finding, Verdict, verdict};

fn body_path(args: &[String]) -> Result<Option<&String>, Vec<Finding>> {
    match args {
        [] => Ok(None),
        [flag, path] if flag == "--pr-body" => Ok(Some(path)),
        _ => Err(vec![Finding::new(
            "plan.usage",
            "",
            format!("unexpected arguments: {}", args.join(" ")),
            "pass --pr-body <file> or nothing",
        )]),
    }
}

pub fn run(repo: &Repo, args: &[String]) -> Verdict {
    let body = match body_path(args)? {
        Some(path) => Some(Repo::read_outside(path).map_err(|e| vec![e])?),
        None => None,
    };
    let base = repo.base().map_err(|e| vec![e])?;
    let done = footers(&repo.main_log().map_err(|e| vec![e])?);
    let head = plan::planned(&repo.read(PLAN).map_err(|e| vec![e])?, |path| repo.read(path).ok());
    let at_base = plan::planned(&repo.at(&base, PLAN).unwrap_or_default(), |path| repo.at(&base, path));
    check(&at_base, &head, &done, body.as_deref())
}

fn ids(planned: &[Planned]) -> BTreeSet<&str> {
    planned.iter().flat_map(|item| item.steps.iter().map(String::as_str)).collect()
}

/// Steps claimed out of the order `just next` gives: a claimed step's milestone must be open, and every step it needs
/// finished on `main` or claimed with it.
fn order(base: &[Planned], done: &BTreeSet<String>, claimed: &BTreeSet<&str>) -> Vec<Finding> {
    let open = plan::open(base, done);
    let mut findings = Vec::new();
    for item in base {
        let name = &item.milestone.name;
        let is_open = open.iter().any(|other| other.milestone.name == *name);
        for id in item.remaining(done).filter(|id| claimed.contains(id.as_str())) {
            let remedy = match (is_open, item.waiting(id, done, claimed).as_slice()) {
                (true, []) => continue,
                (false, _) => format!("{name} is not open yet: `just next` lists the milestones that are"),
                (true, waiting) => format!("finish {} first, on main or in this PR", waiting.join(", ")),
            };
            findings.push(Finding::new("plan.out-of-order", PLAN, format!("{id} is finished out of order"), remedy));
        }
    }
    findings
}

/// What is wrong with each milestone's diagram: a node that is no step of its milestone, neither written nor `known`
/// to be finished; a step written but not drawn; or a cycle, whose steps could never start.
fn diagram_findings(head: &[Planned], known: &BTreeSet<&str>) -> Vec<Finding> {
    let mut findings = Vec::new();
    for item in head {
        let Some(diagram) = &item.diagram else { continue };
        let (name, file) = (&item.milestone.name, item.milestone.file.as_str());
        for slug in diagram.nodes.iter().filter(|slug| !known.contains(item.id(slug).as_str())) {
            findings.push(Finding::new(
                "plan.diagram-unknown-step",
                file,
                format!("its diagram names `{slug}`, which is no step of {name}, written or finished"),
                format!("name a step by its slug, or write the step `{name}.{slug}`"),
            ));
        }
        let drawn: BTreeSet<String> = diagram.nodes.iter().map(|slug| item.id(slug)).collect();
        for id in item.steps.iter().filter(|id| !drawn.contains(*id)) {
            let slug = id.split_once('.').map_or(id.as_str(), |(_, slug)| slug);
            findings.push(Finding::new(
                "plan.diagram-missing-step",
                file,
                format!("{id} is not in its milestone's diagram"),
                format!("add `{slug}` to the diagram, with an arrow from each step it needs"),
            ));
        }
        if let Some(cycle) = diagram.cycle() {
            findings.push(Finding::new(
                "plan.diagram-cycle",
                file,
                format!("its diagram has a cycle, so no step on it can start: {}", cycle.join(" --> ")),
                "remove an arrow from the cycle, so that no step needs itself",
            ));
        }
    }
    findings
}

/// What a PR's footers say against the steps it deleted.
fn footer_findings(
    base: &[Planned],
    head: &BTreeSet<&str>,
    done: &BTreeSet<String>,
    body: &str,
) -> (usize, Vec<Finding>) {
    let claimed = footers(body);
    let at_base = ids(base);
    let removed: BTreeSet<&str> = at_base.difference(head).copied().collect();
    let mut findings = Vec::new();
    for id in removed.iter().filter(|id| !claimed.contains(**id)) {
        findings.push(Finding::new(
            "plan.missing-footer",
            PLAN,
            format!("this PR deletes {id} but has no footer for it"),
            format!("add `Plan-Step: {id}` to the PR body"),
        ));
    }
    for id in claimed.iter().filter(|id| !removed.contains(id.as_str())) {
        findings.push(Finding::new(
            "plan.footer-without-deletion",
            PLAN,
            format!("this PR claims {id} but does not delete its text"),
            "delete the finished step's text (L42), or drop the footer",
        ));
    }
    let matched: BTreeSet<&str> = claimed.iter().map(String::as_str).filter(|id| removed.contains(id)).collect();
    findings.extend(order(base, done, &matched));
    (removed.len(), findings)
}

fn check(base: &[Planned], head: &[Planned], done: &BTreeSet<String>, body: Option<&str>) -> Verdict {
    if head.is_empty() {
        return Err(vec![Finding::new(
            "plan.no-milestones",
            PLAN,
            "section 6 lists no milestones",
            "restore the table",
        )]);
    }
    let left = ids(head);
    let mut findings = Vec::new();
    for id in left.iter().filter(|id| done.contains(**id)) {
        findings.push(Finding::new(
            "plan.stale-step",
            PLAN,
            format!("{id} has a footer on main but its text remains"),
            "delete the finished step's text (L42)",
        ));
    }
    let known: BTreeSet<&str> = left.iter().copied().chain(ids(base)).chain(done.iter().map(String::as_str)).collect();
    findings.extend(diagram_findings(head, &known));
    let mut finished = 0;
    if let Some(body) = body {
        let (removed, more) = footer_findings(base, &left, done, body);
        finished = removed;
        findings.extend(more);
    }
    verdict(format!("{} steps left, {finished} finished by this PR", left.len()), findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verdict::codes;

    const PLAN_TEXT: &str = "## 6. Milestones\n\n| Milestone | What | Detail |\n|---|---|---|\n| `alpha` | a | [plan/alpha.md](plan/alpha.md) |\n| `beta` | b. Runs alongside `alpha` | [plan/beta.md](plan/beta.md) |\n| `gamma` | c | [plan/gamma.md](plan/gamma.md) |\n";

    /// The plan with `alpha`, `beta` and `gamma` holding the given step ids.
    fn planned(alpha: &[&str], beta: &[&str], gamma: &[&str]) -> Vec<Planned> {
        let text = |ids: &[&str]| ids.iter().map(|id| format!("### `{id}`\n")).collect::<Vec<_>>().concat();
        plan::planned(PLAN_TEXT, |path| match path {
            "docs/plan/alpha.md" => Some(text(alpha)),
            "docs/plan/beta.md" => Some(text(beta)),
            _ => Some(text(gamma)),
        })
    }

    const ALL: (&[&str], &[&str], &[&str]) = (&["alpha.one", "alpha.two"], &["beta.one"], &["gamma.one"]);

    fn base() -> Vec<Planned> {
        planned(ALL.0, ALL.1, ALL.2)
    }

    fn set(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn a_pr_that_is_not_a_step_passes_without_footers() {
        assert_eq!(
            check(&base(), &base(), &set(&[]), Some("Just docs.\n")),
            Ok("4 steps left, 0 finished by this PR".to_string())
        );
    }

    #[test]
    fn without_a_pr_body_only_the_tree_is_checked() {
        assert_eq!(check(&base(), &base(), &set(&[]), None), Ok("4 steps left, 0 finished by this PR".to_string()));
    }

    #[test]
    fn a_finished_step_deleted_with_its_footer_passes() {
        let head = planned(&["alpha.two"], ALL.1, ALL.2);
        assert_eq!(
            check(&base(), &head, &set(&[]), Some("Plan-Step: alpha.one\n")),
            Ok("3 steps left, 1 finished by this PR".to_string())
        );
    }

    #[test]
    fn deleting_a_step_without_its_footer_fails() {
        let head = planned(&["alpha.two"], ALL.1, ALL.2);
        let found = check(&base(), &head, &set(&[]), Some("No footer.\n")).unwrap_err();
        assert_eq!(found[0].code, "plan.missing-footer");
        assert!(found[0].message.contains("alpha.one"), "{found:?}");
        assert!(found[0].remedy.contains("Plan-Step: alpha.one"), "{found:?}");
    }

    #[test]
    fn a_footer_for_a_step_still_written_fails() {
        let found = check(&base(), &base(), &set(&[]), Some("Plan-Step: alpha.one\n")).unwrap_err();
        assert_eq!(found[0].code, "plan.footer-without-deletion");
        assert!(found[0].message.contains("alpha.one"), "{found:?}");
    }

    #[test]
    fn a_footer_for_no_step_at_all_fails() {
        assert_eq!(
            codes(&check(&base(), &base(), &set(&[]), Some("Plan-Step: alpha.made-up\n"))),
            ["plan.footer-without-deletion"]
        );
    }

    #[test]
    fn steps_must_finish_in_the_order_next_gives() {
        let head = planned(&["alpha.one"], ALL.1, ALL.2);
        let found = check(&base(), &head, &set(&[]), Some("Plan-Step: alpha.two\n")).unwrap_err();
        assert_eq!(found[0].code, "plan.out-of-order");
        assert!(found[0].message.contains("alpha.two"), "{found:?}");
        assert!(found[0].remedy.contains("alpha.one"), "{found:?}");
    }

    #[test]
    fn one_pr_may_finish_the_first_steps_together() {
        let head = planned(&[], ALL.1, ALL.2);
        assert!(check(&base(), &head, &set(&[]), Some("Plan-Step: alpha.one\nPlan-Step: alpha.two\n")).is_ok());
    }

    #[test]
    fn milestones_open_together_may_each_finish_a_step() {
        let head = planned(&["alpha.two"], &[], ALL.2);
        assert!(check(&base(), &head, &set(&[]), Some("Plan-Step: alpha.one\nPlan-Step: beta.one\n")).is_ok());
    }

    #[test]
    fn a_step_of_a_milestone_not_open_is_out_of_order() {
        let head = planned(ALL.0, ALL.1, &[]);
        assert_eq!(codes(&check(&base(), &head, &set(&[]), Some("Plan-Step: gamma.one\n"))), ["plan.out-of-order"]);
    }

    #[test]
    fn a_step_on_main_whose_text_remains_fails() {
        let found = check(&base(), &base(), &set(&["alpha.one"]), None).unwrap_err();
        assert_eq!(found[0].code, "plan.stale-step");
        assert!(found[0].message.contains("alpha.one"), "{found:?}");
    }

    #[test]
    fn a_finished_step_with_no_text_left_passes() {
        let head = planned(&["alpha.two"], ALL.1, ALL.2);
        assert!(check(&head, &head, &set(&["alpha.one"]), None).is_ok());
    }

    #[test]
    fn a_plan_with_no_milestones_fails_closed() {
        assert_eq!(codes(&check(&[], &[], &set(&[]), None)), ["plan.no-milestones"]);
    }

    /// `alpha`'s diagram: `one` before `two` and `three`, and both before `four`.
    const FAN: &[&str] = &["one --> two & three", "two & three --> four"];

    /// The plan with `alpha` holding the steps `written` and a diagram of `lines`, and `beta` and `gamma` as in `ALL`.
    fn drawn(written: &[&str], lines: &[&str]) -> Vec<Planned> {
        let headings = written.iter().map(|slug| format!("### `alpha.{slug}`\n")).collect::<Vec<_>>().concat();
        let arrows = lines.iter().map(|line| format!("  {line}\n")).collect::<Vec<_>>().concat();
        let alpha = format!("```mermaid\nflowchart LR\n{arrows}```\n\n{headings}");
        let text = |ids: &[&str]| ids.iter().map(|id| format!("### `{id}`\n")).collect::<Vec<_>>().concat();
        plan::planned(PLAN_TEXT, |path| match path {
            "docs/plan/alpha.md" => Some(alpha.clone()),
            "docs/plan/beta.md" => Some(text(ALL.1)),
            _ => Some(text(ALL.2)),
        })
    }

    fn fan(written: &[&str]) -> Vec<Planned> {
        drawn(written, FAN)
    }

    const FOUR: &[&str] = &["one", "two", "three", "four"];

    #[test]
    fn a_step_whose_needs_are_finished_on_main_may_be_claimed() {
        let (base, head) = (fan(&FOUR[1..]), fan(&["two", "four"]));
        assert_eq!(
            check(&base, &head, &set(&["alpha.one"]), Some("Plan-Step: alpha.three\n")),
            Ok("4 steps left, 1 finished by this PR".to_string())
        );
    }

    #[test]
    fn a_step_whose_needs_the_same_pr_finishes_may_be_claimed() {
        let both = check(
            &fan(FOUR),
            &fan(&["two", "four"]),
            &set(&[]),
            Some("Plan-Step: alpha.one\nPlan-Step: alpha.three\n"),
        );
        assert!(both.is_ok(), "{both:?}");
        let fan_in = check(
            &fan(&FOUR[1..]),
            &fan(&[]),
            &set(&["alpha.one"]),
            Some("Plan-Step: alpha.two\nPlan-Step: alpha.three\nPlan-Step: alpha.four\n"),
        );
        assert!(fan_in.is_ok(), "{fan_in:?}");
    }

    #[test]
    fn a_step_claimed_before_its_needs_is_out_of_order() {
        let found =
            check(&fan(FOUR), &fan(&["one", "two", "four"]), &set(&[]), Some("Plan-Step: alpha.three\n")).unwrap_err();
        assert_eq!(codes(&Err(found.clone())), ["plan.out-of-order"]);
        assert!(found[0].message.contains("alpha.three"), "{found:?}");
        assert!(found[0].remedy.contains("alpha.one"), "{found:?}");
        let body = "Plan-Step: alpha.two\nPlan-Step: alpha.four\n";
        let found = check(&fan(&FOUR[1..]), &fan(&["three"]), &set(&["alpha.one"]), Some(body)).unwrap_err();
        assert_eq!(codes(&Err(found.clone())), ["plan.out-of-order"]);
        assert!(found[0].message.contains("alpha.four") && found[0].remedy.contains("alpha.three"), "{found:?}");
        assert!(!found[0].remedy.contains("alpha.two"), "{found:?}");
    }

    #[test]
    fn a_finished_steps_node_without_a_heading_is_still_a_step() {
        let (on_main, here) = (fan(&FOUR[1..]), fan(&["three", "four"]));
        assert!(check(&on_main, &on_main, &set(&["alpha.one"]), None).is_ok());
        assert!(check(&fan(FOUR), &on_main, &set(&[]), None).is_ok());
        assert!(check(&on_main, &here, &set(&["alpha.one"]), None).is_ok());
    }

    #[test]
    fn a_diagram_node_that_is_no_step_fails() {
        let lines = [FAN, &["four --> five"]].concat();
        let found = check(&drawn(FOUR, &lines), &drawn(FOUR, &lines), &set(&[]), None).unwrap_err();
        assert_eq!(codes(&Err(found.clone())), ["plan.diagram-unknown-step"]);
        assert_eq!(found[0].file, "docs/plan/alpha.md");
        assert!(found[0].message.contains("`five`") && found[0].remedy.contains("alpha.five"), "{found:?}");
    }

    #[test]
    fn a_step_missing_from_its_diagram_fails() {
        let written = [FOUR, &["extra"]].concat();
        let found = check(&fan(&written), &fan(&written), &set(&[]), None).unwrap_err();
        assert_eq!(codes(&Err(found.clone())), ["plan.diagram-missing-step"]);
        assert_eq!(found[0].file, "docs/plan/alpha.md");
        assert!(found[0].message.contains("alpha.extra") && found[0].remedy.contains("`extra`"), "{found:?}");
    }

    #[test]
    fn a_cycle_in_a_diagram_fails() {
        let lines = ["one --> two --> one", "two --> three --> four"];
        let found = check(&drawn(FOUR, &lines), &drawn(FOUR, &lines), &set(&[]), None).unwrap_err();
        assert_eq!(codes(&Err(found.clone())), ["plan.diagram-cycle"]);
        assert_eq!(found[0].file, "docs/plan/alpha.md");
        assert!(found[0].message.contains("two --> one --> two"), "{found:?}");
    }

    #[test]
    fn a_milestone_with_no_diagram_keeps_the_file_order() {
        let head = planned(&["alpha.one"], ALL.1, ALL.2);
        assert_eq!(codes(&check(&base(), &head, &set(&[]), Some("Plan-Step: alpha.two\n"))), ["plan.out-of-order"]);
        let drawn_head = drawn(&["one"], &["two", "one"]);
        assert!(
            check(&drawn(&["one", "two"], &["two", "one"]), &drawn_head, &set(&[]), Some("Plan-Step: alpha.two\n"))
                .is_ok()
        );
    }

    #[test]
    fn the_body_flag_is_parsed() {
        let args = |list: &[&str]| list.iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(body_path(&[]), Ok(None));
        assert_eq!(body_path(&args(&["--pr-body", "f"])), Ok(Some(&"f".to_string())));
        assert!(body_path(&args(&["--pr-body"])).is_err());
        assert!(body_path(&args(&["--other", "f"])).is_err());
    }
}
