# Steps merge in small PRs, not one PR per milestone

**Decided by the user, 2026-11-14,** in answer to a measured proposal of seven changes to how work is checked: "lets do
all 7". This is one of them. It reverses the practice since 2026-11-11, when the user asked that a milestone's steps
share one PR with a commit each, so that CI ran once ([`maths`](../plan/maths.md)).

What was measured, 12 to 14 November:

- A milestone's diff is every step's diff, so each push made CI mutate all of it again. The geometry milestone's
  mutants shards took 4.5 to 6 hours per push.
- A branch that holds a whole milestone stays open for days, and was rebased onto `main` repeatedly while it did.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- A step merges in its own PR. A small group of steps shares a PR only where they make sense only together, and the
  milestone's file names the group.
- `construction` was built before this decision, as one branch with a commit per step. It still merges as one PR.
- The finished milestones' files say how they merged, and are not rewritten.
- The rule itself lives in [the plan](../PLAN.md#1-how-to-use-this-plan).
