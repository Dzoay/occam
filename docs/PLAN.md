# The plan

**Status: [approved by the user](decisions/2026-11-08-the-plan-and-its-first-answers.md), 2026-11-08.**

[BRIEF.md](BRIEF.md) says what is wanted and [decisions/](decisions/) what the user has decided. This file is the map.
Each milestone has its own file in [plan/](plan/), and detail is written only when that milestone is close. `L` numbers
cite [the phasekit lessons](evidence/2026-11-08-phasekit-lessons.md), with
[L29's first sentence narrowed](decisions/2026-11-11-the-architecture-and-its-answers.md). "Phasekit" means the sibling
checkout `../phasekit`.

## 1. How to use this plan

- **Step ids.** A step is a level-3 heading or a table row whose text begins with a backticked id of the form
  `<milestone>.<slug>`. Nothing else is a step.
- **What is next comes from git.** `just next` prints, for each open milestone, its first step with no `Plan-Step:`
  footer on `main`. Milestones run in §6's order. `bedrock` and `census` are open together; every other milestone opens
  when the one before it closes.
- **Footers.** `main` changes only by squash merge, which takes its message from the PR body, so footers go in the PR
  body: one `Plan-Step: <id>` line per step. `gates plan` fails a step PR that lacks them.
- **Working a step:** write the failing test first, show it red, implement, get every gate green, open the PR. The PR
  body holds what was measured, and the finished step's text is deleted from the plan (L42).
- **PR grouping:** each milestone file says which steps share a PR (L47).
- **Stop and ask the user** when a step fails its gate in a way the plan did not foresee, when a finding contradicts the
  plan, or when a decision in §7 comes up.
- **Gate or goal:** every target is labelled (L46). A gate fails the PR. A goal is measured and reported.

## 2. What done means

**The first release** is done when the *bracket test* passes. An agent, given only the `occam` command-line tool and
its reports, builds a bracket: extrude a profile, cut a bolt hole, fillet the edges. One fillet is requested with too
large a radius. The report names the edge and the largest feasible radius, and following it succeeds. The result is
viewed in a browser from the wasm build, exported to STEP, and read by the pinned OCCT oracle with matching volume and a
valid shape.

**Replacement done** is every row of `docs/PARITY.md`, which `census.parity` creates from the OCCT classes FreeCAD really
uses (L35). The user approves that list and owns its order.

## 3. Commitments

These hold from the first commit. The first thirteen are [the user's priorities](decisions/2026-11-08-the-lessons.md).

| Lesson | Commitment | Held by |
|---|---|---|
| L1 | Errors are the agent API: code, entities, named values, remedies, before any geometry | `bedrock.report` |
| L2 | Every code and remedy pair has a test that follows the remedy and succeeds | `gates errors` |
| L5 | Solvers carry failure state: failing pair, curves found, residual gap. The input model is unchanged on failure | `Report`'s partial state; review of each solver step |
| L11 | Geometric invariants are the arbiter; OCCT is a provisional oracle | `occam-verify` |
| L18 | Layers are crates, and the allowed graph is checked | `gates layers` |
| L19 | Every lint is on from the start, indexing and arithmetic included | workspace lints, probes |
| L23 | Same inputs give identical bits at any thread count, and on x86-64 and single-threaded wasm32 for what `Real` computes, through [one maths choke point](decisions/2026-11-09-one-maths-choke-point.md) | `gates determinism` |
| L28 | Threads and SIMD lanes compose: threads across items, lanes inside each task | `proving-ground.p4`; perf gate |
| L30 | The browser build is timed on every PR from the first kernel | `gates perf` |
| L34 | Provenance is clean ([decision](decisions/2026-11-08-licence-and-provenance.md)) | `gates provenance`, session split |
| L35 | Replacement done is a census-derived list | `census.parity` |
| L43 | Detail only to the first release | this file's layout |
| L51 | Curved booleans are proven in throwaway prototypes before data structures freeze | `proving-ground` |
| Brief | One clear unit system: convert once at the API, typed quantities and frames inside, per-model `Tolerance` (L22) | `occam-units`, compile-fail fixtures |
| Brief | Agents can drive everything: operations are data, every result reports diagnostics, per-item batch results (L6, L7, L8) | `occam-cli`, one schema |
| Brief | Every entity has a stable id, and every operation returns what it created, changed and deleted | `occam-topo` |
| Brief | Minimal dependencies: the kernel is std-only unless the user agrees otherwise | `gates deps` |

## 4. Layers

Each crate may depend only on crates above it in this list. [ARCHITECTURE.md](ARCHITECTURE.md) is the design behind it.

```text
occam-report      Report, codes, remedies, fault classes, entity references and descriptions, the JSON schema
occam-math        Real and lanes, owned sin_cos and atan2, dispatch, intervals, exact predicates, the execution layer
occam-units       Length, Angle, frames, points, directions, tangents, placements, bounds, Tolerance; units at the API
occam-geom        curves, parameter curves and surfaces: analytic, NURBS, mesh-backed; evaluation by order; presentation
occam-intersect   curve and surface intersection: closed forms, then the hybrid; failures in the result
occam-topo        the B-rep: records in copy-on-write chunks, stable ids, snapshots, the edit, history, the checker
occam-mesh        tessellation: shared edges first, then faces; the watertight check
occam-ops         operations as data: validation, construction, measurement, booleans, blends (split as it grows)
occam-io          the native format and its readable dump, STEP
occam             the composition crate, with the request runner and the describer: all below bind it alone (L25)
occam-wasm, occam-cli, occam-capi, occam-mcp          thin facades
occam-verify, occam-xtask                             oracles, corpus, gates; never published
web/                                                  the renderer; consumes data only
```

## 5. Gates

The one definition. Every `gates` subcommand has `--json` (L8) and a test proving that a seeded violation fails it.
`just gates` runs everything except mutants and perf, which are `just mutants` and `just perf`. CI runs all of them, with
the same commands. Goal: a PR's CI finishes within 45 minutes (L47).

| Gate | What it checks | Built by |
|---|---|---|
| `fmt`, `clippy` | Formatting; every lint with warnings denied, including no panics, indexing, arithmetic, printing, `Any` and hashed collections in kernel crates (L19, L23, L26) | `bedrock.repo` |
| `test` | Every test through nextest on the host, and doctests | `bedrock.repo` |
| `tools-test` | The Python tools' tests (the surveys, the corpus, the determinism record, the trigonometry's coefficients), standard library only | `census.freecad-api` |
| `wasm` | Tests on wasm32-wasip2; the browser target builds; a Windows check | `bedrock.repo` |
| `gates layers` | The crate graph matches §4 (L18) | `bedrock.gates` |
| `gates complexity` | Function and file length, duplication, with a shrink-only exemption list (L20) | `bedrock.gates` |
| `gates provenance` | Rust identifiers in `crates/` and `web/` against `ci/forbidden-identifiers.txt`, with `tools/oracle/` exempt. Every file in a kernel crate carries `//! Source: <key>` resolving in `docs/SOURCES.md`, or `//! Source: original`. Derived files are declared in `REUSE.toml` (L36) | `bedrock.gates` |
| `gates mutants` | Mutation testing on the PR's diff, zero survivors. Runs the unmutated suite first; the timeout floor exceeds a full suite (L13, L14) | `bedrock.gates` |
| `gates counts` | Test-count ratchet, with a merge driver (L44) | `bedrock.gates` |
| `gates plan` | A step PR carries its `Plan-Step:` footers, and `just next` agrees (L40) | `bedrock.gates` |
| `gates docs` | Links resolve, step and decision ids are unique, files in `docs/evidence/` are unchanged once merged (L41, L45) | `bedrock.gates` |
| `gates deps` | Dependency allow-list, `cargo deny`, `cargo shear`, REUSE | `bedrock.gates` |
| `gates errors` | Every code is in the catalogue; every code and remedy pair is registered by a followed-remedy test (L2); matches are exhaustive (L4) | `bedrock.report` |
| `gates determinism` | [The contract](DETERMINISM.md): output hashes match on x86-64 at 1 and N threads and on single-threaded wasm32, with NaNs canonicalised before hashing (L23) | `bedrock.maths` |
| `gates perf` | Native instruction counts per operation class, scalar and lanes. Browser timing is reported on every PR; only the same-run simd128-against-scalar ratio is gated (L30, L31) | `bedrock.perf` |

Nightly, not per PR: the full corpus, randomised boolean sequences, and the OCCT comparison (L16, L17). The workflow comes
from `bedrock.ci`, and jobs join it as the corpus and operations arrive.

## 6. Milestones

Named, not numbered. They run in this order.

| Milestone | What it delivers | Detail |
|---|---|---|
| `bedrock` | Repo, all gates, the error model, units, one trivial operation through every layer | [plan/bedrock.md](plan/bedrock.md) |
| `census` | FreeCAD usage census, parity list, OCCT oracle, corpus, whole-operation baselines. Runs alongside `bedrock` | [plan/census.md](plan/census.md) |
| `proving-ground` | Throwaway prototypes, the parallel investigations, then the architecture | [plan/proving-ground.md](plan/proving-ground.md) |
| `maths` | The first milestone of the first release: lanes, dispatch, threads, robust decisions, owned `sin_cos` and `atan2` | [plan/maths.md](plan/maths.md) |
| first release | `geometry`, `topology`, `construction`, `view`, `intersection`, `booleans`, `exchange`, `blends`, `release` | [plan/first-release.md](plan/first-release.md) (outline) |
| after that | Named below; planned when reached | none yet |

**After the first release**, in no fixed order and each needing the user's go-ahead: the 2D sketch solver; the document
layer (feature tree, persistent naming, incremental recompute);
[assemblies](decisions/2026-11-11-assemblies-after-release.md);
[view-dependent detail](decisions/2026-11-11-view-dependent-detail-after-release.md); shell, offset and draft; sweep
and loft; hidden-line drawings; the full web application; server-side rendering; a C ABI and a side-by-side shim at
FreeCAD's `Part` layer (L39); an MCP server.

## 7. Decisions the user still owns

O1 to O4 are [answered](decisions/2026-11-08-the-plan-and-its-first-answers.md). O21, whether anything needs an exact derivative above the second, is answered by [the approved parity list](PARITY.md): rows 2.1 and 2.2 ask for evaluation to any derivative in `geometry`. O11, O12, O14, O16, O18 and O22 to O26 are [answered with the architecture](decisions/2026-11-11-the-architecture-and-its-answers.md), and [its decisions page](architecture/open-decisions.md) keeps the reasoning. O15 is [answered](decisions/2026-11-11-own-coefficients.md): occam fits its own coefficients.

| Id | Decision | Recommendation | Needed by |
|---|---|---|---|
| O5 | Cargo dependencies, asked one at a time when their step arrives. Answered: [`gungraun`](decisions/2026-11-09-gungraun.md) (test only, `bedrock.perf`), [`wasm-bindgen`](decisions/2026-11-09-wasm-bindgen.md) (`bedrock.thread`), [`fearless_simd`](decisions/2026-11-10-fearless-simd.md) and [`rayon`](decisions/2026-11-10-rayon.md). Still expected: a property-test crate (test only) | Decide each on the evidence | when reached |
| O6 | A lawyer's view of the provenance policy and oracle outputs | Worth having before the first public release | first release |
| O7 | Reserve `occam` on crates.io and an npm scope | Reserve once there is something real to publish | first release |
| O8 | The framework for the viewer's page | No framework for the viewer; decide again for the full web application | `view` |
| O9 | A machine with more physical cores for the parallel investigations. This laptop has 4 | Start here, and decide after `proving-ground.p1` shows how far scaling matters | `proving-ground.p2` |
| O10 | A nightly toolchain for wasm threads, which need `-Zbuild-std`. [Spikes may use one already](decisions/2026-11-09-early-spikes.md) | P10 recommends keeping this open and not taking on nightly for real use. Nothing in the first release needs it. Reopen it if a real kernel has a model large enough that a copy per worker hurts, or tasks fine-grained enough that message passing dominates, or when `-Zbuild-std` stabilises | when one of those happens |
| O13 | A stored per-entity accuracy for imported geometry, fixed at import and never grown (S2). Foreign geometry was not measured | Decide when import is in scope | when import is in scope |
| O17 | x86-64-v3 code paths (S3/P3), and whether [`DETERMINISM.md`](DETERMINISM.md) rule 5, "No `target-cpu`", stays as written (P4). The NURBS gain exists only there. Having it means hand-written detection, a new dependency, or a build that excludes older CPUs | Leave this until a real kernel profile shows NURBS evaluation is worth it. Rule 5 needs no change if `dispatch!` is taken under O5 | when a kernel profile asks for it |
| O19 | Which browsers the first release must support (P10, P12). Safari was not measured, and P12 measured only Firefox on two GPUs. [This also picks the renderer's API](decisions/2026-11-09-gpu-drawing.md) | Measure Safari before any release that claims to support it | `view` |
| O20 | Whether the first release must serve continuous whole-surface editing of very large faces (P12) | No: draw the CPU mesh with local updates, and add write-through if a real workload asks for it | `view` |
| O27 | Whether P1's comparison with OCCT should reach `main`. It holds family medians of the time ratio and counts of the cases where the prototype was faster, with no per-case OCCT number. [Kept out for now](decisions/2026-11-09-p1-occt-comparison.md) | Decide with O6's answer, because the figures are derived from oracle outputs | when O6 is answered |

O11 onward come from the spike findings named in each row, which give the detail.
