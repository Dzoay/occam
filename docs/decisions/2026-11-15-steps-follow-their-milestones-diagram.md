# Steps follow their milestone's diagram, not the file's order

**Decided by the user, 2026-11-15,** choosing "Gate follows the diagram" when asked how a milestone's steps should be
ordered. Until now `just next` gave each open milestone's first unfinished step in its file, and `gates plan` let a PR
finish only the first steps in that order (`plan.out-of-order`).

With [a step a PR](2026-11-14-steps-merge-in-small-prs.md) and independent work [run beside other
work](2026-11-15-view-and-intersection-together.md), that order held finished work back that its milestone's diagram
shows as independent. On the day of the decision `intersection.trace` was finished but could not merge before
`intersection.closed` and `intersection.quadrics`, `intersection.hits` would have waited for `intersection.resolution`,
and `view.renderer` for `view.workers`.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- A milestone's Mermaid flowchart is the order of its steps: an arrow `a --> b` says `b` needs `a`. A step may be
  finished once every step it needs is finished on `main` or in the same PR.
- `just next` lists, for each open milestone, every step whose needs are all finished.
- A milestone with no diagram keeps the file's order.
- Milestones still open and close by [the plan's milestone table](../PLAN.md#6-milestones).
