# Parity list

**Status: [approved by the user](decisions/2026-11-09-parity-approved.md), 2026-11-09 (L35).** The user owns this list and its order. "Replacement done" in
[the plan](PLAN.md#2-what-done-means) means every row here.

Each row is a behaviour occam must provide, written from the outside: what goes in and what comes out. Implementers work
from this file and from published literature, under [the provenance rules](decisions/2026-11-08-licence-and-provenance.md),
so it names no Open CASCADE class. Tiers are ordered by what FreeCAD really uses, and rows within a tier by call sites.

**Where the numbers come from.** The [FreeCAD 1.1.4 census](evidence/2026-11-09-freecad-occt-usage.md) counts call sites
in FreeCAD's own modules, leaving out tests and bundled third-party code. Its names were grouped by hand into the rows
below, and totals are rounded, so treat them as weights rather than measurements. Call sites understate an operation
FreeCAD wraps once and then uses everywhere: fillets and booleans are the clearest cases, which is why the first tier is
the bracket test and not the top of the count.

**Arrives in** names a milestone in [the plan](PLAN.md#6-milestones), or *later* for what follows the first release.

## Tier 1: the bracket test

What [the bracket test](PLAN.md#2-what-done-means) exercises: extrude a profile, cut a bolt hole, fillet the edges, then
view the result and export it. Nothing else belongs in this tier.

| # | Behaviour | Call sites | Arrives in |
|---|---|---:|---|
| 1.1 | A solid is a B-rep of shells, faces, wires, edges and vertices, with orientation, shared sub-entities and stable ids. Entities can be listed by kind, without duplicates, and adjacency queried (the faces around an edge, the edges of a face) | 7,800 | `topology` |
| 1.2 | Points, vectors, unit directions, axes, coordinate frames and rigid transforms in 3D, with their counterparts on a face's UV plane; units explicit, one conversion at the API | 3,800 | `maths`, `geometry` |
| 1.3 | Every model carries one numeric tolerance for length and one for angle, and operations state which they used | 950 | `geometry` |
| 1.4 | Lines and circles, trimmed by parameter, evaluated to point and tangent | part of 2,100 | `geometry` |
| 1.5 | Planes, cylinders, cones, spheres and tori, evaluated to point and normal | part of 1,100 | `geometry` |
| 1.6 | Edges from points, lines and arcs; a wire from edges, with a gap reported by location; a planar face from a closed wire, with holes | 600 | `construction` |
| 1.7 | A tessellation with chord and angle limits: triangles and normals as typed buffers, watertight across shared edges | 120 | `view` |
| 1.8 | Volume, surface area and centre of mass of a solid | part of 170 | `construction` |
| 1.9 | STEP export of a solid that the pinned OCCT oracle reads back with matching volume and a valid shape | part of 110 | `exchange` |
| 1.10 | A check of any shape: closure, Euler characteristic, orientation, self-consistency; each failure names its entities | 80 | `topology` |
| 1.11 | Fuse, cut and common of solids, reporting which input faces produced which output faces, and on failure the failing pair | 60 | `booleans` |
| 1.12 | Primitive box and cylinder at a given frame | 50 | `construction` |
| 1.13 | Extrude a face along a vector into a solid | 10 | `construction` |
| 1.14 | Constant-radius fillet on chosen edges, per-edge results; a radius too large is refused naming the edge and the largest feasible radius, and following that remedy succeeds | 5 | `blends` |

## Tier 2: what FreeCAD uses daily

The curve and surface kinds and the measuring, repairing and exchange behaviours used throughout FreeCAD's part design,
sketching and drawing modules.

| # | Behaviour | Call sites | Arrives in |
|---|---|---:|---|
| 2.1 | All 3D curve kinds: ellipse, hyperbola, parabola, B-spline, Bézier, offset, trimmed; conversion to B-spline; evaluation to any derivative; length; a uniform discretisation by distance or deflection | 2,100 | `geometry` |
| 2.2 | All surface kinds: B-spline, Bézier, surface of revolution, surface of linear extrusion, offset, trimmed; evaluation to any derivative; a kind query | 1,100 | `geometry` |
| 2.3 | 2D curves in a face's parameter space, with the same kinds as 3D, and joining them to 3D edges | 900 | `geometry` |
| 2.4 | Curve and surface continuity classes (C0 to CN, G1, G2), reported and required at joins | 200 | `geometry` |
| 2.5 | Repair of nearly valid input: wire ordering and gap closing, face orientation, small-edge and small-face removal, and a report of what changed | 180 | *later* |
| 2.6 | Moments of inertia and principal axes, per solid and per face | part of 170 | *later* |
| 2.7 | Tight axis-aligned bounding box of any shape | 140 | `construction` |
| 2.8 | Tangent, normal and curvature at a parameter along a curve or on a surface | 130 | `geometry` |
| 2.9 | STEP import of solids with names and colours kept; unsupported entities reported, never dropped silently | part of 110 | `exchange` |
| 2.10 | Minimum distance between two shapes, with the nearest points; closest point on a curve or surface | 100 | *later* |
| 2.11 | Rigid and scaling transforms of a shape, with a copy that keeps ids distinct | 75 | `construction` |
| 2.12 | Sewing a set of faces into a shell and shells into a solid, reporting free edges | part of 75 | *later* |
| 2.13 | Primitive sphere, cone, torus and wedge | part of 50 | `construction` |
| 2.14 | Whether a point is inside, on or outside a solid or a face | 15 | `booleans` |
| 2.15 | Revolve a face or a wire about an axis | part of 10 | `construction` |
| 2.16 | Equal- and two-distance chamfer on chosen edges, refused with a feasible value as for fillets | part of 5 | `blends` |
| 2.17 | Native model format with a readable text dump | small | `construction` |

## Tier 3: less common

Used by fewer modules, or by one specific tool. These follow the first release and need the user's go-ahead.

| # | Behaviour | Call sites | Arrives in |
|---|---|---:|---|
| 3.1 | Sweep along a path, loft through sections, and thick shells and offsets of solids, with their failures reported per entity | 110 | *later* |
| 3.2 | Section: the curves where a plane or a solid meets a shape | part of 60 | *later* |
| 3.3 | Hidden-line projection of a solid to 2D drawing edges, grouped by visibility | 60 | *later* |
| 3.4 | Intersection as a query: curve with curve, curve with surface, surface with surface | part of 100 | *later* |
| 3.5 | Filling a closed boundary with a surface, with point and curve constraints | 55 | *later* |
| 3.6 | Variable-radius fillets and full-round blends | small | *later* |
| 3.7 | Draft angle on faces | small | *later* |
| 3.8 | Projection of a curve or wire onto a surface | part of 100 | *later* |
| 3.9 | Feature-style prism and revolution bounded by faces of the model | small | *later* |
| 3.10 | IGES import and export; STL and other mesh export | small | *later* |

## What will not be built

| Open CASCADE provides | Why not |
|---|---|
| Foundation collections, strings, exceptions, memory management and OS abstraction | Rust's standard library; every failure is a `Report`, never an exception (L1) |
| A label-and-attribute document store, including its undo and persistent-naming attributes | The document layer is planned afresh [after the first release](PLAN.md#6-milestones), around stable ids rather than labels |
| A 3D viewer, presentation layer and selection structures | The renderer in `web/` consumes data only |
| Its native shape formats, as the save format | occam has its own native format; only STEP is read and written for exchange |
| Topology and parameterisation identical to Open CASCADE's results: its edge and face counts and curve parameters | Compared geometrically only, never by topology or bits (L12) |
| Bug compatibility and legacy quirks: tolerance growth per entity, results that FreeCAD works around | A side-by-side shim at FreeCAD's layer may absorb these later (L39); they are not requirements |
| A plugin loader, resource files and unit-table machinery | Dependencies are compiled in; units are typed quantities |
