# `intersect.incomplete` has no remedy yet

**Decided by the user, 2026-11-15,** choosing "No remedy for now" when asked whether the report a section gives when a
method runs out of its budget should carry a new remedy, `use-budget`, that runs the section again with twice the
steps.

No agent can set an intersection's budget: intersection is not an operation an agent calls, and has no command or
schema of its own ([the plan](../plan/intersection.md)). A remedy that names one would be one no agent can follow, so
[`booleans`](../plan/booleans.md) would have had to translate it or drop it.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- `intersect.incomplete` keeps both its reasons, `budget-spent` and `pair-not-supported`, and the curves, contacts and
  budget it carries. Neither reason offers a remedy, as `measure.not-converged` offers none.
- No remedy kind is added to the catalogue for it.
- When `booleans` carries the report to an agent, it adds a remedy the agent can follow, through the request, under
  the rule that every code and remedy pair has a test that follows it (L2).
