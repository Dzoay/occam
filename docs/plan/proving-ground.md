# Milestone: proving-ground

Answer the questions that decide the data structures, using code that is thrown away (L51). Needs `census.oracle` and
`census.corpus`, apart from the investigations that start early: [the first six](../decisions/2026-11-09-early-spikes.md)
and [the second wave](../decisions/2026-11-09-second-wave.md).

**Rules for spikes:** spike code lives on a long-lived `spikes` branch that is never merged, so later spikes can reuse
earlier ones. Each spike states its question and measure first. Its PR to `main` holds only its written finding, in
`docs/evidence/`. `proving-ground.design` deletes the branch. Nothing from it is copied into a crate; anything that
survives is rewritten test-first. The provenance rules still apply, so run `gates provenance` over the branch before each
findings PR.

**Exit (gate):** every question below has a written answer, `docs/ARCHITECTURE.md` exists, and the user has approved it.

**PR grouping:** one findings PR per step. Investigations that share an experiment may share a PR.

## Spikes

### `proving-ground.s1`: crossed cylinders, end to end

Two crossing cylinders, a sphere cut by a box, and a NURBS face cut by a plane, taken from surfaces to a watertight
solid. Checked by closed-form volume, Euler characteristic, the volume identity and the OCCT oracle, over randomised
placements including near-tangent ones. It reports its failure rate, and what state each failure could have handed back
(L5).

*Answers:* what the topology and tolerance data really have to hold.

### `proving-ground.s2`: tolerance model

The same near-tangent corpus through (a) per-entity growing tolerances, (b) one model precision with certified
intervals, and (c) floating-point filters with an exact fallback.

*Measures:* failure rate, wrong-answer rate, speed. *Answers:* the tolerance model.

### `proving-ground.s3`: topology layout

Array-of-structs arenas, struct-of-arrays arenas, and copy-on-write chunks, each with stable ids and operation history.

*Measures:* traversal, copy-on-edit cost, parallel reads, memory. *Answers:* the layout, jointly with
`proving-ground.p3`.

### `proving-ground.s4`: seams across families

One `Surface` and `Curve` interface exercised by analytic, NURBS and mesh-backed bodies, with closed enums within a
family, open traits across families, and no downcasting (L21, L26).

*Answers:* the geometry seams, before they freeze.

### `proving-ground.s5`: the maths contract

What really needs to be identical on native and wasm, which of `sin`, `cos`, `atan2`, `acos` and others that means occam
must own, and what that costs against std. Computing the same model in the browser and on a server is the case that would
need it ([decision](../decisions/2026-11-09-one-maths-choke-point.md)).

*Answers:* the contents of `occam-math` (L23).

### `proving-ground.s6`: can an agent repair a model?

An agent is given only the prototype's JSON reports and asked to repair failing operations.

*Measures:* the fraction repaired without reading source (a goal). *Answers:* what `Report` is missing.

## Parallel investigations

These test the brief's question directly: which formulation of a CAD kernel takes advantage of parallel processing. Each
one reports speed-up at 1, 2 and 4 threads and at the recorded machine's physical core count, on physical cores, and
checks that the output is identical at every thread count. O9 covers wider hardware.

| Id | Question | Candidates | Feeds |
|---|---|---|---|
| `proving-ground.p1` | Where does the time go? | OCCT timings from `census.baseline`; a per-phase profile of S1; the Amdahl limit for each operation | Priorities for everything below |
| `proving-ground.p2` | What shape should the boolean pipeline take? | Pure phases (broad phase, pair intersection, edge split, face split, classify, stitch), each a map with a deterministic merge; against one shared graph with fine-grained locks | Boolean design |
| `proving-ground.p3` | What data layout lets tasks run without locks? | Immutable snapshots plus per-task result buffers merged in a stable order; struct-of-arrays by surface kind and degree | Topology and geometry layout |
| `proving-ground.p4` | How do threads and SIMD lanes nest? | Threads across faces with lanes inside each task; lane width swept per kernel; plain array lanes against explicit SIMD (L28, L29) | `occam-math`, batch evaluation |
| `proving-ground.p5` | Which intersection method? | Closed forms for analytic pairs; marching (scalar, parallel across curves); subdivision such as Bézier clipping (data-parallel); hybrids | `occam-intersect` |
| `proving-ground.p6` | What does robustness cost in parallel? | Filters and exact fallback under lanes; how intervals perform; where work piles up on hard cases | Tolerance model, with S2 |
| `proving-ground.p7` | Does another representation help? | Pure B-rep; a mesh-first topology seed refined to exact; implicit fields as a broad-phase accelerator | Boolean design |
| `proving-ground.p8` | How much work can be avoided? (L27) | Bounds culling; evaluating only the derivatives needed; content-addressed caching of intersection results; recomputing only what changed | Caches, document layer |
| `proving-ground.p9` | How should tessellation parallelise? | Shared edges first, then faces independently, watertight by construction; CPU lanes against WebGPU compute | `occam-mesh`, renderer |
| `proving-ground.p10` | What runs in the browser? (L30, L33) | wasm threads over `SharedArrayBuffer`, which needs COOP/COEP hosting and a nightly toolchain (O10); a worker pool with separate memories; one thread with simd128 | wasm facade, hosting |
| `proving-ground.p11` | What runs the threads? | `std::thread::scope` with occam's own queue; `rayon`, which would be a dependency to ask about | Execution layer |
| `proving-ground.p12` | Can the GPU generate display geometry? ([decision](../decisions/2026-11-09-gpu-drawing.md)) | Surfaces evaluated in a WebGPU shader and written straight to a vertex buffer, never read back; against CPU meshes uploaded for drawing; in a page with a render loop | Renderer |

## Closing the milestone

### `proving-ground.design`

Two design proposals from the findings, adversarial critics on each, no numeric scoring, and a compiled type sketch of
the seams (L50). The result is `docs/ARCHITECTURE.md`. §4 of the plan is updated, the `spikes` branch is deleted, and
[first-release.md](first-release.md) is revised.
