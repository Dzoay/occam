# The view milestone's figures on real GPUs are goals, not gates

**Decided by the user, 2026-11-13,** asked [O30](../PLAN.md#7-decisions-the-user-still-owns) while the
[`view`](../plan/view.md) milestone was drafted: whether its figures on real GPUs are a gate or a goal, and what the
target for a pick's time is. The answer was the recommended option.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- CI has no GPU, so these figures are goals. They are measured on two real GPUs, one discrete and one integrated, in
  Chromium and Firefox, and recorded in the milestone's PR, as [P12's](../evidence/2026-11-09-p12.md) were. Missing
  one is reported in the PR for the user to weigh. It does not fail CI.
- **Write-through:** 60 Hz with a whole face re-evaluated every frame, at 1,000,000 and 4,000,000 points on the
  discrete GPU and at 1,000,000 on the integrated one, with under 1 ms a frame on the main thread. 60 Hz is P12's rule:
  a median frame of at most 17.5 ms and a 95th percentile of at most 25 ms. P12 measured write-through at these sizes,
  with 0.1 to 0.2 ms a frame on the main thread.
- **A pick:** within one frame, 16.7 ms, on [P9's](../evidence/2026-11-09-p9.md) 10,001-face model. **No measurement
  grounds this target.** Neither P9 nor P12 timed a pick.
- No runner with a GPU is needed for this, and no account is added.
