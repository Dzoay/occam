# occam-verify and occam-xtask may test against any crate

**Decided by the user, 2026-11-15,** choosing "Verify/xtask dev-deps on any" when asked where `intersection.hits`'
corpus test should reach the crates it tests. The test builds each corpus case's operands and runs every edge of one
against every face of the other, culled and unculled. [The layers](../PLAN.md#4-layers) put the corpus in
`occam-verify`, but under L25 every crate below `occam` binds `occam` alone, and `occam` does not expose
`occam-intersect`, or the topology and geometry the test reads. `intersection.timing` and `booleans`' corpus tests
need the same reach.

The other choices were to put the test in `occam`'s own tests, outside the crate the layers give the corpus to, or to
re-export intersection's internals through `occam`, widening the API every facade binds with calls no agent makes.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- `occam-verify` and `occam-xtask`, which are never published, may name any crate above them in the layers in their
  `[dev-dependencies]`. Their `[dependencies]` still bind `occam` alone.
- No other crate changes. `occam`'s public API stays as it is.
- `gates layers` holds this, and its test shows a dev-dependency allowed and the same dependency refused outside
  `[dev-dependencies]`.
