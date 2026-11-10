# Architecture

**Status: proposed, for the user's approval.** Approval is the exit gate of
[`proving-ground`](plan/proving-ground.md), and it is recorded in [decisions/](decisions/).

This is the design of occam's first release, made from the 17 spike findings in [evidence/](evidence/). As L50 asks, two
proposals were written to opposite emphases, and an adversarial critic attacked each one, with no scoring. The four
documents are evidence: [A, "columns first"](evidence/2026-11-10-design-a.md) and
[its critique](evidence/2026-11-10-design-a-critique.md), and [B, "records first"](evidence/2026-11-10-design-b.md)
and [its critique](evidence/2026-11-10-design-b-critique.md). This file takes B's shape and A's batch discipline, and it
fixes what both critics found. "A:3" below means point 3 of A's critique, and "B:3" point 3 of B's.

Where things live:

- **The crate list** is [§4 of the plan](PLAN.md#4-layers), which `gates layers` reads.
- **The signatures** are in [the type sketch](sketch/src/lib.rs). It is a crate outside the workspace, with one module
  per crate, and `just sketch` type-checks it against the real `occam-report`, `occam-math` and `occam-units`. Each
  milestone below deletes the modules its crates now hold.
- **The open decisions** are in [§7 of the plan](PLAN.md#7-decisions-the-user-still-owns). [Section 8](#8-the-open-decisions-it-assumes)
  says which answer each part of this design assumes, and what changes if the answer differs.

## 1. What it optimises

It is the simplest design that passes the bracket test and keeps every commitment in [§3 of the plan](PLAN.md#3-commitments).
It adds structure only where a finding showed a case that needed it. Batch paths exist where a finding measured a gain:
one surface at many points, predicate filters in lanes, and threads across faces, pairs and regions. Layouts sit behind
interfaces, so the later needs the spikes measured can arrive without a rewrite. Those are struct-of-arrays topology,
grouped geometry, caching and incremental recompute.

What it gives up: struct-of-arrays read scaling on memory-bound traversals (S3-P3), lanes across many surfaces at once
(S4's store), and a binary native format. Each is a change inside one module if a measurement later asks for it.

## 2. Crates

The list is [§4 of the plan](PLAN.md#4-layers). Against the provisional list it replaces:

- **`occam-check` becomes a module of `occam-topo`.** The checker runs inside `Edit::commit`, which is the only way to
  make a snapshot. So no unchecked model can exist, in release builds too, and no capability has to be smuggled across
  crates (A's `Verifier` trait needed a source gate to stay honest).
- **The execution layer lives in `occam-math`.** Both approved crates that can touch bits sit in the one crate that
  `gates determinism` already builds at 1 and N threads. `rayon` is named only by `Exec`, and `fearless_simd` only by
  `Level` and `dispatch` ([the rayon decision](decisions/2026-11-10-rayon.md),
  [the fearless_simd decision](decisions/2026-11-10-fearless-simd.md)). A separate `occam-exec` crate (proposal A) would
  have put the threading runtime under the numbers crate for one test (A:9).
- **Intervals, expansions, exact predicates and their batch filter go in `occam-math`.** They are numbers only, and
  [the maths choke point decision](decisions/2026-11-09-one-maths-choke-point.md) puts all computing there. S2's
  predicates D1 to D5 are written over its `Ring` in `occam-geom` and `occam-intersect`.
- **`Tolerance` keeps its home in `occam-units` and gains S2's meaning.** The certified error radius is an `Interval`
  while computing, and a `Length` on a stored vertex.
- **Geometry codes join the one catalogue in `occam-report`.** A description of an entity is plain data there, a form
  name and named values, so the crate still depends on nothing of occam's.
- **The composition crate `occam` gains the request runner and the one resolver of descriptions** (section 3.8).

## 3. The seams

### 3.1 Units and quantities

Sketch: [`units`](sketch/src/units.rs). Conversion at the API is unchanged: a request carries its units, and
`Length::new` and `Angle::new` stay the one place numbers become quantities.

- **`Vec3`** is three plain numbers with dot, cross and norm, for the inside of a formula, typed again at its ends. S1
  found that the typed layer stopped where formulas began.
- **`Direction<F>`** is a dimensionless unit axis, and **`Tangent<F>`** a derivative: length per unit of a parameter
  that may be a length, an angle or a knot unit. Neither is three lengths (S4; A's `Tangent` was, A:5).
- **`ParamKind`** records what each parameter measures in its domain, so a test in parameter space uses the right
  tolerance (S4's cone compared a length with an angle tolerance). Parameters stay bare numbers, so lane kernels read
  plain `f64`.
- **`Placement<A, B>`** is a rigid map, built once with the owned `sin_cos`. Inputs arrive in `Local` and are placed into
  `World`, where every stored curve and surface lives.
- **`Bounds`** holds everything it bounds, rounding included. A box too small is a silent wrong answer (S1 item 13), so
  every cull is tested against the unculled run's digest (P8 found a mis-ordered minimum only that way).
- **Stored quantities stay typed.** Vertex positions are `Point<World>`, and radii and deviations are `Length`. S1 found
  those fine, and the layout question (O22) does not need them untyped (A:5).

**`Tolerance`** stays one length and one angle per model, never per entity, with no default and no growth. It gains
S2's meaning:

- `band()`: what every decision compares with, exactly. `|q| <= eps` means on, tangent or the same.
- `near_tangent_shift(R)`: `sqrt(2 R eps)`, how far a band decision at near tangency moves a vertex. It goes into the
  report as a named value. It never goes into the vertex's certified radius, because that would bring back the hull S2's
  snapping drops: 86% of chain steps failing instead of 44% (A:2).
- `in_parameter(kind, rate)`: the band in a parameter of that kind.

### 3.2 Maths

Sketch: [`math`](sketch/src/math.rs).

- **`Real` gains comparison and selection** (`lt`, `le`, `select`, a mask type), which S4 and P5 had to build through
  `to_array`. `f64::min` stays banned, because which zero it returns is unspecified.
- **`Real` gains the owned `sin_cos` and `atan2`** (S5, O14): the same bits on every target, lane width and build, within
  about 0.75 and 1.5 ulps. `tan`, `atan`, `asin` and `acos` follow as thin derivations when a caller first needs one.
  Everything else in the ban list stays banned. The coefficients wait on O15.
- **`Lanes::from_fn` becomes a plain loop.** `array::from_fn` falling out of line cost a rational kernel 2.3 times (P4).
- **Lane dispatch.** `Level::detect()` finds the widest instruction level once. `dispatch(level, kernel)` runs a
  `LaneKernel` compiled for it. A kernel is a trait, not a closure, because `run` and all it calls must be
  `#[inline(always)]` or a leaf compiles without the level's features (P4), and a closure cannot carry that attribute on
  stable Rust (B:8). Dispatch happens inside a family's batch method, because it cannot cross `dyn` (A:10). Every level
  gives the same bits, since nothing fuses, so [`DETERMINISM.md`](DETERMINISM.md) rule 5 stands.
- **Robust decisions follow S2.** `Interval` is a midpoint and radius, rounded outward by inflation with `+ - * /` and
  `sqrt` only, so no rounding mode changes. `Expansion` is the exact stage. `Predicate<N>` is written once over `Ring`.
  - `decide_stored` takes stored numbers: the filter in `Lanes<4>` with the exact stage inline. It is never undecided.
  - `decide_computed` takes computed values with their certified radius. It answers `Undecided` when an enclosure
    straddles the band: "cannot decide at this resolution" (O11).
  - No exact towers by default: their work grows about five times a step (S2).
- **`Tally`** counts decisions, exact fallbacks and undecided ones. Every operation reports it as a `measure.decisions`
  note, because each 1% that reaches the exact stage adds about 33 ns a decision on one thread (P6).

### 3.3 Execution

Sketch: [`math`](sketch/src/math.rs), `Exec`. This is P11's interface with `rayon` behind it. `Exec` is a concrete type,
not a trait, so no kernel function is generic over its runner.

- **`Exec::map(n, grain, f)`** returns results in index order, whatever order they finished in. There are no reductions,
  so nothing can depend on a split tree (P11: `rayon`'s float `sum` gave 7 bit patterns in 8 runs). Closures are
  `'static`, which keeps a swap to a small pool possible.
- **`Grain::Each`** puts one item in a task, for items of 10 µs and more: a face, a pair, a region. **`Grain::Auto`**
  leaves cheap items to the runner: under `rayon`, items of 100 ns reached 5.65 times at six threads on `Auto` against
  3.94 with one item a task (P11; B:8).
- **`Exec::map_chunks(len, chunk, f)`** runs a batch in fixed chunks claimed dynamically, concatenated in item order.
  The chunk is both the task and the lane group, as P6's was. Predicate batches run here at 256 to 1,024 items a chunk.
  Chunk boundaries depend on the chunk size, never on the thread count.
- **`Exec::serial()`** is the one-thread baseline and the runner inside each browser worker (P10). On wasm, `rayon`
  is not built.

### 3.4 Geometry

Sketch: [`geom`](sketch/src/geom.rs). This is S4's two layers, with one blanket impl between them.

**All stored geometry is in `World`.** The traits carry no frame parameter, which removes S4's E0119 case (an
object-only family generic over its frame). Instancing in a local frame (P8) is a later decision, as a placed family.

**Layer 1, `Parametric`,** is what a family implements, generic over `Real` so one formula runs as `f64` or in lanes.

- `eval_point`, `eval_first` and `eval_second` are the three rungs the hot paths use. Nothing beyond the order asked for
  is computed (P8 item 1: 33 to 34% of whole-run time). `eval_*` is total: an undefined parameter gives NaN in that lane.
- **`derivatives(uv, order)`** gives every partial derivative to any total order, exact where the family can be. This
  answers [PARITY](PARITY.md) rows 2.1 and 2.2, which the user approved and which ask for "evaluation to any derivative"
  in `geometry`. That settles O21 against the plan's recommendation of two orders (section 8; both critics confirmed the
  conflict, A:4 and B's "claims about the sources").
- **`eval_batch(uv, order, out, scratch, level)`** evaluates one surface at many points. That shape is P1's sampling,
  P9's refinement and P12's dirty rectangles, and batching it gained 1.8 to 3.3 times on analytic surfaces (P9). The
  outputs and the `Scratch` workspace are the caller's, so nothing allocates (S4: degree 5 by 4 allocated on every call).
  The result is a `Defined` bitset, one bit per item: L6's result per item, which S4 could reach only by scanning. The
  default runs the scalar rungs. A family overrides it to choose its lane width per order and to call `dispatch` inside.
- **`invert`** gives a foot's parameters without derivatives (P8: half of a point query). It is required, so that no
  family inherits the full closest-point solve by default (B:13). `closest` is the full solve. Both take a seed and a
  scratch, and fail with a report that carries where the solver stopped (L5).
- `domain` (with each parameter's `ParamKind`), conservative `bounds(patch)`, `degeneracies` (declared at construction:
  a sphere's pole has a normal, a cone's apex does not), `family`, `batch_key` and `content`.
- **`content`** writes the canonical bits of everything that defines the geometry, family name first. The pair order and
  any later cache key compare these words, never a pointer (P8).
- **`shape`** gives the form name and parameters that a description shows (section 3.8). The form is text held to the
  catalogue's list by a test, so a new family edits no enum in `occam-report` (B:4).
- **What a surface can present** is asked, not what it is, and each answer defaults to nothing, so a new family edits
  no list (S4): `analytic()` (plane, cylinder, cone, sphere, torus) for the closed forms, `to_bspline()` for conversion
  (PARITY 2.1, 2.2), and `triangles()` for mesh-backed surfaces.

**Layer 2, `Surface`,** is what everything outside a family calls: the same questions at `f64`, dyn-compatible. Curves
repeat both layers as `ParametricCurve` and `Curve`, and a sampled curve states its achieved fit (S1 item 10: without
one, 33 NURBS operations broke). Parameter curves (`ParamCurve`) are the same over `UvPoint` (PARITY 2.3).

**The families** are those [PARITY](PARITY.md) places in `geometry`: lines, circles, conics, B-spline and Bézier curves,
offset and trimmed curves; the five analytic surfaces, B-spline and Bézier surfaces, surfaces of revolution and of linear
extrusion, offsets and trimmed surfaces; parameter curves of the same kinds. Mesh-backed surfaces stay too, because the
plan's `geometry` exit asks for one seam proven on all three families before it freezes (L21). Proposal B dropped them
without asking, which was a change of scope (B:6).

**There is no cross-family column store.** S4 said homogeneous data cannot be recovered from `dyn Surface` without a
downcast, and that is true. But nothing measured needs it. P9's 1.8 to 3.3 times is one surface at many points, which
`eval_batch` serves. S4's 14 to 18% is from sorting calls by kind, not from storing them by kind. The one store S4 built
was 2.6 times slower on analytic surfaces, a figure S4 warns measures its implementation, not batching. S3-P3 found no
lane gain on baseline x86-64, and P5 found batching worth under 3% of intersection (A:1). So calls are sorted by
`batch_key`, and geometry is stored one item a slot, in the snapshot's tables. Grouping storage by family saves 15% of
surface memory (S3-P3), and it waits until memory asks (O23).

### 3.5 Topology

Sketch: [`topo`](sketch/src/topo.rs).

**What the entities hold.** S1's list, each item backed by a case that needed it, plus what a model needs beyond a closed
solid (B:1, A:7):

- **Bodies** of kind solid, sheet or wire (PARITY 1.1, 1.6). A body holds shells or wires.
- **Shells** of faces, open or closed.
- **Faces,** each with its surface, the sheet of each periodic parameter its loops lie in (S1 item 2: the most damaging
  removal, 152 failures and 20 wrong answers), orientation, and its outer loop.
- **Loops,** with a strict interior point kept as a witness for which one is outer. Area alone was the assumption whose
  bug broke 52 cases (S1 item 3).
- **Wires** that bound no face: a profile, or a wire body.
- **Edge uses,** each with its edge, owner (a loop or a wire), direction, the next use around, and its parameter curve
  (S1 item 7: without it, the same answers 3.45 times slower). Each use also links to the next use of the same edge, so
  an edge can have none (free), one (a sheet's boundary or a wire), two (a closed solid) or more. The checker decides
  which counts are valid for the body, so no count is impossible to store (PARITY 1.10, 2.12).
- **Edges,** with curve, range, ends, role (ordinary, seam or section), and a measured deviation from every face that
  uses it. The deviation is measured, never asserted: in 45% of near-external crossings an edge lay up to 140 times the
  tolerance from its face with every volume right (S1 item 9).
- **Vertices,** shared by identity and merged by the band as they are made (S1 item 1). Each holds a position, the
  certified radius of a computed position (zero for a given one, S2), and one edge of its star, so a fillet walks the
  star without indexing the model (B:3). A vertex may join any number of edges, which is how a pinch is stored (S1 item
  6), and the checker counts shells, the one check that notices a missing pinch.
- **Seams are real edges** between chart-sized patches, with no poles, and no face crosses its own seam (S1 item 5).
  Whether a seam edge used twice would serve better was not compared (section 9).
- **Provenance:** bodies, faces and edges record the step that made them and their part in it: profile, target, tool,
  input or made. `Edit::derive` carries it forward. Step numbers count over the model's whole life, so they are unique
  across requests (B:4). This is what S6's agents asked for first.

**Ids.** `Id<K>` is a slot and a generation, with the kind as a type (S3-P3). The generation changes only when a slot is
reused. So an entity changed in place keeps its id and is listed as changed, and a stale id is a `topo.stale-id` report,
not a wrong entity. Free slots are reused lowest first, so ids depend only on the table's state, and saving each slot's
generation is enough for ids to survive a save and a load (B:12).

**Storage.** Records sit in copy-on-write chunks of 1,024 per kind. At a million faces a snapshot takes 183 µs against
31 ms for a plain arena, and 1,024 suits edits that scatter, as a boolean's do: 14 µs an edit for 100 scattered edits,
against 41 with chunks of 4,096 (S3-P3). A snapshot is immutable and cloning it shares every chunk.

**Records or struct-of-arrays (O22).** Callers read through per-kind views, `FaceView`, `EdgeView` and `VertexView`,
field by field, never a whole record. So the layout belongs to the table module alone, and changing O22's answer edits
nothing outside it. This answers B:3, where whole-record reads would have made struct-of-arrays worse than records.
Writes go through `Edit` as records, which are also what the native file and the dump hold. Section 8 recommends records,
against the plan's earlier recommendation of struct-of-arrays.

**The edit.** `Edit` is the sequential apply and the only place ids are given (S3-P3, P2).

- `reserve` gives an id for a record written later, so that cycles (an edge and its uses, a face and its outer loop)
  can be built from an empty model (B:1). `commit` refuses a reserved id that was never set.
- `add`, `set`, `remove`, and `derive(new, old, how)`, where `how` is split, trimmed, merged or copied.
- `commit` checks what was created or changed and its neighbours, plus the global counts (Euler characteristic, shells
  per body), and only then makes the snapshot. Checking what changed keeps the check's cost following the edit, which is
  P1's plate ladder again: one more hole cost 38 times as much on a plate with 64 holes (B:5, A:12).
- `Snapshot::check_all` checks the whole model, for loading, importing and tests (PARITY 1.10).
- A commit gives `Changes`: created, changed, deleted, and the lineage of each new entity (PARITY 1.11).

### 3.6 Intersection

Sketch: [`intersect`](sketch/src/intersect.rs). This is P5's answer through S4's ordered method list.

- **Closed forms first,** through `analytic()`: 58 of 58 correct within 1e-12 (P5).
- **Then the hybrid,** subdivision for start points then scalar marching, traced in rounds merged by index: 104 of 110,
  with 2 silent failures against 13 for marching alone (P5). It works through evaluation and `bounds`, not through a
  B-spline form, so offsets and mesh-backed surfaces reach it too (B:13).
- **Lattice-seeded marching is not a route to completeness.** It returned 256 of 577 curves with no warning (P5).
- **A `Section` keeps what was found beside what stopped it,** because marching reported none of its 13 wrong answers.
  It holds curves (each with its images on both surfaces), contacts, a coincidence flag (coplanar or coaxial, for O25),
  failures (a stalled trace, a spent `Budget`, an unresolved leaf, an undecided predicate) and the `Tally`.
- **`Contact::BelowResolution { at, bound }`** is what a loop below the resolvable size returns: a reported "contact or
  loop smaller than `bound`", not a silent contact point (O24).
- **The pair is put in canonical order first,** by family, batch key and content, so argument order cannot change the
  answer (S4). `sections` runs many pairs, one per task, with one result each in pair order (L6).
- **An edge is tested against a surface only after its conservative box passes:** 66 to 86% of scans removed (P8). The
  culled and unculled runs are compared by digest in the tests. Rejection bounds carry a rounding margin (P5).

### 3.7 Operations

Sketch: [`ops`](sketch/src/ops.rs).

**A request** is a schema version, units, an optional tolerance and a list of steps, each with an optional label that
later steps can name. It is validated and converted before any geometry, and each kind of invalid input has its own code
naming the operation, parameter, value and rule (S6: blind on S1's report, an agent "repaired" a zero-radius hole by
setting 0.0001 mm). Operands are an earlier step's label or an entity by id.

**The tolerance belongs to the model** (B:7). A new model takes the request's tolerance or, if none is given, O12's
default, and the outcome says which with `measure.tolerance-used`, which exists today. On an existing model the model's
tolerance governs, and asking for a finer one is `measure.tolerance-below-model`, as bedrock already does.

**The operations** are the first release's rows of [PARITY](PARITY.md): box, cylinder, sphere, cone, torus and wedge;
wires through points, lines and arcs, with a gap reported by location; planar faces with holes; extrude; revolve;
transform, with a copy that keeps ids distinct; fuse, cut and common; fillet and chamfer; and measure (volume, area,
centre of mass, bounds). Everything else is listed as refused.

**Results per item (L6).** Steps stop at the first failure, and the model returned is the one after the last step that
succeeded, so nothing that failed changed the input (L5). A step with many items returns a `Batch`, one result per item.
Its `commit` field chooses between two behaviours:

- `all-or-nothing`: nothing is applied unless every item succeeds, and an item that would have succeeded says so.
- `feasible`: the items that succeed are applied.

**Fillets take a radius per edge,** so the remedy for one edge is followed by changing one number (A:3, B:2). A radius
too large is `blend.radius-too-large` naming the edge, with `reduce-radius { edge, radius }`. The radius offered is the
largest the kernel certifies, a band inside the supremum, so following it never lands on an undecided case (A:3). If the
checker refuses the result, the failure is attributed through lineage to the item whose new entities it names.

**The boolean is P2's shape (a):** pure phases, each an ordered map from frozen input to frozen output. Its output was
identical at every thread count, and it reported every failing pair, where a shared graph gave 4 different digests at six
threads.

1. **Prepare** touches only faces whose bounds meet the other operand. The rest keep their ids and are labelled by
   propagation, so work follows the edit (P1).
2. **Broad** finds candidate pairs by a sort and sweep over conservative boxes, in stable order. All-pairs grows
   quadratically and must change before models get large (P1, P8).
3. **Pairs** computes one section per distinct surface pair, and an edge against a face only after its box test.
4. **Edge split** runs three rounds with a frozen vertex table between them: stops per edge, then pinch vertices merged,
   then the clips. S1 mutated the table while it clipped, and this is the change that made the output identical (P2).
5. **Face split** runs one face per task.
6. **Classify** runs one region per task, then propagation.
7. **Stitch** is sequential, and its `Edit::commit` runs the checker.

**A phase is a pure function of content** (`Phase` in the sketch). Its output holds indices local to each item, never a
model id or a number from this run. Items therefore name failures in local terms, and the merge maps them to ids (B:4).
That is P8's rule for later reuse, and it is kept now so that caching items later changes no phase. There is no cache in
the first release: P8 found one saves almost nothing once explicit order and edge box tests are in, except on NURBS
marching.

**On failure** the result is every failing item of the phase that failed, in candidate order, with the frozen outputs
reached (`Failure`, `Frozen`). That is L5's state, per pair rather than merged into one report. The first report is the
primary one; S6 found a long list can distract an agent, so the order matters.

**Mass properties** (PARITY 1.8) come from the divergence theorem over the faces with adaptive quadrature. The bracket
test's oracle compares the volume (A:3).

### 3.8 Reports

Sketch: [`report`](sketch/src/report.rs). `SCHEMA` becomes 2.

- **`EntityRef`** becomes a kind, a slot and a generation, which an agent can read: four of ten could not read a packed
  id (S6). It has one text form, `edge:41/1`, used alike by the command line, the JSON and the messages (A:13).
  `EntityKind` gains edge uses, loops, wires, bodies, and geometry with no topology (S4). It also gains the two kinds
  that exist only in a failed operation's frozen output, section and region, which have generation 0.
- **A fault class per occurrence:** invalid input, not implemented, numerically sensitive or internal. It is required
  when a report is made and chosen where the failure is found, never defaulted from the code. S6's c07 could not tell a
  repairable input from an unsupported one, and blind c12 showed that a default per code misleads.
- **Closed reasons** replace free text (S6: `reason` was a message's length, which agents read as a ratio). There are
  also stage names (validate, prepare, broad and so on), and new codes and remedies, each held to a followed-remedy test
  (L2).
- **`Partial`** gains its stage and loses `last_valid`: no agent used it, and the input is unchanged by construction
  (S6).
- **`NamedValue`** can hold a point, so a position or extent is one value. Each component goes through today's `number`
  module, so NaN and the infinities still round-trip (B:11).
- **The tolerance in force** joins the values whenever a tolerance decided the outcome (S6: blind c04 reasoned from the
  wrong one). An empty result is a `boolean.empty-result` note with the operands' separation (S6: c05 got `ok` and a
  volume of 0).

**How an agent learns what an entity is (O26).** Section 8 recommends a third way between the two proposals:

- A failure site names ids and nothing more.
- As the result leaves the kernel, the composition crate's `describe_all` describes every id in it in one batch, against
  the snapshot and, for a failure, its frozen outputs.
- The descriptions go into the report's `described` list: form, parameters, extent and position in the caller's units,
  origin, and neighbours.

So the report is self-contained in the shape S6's enriched blind run tested (6 of 6 handled, against 4 of 6). No site can
forget to describe, and phase outputs stay free of the model's numbering. `occam describe` asks the same resolver later,
for many ids at once.

**"Cannot decide at this resolution" (O11)** is `Verdict::Undecided` in the maths and a failure in a section's list. At
the operation it is `decide.undecided`, with fault `numerically-sensitive`. Its values are the quantity, its certified
radius, the band and the near-tangent shift, and it carries the `Partial` of the phase that met it. Its remedies are
`move-apart` and `use-tolerance`, each with a followed-remedy test.

### 3.9 The mesh and the renderer

Sketch: [`mesh`](sketch/src/mesh.rs).

**`occam-mesh` is P9's method.** It samples each shared edge once, meshes each face against those samples one face per
task, and merges by index: 4.7 to 5.2 times at six threads, with the same bits at every thread count and lane width.

- **Limits come first:** a floor on the step any surface may ask of a shared edge, and caps on samples per edge and
  triangles per face. Without them one collapsed NURBS row took 11.9 GB in seconds (P9).
- **Point location uses the exact predicates on real coordinates,** not P9's integer grid, which caused all three of its
  failures.
- **Chordal error is measured to the triangle.**
- **There is one result per face:** a face that will not mesh is named, and its neighbours are unaffected (L6).
- **The watertight check runs on every output, in production,** because "by construction" covers only the faces that
  meshed (P9).
- **Each face's mesh is a vertex block of its own,** with a normal per vertex from the surface, poles included. Along a
  shared edge its positions are that edge's samples, so neighbouring faces hold the same bits there. That gives P12's
  buffer layout without welding, and it removes cracks (B:13).

**The renderer contract is P12's:**

- buffers that both a storage binding and a vertex fetch can read;
- index buffers that stay stable while topology does;
- per-face ranges and dirty rectangles;
- uploads bounded per frame;
- camera-relative coordinates from the first version, an `f64` origin with `f32` offsets;
- a frame loop that keeps submitting during interaction.

GPU results are display-only. They stay out of the kernel's digests and caches, out of `occam-mesh`'s output, out of
picking, measurement and export, and out of the watertight check (P12, [the GPU drawing decision](decisions/2026-11-09-gpu-drawing.md)).

### 3.10 The browser

Sketch: [`compose`](sketch/src/compose.rs), `Session`. This is P10's pool: workers with separate memories, on stable
Rust, with simd128 the default build, and nothing required of the hosting page (O18).

- **One editing worker** holds the model and runs requests. After each edit it publishes the native text, transferred
  rather than copied.
- **Mesh workers** load that text read-only and mesh ranges of faces, about 64 calls a job. That balanced uneven work
  and avoided Firefox's long-call penalty of 1.4 to 2.3 times (P10).
- **No wasm runs on the page's main thread.**
- **Each call's output is under the determinism contract.** The page's assembly of the mesh parts is display-only, as
  GPU output is (B:12).
- **An edit is one long call on its worker.** Splitting a boolean across calls is left until a measurement asks for it,
  in the `view` milestone's browser timing.

### 3.11 The native format and STEP

Sketch: [`io`](sketch/src/io.rs).

- **The native format is the records as JSON,** through `serde_json` with `float_roundtrip`, so every finite number
  round-trips exactly. It is also the readable dump: one format, not two (PARITY 2.17, L8).
- **It carries a schema version from the first commit,** the tolerance, the step counter and every slot's generation, so
  a loaded model has the same ids and digest as the saved one.
- **Loading checks the whole model:** nothing from outside is trusted.
- **Load time is a risk.** A binary encoding is added if a measurement at the `construction` exit shows JSON is too slow
  for a model of 10^5 faces, or for the browser's workers (section 9).
- **STEP export** writes solids, and anything it cannot write is reported, never dropped (PARITY 1.9). Import arrives in
  `exchange` (PARITY 2.9).

### 3.12 The command line

The bracket test's agent has only the `occam` command and its reports, and the design gives it these commands, each with
`--json`:

| Command | What it does |
|---|---|
| `occam run --model part.occam ops.json` | Runs a request and writes the model back |
| `occam list --kind edge` | Lists every entity of a kind |
| `occam adjacent edge:41/1` | Lists an entity's neighbours |
| `occam describe edge:41/1 face:5/1` | Describes many entities at once |
| `occam check` | Checks the whole model |
| `occam mesh` | Meshes the model |
| `occam export --step` | Writes STEP |

Each takes many ids and gives one result per id (L6). Step outcomes list what they created, changed and deleted, by id.

## 4. Determinism

The contract is [`DETERMINISM.md`](DETERMINISM.md), and this design keeps it in these places:

- **Order is item index everywhere.** `Exec::map` returns results by index. `map_chunks` boundaries depend on the chunk
  size alone. Merges run in candidate order. A pair is put in canonical order by family, batch key and content. Tables
  iterate in slot order.
- **Ids are given only in `Edit`,** on one thread, in plan order. Free slots are reused lowest first. Phases use indices
  local to an item, and scratch ids index a frozen output.
- **What is hashed:** a snapshot's digest (every live record in slot order, every geometry's content, and the tolerance,
  NaNs canonicalised), `Changes`, meshes, and report and outcome JSON. Each must be identical at 1 and N threads, on
  x86-64 and on wasm32 (baseline and simd128), and at every dispatch level.
- **Gate coverage grows by milestone.** `gates determinism` gains the owned functions (hashed against a separate
  implementation, S5), the filter, `Exec::map` and `dispatch` in `maths`, and corpus booleans and meshes as they arrive.
- **What is kept out:** GPU output, `f32` display buffers, timings, thread count, dispatch level, arrival order and
  pointers. `Arc::ptr_eq` and `std::ptr::eq` join `disallowed-methods` in kernel crates. P8 found a cached curve taking a
  slower path because pointer identity said different where content was equal.
- **S5's rule 4:** a decision on a computed value never compares with exactly zero. It uses the band, or a predicate on
  exact inputs. Two cases get rules of their own:
  - A point within the band of a periodic seam belongs to the chart that the face's sheet names.
  - A count taken from a computed value, such as samples on an edge, is taken from its interval's upper end, so a
    value on an integer boundary always rounds the same way.

## 5. Parallelism and lanes

Threads go across items through `Exec`, and lanes go inside each task (L28). They multiply: four lanes (1.48 times) and
six threads (5.5 times) gave 8.2 times one scalar thread, as predicted (P6).

| Work | Grain | Lanes | From |
|---|---|---|---|
| Broad phase, edge box tests | `Auto` | masks at 8 through `dispatch` | P4, P11 |
| Prepare, face split, checker | `Each`, one face a task | one surface at many points | P1, P2 |
| Sections | `Each`, one pair a task; traces in rounds | none: marching is scalar | P5 |
| Edge split, three rounds | `Each`, sequential merges | none | P2 |
| Classify | `Each`, one region a task | crossing number at 8 through `dispatch` | P2, P4 |
| Stitch and commit | sequential | none | P2 |
| Robust decisions | `map_chunks`, 256 to 1,024 items, exact inline | interval filter at 4 | P6 |
| Tessellation | `Each`, edges then faces; in the browser, about 64 calls a job | one surface at many points | P9, P10 |

**Lane width is a named constant per kernel class, inside the family that runs it,** not one global width (P4). Starting
widths:

- masks and reductions: 8 through `dispatch` (5.8 to 5.9 times with AVX2 from a baseline build), and 4 on wasm;
- predicate filters: 4 (1.48 times, P6);
- splines, after reorganising around the grid, which was 1.5 times faster before any lanes: 4 to 8 with nothing
  gathered, 2 where spans change;
- rational derivatives: 1;
- marching: scalar.

**Analytic surfaces are unsettled.** P4's kernel with precomputed trigonometry lost two to three times at width 2. P9's,
with sine and cosine in lanes, went from 30.0 to 18.4 ns. So every lane kernel passes a gate in `gates perf` that checks
it vectorised and beats width 1 on each target, or it runs at width 1 (P4 found kernels that compile cleanly and run
scalar).

**Two limits.** Beyond the last-level cache, cheap kernels gain nothing from threads (1.02 times at six, P4): the remedy
there is fewer bytes per point. A finer grain inside one large face, the face split's ceiling (P1), waits for a real
profile.

## 6. The bracket test, walked

1. **Build the plate.** The agent sends a request with a wire through the profile's points, a planar face from it, an
   extrusion, and a cylinder for the bolt hole. It runs these with `occam run --json`. The outcome lists each step's
   created entities by id, and the reports name the tolerance used.
2. **Cut the hole.** A `boolean` cut runs in the same request or the next. If the tool is exactly as tall as the plate,
   its caps are coplanar with the plate's faces. That is O25's case: until the `booleans` step for coincident surfaces
   lands, it is `boolean.coincident-surfaces` with a `move-apart` remedy, followed by test.
3. **Choose the edges.** `occam list --kind edge` lists the edges, `occam adjacent` gives an edge's neighbours, and
   `occam describe` says what each is: for example a line from (0, 0, 10) to (0, 40, 10), made by step 2, the
   extrusion.
4. **Fillet them, one radius too large.** A `fillet` step gives a radius per edge. The step returns a `Batch`, one
   result per edge. The failing edge's report is `blend.radius-too-large`, fault `invalid-input`, naming the edge and
   describing it, with `reduce-radius { edge, radius }` at the largest certified radius. The agent changes that one
   radius and runs again, and it succeeds.
5. **View it.** The page's editing worker loads the native text and its mesh workers mesh face ranges. The renderer draws
   per-face buffers with camera-relative coordinates.
6. **Export and check.** `occam export --step` writes the solid. The pinned OCCT oracle reads it back, and its volume
   matches `measure`'s, which comes from the divergence theorem.

## 7. What changes elsewhere

- **[§4 of the plan](PLAN.md#4-layers)** holds the new list, and is no longer provisional.
- **[§7 of the plan](PLAN.md#7-decisions-the-user-still-owns)** carries section 8's recommendations, and O21 moves to
  answered.
- **[The first-release outline](plan/first-release.md)** names its parity rows per milestone, and gains the work above.
- **[`DETERMINISM.md`](DETERMINISM.md)** gains S5's four statements in the `maths` milestone, when the owned functions
  land.
- **The `spikes` branch** is deleted once this is approved. P1's full text is kept outside the repository.

## 8. The open decisions it assumes

Each row is a decision in [§7 of the plan](PLAN.md#7-decisions-the-user-still-owns). Approving this file approves the
assumed answers, except where the user says otherwise. The rows marked **ask** are the ones where the evidence leaves a
real choice.

| Id | Assumed | If answered otherwise |
|---|---|---|
| O11 | Yes: "cannot decide at this resolution" is a reported outcome with its failure state | The kernel must pick a side. S2's only ways to always pick were wrong against nominal in 25 to 44% of chain steps (B:10), or exact towers at about 5 times the work a step. `Verdict` loses `Undecided`, and `occam-ops` gains a policy |
| O12 **ask** | 1e-7 mm and 1e-9 rad, the default for a new model when the request gives none | Only the default and the fixtures change. Against 1e-6 mm: every finding measured at 1e-7, and S1's three "open" results there come from its flux test, which S1 attributes to quadrature noise (B:7); a near-tangent vertex moves 1.1e-3 mm at R = 6 mm against 3.5e-3 at 1e-6 (S2). Against 1e-9: S2's intervals began to fail inside the band (11.1% against 0.2% at 1e-7), likely at its largest scale. 1e-9 rad matches 1e-7 mm on a 100 mm lever, S1's model size |
| O14 | Own `sin_cos` and `atan2` | `Real` loses both, analytic families run at width 1 (1.5 to 2.7 times slower in lanes), and `DETERMINISM.md` exempts std's functions, whose bits differ by target |
| O16 | The promise covers only owned functions, and the browser-and-server promise waits for that feature | If promised now: the nightly run compares wasm and native corpus booleans, and every new transcendental is owned first |
| O18 | Nothing is required of the hosting page | Shared-memory threads need nightly and `-Zbuild-std` (O10), a shared-memory arm in `Exec`, and an adapter crate for `rayon` on browser threads, which is a new dependency (B:10) |
| O21 | Answered by [PARITY](PARITY.md) rows 2.1 and 2.2: evaluation to any derivative, exact where the family can be | If the user amends those rows to "two orders": `derivatives` goes, and an offset's second derivative is a difference, about 1e-5 (S4) |
| O22 **ask** | Records in chunks of 1,024, read through per-kind views | Struct-of-arrays in chunks of 4,096, the size S3-P3 measured, inside the table module only. The reasons for records: struct-of-arrays wins read scaling only on a memory-bound microbenchmark (4.83 against 3.23 times), while heavier per-face work scales 5.4 to 5.6 times either way. Records win what booleans and edits do at a million faces: taking a snapshot (183 against 1,191 µs at 1,024 a chunk), 100 scattered edits behind it (14 against 23 µs an edit), an edit in place (1.37 against 1.98 µs at 4,096) and bounds in scrambled order (297 against 379 ns) (S3-P3). The `topology` exit re-measures both on the real records, which are larger than the spike's (B:3) |
| O23 **ask** | L29 becomes: "Sort calls by family and degree (S4: 14 to 18%). Evaluate one surface at many points in a batch (P9: 1.8 to 3.3 times). Use lanes per kernel class only where the vectorisation gate measures a gain (P4, S3-P3). Group stored surfaces by family when memory asks (S3-P3: 15% less)" | The plan's wording also groups storage by kind now, which adds a store per family and an id for each item, with no speed measured for it |
| O24 | A reported `BelowResolution` contact, and `intersect.below-resolution` when it changes topology | A contact point: the variant goes, and P5's silent wrong shapes stay silent |
| O25 | No follow-up spike. Coincident surfaces are a step of `booleans`, written test-first from S1's 32 reports and S6's c01 to c03. Until then they are a reported failure with `move-apart` | A spike first: that step waits for it. The analytic presentation detects coincidence only where a family presents itself, so a rational cylinder goes to the general method (A:15) |
| O26 **ask** | Ids in the report, described by one resolver as the result leaves the kernel, with the descriptions inside the report | *Ids only* (proposal A): the descriptions go beside the report in an envelope, scratch ids live only for the call that failed, and the agent works from two documents, a shape S6 did not test. *Each site describes* (proposal B): every failure site must remember to, and item outputs carry model ids, against P8's rule (B:4) |

## 9. What it is least sure of

- **Fillets were never prototyped,** though L51 asks for prototypes before structures freeze. Topology's records do not
  freeze at the `topology` exit (only the geometry seam freezes, L21), and the native format is versioned from its first
  commit. So if fillets need what the records lack, such as edge convexity, the records grow. `blends` starts with the
  feasible-radius computation, the one part of the bracket test no spike touched. *Shown wrong by:* a fillet needing a
  change to the geometry seam after it froze.
- **Records against struct-of-arrays.** *Shown wrong by:* a phase whose profile is limited by traversing the store, such
  as the checker on 10^6 faces below 2 times at six cores.
- **No cross-family store.** *Shown wrong by:* NURBS-heavy work where lanes across instances would be worth more than
  about 20% (S3-P3's AVX2 figure, O17).
- **JSON as the only native format.** *Shown wrong by:* loading a model of 10^5 faces taking over a second, or the
  browser's workers spending more time parsing than meshing.
- **Descriptions inside reports.** *Shown wrong by:* blind runs on the real kernel doing no better than S6's, or reports
  routinely passing a few kilobytes.
- **How often `Undecided` happens.** P6 found the corpus never reaches the band edge, and no real model was counted.
  *Shown wrong by:* an exact-path share above about 1% in the first booleans on the NIST parts. The exact stage would then
  need an arena (P6).
- **Seams as edges.** S1 never compared them with a seam edge used twice. *Shown wrong by:* STEP faces that cross a seam,
  or agents puzzled by a hole made of two faces.
- **The default tolerance.** *Shown wrong by:* imported models holding real features between 1e-7 and 1e-6 mm, or
  reaching sizes where 1e-7 is too fine (O13).
