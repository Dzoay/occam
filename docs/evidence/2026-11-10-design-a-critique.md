<!-- `proving-ground.design`: the critic of proposal A, given the proposal and every source, and not the other proposal or its critique. "The proposal" is docs/evidence/2026-11-10-design-a.md. -->

# Critique of proposal A, "columns first"

What was read: the proposal; docs/BRIEF.md; docs/PLAN.md; docs/plan/proving-ground.md and first-release.md; every file in
docs/decisions/; docs/DETERMINISM.md; docs/PARITY.md; all 17 findings; the lessons file for the L-numbers used; and
`crates/occam-report/src`, `crates/occam-math/src`, `crates/occam-units/src`, `Cargo.toml` and `clippy.toml`. Nothing was
built or run. Whether the sketch compiles was judged by reading it against those crates.

O22 (struct-of-arrays) and O26 (ids only) were assigned to this proposal. The points below do not fault it for making
those choices, only for how it carries them out and what it claims for them.

Points are in order of harm if the architecture adopted them unchanged, most harmful first.

## 1. The evidence for column-first geometry measured something else, and the gate meets the seam freeze

**Claim** (§1, §3c, O23, §7 `geometry`): bulk work of one kind dominates, so geometry lives in one column per family and
degree, "batch calls are the primary API; a per-item call is a view over the columns", and `SurfaceColumn` is the layer
everything outside a family calls. O23 rewords L29 as "group geometry by family and degree, always", partly "for one
dispatch per batch (sorting by kind 14 to 18%, S4; batching 1.8 to 3.3 times on analytic surfaces and 1.2 to 1.4 on
NURBS, P9)".

**What is wrong.** The three numbers in §1 do not support storing geometry in columns or exposing columns as the seam:

- **P9's 1.8 to 3.3 is not about grouping.** In P9's "CPU lanes" table, "One at a time" is "a batch of 1 through the same
  dispatch". The 1.8 to 3.3 compares 256 points on *one surface* with one point per call. That is the "one surface, many
  parameters" shape, which the proposal itself calls "the dominant batch" (§3c). It needs a slice method on one surface,
  not a store of many surfaces: S4's per-slice table already gets the cylinder from 16.6 ns per call to 14.5 ns at a slice
  of 4,096 through `dyn Surface`.
- **S4's 14 to 18% is sorting, not storing.** S4 ("Mixed surfaces in one loop") says: "Sorted per-call is the cheap
  version of L29 that these numbers support." The one store S4 built was 2.6 times worse than the enum on analytic
  surfaces, and S4's "What it decides" item 4 says "the store still has to earn its place".
- **P1's 61 to 67% is removable waste.** P1 ("Phase shares") says of that sampling: "the cost is a choice, not a law".
  P8 ("Together") took near-crossed from 307 to 157 ms with explicit order and culling alone. The proposal concedes this in
  §8 but still leads §1 with it.
- **P6's lanes are worth 1.48 times** on a filter that costs 44 to 56 ns scalar (P6, "What a decision costs"). "Robust
  decisions are cheap only when a lane filter takes them in groups" (§1) overstates that. At 2.8% exact-path decisions,
  the exact path takes 75% of the time and lanes gain 1.12.

The design also makes one of the three required families worse. In S4 the mesh-backed surface is 84.3 ns as `f64` and
98.9 ns through a store column, and slicing makes it "about 20% slower". L21 and first-release.md's `geometry` exit ("One
seam serves all three families; then the seam is frozen") require the seam to be proven on mesh-backed bodies before it
freezes. The proposed `geometry` gate (§7) covers only S4's analytic and NURBS workloads.

**Why it matters.** The gate that tests the premise is at the `geometry` exit, which is also when the seam freezes. §8's
fallback ("analytic evaluation would go per item, and column batches would serve only NURBS and predicates") keeps
`SurfaceColumn`, `GeomId { column, item }`, `appended` and `decode` as the seam. So if the gate fails, the seam has to be
redesigned at the moment the plan says to freeze it.

**Fix.** Make the per-surface object trait the layer-2 seam: S4's `Surface<F>` with batch methods that take `Order`,
caller-owned outputs, `Scratch` and `Defined`. Keep family-grouped storage as an internal detail of `Geometry`, for memory
(S3-P3: 15.4%) and for sorting. Add the across-item path (many surfaces, one parameter each) as an internal kernel for
predicates and bounds, where P6 measured it, not as the seam. Add the mesh-backed family to the `geometry` gate. Word O23
from S4's sorting and S3-P3's memory result, not from P9.

## 2. O11 stores the near-tangent shift as the certified radius, which is S2's `b-interval`, not `b-interval-snap`

**Claim** (O11, §3d `Vertices.radius`, "certified (S2 item 3); not a tolerance"): "A band decision at near tangency snaps
one vertex to the foot and stores `near_tangent_shift` as its radius: S2's `b-interval-snap`, which ... failed 44%."

**What is wrong.** S2's model table defines `b-interval-snap` as placing "one vertex at the foot, **with no hull of the
two roots**". The hull of about `sqrt(2 R eps)` is what plain `b-interval` carries forward, and that model fails 86% of
chain steps, rising to 100% from step seven (S2, "Composition: construction chains"). S2 credits the snap's lower rate to
the fact that "a snapped vertex keeps a tight interval". S2's Answer item 4 puts `sqrt(2 R eps)` in *the report* ("the
report records the position uncertainty"), not in the vertex's certified radius. On the snap the stored interval
encloses the true vertex in only 240 of 320 steps, so the radius certifies the computation, not the distance from
nominal.

Storing 1.1e-3 mm (O12's own figure at R = 6 mm) as the radius of a vertex in a 1e-7 mm band brings the hull back. Every
later decision about anything within about a micrometre of that vertex then straddles the band and returns `Undecided`.
That is the 86% to 100% behaviour, not the 44% the proposal quotes. The tolerance model is among the hardest choices to
change later, and here it contradicts the finding it cites.

**Fix.** Keep the interval's own radius on the vertex. Report the shift as a named value on the band decision's
diagnostic, and if it must be stored, give it a separate field that no decision reads. Also state the checker's rule for a
snapped vertex: within radius plus band of its curves, not within radius alone.

## 3. The bracket test cannot follow its own remedy

**Claim** (§3f, §3g, §7 `blends`): `Op::Fillet { edges: Vec<Selector>, radius: f64, commit: Commit }`; "Filleting 20 edges
gives a `Batch` of 20 outcomes"; `reduce-radius` carries "the largest feasible radius"; and the envelope example names
`edge:41/1` and 4.98 mm.

**What is wrong.** Walk PLAN §2's bracket test through it:

- **The agent cannot name the edges.** `Selector` is undefined. Nothing lists entities by kind or answers adjacency
  queries, which PARITY 1.1 (tier 1) requires: "Entities can be listed by kind, without duplicates, and adjacency
  queried". On success, the `created` ids in `Changes` are not resolved, because the runner resolves only ids "in a
  result's reports, diagnostics, remedies and partial state" (§3g). The agent gets bare ids and must call `occam
  describe` once for each.
- **The remedy cannot be followed in one request.** `radius` is one number per operation. Reducing edge 41 alone means
  splitting it into a second fillet operation. By then its neighbours carry 6 mm fillets, so the 4.98 mm limit computed
  for 20 simultaneous fillets no longer applies, and edge 41's id may have changed. The proposal never says when
  `generation` changes: on slot reuse only, or on any change.
- **`Batch` and `Commit::AllOrNothing` are never reconciled.** If one item fails and nothing is committed, what do the
  other 19 "succeeded" outcomes refer to? The example envelope is a single `report`, not a batch.
- **A failure cannot be pinned to one item.** The checker runs once in `Draft::seal`, which returns one `Report`, so a
  bad fillet is not attributed to an edge. Fillets that meet at a shared vertex interact, and which edge fails there is
  not defined. L6 and PARITY 1.14 depend on that answer.
- **"Largest feasible radius" is a band-edge value.** At the supremum the fillet consumes the neighbouring face. Following
  the remedy exactly lands on the "cannot decide" outcome of point 2. A margin rule is needed, and none is stated.
- **Volume is not designed.** The oracle check is "matching volume", and PARITY 1.8 (volume, area, centre of mass) is
  tier 1, but no volume computation appears anywhere.

**Fix.** Give `Fillet` per-edge radii (`Vec<(Selector, Length)>`) so the remedy is followed in the same request. Define
`Selector` and a `list`/`adjacent` query on the CLI. Resolve the ids in `Changes` too. Define `generation`: bump on slot
reuse only, and record changes in `Changes.changed`. Define what `Batch` items mean under `AllOrNothing`. Have `seal`
return failures per item, or check each item's local region before the global seal. State the remedy's margin: the
largest radius the kernel certifies, not the supremum.

## 4. O21 contradicts the approved parity list

**Claim** (O21, §3c `Order { Point, First, Second }`): "two orders, differencing above", needed "if a named feature needs
a third".

**What is wrong.** PARITY rows 2.1 and 2.2, approved by the user (2026-11-09-parity-approved.md), say "evaluation to any
derivative" for all curve and surface kinds, offsets included. That is the named feature. S4's open question 1 says the
fixed ladder "is the choice that is hardest to change after the seam freezes", and S4 measured differencing at about
1e-5 for an offset's second derivative. `Order` is a closed enum inside the frozen seam, so "`Order` gains `Third`" later
is a seam change. The proposal does not mention PARITY at all.

**Fix.** Raise the conflict with the user, as AGENTS.md requires when sources disagree. Either PARITY 2.1 and 2.2 are read
as "differenced above two" and amended, or the seam takes a jet of chosen order (S4's alternative) or an `Order(n)` with
a per-family maximum that is exact where the family can be.

## 5. Typed units stop at the topology tables

**Claim** (§3a, §3d, §3g): parameters stay plain `f64` "for lane kernels". The tables hold `x`, `y`, `z`, `radius`,
`deviation`, `t0`, `t1`, `witness_u` and `witness_v` as `Chunked<f64>`. `Tangent` is `Triple<Rate, F, R>`.
`Description.extent` and `at` are bare `[f64; 3]`.

**What is wrong.**

- PLAN §3 (from the brief) commits to "typed quantities and frames inside", and L22 to frame types from day one. Vertex
  positions, a certified radius and a measured deviation are lengths in a frame. Storing them untyped is a choice this
  proposal made, not a consequence of O22: `Measure<U, R>` in `measure.rs` is a plain newtype, so `Chunked<Length>` and
  `Chunked<Point<World>>` are possible.
- S1 ("Where `occam-units` got in the way") found "Vertices hold `Point<World>` ... which were both fine". The proposal
  removes what S1 found fine.
- S4 concluded that a derivative is *not* three lengths, and its sketch made `Tangent` a newtype over `[R; 3]`.
  `Triple<M, F, R>` in `frame.rs` stores `Length<R>` fields, and `x()` returns a `Length`. So the proposal's `Tangent` is
  three lengths with a different marker, which is exactly what S4 rejected.
- `Description`'s arrays are internal millimetres, while `message` is shown in the caller's units (L9; `NamedValue::shown`).
  An agent working in inches gets two unit systems in one envelope.
- Splitting x, y and z into three columns is the layout S3-P3 (Answer 3) blames for SoA's loss on scrambled bounds (379
  against 297 ns): "a vertex's three coordinates are then three cache misses".

**Fix.** Use `Chunked<Point<World>>` for vertices, which is still struct-of-arrays across fields and avoids the
three-miss read, and typed lengths for `radius` and `deviation`. Give `Tangent` S4's newtype. Put `Description`'s numbers
through `NamedValue`, or give them a quantity and convert them like the message.

## 6. Geometry has no id, no lifecycle, and no way to add a family

**Claim** (§3c, §3d, §3g): `GeomId { column: u16, item: u32 }`; `appended` returns "a new version with `keys` appended";
`Family::decode(key)` is "how a column absorbs another's items"; columns are "created only through the registry `occam`
fills"; `EntityKind` gains `Surface` and `Curve`.

**What is wrong.**

- **A surface cannot appear in a report.** A `GeomId` is 48 bits (column and item). `EntityRef` is `{ kind, slot: u32,
  generation: u32 }`. S4's "What it decides" item 5 asked for "a way to name a surface with no topology", and the new
  `EntityKind::Surface` has no defined mapping.
- **Geometry is append-only.** It has no generation and no deletion. A long session keeps every surface it ever made, and
  compacting would renumber every `GeomId` stored in `Edges.curve`, `Faces.surface` and `Coedges.pcurve`. Geometry is
  then not a stable-id entity, though PLAN §3 says every entity is.
- **`Draft::apply` cannot create a column.** `apply(&mut self, plans)` lives in `occam-topo` and has no route to the
  registry in `occam`, the top crate. The bracket's fillet between the plate and the bolt hole makes a torus, probably the
  model's first, and a NURBS of a new degree needs a new batch-key column. Neither has a column to append to.
- **Composite families cannot decode their base.** `decode` is a static function with no registry. An offset, an
  extrusion of any curve or a surface of revolution (PARITY 2.2; S4's own extrusion and offset families) holds a base of
  another family, which it cannot decode. Referring to the base by `GeomId` instead would put run-specific ids in a
  content key, against P8.

**Fix.** Give geometry a slot-and-generation table that maps to (column, item), so `EntityRef` can name it. Define when
geometry is freed. Pass a registry handle into `apply` and `decode`, for example held by `Geometry`, and show how a nested
content key decodes.

## 7. The tables cannot hold a wire or a free edge

**Claim** (§3d): seven tables. `Loops.face` is mandatory, `Coedges.owner` is a `Loop`, and `Edges.first_use` is a
`Coedge`.

**What is wrong.** PARITY 1.1 (tier 1, `topology`) lists "shells, faces, wires, edges and vertices". PARITY 1.6 (tier 1,
`construction`) needs "a wire from edges, with a gap reported by location", and 2.12 needs "reporting free edges". A wire
that bounds no face, or an edge that belongs to no loop, has nowhere to live. `Op::Extrude { profile: Profile }` avoids
the problem for the bracket by keeping the profile as plain data, but the topology milestone's own parity rows need these
entities.

**Fix.** Add a wire owner (a loop whose face is optional, or a wire table), allow an edge with no use, and show the shell,
solid and compound tables the sketch only mentions.

## 8. Struct-of-arrays at 1,024 needs a directory tree, and the costs are quoted at 4,096

**Claim** (§1, §3b `Chunked`, §3d, O22, §8): chunks of 1,024. "An edit in place costs more (1.98 against 1.37 µs), and so
does a snapshot (314 against 47 µs)." A two-level tree of fan-out 64 "attacks the snapshot cost ... unmeasured".

**What is wrong.** These are the 4,096 figures. In S3-P3's "Snapshots and edits" table at the chosen 1,024, a
struct-of-arrays snapshot is 1,191 µs against 183 µs for array-of-structs. That is 6.5 times, where the figure in §1 and
O22 reads as 6.7 times on a smaller base. §3d does mention the 1,191, but only as a cost the tree "should remove". The
read-scaling case (4.83 against 3.23, 2.42 against 1.86) and the walk (2.48 against 3.55 ns) are also 4,096 figures, from
what S3-P3 calls "a memory-bound microbenchmark" ("What this does not show"). It adds that "heavier per-face work scales
much better", 5.4 to 5.6 times whatever the layout.

The tree is new structure that exists only to repair a cost the chunk-size choice created. No operation has been shown to
care: about 1 ms per operation at 10^6 faces, against booleans measured in hundreds of milliseconds. The type does not
express the prose either. `Arc<Vec<Arc<Vec<Arc<[T; CHUNK]>>>>>` bounds neither level to 64, and "a third level is added
past that" is impossible without changing the type.

**Fix.** Take struct-of-arrays at 4,096, the measured configuration (snapshot 314 µs, scattered edits 29.7 µs each), with
a flat directory. Add a tree only when a profile asks for one, or measure struct-of-arrays at 1,024 with the tree before
choosing it. This does not reopen O22.

## 9. Structure that no finding asked for

The brief asks for a "clean, simple, FAST library". Beyond what points 1 and 8 cover, the proposal adds:

- **`occam-exec` as its own crate, with `occam-math` depending on it** "so the determinism stream runs through `Exec`"
  (§2). That pulls the threading runtime under the numbers crate for one test. The rayon decision needs only that one
  crate name `rayon`, and `Chunked` is storage, not execution.
- **`Width` types per family** (`OnWidth`, `AcrossWidth`). §8 admits they "become dead weight" if the vectorisation gate
  sets most kernels to width 1, and S3-P3, P5 and P9 all found little lane gain on the shipped build.
- **`Phase::key`, "unused for now"**, with `ContentKey` (which requires a `FamilyTag`) reused for phase inputs that are
  not geometry.
- **One chunk as the unit of storage, copy-on-write, task and lane batch** (§1). P6 and P11 size tasks by time (15 to
  60 µs; `Each` above about 10 µs), not by records. A 1,024-record walk is about 2.5 µs. With `rayon` the loss is small
  (P11: 5.21 times at 1 µs per item), so the coupling costs little, but it buys nothing either.

The simpler alternative keeps PLAN §4's crates, puts the runner in `occam-math` or a module beside it, fixes `CHUNK` at
4,096 with a flat directory, stores geometry in per-family vectors sorted by kind, and exposes S4's per-surface seam with
batch methods. What it gives up has not been measured to matter: an across-item column API whose only measured version
lost 2.6 times.

## 10. Lane dispatch cannot cross the `dyn` seam, and some width figures come from forbidden builds

**Claim** (§3b, §3c, §5): `dispatch<K: Kernel>`, with "`run` and all it calls must be `#[inline(always)]` (P4)", and a
lane width per family.

**What is wrong.**

- P4 ("Plain arrays against `fearless_simd`") says the chain from the dispatch closure to every leaf must be
  `#[inline(always)]`, or the leaf is compiled without the target features. A call through `dyn SurfaceColumn` cannot be
  inlined. Dispatch therefore has to happen inside each family's column, but `eval_on` and `eval_across` take no `Level`,
  and the sketch never says where dispatch happens.
- P4 sets widths "per kernel class", and a family runs several classes: NURBS point grids at width 2, rational
  derivatives at width 1 (P4's Answer table). One `OnWidth` per family cannot express that.
- "`bounds` at eight lanes with AVX2 reached 30.7 times" (§5) is P4's `-C target-cpu=x86-64-v3` build, which rule 5
  forbids. Through `dispatch!` from the baseline build, P4 measured 24.3.
- "`sin_cos` ... 4.3 at 4 in an AVX2 build", used for "4 via dispatch", is S5's "AVX2 and FMA" build column, which S5
  calls "what occam gives up by building one baseline". P4 found that `dispatch!` from the baseline build does not reach a
  v3 build's speed on `analytic` (1.39 against 0.80 ns per point).
- O14's "S4's lanes bought nothing because `sin_cos` ran one lane at a time" is narrower than S4, which also names knot
  spans and mesh point location. S4's plane, which has no trigonometry, lost 0.52 times in lanes.

**Fix.** Pass `Level`, or a dispatched kernel set, into each column, and dispatch inside it. Choose widths per pair of
family and order. Cite the baseline-build figures, and treat AVX2 widths for `sin_cos` as unmeasured until the
vectorisation gate measures them.

## 11. Two column types that do not meet

**Claim** (§3a, §3b, §3d): batch kernels read and write `Points<F>` and `UvPoints` (`Vec<f64>` per axis), `decide` takes
`columns: &[&[f64]]`, and topology stores `Chunked<f64>`. "Each task returns a whole output chunk and the chunk list is
the output column, so the merge moves pointers."

**What is wrong.** `Chunked` exposes only `get` and `set`, with no slice per chunk, so every batch over topology data has
to gather into `Points` first. The pointer-moving merge works only for one-to-one maps. Sections per pair, triangles per
face and split edges are variable-sized, and their merge is the concatenation S3-P3 timed at 6.9 of 14.6 ns.

**Fix.** Use one column type with chunk-slice access, have `decide` and the evaluators read chunk slices, and say how
variable-sized outputs merge (P9 merges by index using known vertex numbers).

## 12. The largest avoidable cost is missing

L27 puts "algorithms and avoided work first". P1 found that one more hole costs 38 times as much on a plate with 64 holes
as on an empty plate, and named the fix: "do not touch faces the boolean did not touch". The sketch prepares whole
operands (`prepare_input(model, target, tool)`). `seal` runs the whole-model checker after every operation. The broad
phase appears only as "masks at 8" over chunks, although P1 and P8 say all-pairs "has to change before models get
large". Explicit order and culling are in, which is good, but localisation, the largest avoidable cost P1 measured, is
not.

**Fix.** Prepare only faces whose bounds meet the other operand. Check changed entities and their neighbours, plus a
global Euler count. Name a hierarchy for the broad phase.

## 13. How O26 is carried out

The choice is assigned. These problems are in the implementation.

- **Two spellings of one id.** The report writes `{"kind":"edge","slot":41,"generation":1}`, while `resolved` keys, the
  `adjacent` lists and the CLI use `"edge:41/1"`. S6's agents failed on an unreadable id. Two forms that have to be
  matched by hand are a smaller version of the same problem.
- **The example's message cannot be generated.** `Report::message` fills placeholders only from `report.values`
  (`report.rs`). The example has no `values`, yet its message quotes "6 mm" and "4.98 mm".
- **The envelope is a third JSON form** beside `Outcome::to_json` and `Batch::to_json` (L4: "one serialisable error
  record"). How `resolved` attaches to a 20-item batch is not shown.
- **`Details.snapshot`** is an `EntityRef` of kind `model`, which no table holds. It also overlaps `Partial.last_valid`.
- **The risk is tested too late.** The test of the proposal's own risk (§8: "the blind control rerun") is a goal at
  `construction` (§7). Five of S6's six blind cases (c01, c04, c05, c08, c09) are boolean failures, and booleans arrive
  six milestones later.
- **"What changes" is incomplete.** O26's "if answered otherwise" names only per-site filling. It leaves out the
  `Scratchpad` trait, the `Frozen` lifetime, scratch-kind `EntityKind`s and `--keep-partial`, all of which would go.

**Fix.** Use one id form everywhere, keyed the same way in `resolved`. Put the requested and feasible radii in `values`.
Define the envelope as the existing forms plus a `resolved` field. Move the blind rerun to a gate at the `booleans` exit,
before `blends` builds on the schema.

## 14. The browser session

`Session { model: occam::Document }` lives "as long as its worker" (§3h), so a P10 pool of N workers holds N model copies,
and `run` in one worker leaves the others stale. P10 names this ("a boolean over a large model, where every worker needs
the whole model, is where the pool's copies would cost most"). `run` is one long call, against P10's rule 3, and Firefox's
long-call penalty is 1.4 to 2.3 times. "Document" is also the name PLAN §6 gives the post-release document layer, and
`MeshChunk` is undefined. **Fix:** use one editing worker that publishes native bytes after each edit, with read-only mesh
workers that reopen from them, and pick another name.

## 15. Smaller points

- **O25** says "the analytic presentation detects both kinds exactly". S4 says presentation is opt-in ("a rational
  cylinder ... goes to the general method") and presents the carrier, not the patch. The interim remedy, `move-apart`,
  changes the user's geometry, which is the very thing the proposal faults S6's agents for doing.
- **Phase keys and reuse.** P8 asks for "no run-specific numbering in cached outputs". The phase outputs here carry
  scratch and model ids, and only the inputs are keyed.
- **Sketch nits.**
  - `Code::ZeroLength` (§3a) is in neither the existing catalogue nor §3g's additions.
  - S4's blanket `impl<S: Parametric<F>> Surface<F> for S` forwards `family`, `batch_key`, `fingerprint` and
    `specialised`, which the new `Parametric` drops, so "S4's `Surface<F>` stays" does not compile as written.
  - `Id<K>`'s derives need `K: Ord` and the rest on every marker.
  - `[T; CHUNK]` needs a filler for the last chunk.
  - `bool` and `EdgeRole` chunks are 1 KiB, not "4 or 8 KiB".
  - "Welded `f64` positions (hashed)" must not mean a `HashMap` (`clippy.toml`). P9 welds by known vertex numbers.
  - `EntityRef::new` becomes a three-argument `const fn`, a breaking change to `occam-report` that is admitted as
    `SCHEMA` 2.
- **S6** found that the list of every failing pair was "not shown to be needed", and that blind c09 lost two attempts
  because of it. The proposal adopts the list without weighing that.
- **Browser widths.** "simd128 at 2" for the browser is not supported by P9 ("not faster than wasm scalar in most rows")
  or P4 (wasm `analytic` never beats scalar).

## The findings, checked

- **S1:** every figure cited is accurate (items 1 to 7, 9 and 10; the open counts of 3, 9 and 14; the 2 reports). The
  proposal ignores that `Point<World>` vertices were fine (point 5).
- **S2:** the figures are accurate (11.1 and 35.2%, 0.2 and 5.2%, 44 and 8%, 32 against 48 bytes). `b-interval-snap` is
  misread (point 2), and S2's "32 if one radius per vertex is enough" is adopted without being tested.
- **S3-P3:** the numbers are accurate, but quoted at 4,096 for a design at 1,024, and from a microbenchmark S3-P3 calls
  memory-bound (point 8).
- **S4:** the 2.6 times and 14 to 18% are accurate. Missed: "sorted per-call is the cheap version", the mesh rows, item
  5's surface naming, and `Tangent` not being lengths (points 1, 5, 6).
- **S5:** accurate on accuracy and on the 2-lane speed. The 4-lane figure is from the AVX2-and-FMA build (point 10).
- **S6:** accurate. The proposal ignores the caution about failing-pair lists and schedules the rerun where it cannot run
  (point 13).
- **P1:** accurate. Its "every analytic boolean" is P1's "every analytic family's boolean", and P1's localisation result
  is not used (point 12).
- **P2:** accurate (4 raw digests at six threads; 5.4 against 3.9; three rounds).
- **P4:** mostly accurate. The 30.7 is from a forbidden build, and the inline chain is not carried to the seam (point 10).
- **P5:** accurate (58 of 58; 104 of 110; 2 against 13 silent failures; 22 to 28%; under 3%).
- **P6:** the figures are accurate, but the 1.48 times is stretched into "cheap only when" (point 1).
- **P7:** not cited. The proposal is consistent with it (no mesh seed in the boolean), so nothing is lost.
- **P8:** accurate (33 to 34%; 66 to 86%; 307 to 157 ms; 4.7 times). The rule against run-specific ids is applied to
  inputs, where P8 asked it of outputs.
- **P9:** the figures are accurate, but the 1.8 to 3.3 is misapplied to grouping by family (point 1).
- **P10:** accurate. Rule 3 and the per-worker copies are not applied to `run` (point 14).
- **P11:** accurate (7 bit patterns in 8 runs; "Not supported"; 10 µs).
- **P12:** accurate (32 bytes a point; 1,200 pixels; dirty rectangles).

The proposal's own claims of gaps in the sources hold: S4 did not break its store cost down, S2 did not split its 1e-9
failures by scale, and S3-P3 measured flat directories only. Its claim that P9 overrides S3-P3 on analytic lanes holds for
the width-2 cylinder figure (trigonometry in lanes, which S3-P3 lacked), but not for the "batching" figure used in O23.

**The open decisions in brief.**

- O12 is argued from S1, S2 and P6, and labels its warning threshold honestly as a guess.
- O16, O18 and O24 are consistent with the evidence, and their "otherwise" sections are fair.
- O11 misreads S2 (point 2).
- O14 overstates its reason (point 10).
- O21 conflicts with PARITY (point 4).
- O22's "otherwise" quotes the wrong chunk size (point 8).
- O23 misattributes P9 (point 1).
- O25 overclaims detection (point 15).
- O26's "otherwise" is incomplete (point 13).

## What the proposal gets right, and an architecture should keep

- **Boolean phases.** The boolean is P2's shape (a): pure phases, ids assigned only in the merge, plans applied in item
  order, and `Frozen` phase outputs handed back on failure. This is the strongest answer to L5 and L23 in the evidence.
- **The checker in `seal`.** No snapshot exists that the checker has not passed, in release builds too, and the checker
  counts shells to catch the missing pinch.
- **The `Exec` interface.** It is P11's: index-ordered results, no reductions, explicit `Grain`, and `Serial` in browser
  workers. `rayon` sits behind it, as the decision says.
- **Determinism.** Its section is careful: chunk boundaries never set by the thread count, stable partitioning for lane
  groups, `Defined` bitsets for per-item failure, and GPU and `f32` data kept out.
- **Explicit-order evaluation and the parameter-only query** (P8's "cannot be retrofitted"). With them come caller-owned
  outputs and `Scratch` (S4's 8.4 times) and frames computed at construction.
- **Intersection.** S4's ordered method list in P5's order, failure as part of the result, `BelowResolution` for O24, and
  a per-edge cull that is tested against the unculled digest.
- **Meshing and the renderer.** P9's mesher, with its limits and the watertightness check on every output, and P12's
  camera-relative renderer contract.
- **Reports.** A closed `FaultClass` chosen per occurrence, closed `Reason`s, validation before geometry, the tolerance in
  force in reports, and readable `EntityRef` fields in place of a packed integer.
- **Honesty about risk.** §8 names each risk with what would show it wrong. Point 1's main fix is only to act on it
  before the seam freezes, not after.
