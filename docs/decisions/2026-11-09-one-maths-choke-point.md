# One maths choke point, and measure what must be identical

**Decided by the user, 2026-11-09:** "it is a likely featur to compute on the browser and server the same model, but NOT a
necessary one initially. SO, having one choke point is the way forward and as you say testing what is really necessary."

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- Every number the kernel computes goes through `occam-math`, and the float bans in `clippy.toml` stay, so what a function
  does can change in one place without touching the rest of the kernel.
- What costs almost nothing stays promised: the same bits at any thread count, scalar and in lanes, and basic arithmetic
  identical on every target. [The contract](../DETERMINISM.md) and `gates determinism` hold it.
- Identical `sin`, `cos`, `atan2` and the like across targets is not assumed. [`proving-ground.s5`](../plan/proving-ground.md)
  measures what really needs to be identical and what that costs, and occam writes no such function before then.
- Computing the same model in the browser and on a server is a likely later feature, and the case that would make this
  necessary. It is not a requirement of the first release.
