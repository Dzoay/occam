# Swept surfaces take unbounded curves

**Decided by the user, 2026-11-12,** asked as [`geometry.swept`](../plan/geometry.md) closed: surfaces of revolution and
of linear extrusion refused a curve that does not run between finite ends, so an infinite line had to be trimmed first.
FreeCAD's `Part.Line` is infinite, so a script can sweep one. The recommendation was to keep the refusal for now and
note the parity gap. The user chose the other option: support unbounded curves in this step.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- A revolution or an extrusion of any curve family is made, unbounded or not, and its domain is unbounded where the
  curve's range is. An unbounded patch gives all of space as its box and no B-spline, as an unbounded analytic
  surface does ([`to_bspline` takes a range](2026-11-11-to-bspline-over-a-range.md)).
- A foot is still found over a finite window: each curve family can say which of its parameters can come within a
  given distance of a point, and the swept surfaces search there.
