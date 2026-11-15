# Risks

[Architecture](../ARCHITECTURE.md) › Risks

What the design is least sure of, and the measurement that would show it wrong.

| Belief | Shown wrong by |
|---|---|
| Fillets fit the records, though never prototyped. `blends` starts with the feasible-radius computation, the one part of the bracket test no spike touched | A fillet needing a change to the geometry seam after it froze. The records themselves can still grow: they do not freeze at the `topology` exit, and the file format is versioned |
| [Records](open-decisions.md#o22-how-topology-is-laid-out-in-memory) beat struct-of-arrays | A phase limited by traversing the store, such as the checker on 10⁶ faces scaling below 2× at six cores |
| [No cross-family store](geometry.md#no-cross-family-store) | NURBS-heavy work where lanes across instances are worth more than about 20% (O17) |
| [The binary twin](files.md) loads fast enough, JSON alone having been shown too slow at 1.445 s ([decision](../decisions/2026-11-12-binary-twin.md)) | Parsing 10⁵ faces from the twin taking over a second, timed apart from `check_all`, or browser workers spending more time parsing than meshing |
| [Descriptions inside reports](reports.md#how-an-agent-learns-what-an-entity-is) | Blind runs on the real kernel doing no better than S6's, or reports routinely passing a few kilobytes. [Construction's run](../evidence/2026-11-13-construction-blind-run.md): 8 of 8, but a failing run's output was 36.7 KB, nearly all of it the steps before the failure |
| [`Undecided`](maths.md#robust-decisions) is rare | More than about 1% of decisions reaching the exact path in the first booleans on the NIST parts. The exact stage would then need an arena (P6) |
| [Seams as edges](topology.md#entities) | STEP faces that cross a seam, or agents puzzled by a hole made of two faces. Never compared with a seam edge used twice |
| [1e-7 mm](open-decisions.md#o12-the-default-tolerance) as the default | Imported models with real features between 1e-7 and 1e-6 mm, or sizes where 1e-7 is too fine (O13) |
