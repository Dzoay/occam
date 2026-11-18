# The shared helpers' names

**Decided by the user, 2026-11-18,** agreeing to the names proposed with items 11 and 12 of
[the review backlog](../plan/review-2026-11-17.md): the public items `occam_math::smaller`, `occam_math::larger` and
`occam_math::Orientation`, and the recipe `just catalogue-record`.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- `smaller` and `larger` are what code writes in place of `f64::min` and `f64::max`, which
  [rule 2 of the determinism rules](../DETERMINISM.md) bans. A crate does not keep its own copy.
- `Orientation` is the one orientation predicate. A crate that needs the side of a point takes it from occam-math.
- `just catalogue-record` writes `ci/catalogue.json` from the catalogue. A new or changed code is reviewed as a line of
  that file's diff.
