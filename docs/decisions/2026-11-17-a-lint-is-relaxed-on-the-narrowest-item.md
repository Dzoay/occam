# A lint is relaxed on the narrowest item

**Decided by the user, 2026-11-17,** saying "amend the lint rule" when asked whether the root manifest's rule, that a
crate relaxes a lint only at its root, should be amended or the 62 `#[expect]`s on items that fell outside it should go
on a shrink-only list. Amending was the recommendation.

The rule is now: a crate relaxes a lint at its root, with a reason, or on the narrowest item that needs it, as an
`#[expect]` with a reason. An `expect` on an item covers less than an `allow` at the root would, and stops compiling
once nothing under it needs it. An `allow` away from a crate's root stays refused.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- `gates complexity` refuses a relaxation with no reason, and an `allow` anywhere but a crate's root or a module the
  root declares. It does not judge whether a reason is a good one: review does.
- The 62 read on 2026-11-17 were: 25 for integer arithmetic, 21 of them in tests or the tests' models; 10 for
  single-letter names that follow a source's notation; 8 for casts whose bound the reason states; 5 for floats
  compared exactly on purpose; and 14 others, such as a signature serde or clap fixes.
- Three relax panics or indexing in code a feature builds into a library: the tests' models in occam-topo and
  occam-mesh, on modules their roots declare. They go when the fixtures leave the kernel crates, which is the review
  backlog's item 9.
