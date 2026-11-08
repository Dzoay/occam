//! `gates plan`: a step PR carries its `Plan-Step:` footers, and `just next` agrees (L40). Finished text leaves the plan
//! (L42), so the steps a PR deletes are the steps it claims.

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

/// Steps claimed out of the order `just next` gives: within a milestone the claimed steps must be its first, and it
/// must be open.
fn order(base: &[Planned], done: &BTreeSet<String>, claimed: &BTreeSet<&str>) -> Vec<Finding> {
    let open = plan::open(base, done);
    let mut findings = Vec::new();
    for item in base {
        let is_open = open.iter().any(|other| other.milestone.name == item.milestone.name);
        let here = item.remaining(done).filter(|id| claimed.contains(id.as_str())).count();
        for (position, id) in item.remaining(done).enumerate().filter(|(_, id)| claimed.contains(id.as_str())) {
            if !is_open || position >= here {
                let first = item.remaining(done).next().map_or("", String::as_str);
                let remedy = format!("`just next` says {first} comes first for {}", item.milestone.name);
                findings.push(Finding::new(
                    "plan.out-of-order",
                    PLAN,
                    format!("{id} is finished out of order"),
                    remedy,
                ));
            }
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

    #[test]
    fn the_body_flag_is_parsed() {
        let args = |list: &[&str]| list.iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(body_path(&[]), Ok(None));
        assert_eq!(body_path(&args(&["--pr-body", "f"])), Ok(Some(&"f".to_string())));
        assert!(body_path(&args(&["--pr-body"])).is_err());
        assert!(body_path(&args(&["--other", "f"])).is_err());
    }
}
