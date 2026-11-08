# The JSON of occam-report

**Decided by the user, 2026-11-08:** `occam-report` may depend on `serde` and `serde_json`. The approved plan said its JSON
would be written by hand and the crate std-only. The user advised against that: "I don't recomend cutting your own
serialization for example."

`occam-report` is the bottom kernel crate, so `serde`'s derive macro joins every kernel build at compile time. Every other
kernel crate stays std-only unless the user agrees otherwise, as O5 in
[the plan](../PLAN.md#7-decisions-the-user-still-owns) says.
