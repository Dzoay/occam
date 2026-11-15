# Curves and surfaces move through the seam

**Decided by the user, 2026-11-12,** asked as [`construction.transform`](../plan/construction.md) began: moving or
scaling a body means moving every curve, surface and parameter curve in it, and the frozen seam of `occam-geom` had no
way to do that. Each family's numbers are private to it, and [`Placement`](../architecture/units.md) is rigid only. The
question as asked:

> Option A (recommended): add `moved` to the frozen API: `moved(&self, by)` on the curve, surface and parameter-curve
> traits, an `Arc<dyn …>` version for each, and a map type in occam-units. Every family implements it exactly (axes
> moved, lengths scaled, spline poles mapped, offsets, trimmed, revolutions, extrusions and meshes through their bases),
> a conformance check runs over every family (moving then evaluating gives the same points as evaluating then moving),
> and the seam is re-recorded under a new decision file.
>
> Option B: no change to the frozen API. occam-ops reads each family's description (`shape()`: origin, normal, x-axis
> and sizes, by name) and rebuilds it with the family's public constructor, B-splines through `to_bspline`; families it
> can't rebuild are refused with a new code. Smaller, but a second list of every family outside occam-geom, depending
> on the value names in descriptions.

The answer, as the session that asked relayed it: "add the moving operation to the frozen API (your option A)". How far
a map may stretch is [its own decision](2026-11-12-non-uniform-scale.md).

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- `occam-units` gains `Affine`, a map of `World` to itself that keeps its sense: a rigid placement, a scale the same in
  every direction or along the axes of a frame, or these one after another. Where it is a similarity it carries the
  placement and the scale as given (`Similar`), so a family keeps its form under it by its own turn, not by a product of
  matrices.
- The seam gains `ParamMap` and `UvMap`, how a curve's parameter and a surface's parameters move, and `moved` on its six
  traits: `Parametric`, `ParametricCurve` and `ParametricParamCurve` give their own family and the parameter map, and
  `Surface`, `Curve` and `ParamCurve` give it shared. A family gives `None` where it does not keep its form under the
  map, and the default is `None`, as for `analytic` and `to_bspline`, so a new family edits no list.
- The conformance suite's new check, `Moves`, holds every family to keeping its form under a rigid map and under a scale
  up and down, and holds any family that keeps its form under a stretch to being exact the same way. `every_family.rs`
  runs it over every family, and names which ones a stretch keeps.
- A family keeps its own form only: a circle that a stretch would make an ellipse gives `None`, because one family
  making another would name it, which the seam's source check forbids. Its B-spline is exact, and the native format
  saves it.
- `ci/seam.txt` is re-recorded under this file.
