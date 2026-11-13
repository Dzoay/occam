# Quadric pairs with no closed form are intersected exactly, by Levin's pencil method

**Decided by the user, 2026-11-13,** asked [O31](../PLAN.md#7-decisions-the-user-still-owns) while the
[`intersection`](../plan/intersection.md) milestone was drafted. The question was whether quadric pairs with no closed
form, such as a sphere or a cone with a cylinder off their common axis, stay exact in the first release. The
recommendation was no. Such pairs would go to the hybrid and come back as fitted B-splines with their fit recorded,
because [P5's](../evidence/2026-11-09-p5.md) three such cases were all correct that way and a fit carries its error.
Levin's general method would be added only if `booleans` showed the fits cost accuracy or speed. **The user chose
against the recommendation: "Levin's method now".**

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- Any two of plane, sphere, cylinder and cone with no closed form in `intersection.closed` are intersected by the
  pencil method in a new step, `intersection.quadrics`. The two surfaces' matrices span a pencil, and a ruled member of
  it carries the curve (Levin 1976, 1979). The pencil is classified before it is parametrized (Dupont, Lazard, Lazard
  and Petitjean 2008; Wang, Goldman and Tu 2003). All four were checked against the publishers' records on
  2026-11-13: Levin, CACM 19(10):555–563, 1976; Levin, Computer Graphics and Image Processing 11(1):73–87, 1979;
  Dupont, Lazard, Lazard and Petitjean, Journal of Symbolic Computation 43(3), 2008, parts I–III, pp. 168–232; Wang,
  Goldman and Tu, Computer Aided Geometric Design 20(7):401–422, 2003. They enter [SOURCES.md](../SOURCES.md) when
  the step's code lands.
- The curves are exact: closed forms evaluated in floating point, never fitted. A smooth quartic becomes a new curve
  family in `occam-geom`, and its images on the two surfaces become a new parameter-curve family. Both fit the frozen
  traits, so the seam does not change. Lines and conics use the existing exact families, and singular quartics use
  rational B-splines.
- Degenerate pencils are detected and classified in the result: tangencies, singular points, and pencils that split
  into lines and conics. They are parametrized exactly and never fitted. A classification the band cannot decide is
  `decide.undecided`.
- A torus is not a quadric. A torus pair, any quadric against any other family, and every NURBS pair still go through
  the hybrid and come back fitted, except the torus cases that `intersection.closed` answers.
- A smooth quartic has no rational form. So `to_bspline` gives none, and anything that needs a B-spline of such a
  curve, such as STEP export in `exchange`, gets a fitted one, and says so.
- The milestone grows by one step, about as large as the hybrid.
