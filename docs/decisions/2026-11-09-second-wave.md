# The second wave of investigations starts now

**Decided by the user, 2026-11-09:** "start the second wave if we have comput resources to satisfy them, you can just queue
it up".

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- `proving-ground.s1`, `.s2`, `.s6`, `.p1`, `.p2` and `.p5` to `.p9` may start before the milestone opens, on the same terms as
  [the first six](2026-11-09-early-spikes.md): spike code and findings wait on the never-merged `spikes` branch, the findings
  PRs to `main` follow when `proving-ground` opens, and no gate changes.
- They are queued so that the measuring machine is not swamped. Those that build on `s1`'s prototype or on
  `census.baseline` (`s6`, `p1`, `p2`, `p6`, `p7` and `p8`) wait for it.
