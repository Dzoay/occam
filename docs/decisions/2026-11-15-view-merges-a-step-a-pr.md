# `view` merges a step a PR

**Decided by the user, 2026-11-15,** choosing "A step a PR" when asked whether `view.edges` should merge on its own.
[`view`'s plan](../plan/view.md) had grouped its first eight steps, `view.edges` to `view.agent`, into one PR so that
the live demo reached `main` in one go, and its last two into a second.

That grouping was written beside [the decision that steps merge in small PRs](2026-11-14-steps-merge-in-small-prs.md),
which lets a milestone's file name a group only of steps that make sense only together. `view.edges` makes sense alone:
its crate, `occam-mesh`, is tested on its own, and each later step builds on what is merged.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- Each `view` step merges in its own PR, the demo's included. The demo reaches `main` with `view.agent`'s PR.
- The agent test's recipe runs once, on `view.agent`'s PR, its log kept as evidence.
