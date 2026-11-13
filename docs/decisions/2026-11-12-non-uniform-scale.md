# A transform may scale differently along three axes

**Decided by the user, 2026-11-12,** asked as [`construction.transform`](../plan/construction.md) began, with
[how geometry moves](2026-11-12-moving-geometry.md). The plan's step reads "rigid and scaling transforms of a body", and
its failing test refuses "a scale that is not positive", one number. The question as asked:

> I recommend one uniform factor. The plan's "a scale that is not positive" implies a single number, and a non-uniform
> scale turns circles into ellipses and spheres into shapes no family covers.

The user's answer: "what does FreeCAD offer here. I would like parity of not better capability so I think non-uniform is
my preferred option." FreeCAD 1.0's user documentation (not its code) was read to answer the first part: Part Scale
scales by one factor or by separate X, Y and Z factors, warning that "the shapes may be distorted", and Draft Scale also
takes X, Y and Z factors.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- `transform` and `copy` take `scale`: one number, or three factors along the X, Y and Z axes of an optional frame,
  `about`, whose origin stays where it is. Without `about` the factors are along the world's axes, about its origin, as
  FreeCAD's are; a frame of the request's own is the step past parity that costs little. Every factor is more than zero:
  a scale that is not is `input.scale-not-positive`, naming the factor. A mirror is not a scale.
- Rigid moves and a scale the same in every direction keep every curve and surface in its own form, exactly.
- A scale that differs keeps the forms closed under it: planes, lines, B-splines, meshes, and extrusions and trimmed
  curves of those. Cylinders, cones, spheres, tori, conics and revolutions become the exact rational B-splines
  `to_bspline` gives over the face's patch or the edge's range
  ([`to_bspline` over a range](2026-11-11-to-bspline-over-a-range.md)), mapped. A circle becomes a B-spline rather than
  an ellipse, for the reason [the seam decision](2026-11-12-moving-geometry.md) gives.
- An offset has no exact rational form under a stretch, and nothing is approximated yet: a body holding one, or a curve
  on a face that becomes a B-spline that is not a line of constant parameter there, is refused with
  `construct.no-exact-form`, naming it.
