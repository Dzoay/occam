# Intersection

[Architecture](../ARCHITECTURE.md) › Intersection · crate [`occam-intersect`](../../crates/occam-intersect/src/lib.rs) · sketch [`intersect.rs`](../sketch/src/intersect.rs)

Where two surfaces meet, and where an edge crosses a surface. An ordered list of methods; what failed stays beside
what was found.

## Surface against surface

```mermaid
flowchart TD
  pair(["Surface pair"]) --> canon["Canonical order<br/>family, batch key, content"]
  canon --> cf{"Both present<br/>an analytic form?"}
  cf -->|"yes"| closed["Closed form"]
  cf -->|"no"| hybrid["Hybrid: subdivide for start points,<br/>march, trace in rounds"]
  closed -->|"declines a quadric pair"| pencil["Levin's pencil method:<br/>classify, then parametrize"]
  closed --> sec[("Section")]
  pencil --> sec
  hybrid --> sec
```

| A `Section` holds | Meaning |
|---|---|
| curves | Each with its image on both surfaces |
| contacts | A point of tangency, or `BelowResolution`: a contact or loop smaller than this tolerance can tell apart (O24) |
| coincidence | Coplanar or coaxial (O25) |
| failures | A stalled trace, a spent budget, an unresolved leaf, an undecided predicate |
| tally | The decisions made, from [maths](maths.md#robust-decisions) |

- **Canonical order** means swapping the arguments cannot change the answer.
- **A closed form's images** are its curve read through each surface's [chart](geometry.md#parameter-curves), the
  closed form of its parameters. A surface without one, such as a plane's B-spline with uneven poles, declines to the
  next method.
- **The pencil method** answers two of a sphere, a cylinder and a cone that the closed forms leave. It classifies the
  pencil by the real roots of its determinant, their multiplicities and the members' ranks there, each decided against
  the band through predicates on the matrices' entries. No multiple root is a smooth quartic, in `Quartic` loops; a
  double root of rank three with its vertex on both surfaces a nodal quartic, or a triple one a cusp, a rational
  B-spline from its vertex; a double root of rank two a split into the conics of its pair of planes, which may cross
  where no point is real. A determinant zero for every member is two cones with one apex, meeting in rulings, or a
  cylinder tangent to a cone along a ruling, which splits into that ruling and a conic. Singular points are contacts. A
  degenerate pencil of any other kind is reported as not implemented, never fitted, and a decision the band cannot make
  is `decide.undecided`. Its curves' images count their turns about each chart's axis.
- **The hybrid works through evaluation and bounds,** not a B-spline form, so offsets and mesh-backed surfaces reach it.
- **Many pairs** run one pair a task, one result each, in pair order.

## Edge against surface

An edge is tested only after its conservative box meets the surface. The culled and unculled runs are compared by
digest in the tests.

<details>
<summary>Evidence</summary>

- [P5](../evidence/2026-11-09-p5.md): closed forms 58 of 58 within 1e-12. The hybrid 104 of 110, with 2 silent failures,
  against 13 for marching alone, none of which marching reported: hence failures kept in the result. Lattice-seeded
  marching returned 256 of 577 curves with no warning, so it is not a route to completeness. Rejection bounds carry a
  rounding margin.
- [S4](../evidence/2026-11-09-s4.md): the ordered method list, and canonical pair order.
- [P8](../evidence/2026-11-09-p8.md): the edge box test removed 66 to 86% of scans.
- The hybrid reaching offsets and mesh-backed surfaces: B:13.
- Coincidence is found only where a family presents an analytic form, so a rational cylinder goes to the general
  method (A:15). Coincident surfaces become a `booleans` step, written test-first from S1's 32 reports and S6's c01 to
  c03 ([O25](open-decisions.md#assumed-answers-no-question)).

</details>
