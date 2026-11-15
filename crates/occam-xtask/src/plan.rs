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
    /// The milestone whose closing opens it, whatever else is still open.
    pub after: Option<String>,
}

fn milestone(row: &str) -> Option<Milestone> {
    let cells = cells(row)?;
    let (name, what, detail) = (cells.first()?, cells.get(1)?, cells.get(2)?);
    let (_, target) = detail.split_once("](")?;
    let (target, _) = target.split_once(')')?;
    let named = |phrase: &str| {
        let (_, rest) = what.split_once(phrase)?;
        rest.split_once('`').map(|(name, _)| name.to_string())
    };
    Some(Milestone {
        name: name.trim_matches('`').to_string(),
        file: format!("docs/{target}"),
        alongside: named("Runs alongside `"),
        after: named("Opens when `"),
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

/// A milestone's Mermaid flowchart. Its nodes are its steps' slugs, and an arrow `a --> b` says that `b` needs `a`.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Diagram {
    /// Each node once, in the order they first appear.
    pub nodes: Vec<String>,
    /// Each arrow, from the step needed to the step that needs it.
    pub arrows: Vec<(String, String)>,
}

/// The lines inside each fenced Mermaid block of `text`, trimmed.
fn mermaid(text: &str) -> Vec<Vec<&str>> {
    let mut blocks = Vec::new();
    let mut lines = text.lines().map(str::trim);
    while lines.any(|line| line == "```mermaid") {
        blocks.push(lines.by_ref().take_while(|line| !line.starts_with("```")).collect());
    }
    blocks
}

/// The first Mermaid `flowchart` of a milestone file, `None` if it has none. Each line after its header is a chain of
/// groups joined by `-->`, a group being one node or several joined by `&`, and each node of a group points to each
/// node of the next: `a & b --> c --> d & e` is four arrows. Blank lines and `%%` comments are skipped.
pub fn diagram(text: &str) -> Option<Diagram> {
    let drawn = |line: &&str| !line.is_empty() && !line.starts_with("%%");
    let flowchart = |lines: &Vec<&str>| lines.first().is_some_and(|line| line.starts_with("flowchart"));
    let lines = mermaid(text).into_iter().map(|block| block.into_iter().filter(drawn).collect()).find(flowchart)?;
    let mut found = Diagram::default();
    for line in lines.iter().skip(1) {
        let group = |nodes: &str| nodes.split('&').map(|node| node.trim().to_string()).collect::<Vec<_>>();
        let groups: Vec<Vec<String>> = line.split("-->").map(group).collect();
        for node in groups.iter().flatten() {
            if !found.nodes.contains(node) {
                found.nodes.push(node.clone());
            }
        }
        for (from, to) in groups.iter().zip(groups.iter().skip(1)) {
            found.arrows.extend(from.iter().flat_map(|a| to.iter().map(move |b| (a.clone(), b.clone()))));
        }
    }
    Some(found)
}

impl Diagram {
    /// A node in `left` with an arrow into `node`.
    fn feeding<'a>(&'a self, node: &str, left: &BTreeSet<&str>) -> Option<&'a str> {
        let arrow = self.arrows.iter().find(|(from, to)| to == node && left.contains(from.as_str()));
        arrow.map(|(from, _)| from.as_str())
    }

    /// A cycle of its arrows, as the nodes along it from one back to itself, or `None` if it has none.
    pub fn cycle(&self) -> Option<Vec<&str>> {
        // Take away each node that nothing left points into, until none can go: what stays is on a cycle or after one.
        let mut left: BTreeSet<&str> = self.nodes.iter().map(String::as_str).collect();
        while let Some(free) = left.iter().copied().find(|node| self.feeding(node, &left).is_none()) {
            left.remove(free);
        }
        // Each node left has an arrow into it from another, so walking those back from any of them repeats a node.
        let mut path = vec![left.first().copied()?];
        loop {
            let from = self.feeding(path.last()?, &left)?;
            if let Some(start) = path.iter().position(|node| *node == from) {
                let mut cycle = path.split_off(start);
                cycle.push(from);
                cycle.reverse();
                return Some(cycle);
            }
            path.push(from);
        }
    }
}

/// A milestone with the steps still written in its file.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Planned {
    pub milestone: Milestone,
    pub steps: Vec<String>,
    /// Its diagram, which orders its steps. With none, the file's order does.
    pub diagram: Option<Diagram>,
}

impl Planned {
    /// Its steps not yet done, in order.
    pub fn remaining<'a>(&'a self, done: &'a BTreeSet<String>) -> impl Iterator<Item = &'a String> {
        self.steps.iter().filter(move |step| !done.contains(*step))
    }

    /// The id of its step `slug`.
    pub fn id(&self, slug: &str) -> String {
        format!("{}.{slug}", self.milestone.name)
    }

    /// The steps `step` needs: those with an arrow into it in its milestone's diagram or, with no diagram, every step
    /// written before it.
    pub fn needs(&self, step: &str) -> Vec<String> {
        match &self.diagram {
            Some(diagram) => {
                let into = diagram.arrows.iter().filter(|(_, to)| self.id(to) == step);
                into.map(|(from, _)| self.id(from)).collect()
            }
            None => self.steps.iter().take_while(|id| *id != step).cloned().collect(),
        }
    }

    /// The steps `step` needs that are still written and not done, leaving out those `claimed` with it. A need whose
    /// text is gone is finished (L42); `gates plan` names a node that never was a step.
    pub fn waiting(&self, step: &str, done: &BTreeSet<String>, claimed: &BTreeSet<&str>) -> Vec<String> {
        let remaining: BTreeSet<&str> = self.remaining(done).map(String::as_str).collect();
        let unmet = |need: &String| remaining.contains(need.as_str()) && !claimed.contains(need.as_str());
        self.needs(step).into_iter().filter(unmet).collect()
    }

    /// Its steps not done whose needs are all finished, in file order.
    pub fn ready(&self, done: &BTreeSet<String>) -> Vec<String> {
        let alone = BTreeSet::new();
        self.remaining(done).filter(|step| self.waiting(step, done, &alone).is_empty()).cloned().collect()
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
        Planned { milestone, steps: steps(&text).into_iter().map(|(_, id)| id).collect(), diagram: diagram(&text) }
    };
    milestones(plan).into_iter().map(load).collect()
}

/// The milestones open now and not yet closed. One opens with the one it runs alongside, or when the one it names closes,
/// or else when every one before it has closed.
pub fn open<'a>(planned: &'a [Planned], done: &BTreeSet<String>) -> Vec<&'a Planned> {
    let mut opened = BTreeSet::new();
    let mut finished = BTreeSet::new();
    let mut found = Vec::new();
    let mut blocked = false;
    for item in planned {
        let Milestone { name, alongside, after, .. } = &item.milestone;
        let is_open = match (alongside, after) {
            (Some(with), _) => opened.contains(with),
            (None, Some(before)) => finished.contains(before),
            (None, None) => !blocked,
        };
        let closed = item.closed(done);
        if is_open {
            opened.insert(name);
            if !closed {
                found.push(item);
            }
        }
        if closed {
            finished.insert(name);
        }
        blocked |= !closed;
    }
    found
}

/// One line of `just next`.
#[derive(Debug, PartialEq, Eq, Serialize)]
pub struct Next {
    pub milestone: String,
    /// Its steps that can start, in file order. Empty when it has no steps written yet, or when they all wait on a
    /// cycle in its diagram.
    pub steps: Vec<String>,
    /// Whether it has steps written but none can start.
    pub stuck: bool,
}

/// For each open milestone, every step not done whose needs are all finished.
pub fn next(planned: &[Planned], done: &BTreeSet<String>) -> Vec<Next> {
    let line = |item: &&Planned| {
        let steps = item.ready(done);
        let stuck = steps.is_empty() && item.remaining(done).next().is_some();
        Next { milestone: item.milestone.name.clone(), steps, stuck }
    };
    open(planned, done).iter().map(line).collect()
}

/// The text form of `just next`: a line a milestone, its steps that can start joined by commas.
pub fn render(next: &[Next]) -> String {
    let line = |n: &Next| match (n.steps.as_slice(), n.stuck) {
        ([], false) => format!("{}: no steps written yet: detail its milestone file (L43)\n", n.milestone),
        ([], true) => {
            format!("{}: no step can start: its diagram has a cycle, which `just gate plan` names\n", n.milestone)
        }
        (steps, _) => format!("{}: {}\n", n.milestone, steps.join(", ")),
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

    /// `construction`, `view` beside `intersection`, `booleans` after `intersection`, and one more in order.
    const SIDE_BY_SIDE: &str = "\
## 6. Milestones

| Milestone | What | Detail |
|---|---|---|
| `alpha` | First | [plan/alpha.md](plan/alpha.md) |
| `beta` | Second | [plan/beta.md](plan/beta.md) |
| `gamma` | Third. Runs alongside `beta` | [plan/gamma.md](plan/gamma.md) |
| `delta` | Fourth. Opens when `gamma` closes | [plan/delta.md](plan/delta.md) |
| `epsilon` | Fifth | [plan/epsilon.md](plan/epsilon.md) |
| `zeta` | Never. Opens when `omega` closes | [plan/zeta.md](plan/zeta.md) |
";

    fn side_by_side(ids: &[&str]) -> Vec<String> {
        let one_step = |path: &str| {
            let name = path.strip_prefix("docs/plan/")?.strip_suffix(".md")?;
            Some(format!("### `{name}.one`\n"))
        };
        next(&planned(SIDE_BY_SIDE, one_step), &done(ids)).into_iter().map(|n| n.milestone).collect()
    }

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

    fn next_of(ids: &[&str]) -> Vec<(String, Vec<String>)> {
        let planned = planned(PLAN_TEXT, read);
        next(&planned, &done(ids)).into_iter().map(|n| (n.milestone, n.steps)).collect()
    }

    fn line(milestone: &str, step: &str) -> (String, Vec<String>) {
        (milestone.to_string(), vec![step.to_string()])
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
        assert_eq!(next_of(&all), [("outline one".to_string(), vec![])]);
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
    fn a_milestone_names_the_one_whose_closing_opens_it() {
        let found = milestones(SIDE_BY_SIDE);
        let after: Vec<Option<&str>> = found.iter().map(|m| m.after.as_deref()).collect();
        assert_eq!(after, [None, None, None, Some("gamma"), None, Some("omega")]);
        assert_eq!(found[2].alongside.as_deref(), Some("beta"));
        assert_eq!(found[3].alongside, None);
    }

    #[test]
    fn two_milestones_side_by_side_open_when_the_one_before_them_closes() {
        assert_eq!(side_by_side(&[]), ["alpha"]);
        assert_eq!(side_by_side(&["alpha.one"]), ["beta", "gamma"]);
    }

    #[test]
    fn a_milestone_opens_when_the_one_it_names_closes_whatever_else_is_open() {
        assert_eq!(side_by_side(&["alpha.one", "gamma.one"]), ["beta", "delta"]);
        assert_eq!(side_by_side(&["alpha.one", "gamma.one", "delta.one"]), ["beta"]);
    }

    #[test]
    fn a_milestone_waits_for_the_one_it_names_and_no_other() {
        assert_eq!(side_by_side(&["alpha.one", "beta.one"]), ["gamma"]);
        assert_eq!(side_by_side(&["alpha.one", "beta.one", "gamma.one"]), ["delta"]);
    }

    #[test]
    fn a_milestone_that_names_none_waits_for_every_one_before_it() {
        let all = ["alpha.one", "beta.one", "gamma.one", "delta.one"];
        assert_eq!(side_by_side(&all), ["epsilon"]);
        assert_eq!(side_by_side(&[&all[..], &["epsilon.one"]].concat()), Vec::<String>::new());
    }

    #[test]
    fn rendering_names_the_step_or_says_there_is_none() {
        let line = |milestone: &str, steps: &[&str], stuck| Next {
            milestone: milestone.into(),
            steps: steps.iter().map(ToString::to_string).collect(),
            stuck,
        };
        let next = [line("a", &["a.one"], false), line("b c", &[], false)];
        assert_eq!(render(&next), "a: a.one\nb c: no steps written yet: detail its milestone file (L43)\n");
        assert_eq!(render(&[line("b", &["b.one", "b.two"], false)]), "b: b.one, b.two\n");
        let stuck = render(&[line("c", &[], true)]);
        assert!(stuck.starts_with("c: no step can start") && stuck.contains("cycle"), "{stuck}");
    }

    /// A milestone `m` whose file holds the steps `written`, in that order, and `lines` in a Mermaid flowchart.
    fn drawn(written: &[&str], lines: &[&str]) -> Planned {
        let headings = written.iter().map(|slug| format!("### `m.{slug}`\n\n")).collect::<Vec<_>>().concat();
        let arrows = lines.iter().map(|line| format!("  {line}\n")).collect::<Vec<_>>().concat();
        let text = format!("# m\n\n```mermaid\nflowchart LR\n{arrows}```\n\n## Steps\n\n{headings}");
        let milestone = Milestone { name: "m".into(), file: "docs/plan/m.md".into(), alongside: None, after: None };
        Planned { milestone, steps: steps(&text).into_iter().map(|(_, id)| id).collect(), diagram: diagram(&text) }
    }

    fn ready(planned: &Planned, finished: &[&str]) -> Vec<String> {
        planned.ready(&slugs(finished).into_iter().collect())
    }

    fn slugs(ids: &[&str]) -> Vec<String> {
        ids.iter().map(|slug| format!("m.{slug}")).collect()
    }

    fn arrows(pairs: &[(&str, &str)]) -> Vec<(String, String)> {
        pairs.iter().map(|(from, to)| (from.to_string(), to.to_string())).collect()
    }

    #[test]
    fn a_diagram_reads_chains_fan_out_and_fan_in() {
        let found = drawn(&[], &["a --> b --> c", "c --> d & e", "d & e --> f", "g"]).diagram.unwrap();
        assert_eq!(found.nodes, ["a", "b", "c", "d", "e", "f", "g"]);
        let want = [("a", "b"), ("b", "c"), ("c", "d"), ("c", "e"), ("d", "f"), ("e", "f")];
        assert_eq!(found.arrows, arrows(&want));
    }

    #[test]
    fn only_a_mermaid_flowchart_is_a_diagram() {
        assert_eq!(diagram("# m\n\n### `m.a`\n"), None);
        assert_eq!(diagram("```text\nflowchart LR\n  a --> b\n```\n"), None);
        assert_eq!(diagram("```mermaid\nsequenceDiagram\n  a ->> b: hi\n```\n"), None);
        let text = "```mermaid\npie\n```\n\n```mermaid\n\n%% the order\nflowchart TD\n\n  %% x first\n  x-->y\n```\n```mermaid\nflowchart LR\n  z\n```\n";
        assert_eq!(diagram(text), Some(Diagram { nodes: vec!["x".into(), "y".into()], arrows: arrows(&[("x", "y")]) }));
    }

    #[test]
    fn fan_out_and_fan_in_order_the_steps() {
        let planned = drawn(&["a", "c", "b", "d"], &["a --> b & c", "b & c --> d"]);
        assert_eq!(ready(&planned, &[]), slugs(&["a"]));
        assert_eq!(ready(&planned, &["a"]), slugs(&["c", "b"]));
        assert_eq!(ready(&planned, &["a", "b"]), slugs(&["c"]));
        assert_eq!(ready(&planned, &["a", "b", "c"]), slugs(&["d"]));
        assert_eq!(planned.needs("m.d"), slugs(&["b", "c"]));
    }

    #[test]
    fn a_chain_orders_its_steps() {
        let planned = drawn(&["a", "b", "c"], &["a --> b --> c"]);
        assert_eq!(ready(&planned, &[]), slugs(&["a"]));
        assert_eq!(ready(&planned, &["a"]), slugs(&["b"]));
        assert_eq!(planned.needs("m.c"), slugs(&["b"]));
    }

    #[test]
    fn a_finished_steps_node_without_a_heading_still_orders_the_rest() {
        let planned = drawn(&["b", "c"], &["a --> b & c"]);
        assert_eq!(ready(&planned, &["a"]), slugs(&["b", "c"]));
        assert_eq!(ready(&planned, &[]), slugs(&["b", "c"]));
    }

    #[test]
    fn a_milestone_with_no_diagram_keeps_the_file_order() {
        let planned = Planned { diagram: None, ..drawn(&["b", "a", "c"], &[]) };
        assert_eq!(ready(&planned, &[]), slugs(&["b"]));
        assert_eq!(ready(&planned, &["b"]), slugs(&["a"]));
        assert_eq!(planned.needs("m.c"), slugs(&["b", "a"]));
    }

    #[test]
    fn a_step_on_a_cycle_never_starts() {
        let planned = drawn(&["a", "b", "c"], &["a --> b --> c --> b"]);
        assert_eq!(ready(&planned, &["a"]), Vec::<String>::new());
        let found = next(&[planned], &done(&["m.a"]));
        assert_eq!(found, [Next { milestone: "m".into(), steps: vec![], stuck: true }]);
        let free = next(&[drawn(&["a", "b"], &["a --> b"])], &done(&[]));
        assert_eq!(free, [Next { milestone: "m".into(), steps: slugs(&["a"]), stuck: false }]);
        let unwritten = next(&[drawn(&[], &[])], &done(&[]));
        assert_eq!(unwritten, [Next { milestone: "m".into(), steps: vec![], stuck: false }]);
    }

    #[test]
    fn a_cycle_is_found_and_walked_in_the_arrows_direction() {
        let cycle = |lines: &[&str]| drawn(&[], lines).diagram.unwrap().cycle().map(|c| c.join(" "));
        assert_eq!(cycle(&["a --> b --> c --> a", "c --> d"]), Some("a b c a".into()));
        assert_eq!(cycle(&["b --> c --> b", "c --> a"]), Some("c b c".into()));
        assert_eq!(cycle(&["a --> a"]), Some("a a".into()));
        assert_eq!(cycle(&["a --> b & c", "b & c --> d"]), None);
        assert_eq!(cycle(&[]), None);
    }
}
