# `view` and `intersection` are open together

**Decided by the user, 2026-11-15:** "run view and intersection side by side". Until now only `bedrock` and `census`
were open together, and every other milestone opened when the one before it closed.

The review of build speed of 12 to 14 November, which led to [two tiers of
verification](2026-11-14-two-tiers-of-verification.md), found work queued in sequence that did not depend on each
other. The two milestone plans were then checked. [`view`](../plan/view.md) needs `construction`'s model and a new
crate, `occam-mesh`. [`intersection`](../plan/intersection.md) needs only geometry and maths, in a new crate,
`occam-intersect`. Neither needs the other.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- Both open now that `construction` has closed, and `just next` prints the first step of each.
- [`booleans`](../plan/booleans.md) needs `intersection` and not `view`, so it opens when `intersection` closes,
  whether or not `view` has.
- `exchange` and the milestones after it stay in order: each opens when every milestone before it has closed, `view`
  included. Whether `exchange` depends on any of them was not checked.
- The order is written in [the plan's milestone table](../PLAN.md#6-milestones), where `just next` and `gates plan`
  read it: a row says "Runs alongside" or "Opens when … closes".
