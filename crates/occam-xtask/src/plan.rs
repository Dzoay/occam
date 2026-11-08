//! What `docs/PLAN.md` and the milestone files say, read rather than copied (one home per fact, L40, L41).

use std::collections::BTreeSet;

use serde::Serialize;

use crate::manifest::is_internal;

/// The plan.
pub const PLAN: &str = "docs/PLAN.md";

/// The lines of the `## ` section whose heading contains `word`.
fn section<'a>(text: &'a str, word: &'a str) -> impl Iterator<Item = &'a str> {
    let heading = move |line: &str| line.starts_with("## ") && line.contains(word);
    text.lines().skip_while(move |line| !heading(line)).skip(1).take_while(|line| !line.starts_with("## "))
}

/// The cells of a table row, or `None` for any other line.
pub fn cells(line: &str) -> Option<Vec<&str>> {
    let inner = line.trim().strip_prefix('|')?.strip_suffix('|')?;
    Some(inner.split('|').map(str::trim).collect())
}

/// The allowed crate layers, top first: the leading names of each line of section 4's diagram.
pub fn tiers(plan: &str) -> Vec<Vec<String>> {
    let fence = |line: &&str| line.starts_with("```");
    section(plan, "Layers")
        .skip_while(|line| !fence(line))
        .skip(1)
        .take_while(|line| !fence(line))
        .filter_map(|line| {
            let names = line.split("  ").next()?.split(',').map(str::trim).filter(|name| is_internal(name));
            let names: Vec<String> = names.map(str::to_string).collect();
            (!names.is_empty()).then_some(names)
        })
        .collect()
}

/// The kernel crates: every layer down to and including the composition crate.
pub fn kernel(tiers: &[Vec<String>]) -> BTreeSet<String> {
    let composition = |tier: &&Vec<String>| tier.iter().any(|name| name == "occam");
    let above = tiers.iter().take_while(|tier| !composition(tier));
    above.chain(tiers.iter().find(composition)).flatten().cloned().collect()
}

/// A row of section 6's table that has a milestone file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Milestone {
    pub name: String,
    /// Its file, relative to the repository root.
    pub file: String,
    /// The milestone it opens together with.
    pub alongside: Option<String>,
}

fn milestone(row: &str) -> Option<Milestone> {
    let cells = cells(row)?;
    let (name, what, detail) = (cells.first()?, cells.get(1)?, cells.get(2)?);
    let (_, target) = detail.split_once("](")?;
    let (target, _) = target.split_once(')')?;
    let alongside = what.split_once("Runs alongside `").and_then(|(_, rest)| rest.split_once('`'));
    Some(Milestone {
        name: name.trim_matches('`').to_string(),
        file: format!("docs/{target}"),
        alongside: alongside.map(|(name, _)| name.to_string()),
    })
}

/// The milestones in the order they run.
pub fn milestones(plan: &str) -> Vec<Milestone> {
    section(plan, "Milestones").filter_map(milestone).collect()
}

fn step_id(line: &str) -> Option<&str> {
    let rest = line.strip_prefix("### `").or_else(|| line.strip_prefix("| `"))?;
    let (id, _) = rest.split_once('`')?;
    let (milestone, slug) = id.split_once('.')?;
    let valid =
        |part: &str| !part.is_empty() && part.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    (valid(milestone) && valid(slug)).then_some(id)
}

/// Every step of a milestone file with its line number, in order (PLAN section 1 says what a step is).
pub fn steps(text: &str) -> Vec<(usize, String)> {
    let numbered = text.lines().zip(1..);
    numbered.filter_map(|(line, n)| Some((n, step_id(line)?.to_string()))).collect()
}

/// The `Plan-Step:` footers in a commit log or a PR body.
pub fn footers(log: &str) -> BTreeSet<String> {
    log.lines().filter_map(|line| line.trim().strip_prefix("Plan-Step: ")).map(|id| id.trim().to_string()).collect()
}

/// A milestone with the steps still written in its file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    pub milestone: Milestone,
    pub steps: Vec<String>,
}

impl Planned {
    /// Its steps not yet done, in order.
    pub fn remaining<'a>(&'a self, done: &'a BTreeSet<String>) -> impl Iterator<Item = &'a String> {
        self.steps.iter().filter(move |step| !done.contains(*step))
    }

    fn closed(&self, done: &BTreeSet<String>) -> bool {
        let finished = |id: &String| id.strip_prefix(&self.milestone.name).is_some_and(|rest| rest.starts_with('.'));
        self.remaining(done).next().is_none() && done.iter().any(finished)
    }
}

/// Every milestone with its steps, through `read` (a path to its text, `None` if absent).
pub fn planned(plan: &str, read: impl Fn(&str) -> Option<String>) -> Vec<Planned> {
    let load = |milestone: Milestone| {
        let text = read(&milestone.file).unwrap_or_default();
        Planned { milestone, steps: steps(&text).into_iter().map(|(_, id)| id).collect() }
    };
    milestones(plan).into_iter().map(load).collect()
}

/// The milestones open now and not yet closed. One opens when the one before it closes, or with the one it runs alongside.
pub fn open<'a>(planned: &'a [Planned], done: &BTreeSet<String>) -> Vec<&'a Planned> {
    let mut opened = BTreeSet::new();
    let mut found = Vec::new();
    let mut blocked = false;
    for item in planned {
        let is_open = item.milestone.alongside.as_ref().map_or(!blocked, |with| opened.contains(with));
        let closed = item.closed(done);
        if is_open {
            opened.insert(item.milestone.name.clone());
            if !closed {
                found.push(item);
            }
        }
        blocked |= !closed;
    }
    found
}

/// One line of `just next`.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Next {
    pub milestone: String,
    /// `None` when the milestone has no steps written yet.
    pub step: Option<String>,
}

/// For each open milestone, its first step not done.
pub fn next(planned: &[Planned], done: &BTreeSet<String>) -> Vec<Next> {
    let line =
        |item: &&Planned| Next { milestone: item.milestone.name.clone(), step: item.remaining(done).next().cloned() };
    open(planned, done).iter().map(line).collect()
}

/// The text form of `just next`.
pub fn render(next: &[Next]) -> String {
    let line = |n: &Next| match &n.step {
        Some(step) => format!("{}: {step}\n", n.milestone),
        None => format!("{}: no steps written yet: detail its milestone file (L43)\n", n.milestone),
    };
    next.iter().map(line).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const PLAN_TEXT: &str = "\
## 4. Layers (provisional)

```text
occam-report      Report, codes
occam-math        Real
occam             the composition crate
occam-wasm, occam-cli          thin facades
web/                           the renderer
```

## 6. Milestones

| Milestone | What | Detail |
|---|---|---|
| `alpha` | First | [plan/alpha.md](plan/alpha.md) |
| `beta` | Things. Runs alongside `alpha` | [plan/beta.md](plan/beta.md) |
| `gamma` | Then | [plan/gamma.md](plan/gamma.md) |
| outline one | Later | [plan/outline.md](plan/outline.md) (outline) |
| after that | Named below | none yet |

## 7. Other

| `alpha.not-a-step` | no | [x](y.md) |
";

    fn read(path: &str) -> Option<String> {
        match path {
            "docs/plan/alpha.md" => Some("### `alpha.one`\n\n| `alpha.two` | x |\n".into()),
            "docs/plan/beta.md" => Some("### `beta.one`: titled\n".into()),
            "docs/plan/gamma.md" => Some("### `gamma.one`\n".into()),
            "docs/plan/outline.md" => Some("| `maths` | no dot, not a step |\n".into()),
            _ => None,
        }
    }

    fn done(ids: &[&str]) -> BTreeSet<String> {
        ids.iter().map(ToString::to_string).collect()
    }

    fn next_of(ids: &[&str]) -> Vec<(String, Option<String>)> {
        let planned = planned(PLAN_TEXT, read);
        next(&planned, &done(ids)).into_iter().map(|n| (n.milestone, n.step)).collect()
    }

    fn line(milestone: &str, step: &str) -> (String, Option<String>) {
        (milestone.to_string(), Some(step.to_string()))
    }

    #[test]
    fn the_diagram_gives_the_tiers_and_the_kernel() {
        let tiers = tiers(PLAN_TEXT);
        let want = [vec!["occam-report"], vec!["occam-math"], vec!["occam"], vec!["occam-wasm", "occam-cli"]];
        assert_eq!(tiers, want.map(|tier| tier.into_iter().map(String::from).collect::<Vec<_>>()));
        assert_eq!(kernel(&tiers), done(&["occam", "occam-math", "occam-report"]));
    }

    #[test]
    fn no_composition_crate_means_an_empty_kernel_tail() {
        assert_eq!(kernel(&[]), BTreeSet::new());
        assert_eq!(tiers("## 5. Gates\n"), Vec::<Vec<String>>::new());
    }

    #[test]
    fn milestones_come_from_the_table_with_a_file() {
        let found = milestones(PLAN_TEXT);
        let names: Vec<&str> = found.iter().map(|m| m.name.as_str()).collect();
        assert_eq!(names, ["alpha", "beta", "gamma", "outline one"]);
        assert_eq!(found[0].file, "docs/plan/alpha.md");
        assert_eq!(found[0].alongside, None);
        assert_eq!(found[1].alongside.as_deref(), Some("alpha"));
    }

    #[test]
    fn a_step_is_a_heading_or_row_with_a_dotted_id() {
        let text = "### `a.b`: x\n\n| `c.d-1` | y |\n## `e.f`\n### `nodot`\n| `Up.per` | z |\n| `.x` | z |\n| `x.` | z |\nplain `g.h`\n### `broken\n";
        assert_eq!(steps(text), [(1, "a.b".to_string()), (3, "c.d-1".to_string())]);
    }

    #[test]
    fn a_row_needs_its_bars() {
        assert_eq!(cells("| a | b |"), Some(vec!["a", "b"]));
        assert_eq!(cells("a | b |"), None);
        assert_eq!(cells("| a | b"), None);
    }

    #[test]
    fn footers_are_collected_from_a_log() {
        let log = "docs: x\n\nPlan-Step: a.one\n  Plan-Step: a.two  \nnot Plan-Step: a.three\nCo-Authored-By: y\n";
        assert_eq!(footers(log), done(&["a.one", "a.two"]));
    }

    #[test]
    fn next_starts_with_the_milestones_open_together() {
        assert_eq!(next_of(&[]), [line("alpha", "alpha.one"), line("beta", "beta.one")]);
    }

    #[test]
    fn next_moves_past_done_steps() {
        assert_eq!(next_of(&["alpha.one"]), [line("alpha", "alpha.two"), line("beta", "beta.one")]);
    }

    #[test]
    fn the_next_milestone_waits_for_the_ones_before_it() {
        assert_eq!(next_of(&["alpha.one", "alpha.two"]), [line("beta", "beta.one")]);
        assert_eq!(next_of(&["beta.one"]), [line("alpha", "alpha.one")]);
    }

    #[test]
    fn a_closed_pair_opens_the_next_milestone() {
        assert_eq!(next_of(&["alpha.one", "alpha.two", "beta.one"]), [line("gamma", "gamma.one")]);
    }

    #[test]
    fn a_milestone_with_no_steps_written_is_open_not_closed() {
        let all = ["alpha.one", "alpha.two", "beta.one", "gamma.one"];
        assert_eq!(next_of(&all), [("outline one".to_string(), None)]);
    }

    #[test]
    fn a_finished_step_of_a_milestone_with_deleted_text_closes_it() {
        let empty = |path: &str| (path != "docs/plan/alpha.md").then(|| read(path)).flatten();
        let planned = planned(PLAN_TEXT, empty);
        let names =
            |ids: &[&str]| -> Vec<String> { next(&planned, &done(ids)).into_iter().map(|n| n.milestone).collect() };
        assert_eq!(names(&["alpha.one"]), ["beta"]);
        assert_eq!(names(&[]), ["alpha", "beta"]);
        assert_eq!(names(&["alphabet.one"]), ["alpha", "beta"]);
    }

    #[test]
    fn rendering_names_the_step_or_says_there_is_none() {
        let next =
            [Next { milestone: "a".into(), step: Some("a.one".into()) }, Next { milestone: "b c".into(), step: None }];
        assert_eq!(render(&next), "a: a.one\nb c: no steps written yet: detail its milestone file (L43)\n");
    }
}
