# Assemblies join the work after the first release

**Decided by the user, 2026-11-11,** asked whether assemblies, parts placed as instances in a tree, should join the
list of work after the first release so they are not forgotten: "yes to both, add them to #42".

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- [The plan](../PLAN.md#6-milestones) names assemblies among the work after the first release. Like the rest of that
  list, they are planned when reached and need the user's go-ahead.
- No row of [the parity list](../PARITY.md) asks for them, and nothing in the first release changes.
- The design keeps them possible:
  - Instances share one immutable snapshot of their part.
  - `Placement` says where each instance sits.
  - The versioned report schema can put an instance path in front of an id.
  - Operations across parts use a transformed copy (PARITY 2.11), or the later placed family that
    [geometry](../architecture/geometry.md#no-cross-family-store) notes.
- One rule is left to settle then: each model has its own tolerance, so an operation across parts must say which
  governs.
