# Lessons from phasekit, 2026-11-08

Six agents read phasekit's docs, decision records, code and history. Their 57 findings were merged into L1-L50, and L51
comes from the `fornjot` review. Numbers are as the agents reported them and were not re-checked.

## Agent access and errors

- **L1.** Design errors as the agent API in the first commit. Phasekit retrofitted them at M28-M29: six PRs and 246 file
  changes.
- **L2.** Treat every suggested fix as a claim with a test that follows it. From reading the code, phasekit still offers
  some that do not work, and only 3 of its 7 are tested that way.
- **L3.** Give fixes parameters, such as `ReduceRadius { edge, max_feasible }`. Bare codes leave the agent guessing.
- **L4.** Use one serialisable error record across Rust, wasm and C. Phasekit's facades flatten errors separately and
  have drifted apart.
- **L5.** Build failure state into solvers from the start: failing face pair, curves found, residual gap, last valid
  model. Phasekit had to convert 57 sites.
- **L6.** Return per-item results from the first batch API, so filleting 20 edges gives 20 outcomes.
- **L7.** Report diagnostics on success too: tolerance used, healing applied, non-manifold edges.
- **L8.** Make everything machine-readable: `--json` on tools, a listable error catalogue, and readable dumps that
  round-trip binary data exactly.
- **L9.** Let the core own error wording, with numbers tagged by quantity and shown in the caller's units.

## Verification

- **L10.** Passing against the oracle proves little. Mutation testing kept finding survivors in green suites (37 of 146,
  47 of 172).
- **L11.** Treat OCCT as a provisional oracle and geometric invariants as the arbiter: Euler characteristic,
  watertightness, exact volumes, vol(A∪B) + vol(A∩B) = vol A + vol B. CoolProp and the printed literature were both
  wrong at times.
- **L12.** Compare with OCCT geometrically, never by topology or bits. Bit-level matching imported CoolProp's quirks.
- **L13.** Check the mutation gate itself. Timeouts and broken trees gave false greens, so run the unmutated suite first.
- **L14.** Set up mutation speed before the first PR. One PR spent 4 hours in mutants, and the weekly run overran
  GitHub's 6-hour limit.
- **L15.** Derive tolerances from conditioning, and tally asserted against exempt cases. One exemption covered 217,018
  cells.
- **L16.** Keep a locked fixture pipeline with a nightly full run. The first pinned nightly found 664 stale cells.
- **L17.** Make randomised boolean sequences the main property test, and assert mesh buffers rather than pixels.

## Architecture

- **L18.** Make layers into crates, or gate the module graph. In `phasekit-core`, 9 of 25 modules import upward despite
  the documented layering.
- **L19.** Turn on every lint at the start, including indexing and arithmetic. Adding them late touched 731 sites.
- **L20.** Start complexity and duplication gates at the beginning, with a shrink-only exemption list. Arriving seven
  milestones late cost six PRs (+8.4k/-6.3k).
- **L21.** Prove the seams on really different families before freezing them: analytic, NURBS and mesh-backed. Toy
  families passed, and the first real one still added four seams.
- **L22.** Convert units once at the API and use typed quantities from day one. For CAD, add frame types (world, local,
  UV) and a per-model `Tolerance`, never a global one.
- **L23.** Fix the determinism contract and one maths choke point before the first predicate. Feature flags changed
  numerics three times in phasekit, and in CAD a flipped last bit changes topology.
- **L24.** Use immutable values with compile-time assertions, which made thread safety a type property.
- **L25.** Use one composition crate for C, wasm and the renderer. In phasekit each new family edited every facade.
- **L26.** Do not use `Any` or downcasting across seams. Use closed enums within a family and open traits across
  families.

## Speed, parallelism and web

- **L27.** Put algorithms and avoided work first. Water PH went from 83.3 to 9.4 µs that way, while SIMD's best was
  2.87×.
- **L28.** Make SIMD and threads compose from day one: threads across faces, lanes inside each task. Phasekit's still do
  not.
- **L29.** Batch surfaces by kind and degree. Keep intersection marching scalar, but parallel across curves.
- **L30.** Measure the browser build from the first kernel. Phasekit's simd128 flashes were slower than scalar, and that
  was found late.
- **L31.** Gate performance per operation class. Instruction counts caught a 46% regression but missed an 8-15% slowdown
  outside their benches.
- **L32.** Compare whole operations with OCCT. Phasekit's early targets timed calls that defer their work.
- **L33.** Compute geometry in Rust and hand flat typed arrays to a worker. Decide wasm threads and COOP/COEP hosting
  early.

## Replacing legacy code

- **L34.** Keep provenance clean: survey agents write behaviour specs, implementers never see OCCT, and algorithms come
  from papers.
- **L35.** Define "replacement done" up front by listing the OCCT calls FreeCAD makes. Phasekit never did: 286 commits
  and no `v0.1.0` tag.
- **L36.** Give provenance its own CI gate. Phasekit's REUSE check let ported code through uncredited.
- **L37.** Survey OCCT's bug tracker, test grids and FreeCAD's usage first, and map subsystems only when they are
  scheduled.
- **L38.** Replay real call patterns before banning legacy behaviour. Phasekit's no-cache rule cost 73 ms against 4.4 ms.
- **L39.** Keep legacy quirks in a side-by-side shim, and register only the problem classes you will actually build.

## Process

- **L40.** Derive project state from git, and gate it. Phasekit's "next step" lookup is wrong today because footers went
  unchecked.
- **L41.** Keep one home per fact, gated from day one. The late clean-up had about 130 findings.
- **L42.** Split the plan by milestone and delete finished text. `PLAN.md` was cut to 61 KB and regrew to 240 KB.
- **L43.** Plan in detail only to the first release. The M0-M10 plan held; M15-M17 never started and M20-M29 were
  invented later.
- **L44.** Make anything parallel sessions touch merge by construction: one file per decision, one test file per
  operation.
- **L45.** Separate evidence from instructions on day one.
- **L46.** Classify each target as gate or goal up front. Exactness invariants are gates; boolean success rates start as
  goals.
- **L47.** Let the plan group steps into PRs and budget CI per PR. One step per PR was too rigid.
- **L48.** Script the environment rules and add `just` at the start. Scratch directories once used 285 of 473 GB.
- **L49.** Keep decision records for choices that bind. 81 files in under a month included tooling trivia.
- **L50.** Use two design proposals and adversarial critics, and skip numeric scoring. The scores came out 7.55 against
  7.45.

## From the existing kernels

- **L51.** Prove curved booleans end to end in throwaway prototypes before freezing data structures, and keep the scope
  narrow. `fornjot`'s author cited small steps instead of prototypes and an unfinished architecture migration when
  shutting it down.
