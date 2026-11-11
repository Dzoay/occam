<!-- `proving-ground.design`: the critic of proposal B, given the proposal and every source, and not the other proposal or its critique. "B:n" is line n of docs/evidence/2026-11-10-design-b.md. -->

# Critique of Proposal B, "records first"

Read against docs/BRIEF.md, every file in docs/decisions/, docs/PLAN.md and docs/plan/, docs/DETERMINISM.md, docs/PARITY.md,
all 17 findings, the phasekit lessons, and the sources of `occam-report`, `occam-math`, `occam-units` and `occam`, with
`Cargo.toml` and `clippy.toml`. Spike code was read with `git show spikes:`. Nothing was built. "§" means a section of the
proposal and "B:n" a line of `proposal-b.md`.

O22 (array-of-structs) and O26 (descriptions in `Report`) were assigned to this proposal. The points below accept both
choices and criticise only what they cost and how the proposal carries them out.

## The points, most harmful first

### 1. The records can hold only a closed solid, and nothing can be built from an empty model

**Claim (§3d, B:381-468).** Topology is a set of records: `Face { shell: ShellId, outer: LoopId, .. }`,
`Shell { solid: SolidId, first_face: FaceId, .. }`, `Loop { face: FaceId, .. }`, `EdgeUse { edge, owner: LoopId,
next: EdgeUseId, .. }` and `Edge { uses: [EdgeUseId; 2], ends: [VertexId; 2], .. }`. They are written through
`Edit::add`, `set`, `remove`, `derive` and `commit`. The proposal fixes this layout, and per §2 it is also the native file
format.

**What is wrong.**

- *Expressiveness.* Every face must belong to a shell, every shell to a solid, and every edge has exactly two uses. So
  the model cannot hold a wire, a lone planar face, an open shell, a free edge, or an edge with more than two faces.
  The first step of the bracket test is "extrude a profile". PARITY 1.6 makes that profile "a wire from edges … a planar
  face from a closed wire, with holes" in `construction`, and 1.13 is "Extrude a face along a vector". Neither the wire
  nor the face can be stored, named by a stable id, described or checked. The same gap blocks 2.15 ("Revolve a face or a
  wire"), 1.10 ("A check of *any* shape") and 2.12 (sewing that reports free edges).
- *Construction.* The links form cycles: an edge and its uses name each other, a face and its outer loop name each other,
  `EdgeUse::next` is a ring, and solid and shell name each other. None of these links is optional. `Id` has private
  fields and no public constructor (B:373), and `Edit` has no way to reserve an id. To add an `Edge` you need two
  `EdgeUseId`s, and to add an `EdgeUse` you need an `EdgeId`. In an empty model neither can be added first, so the very
  first primitive cannot be written.

**Evidence.** B:384-406 and B:458-467; PARITY.md rows 1.6, 1.10, 1.13, 2.12 and 2.15. S1's list (S1, "What the topology
has to hold") came from closed booleans of solids, so it says what a solid needs, not everything a model needs.

**Fix.** Add a body kind that can be a solid, a sheet or a wire. Make `Face::shell`, `Shell::solid` and `Loop::face`
optional, or point them at the body. Give an edge a list of uses, so that 1, 2 or n uses can be checked rather than being
impossible to represent. Add `Edit::reserve::<K>() -> Id<K>`, filled in later by `set`, and have `commit` refuse a
reserved id that was never filled. Prove it in the sketch by building a box from an empty `Snapshot`.

### 2. Per-item results (L6) have no type in the three places the bracket test needs them

**Claim.** §3f: `Fillet { edges: Vec<EntityRef>, radius: f64 }`, "Every edge gets an outcome; the model changes only if
all succeed (L6)". `Ran { steps: Vec<Result<Outcome<History>, Report>> }` (B:558). The boolean returns
`Result<Outcome<(Snapshot, History)>, Report>` (B:568-569) and reports "every failing item … in candidate order" (B:562).
For a fillet whose radius is too large, the remedy is `reduce-radius { edge, radius }` (B:589).

**What is wrong.**

- A fillet step yields one `Result`. If four edges are filleted and one radius is too large, the result is one `Report`,
  and the other three edges get no outcome. "Every edge gets an outcome" cannot be represented. `occam-report` already
  has `Batch<T>` for exactly this ("filleting 20 edges gives 20 outcomes", `crates/occam-report/src/outcome.rs`), and the
  proposal never uses it.
- There is one `radius` per step, but the remedy names one edge. To follow the remedy, the agent must either shrink every
  edge's radius, which changes what the user asked for, or split the step in two, which the proposal never describes.
  L3's own example is `ReduceRadius { edge, max_feasible }`. A remedy whose parameters the request schema cannot hold
  cannot be "followed" by L2's test in the way the bracket test means.
- "Every failing pair" from a boolean (P2, "What it decides") has to fit into one `Report`. `Partial` has a single
  `failing` list and a single flat `values` list (B:645), so each pair's residual gap and curves found are merged
  together. L5 asks for this state per pair.
- Neither `occam-mesh` nor `Surface::points`/`firsts` can report a failure per face or per lane. P9 reports a face that
  failed while its neighbours succeed. S4's "could not express" item 4 is exactly this case, and the proposal does not
  answer it.

**Evidence.** PLAN §3, "per-item batch results (L6, L7, L8)"; lessons L3, L5 and L6; P2, "What it decides" (failures
collected per item, in candidate order); P9, "Watertight by construction"; S4, "What could not be expressed", item 4.

**Fix.** `Fillet { edges: Vec<{ edge, radius }> }` gives a radius per edge, with a step result of `Batch<History>` plus
an atomicity flag that says whether anything was applied. The boolean returns a failure type that holds `Vec<Report>` in
candidate order. Mesh results carry `Vec<Result<Range, Report>>` per face. `points` and `firsts` get a checked variant
that returns the indices it could not evaluate.

### 3. The way back from array-of-structs is not "only `Table` changes" (O22)

**Claim (§6 O22, B:806-810; §3d).** "Records are read by value, so nothing outside the table module learns the layout …
If struct-of-arrays is chosen, only `Table` changes."

**What is wrong.**

- `Snapshot::get` returns a whole `K::Record` (B:441). Under struct-of-arrays, each `get` would gather every column. The
  benefit S3-P3 measured for struct-of-arrays comes from reading only the columns a pass needs: bounds reads only the
  coordinates, which gives 4.83 against 3.23 times speed-up with no output. A whole-record API therefore makes
  struct-of-arrays worse than what it replaces. The layout-neutral interface S3-P3 actually built was per field
  (`fn $field(&self, i) -> $ty`, implemented by both layouts in `spikes/s3-p3/src/tables.rs`). Changing from `get`
  to per-field access edits every caller.
- The S3-P3 numbers the proposal borrows were measured on smaller records. The spike linked records by `u32` slots, so
  its edge was 32 bytes and a face cost about 272 bytes in all. The proposal links every record with an 8-byte `Id` and
  adds fields, so its edge is 64 bytes, or 72 in its `Slot`. By my arithmetic from the sketch, a face costs about 440
  bytes in all. Copy-on-write copies whole chunks, so the "14 against 41 µs per scattered edit" and the snapshot figures
  do not carry over unchanged.
- The proposal also drops the spike's `prev`, `twin` and vertex-to-edge links ("A vertex's star is not stored; an
  operation that needs one indexes its snapshot", B:473). An index over the whole snapshot makes every fillet or chamfer
  cost time in proportion to the model, which is the cost P1 priced at 38 times.
- Reading by value from the surface table clones an `Arc<dyn Surface>` on every read. Parallel readers of one shared
  surface then contend on its atomic count. Faces that share a surface are the O25 case (B:822). S3-P3's parallel reads
  had no such traffic.

**Evidence.** S3-P3 Answer 3; S3-P3 "Bytes per entity"; `git show spikes:spikes/s3-p3/src/tables.rs` (field
accessors, u32 links, `coedge` on the vertex); P1 Answer 1.

**Fix.** Keep AoS, but make the table interface per field, or per named projection such as `bounds_of(face)` and
`ends_of(edge)`, so that SoA really is a change inside one module. Re-measure snapshot and edit cost on the real record
sizes, or pack links as u32 slots and check generations only at the API. Store the vertex-to-edge-use link, which costs
8 bytes a vertex. Let `get` on the geometry tables return `&dyn Surface` from the snapshot rather than a cloned `Arc`.

### 4. Descriptions (O26) as built: a central list of families, origins that are lost, and a clash with the P8 hook

**Claim (§3g, B:591-685).** `Form` is a closed `text_enum` in `occam-report`, "one per family". `Shape { form, values }`
in `occam-geom` repeats `Description { form, values, .. }` in `occam-report`. The origin comes from "the solid's
`step`" (`Solid { first_shell, step: u32 }`, B:382), and failure sites call `ctx.describe` and `ctx.found` (B:660-667).

**What is wrong.**

- A closed `Form` in the bottom crate is a list over families that every new family must edit. That is the pattern S4
  avoided, where each question "defaults to 'nothing', so a new family edits no list", and the one L26 rules out across
  families. It also undercuts B:46's "edits no other family". The native file needs a lossless tag per family, but
  `Shape` is deliberately lossy for B-splines (B:672). That makes a second list of families in `occam-io`.
- S6's top request was "the operation and operand it came from". The proposal stores the origin per solid, as a step
  index. After a cut, every face of the result reports the cut's step, including faces that came from the extrusion. The
  role a face played (target or tool) is not stored at all. `History.derived` records it, but the model does not keep
  it. Step numbers start again with each request, and the bracket test needs at least two requests (see "The bracket
  test, walked" below), so `Origin { step: 0 }` is ambiguous. Labels are not stored in the model, so `Origin.label` is
  always `None` once the request is over.
- The proposal sets a rule for P8: "an item function takes content and returns indices local to its item, never a model
  id" (B:586). Its own sample failure site builds the report inside the item, with `a.entity()` and `ctx.describe` (step
  numbers, extents). So item outputs carry model ids and numbering from this run, which P8 says must stay out of anything
  reusable.

**Evidence.** S4, "Answer" sketch comments and "What felt forced"; lesson L26; S6, "What `Report` is missing", first
row; P8, "What it decides" (no numbering from the run in cached outputs).

**Fix.** Have a family return its form as a text name and its values against a parameter list in the catalogue, held to
that list by a test, as remedies already are, rather than through a closed enum. Store the origin per entity (step,
role, operand label) in the record, carry it forward on `derive`, and make step numbers unique across requests (a model
counter). Items return failures in local terms only. The merge maps them to `EntityRef`s and describes them.

### 5. The checker measures the whole model again after every operation

**Claim (§3d, B:473-475; §3f, B:581).** After every success, in release builds too, the checker runs per face over the
whole model. Its checks include "each stored deviation not below a fresh measurement".

**What is wrong.** The boolean is designed so that work follows the edit ("the rest keep their ids and are labelled by
propagation", B:563). The checker then undoes that. A fresh deviation is a closest-point solve sampled along every edge.
S4 measured closest point on a bicubic at 30.4 µs by Newton, with no seed. On a model of 10^4 edges, checking a one-hole
cut costs far more than the cut. That is P1's plate ladder again, in a different phase.

**Evidence.** P1 Answer 1 (38 times the work for the same edit); S4 "Closest point"; first-release.md "Gates" (every
success passes the checker). Nothing there requires a full re-check of entities that did not change.

**Fix.** Check the entities that `History` names as created or modified, plus their adjacent entities, on every
success. Check everything when a model is loaded or imported, and in tests.

### 6. Scope changes that belong to the user: mesh-backed surfaces, and freezing structures before fillets are proven

**Claim (§2, B:45-46; §7, B:834-835; §8, B:847-848).** "Mesh-backed surfaces leave the first release" and `triangles()`
leaves the seam. Geometry freezes after a suite without them. Topology freezes in `topology`, before `blends`, and
"Fillets were never prototyped".

**What is wrong.** The plan the user approved puts mesh-backed surfaces in `occam-geom` (PLAN §4). first-release.md's
exit for `geometry` is "One seam serves all three families; then the seam is frozen (L21)". L21 exists because "toy
families passed, and the first real one still added four seams". Dropping a family and freezing without it is a change
of scope, and AGENTS.md says scope stops the work until the user decides. The fillet risk is the same. The records and
the file format freeze three milestones before the only part of the bracket test that no spike touched, and the
proposal concedes that "the records grow" if fillets need a vertex star or edge convexity (B:848). Because records are
the file format (§2), growing them means changing the file format.

**Evidence.** PLAN §4 and §6; first-release.md table; lessons L21 and L51; AGENTS.md "Hard rules", "Decisions that belong
to the user".

**Fix.** Put both to the user as decisions. Recommend either keeping a mesh-backed family in the `geometry` exit, or
recording a decision that defers it with `triangles()` held as a defaulted question. Recommend a short fillet prototype
(feasible radius on a box edge, and a corner where three filleted edges meet) before `topology` freezes, or a version
field in the native format from the first commit.

### 7. Tolerance: a default on the request that collides with the model's own, argued from a measuring artefact

**Claim (§3a; §3f, B:536-537; §6 O12, B:782-789).** `Tolerance` keeps "no default", but `Request.tolerance` defaults to
1e-6 mm and 1e-9 rad, because "results flagged open rose from 0 at 1e-6 to 3, 9 and 14 … 1e-6 is the smallest tolerance
with none, and every success must pass the checker".

**What is wrong.**

- The bedrock code already separates the model's tolerance from an operation's. `Model::tolerance` is "The tolerance it
  is built to. No operation on it can be asked for anything finer". `Code::ToleranceBelowModel` and the remedy
  `use-model-tolerance` exist for that case. The proposal never says which tolerance governs when a request runs on a
  loaded model. If the request's default of 1e-6 is finer than the model's, the request either fails or silently
  replaces the model's tolerance, which breaks "per-model `Tolerance`, never a global one" (L22; `occam-units` crate doc,
  "there is no default").
- S1's counts are quoted correctly, but S1 says *open* grows "which is what a test that scales with the tolerance would
  do against fixed quadrature noise". *Open* includes the net-flux test, and the proposal's checker (B:473-475) has no
  flux test. So the argument "every success must pass the checker" does not reach those three results.
- "Leaves room to models of about 100 m" extrapolates a scale effect that S2 says "the failures were not broken down by
  scale" for. S1's own range is "models up to about 100 mm".

**Evidence.** `crates/occam/src/model.rs`; `crates/occam-report/src/code.rs`, `ToleranceBelowModel`; S1 tolerance sweep
(lines 130-138); S2 lines 104-106.

**Fix.** The model owns the tolerance. A new model takes it from the request, which must state it, or from a documented
default that is reported in the outcome. A request on an existing model may only ask for the model's tolerance or a
coarser one, as bedrock already does. Argue O12 from S2's position uncertainty and from P9's 1e-8 feature limit, not
from flux counts, and ask the user.

### 8. Choices about execution and lane width that the interfaces cannot carry out

**Claim (§3b, B:191-255; §3c, B:338-339; §5, B:758-773).**

**What is wrong.**

- *Predicate batches.* `Exec::map` calls `f(i)` once per item (P11's interface: "f returns one result per item").
  `Grain::Fixed(256)` therefore groups calls, not data. P6's chunk was a lane group: items "grouped by family inside each
  chunk, so lanes stay homogeneous", and "A chunk of 64 items also loses lane width". A filter in `Lanes<4>` cannot run
  inside a per-item `f`. The §5 row "Fixed(256) to Fixed(1024) … filter at 4" has to become `Each` over chunks that
  return `Vec<Sign>`, and `Fixed` is then left with no user.
- *Grain.* The `Grain::Each` doc says "for items of 10 us and more" (P11). §5 nevertheless puts broad phase and per-edge
  box tests on `Each`, and those are sub-microsecond items. Under `rayon`, P11 measured 3.94× at 100 ns with `Each`,
  against 5.65× with `Auto`.
- *One surface width.* `SURFACE_LANES = 2` serves every family and both `points` and `firsts` through the blanket impl.
  §5 says "rational with derivatives 1" (P4), and P9's own table has degree-3 NURBS slower at width 2 than at width 1
  (87.2 against 84.4 ns). Most of P9's "1.2 to 3.3" comes from batching, not lanes. The per-class widths in §5 cannot be
  expressed in the seam in §3c.
- *`widest`.* `widest(kernel: impl FnOnce() -> T)` puts a closure inside `dispatch!`. P4 says "The chain from the dispatch
  closure to every leaf must be `#[inline(always)]`, or the leaf is compiled without the target features". The spike's
  working form calls a named `#[inline(always)]` generic function. A closure expression cannot carry that attribute on
  stable Rust, because attributes on expressions are unstable. `widest` also has no `Level`, so it needs a hidden global.

**Evidence.** P6, "Where the work piles up" and "What was measured"; P11 Answer and the overhead tables; P9 "CPU lanes";
P4 "Plain arrays against `fearless_simd`"; `git show spikes:spikes/p4/src/run.rs` lines 106-113.

**Fix.** Give the predicate batch a chunked map, and drop `Fixed` or define it as "the caller's chunk". Put cheap items
on `Auto`. Make the lane width an associated constant per family and per order (`Parametric::LANES_POINT`,
`LANES_FIRST`). Make `widest` take `level: Level` and a `trait LaneKernel` whose `run` the implementer marks
`#[inline(always)]`.

### 9. O23 restated wrongly, and a number its own finding disclaimed used against a store

**Claim (§3c, B:269-273; §6 O23, B:812-813).** "Assumed as recommended: sorted calls, per-surface slices, no
cross-instance store." Against a store: "S4's was 2.6 times slower than per-call on analytic surfaces."

**What is wrong.** PLAN's recommendation for O23 is "batch NURBS by degree, for lanes, on targets with wide vectors,
**and group surfaces by kind to save memory**". The proposal keeps one `Arc<dyn Surface>` per slot, which is S3-P3's
"boxed, mixed order", 15% larger than grouped storage. S4 says of its 2.6× that it "re-sorts 5000 queries and allocates
on every pass … this measures this implementation, not batching". That number is not evidence against a store.

**Evidence.** PLAN §7 O23; S3-P3 lines 329 and 359; S4 lines 157-159.

**Fix.** Say plainly that the proposal departs from O23's recommendation and why: simplicity, at a cost of 15% of surface
memory. Drop the 2.6× argument, or cite it with S4's caveat.

### 10. "What changes if answered otherwise" understated for O11 and O18

- **O11 (B:778-780).** "A forced snap, wrong against nominal in 8%." The 8% is `b-interval-snap`, which still refuses 44%
  of steps (S2 line 165). A policy that always decides was measured as the variants that never fail: wrong against
  nominal in 25% to 44% (`c1-exact-pred+eps` 25%, `c0-exact-pred` 44%, the per-entity models 27% to 38%). So the honest
  cost of answering no is three to five times larger than stated.
- **O18 (B:799-800).** "No kernel crate changes." But `Runner` lives in `occam-math`, a kernel crate, so a shared-memory
  arm changes it. P11 adds that rayon on browser threads needs "an adapter crate", which is a new dependency for the user
  to approve under O5. P10 lists COOP/COEP on every page, `-Zbuild-std` and hand-written start-up.

**Fix.** Restate both answers with these costs.

### 11. The report changes lose NaN and infinity

**Claim (§3g, B:602-605).** `NamedValue.value` becomes `#[serde(untagged)] enum Value { Scalar(f64), Triple([f64; 3]) }`.

**What is wrong.** Today `value` goes through `#[serde(with = "number")]`, which writes NaN and the infinities as text,
because "a solver's residual can be either" (`crates/occam-report/src/value.rs`, the `number` module). A derived
`Scalar(f64)` writes NaN as `null`, and that does not read back into either variant. S6's c12 had `gap` infinite, so this
is a real failure state (L5) that would stop round-tripping. The existing test
`numbers_json_cannot_hold_are_written_as_text` would fail. `shown()`, `message()` and every caller of `.value` change too,
and the proposal lists none of them.

**Fix.** Keep `number` for `Scalar` and for each component of `Triple`, and list the existing items that change in the
sketch.

### 12. Gaps in determinism: save and load, the browser merge, and re-sending the model

- **Save and load.** Ids depend on free slots "reused last-in first-out" (B:740). If the JSON file does not carry each
  slot's generation and the order of the free list, a model that is saved and loaded gives different ids from the same
  model kept in memory. The format section does not say that it carries them.
- **The browser mesh** is put together by the page from 64 `mesh_part` calls, but `gates determinism` runs wasm under
  wasmtime as one instance. The JavaScript merge, with its index offsets and welding, is outside the contract. Either the
  merge moves into wasm, or the gate drives the pool.
- **Model transport.** "Models and requests cross as text" and "Other operations run on one worker" (B:716, 733). After
  every operation, the new model is written as JSON and parsed by every mesh worker. P10 warns that "a boolean over a
  large model, where every worker needs the whole model, is where the pool's copies would cost most". The proposal's own
  §8 doubts JSON load time at 10^5 faces. `Session` also has no method that returns the model, so the page cannot get
  the model it is supposed to re-send.

**Fix.** Write the generation of every slot and the free list into the file, and round-trip a model through a test that
compares ids after save and load. Merge in wasm. Add `Session::save`. Measure JSON parse time at 10^5 faces before
choosing JSON as the only format.

### 13. Smaller problems in the sketch

- `#[derive(.., PartialEq, Eq, PartialOrd, Ord)] struct Id<K: Kind>` puts `K: Eq + Ord + Debug` on those impls. `Kind`
  requires only `Copy + PartialEq`, so in generic code `Id<K>` is neither `Eq`, `Ord` nor `Debug`. Either add those
  bounds to `Kind` or write the impls by hand.
- `Units` (B:535), `ShellId` and the other aliases, the `*K` kind markers, `Jet1`, `Closest` and `Context` are not
  defined, and `todo!()` is denied by `todo = "deny"`. proving-ground.md asks for "a compiled type sketch", and this one
  has not been compiled.
- A `Role` text enum in `occam-report` (B:625) clashes by name with `occam_units::Role`, the Position/Displacement trait.
- `Report::error(code, fault)` sits beside the existing `Report::new(code)`. Severity is per code, but fault is per
  occurrence, so `new` needs to be removed or given a default fault, and S6 warned that a default per code misleads.
- `Parametric::invert` defaults to the full `closest` solve. P8's saving ("half of a point query") then exists only for
  families that override it.
- `Methods::standard` reaches the hybrid only "through `bspline()`" (B:509-510). An offset surface has no exact B-spline
  form, so it has no intersection method, although PARITY 2.2 puts offsets in `geometry`.
- The mesh gives welded positions with normals per corner (B:702-703). P12 asks for vertex buffers that "a storage
  binding and a vertex fetch can read", one per point, with index buffers stable across edits. One index cannot address
  both arrays. The renderer would have to un-weld the mesh, and that loses the stable indices.

### 14. Deferred too cheaply, or not addressed

- **O25 (B:819-823).** "No follow-up spike … No record changes." P2's pipeline excluded coincident surfaces ("no
  coincident surfaces"), and S1 calls them "the largest remaining failure class". Coincidence adds intersections of
  boundaries inside a shared surface, which phase 3's frozen vertex table and classify were never shown to carry. The
  bracket test can hit this case. The `move-apart` remedy keeps it survivable, but "no record changes" is asserted, not
  shown.
- **S5's rule 4.** "the seam's side and a segment count on an integer need a rule of their own" is not carried into the
  design.
- **Later features.** Persistent naming and incremental recompute are served in outline by `History.derived` and by the
  content-in, indices-out rule. Point 4 shows the rule is already broken in the sketch. Server-side rendering is not
  mentioned, but nothing here prevents it.

## The bracket test, walked

1. **The agent's tool.** The CLI is not specified. `occam describe` is named, but `occam run`, how a model is passed in
   and saved, and how outcomes list created entities with their descriptions are not. Fillet edges are `EntityRef`s,
   which exist only after the extrusion and cut have run. So the test needs at least two invocations with a model file
   between them, and the proposal does not describe that flow.
2. **Extrude a profile.** The profile cannot be stored (point 1). Either `Extrude` carries the profile inline as data,
   which the proposal does not say, or this step has no answer.
3. **Cut a bolt hole.** `Cut { target: Label, tool: Label }` is clear. A hole whose tool is as tall as the plate is the
   O25 case. The `move-apart` remedy covers it until coincident handling lands. The hole becomes two half-cylinder faces,
   because of seam edges (§8 flags this).
4. **Fillet with one radius too large.** The report is good: a code, the edge, its description, the feasible radius and
   the remedy. Following the remedy is not well defined (point 2), and the other edges get no outcome. How the feasible
   radius is computed is unspecified, and §7 admits as much.
5. **View in the browser.** The parts are there: `Session`, the pool and `mesh_part`. But the model must reach every
   worker as text, and the page merges the mesh outside the determinism gate (point 12).
6. **STEP and the oracle.** `occam-io` is named and nothing more is said. Volume (PARITY 1.8) does not appear in the
   design.

## The findings, checked

| Finding | What the proposal cites | Verdict |
|---|---|---|
| S1 | items 1-7, 9, 10; sweep counts; 32 coincident reports | Accurate. The O12 argument leans on *open* counts that S1 attributes to the flux test (point 7). |
| S2 | band, interval then expansions, 32-byte vertex, towers, 11.1% and 0.2% | Accurate, except the "8%" under O11 (point 10) and the scale extrapolation (point 7). |
| S3-P3 | 1.62/1.88, 14/41 µs, 183/1,191 µs, 1.37/1.98 µs, 5.4-5.6× tasks | Numbers right, but taken from different chunk sizes and smaller records (point 3). The advice to group surfaces is dropped (point 9). |
| S4 | two layers, cone, 14-18%, E0119, 2.6× | The 2.6× is used against S4's own caveat (point 9). Failure per lane is not addressed (point 2). The frame-generic E0119 case is fixed. |
| S5 | 0.75 and 1.4 ulp, 1.5-2.7×, folding | Accurate. Rule 4 is omitted (point 14). |
| S6 | packed ids, `last_valid` unused, fault per occurrence, c08 | Accurate. The origin request is only half met (point 4). |
| P1 | 38×, propagation | Accurate. The checker contradicts it (point 5). |
| P2 | shape (a), three rounds, 4.7-5.0× | Accurate. "Every failing pair" does not fit one `Report` (point 2). |
| P4 | widths per class, 2.3×, 5.8-5.9×, about 1% | Accurate. The inlining requirement is not met (point 8). |
| P5 | closed forms then hybrid, rounds of 32, margin, <3%, one silent failure | Accurate. |
| P6 | 3.3 µs, 1.48, 256-1024, corpus never at the band edge | Accurate. Its chunks were lane groups (point 8). |
| P7 | not cited | Consistent with the design (no mesh seed). Harmless. |
| P8 | 33-34%, 66-86%, `ptr_eq` | Accurate. The rule against numbering from the run is broken in the sketch (point 4). |
| P9 | limits, 12 GB, grid failures, 1-3%, width 2 | Mostly accurate. "1.2-3.3" is mostly batching. Failure per face is not carried (point 2). |
| P10 | pool, 64 calls, 1.4-2.3× | Accurate. The warning about a copy per worker is not faced (point 12). |
| P11 | interface, refusal on wasm | Accurate. The `Each` threshold is broken in §5 and the adapter crate is omitted under O18 (points 8 and 10). |
| P12 | contract items | Listed, but normals per corner do not fit the vertex layout (point 13). |

**The proposal's claims about the sources.** That the approved PARITY rows 2.1 and 2.2 ("evaluation to any derivative",
arriving in `geometry`) answer O21 against PLAN's "keep two orders" is **confirmed**. AGENTS.md ranks decisions above
PLAN, so PLAN §7 O21 should be corrected in the same PR, and the user should still be told. That PARITY 1.10 omits
self-intersection is **confirmed**. "No parity row needs mesh-backed surfaces" is **true but beside the point** (point 6).
That analytic and B-spline data can be recovered from `dyn Surface` without a downcast is **true only for those two
families**: `bspline()` is a typed accessor for one family, a second asymmetry of the kind S4 flagged for `analytic()`.
That P4 and P9 disagree on two lanes "only because P9 computed sine and cosine in lanes" is **plausible but an
inference**: P4 itself explains its result by loop vectorisation of `f64` and by memory traffic.

## What the architecture should keep

- `Exec` as a concrete ordered map with `Grain`: `rayon` named in one module, `Serial` on wasm, closures `'static`, no
  reductions. This follows P11 and the rayon decision exactly.
- `Real` gaining compare and select and the owned `sin_cos`/`atan2`; `from_fn` as a loop (P4); `Arc::ptr_eq` and
  `std::ptr::eq` banned in kernel crates (P8). The last is cheap and catches a real class of bug.
- `Section` keeping what was found beside what stopped it, with `Unresolved` as a part of its own (P5, O24, L5). This is
  better than S4's `Option<Result<..>>`.
- An evaluator by explicit order and a query for parameters only, in the seam from the start (P8 rank 1), with three
  fixed rungs kept for hot paths.
- A boolean of pure phases with ids given only in the final `Edit`, faces outside the candidate pairs left alone, and
  propagation in classify (P1, P2).
- Validation before geometry with its own codes, a fault class per occurrence, reasons as codes, empty results as a
  diagnostic, and the decision tally as a note (S6, P6). `EntityRef` as kind, slot and generation (S6), and surfaces and
  curves as kinds of entity (S4).
- P9's limits in the mesh API, predicates on real coordinates, every browser worker sampling shared edges itself, and 64
  calls per job (P9, P10).
- §8's honest list of where the design is least sure.
