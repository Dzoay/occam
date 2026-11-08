# Start empty

**Decided by the user, 2026-11-08:** "Start empty."

occam does not fork an existing kernel. `arris` and `solidcore` are used as pinned external test oracles and as possible
sources of test cases, run outside the workspace. Their source is not read for implementation.

Whether code from a permissively licensed kernel is worth porting is a later question, decided module by module by the
user. Anything ported would need a provenance audit first and would have to pass all of occam's gates.
