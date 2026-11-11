# Risks

[Architecture](../ARCHITECTURE.md) › Risks

What the design is least sure of, and the measurement that would show it wrong.

| Belief | Shown wrong by |
|---|---|
| Fillets fit the records, though never prototyped. `blends` starts with the feasible-radius computation, the one part of the bracket test no spike touched | A fillet needing a change to the geometry seam after it froze. The records themselves can still grow: they do not freeze at the `topology` exit, and the file format is versioned |
| [Records](open-decisions.md#o22-how-topology-is-laid-out-in-memory) beat struct-of-arrays | A phase limited by traversing the store, such as the checker on 10⁶ faces scaling below 2× at six cores |
| [No cross-family store](geometry.md#no-cross-family-store) | NURBS-heavy work where lanes across instances are worth more than about 20% (O17) |
| [JSON](files.md) as the only native format | Loading 10⁵ faces taking over a second, or browser workers spending more time parsing than meshing |
| [Descriptions inside reports](reports.md#how-an-agent-learns-what-an-entity-is) | Blind runs on the real kernel doing no better than S6's, or reports routinely passing a few kilobytes |
| [`Undecided`](maths.md#robust-decisions) is rare | More than about 1% of decisions reaching the exact path in the first booleans on the NIST parts. The exact stage would then need an arena (P6) |
| [Seams as edges](topology.md#entities) | STEP faces that cross a seam, or agents puzzled by a hole made of two faces. Never compared with a seam edge used twice |
| [1e-7 mm](open-decisions.md#o12-the-default-tolerance) as the default | Imported models with real features between 1e-7 and 1e-6 mm, or sizes where 1e-7 is too fine (O13) |
