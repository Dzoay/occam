# `to_bspline` takes a range

**Decided by the user, 2026-11-11,** asked as [`geometry.surfaces`](../plan/geometry.md) closed: the geometry topic
gave `to_bspline` no argument, but a plane, a cylinder and a cone are unbounded, and no B-spline is. The user chose the
recommended option: `to_bspline` takes the part to convert, `to_bspline(&self, patch: UvBox)` on surfaces and
`to_bspline(&self, lo, hi)` on curves. It is exact over any finite patch, and gives `None` where the patch is unbounded.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- "Exact" means the rational B-spline is the same set of points, rounding aside: a circle is rational quadratic arcs, a
  cylinder those arcs swept along a straight line, and so on. The B-spline's parameters run over the patch it was asked
  for, and agree with the family's at its knots, though not always between them: an arc's rational parameter is not its
  angle.
- A patch must be one the family could be trimmed to: inside the domain, its high end above its low end, and no more
  than one period of a parameter that comes round, as [`ParamRange::trimmed`](../../crates/occam-geom/src/domain.rs)
  already requires. Outside those, `to_bspline` gives `None`, as it does for an unbounded patch. More than a period would
  overlap itself, and would allocate in proportion to the patch.
- A B-spline family presents itself, restricted to the patch by knot insertion, so the same call works on every family.
