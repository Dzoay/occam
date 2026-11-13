# An edge's deviation from its faces is capped

**Decided by the user, 2026-11-12,** asked during [`topology`](../plan/topology.md): `commit` measured how far each edge
strays from the surface of each face that uses it and stored it, as [the topology topic](../architecture/topology.md)
says, but refused no value, and the check that a parameter curve runs along its edge is widened by that same deviation.
So a face whose surface was replaced by one millimetres away committed. The recommendation was to refuse a deviation
above the band and the near-tangent shift for the edge's length, `sqrt(2 length band)` (S2 item 4); a fixed multiple of
the band, and recording without refusing, were the other options. The user said yes to the recommendation.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- An edge whose measured deviation exceeds `band + sqrt(2 length band)` is a `check.invariant-failed` with reason
  `edge-off-face`, naming the edge and its faces, with the distance, the limit and the length. At the default band
  (1e-7 mm) a 40 mm edge may stray about 3 µm, some 200 times the largest gap S1 saw on a right answer (1.4e-5 mm), so
  honest near-tangent sections pass and a misplaced curve or surface does not.
- The length is that of the chords between the points the deviation is measured at, which is never more than the arc,
  so the limit errs strict.
- What cannot be measured is refused rather than skipped: an edge whose distance from a face cannot be found is
  `deviation-not-measured`, and a loop whose points cannot be found on its face's surface is `loop-not-traced`.
- The whole-model check compares the deviation each record stores with the cap. `commit` is the only way to make a
  model and always measures first, so a stored deviation is never asserted; reading a model from a file, which arrives
  with `construction`, must measure it the same way.
