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
  closed --> sec[("Section")]
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
- **The hybrid works through evaluation and bounds,** not a B-spline form, so offsets and mesh-backed surfaces reach it.
- **Many pairs** run one pair a task, one result each, in pair order.

## Edge against surface

```mermaid
flowchart TD
  pair(["Curve over a span,<br/>surface over a patch"]) --> box{"Boxes within<br/>the band?"}
  box -->|"no"| none[("No hits, no work")]
  box -->|"yes"| form{"Surface presents<br/>an analytic form?"}
  form -->|"yes"| along["Signed distance along the curve:<br/>pieces decided by interval bounds,<br/>runs between them read"]
  form -->|"no"| sub["Span and patch subdivided by bounds,<br/>Gauss-Newton on the leaves"]
  along --> hits[("Hits")]
  sub --> hits
```

| A `Hits` holds | Meaning |
|---|---|
| crossings | Where the curve passes through the surface, with its parameter, point and the surface's parameters |
| contacts | Where it meets the surface without passing through: a touch, or an end of the span on it |
| overlaps | A part of the span lying in the surface within the band, for edges on coincident faces |
| failures | A spent budget, an undecided predicate: what was found stays above |
| tally | The decisions made |

- **The box test is conservative:** points are kept only where their foot lies in the patch, and overlaps where their
  box meets the patch's, so a pair whose boxes miss has nothing to find. `hits` and the unculled `scan` are compared by
  digest in the tests, and a box one ulp too small is caught.
- **Against an analytic form,** the curve is moved into the form's own frame, so its boxes follow the surface. A run
  between the two sides is a crossing, between one side and itself a touch, a near miss or two crossings, and a run
  within the band whose slope and bend keep it there across the span an overlap.
- **Otherwise** a curve lying in the surface is not told from many contacts, and runs to its budget, which is reported.

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
