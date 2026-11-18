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
- **What is next comes from git.** `just next` prints, for each open milestone, every unfinished step whose needs all
  have a `Plan-Step:` footer on `main`. A step needs each step with an arrow into it in its milestone file's Mermaid
  flowchart, whose nodes are slugs and stay when a step's text is deleted, or, with no flowchart, every step before it
  ([decision](decisions/2026-11-15-steps-follow-their-milestones-diagram.md)). A PR may finish a step once every step
  it needs is finished on `main` or in the same PR. A milestone opens when every one before it in §6's table has
  closed, unless its row says it runs alongside another or names the one whose closing opens it.
- **Footers.** `main` changes only by squash merge, which takes its message from the PR body, so footers go in the PR
  body: one `Plan-Step: <id>` line per step. `gates plan` fails a step PR that lacks them.
- **Working a step:** write the failing test first, show it red, implement. Before each commit a worker runs the tests
  and clippy of the crate it changed, and `just preflight`. The full gates, perf and mutants run once on the branch
  that brings the work together, and in CI, not once per worker
  ([two tiers](decisions/2026-11-14-two-tiers-of-verification.md)). Then open the PR. The PR body holds what was
  measured, and the finished step's text is deleted from the plan (L42).
- **PR grouping:** a step merges in its own PR, or with the few steps it only makes sense beside, which its milestone
  file names ([small PRs](decisions/2026-11-14-steps-merge-in-small-prs.md), L47). `construction`, built before that
  decision, merges as one PR.
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
| L19 | Every lint is on from the start, indexing and arithmetic included | workspace lints, probes, `gates complexity` |
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
occam-fixtures, occam-verify, occam-xtask             fixtures, oracles, corpus, gates; never published
web/                                                  the renderer; consumes data only
```

`occam-verify` and `occam-xtask` may also name any crate above them in `[dev-dependencies]`, for their tests
([decision](decisions/2026-11-15-verify-and-xtask-test-against-any-crate.md)). `occam-fixtures` holds what tests,
benchmarks and probes build with: it binds any crate above it, and any crate may name it, in `[dev-dependencies]` only
([decision](decisions/2026-11-17-occam-fixtures.md)).

## 5. Gates

The one definition. Every `gates` subcommand has `--json` (L8) and a test proving that a seeded violation fails it.
`just gates` runs everything except mutants and perf, which are `just mutants` and `just perf`. It runs `just preflight`
first: formatting, the gates that only read the repository (layers, complexity, provenance, docs, plan, seam) and the
mutants gate's check of its exclusions, which take seconds, so that one of them failing does not wait for clippy, the
tests and wasm. CI runs all of them, with the same commands. Goal: a PR's CI finishes within 45 minutes (L47).

| Gate | What it checks | Built by |
|---|---|---|
| `fmt`, `clippy` | Formatting; every lint with warnings denied, including no panics, indexing, arithmetic, printing, `Any` and hashed collections in kernel crates (L19, L23, L26). Clippy also builds `occam-cli` alone, so the `occam` command and the kernel under it compile as they ship, without `trusted` or any other feature only another crate turns on | `bedrock.repo` |
| `test` | Every test through nextest on the host, and doctests | `bedrock.repo` |
| `tools-test` | The Python tools' tests (the surveys, the corpus, the determinism record, the trigonometry's coefficients), standard library only | `census.freecad-api` |
| `wasm` | Tests on wasm32-wasip2; the browser target builds; a Windows check | `bedrock.repo` |
| `web-check`, `web-test` | The viewer's TypeScript, `tsc --noEmit` in strict mode, declaration files included, against `@types/web`'s browser and WebGPU types; its tests in headless Chromium on the software adapter, asserting the buffers it uploads, not pixels, following each report's remedy, holding its reports to the catalogue, its mesh workers' meshes to the native `occam mesh`'s bits, the page `occam view` serves, built in, following the model file, its clicks picking the ids `occam pick` gives, mapping no GPU buffer, and the end-to-end demo with the agent's calls scripted, scored as the live agent test is | `view.renderer`, `view.workers`, `view.live`, `view.select`, `view.agent` |
| `gates layers` | The crate graph matches §4 (L18). Only occam-wasm's manifest turns on the `trusted` feature, the open of a model without its check, which a kernel crate passes on only from its own `trusted`; no workspace build would notice another, since cargo gives a crate every feature any member asks of it | `bedrock.gates` |
| `gates complexity` | Function and file length, duplication, with a shrink-only exemption list (L20). The root manifest's rule on relaxing a lint (L19, [decision](decisions/2026-11-17-a-lint-is-relaxed-on-the-narrowest-item.md)): every `allow` and `expect` in `crates/` gives a reason, and an `allow` stands only at a crate's root, so anywhere else it is an `expect`, which fails once nothing needs it | `bedrock.gates` |
| `gates provenance` | Rust identifiers in `crates/` and `web/` against `ci/forbidden-identifiers.txt`, with `tools/oracle/` exempt. Every file in a kernel crate carries `//! Source: <key>` resolving in `docs/SOURCES.md`, or `//! Source: original`. Derived files are declared in `REUSE.toml` (L36) | `bedrock.gates` |
| `gates mutants` | Mutation testing on the PR's diff, zero survivors, by the whole workspace's tests ([kept, after measuring narrower scopes](decisions/2026-11-14-mutants-keep-the-whole-workspace.md)). Builds and runs the unmutated suite once, outside the checkout, gives a mutant's tests five times as long, at least 300 seconds, and tests the mutants in [copies of that built tree](decisions/2026-11-14-mutants-keep-the-whole-workspace.md#warm-in-place-shards-adopted) (L13, L14). [A copy is not taken at its word](decisions/2026-11-14-mutants-keep-the-whole-workspace.md#what-a-copy-says-is-checked): one that found a mutant unviable must still build the unmutated tests afterwards, or the gate fails, and a mutant that ran out of time is tested again alone before it counts as caught. Before any of it, whatever the diff, every `exclude_re` entry in `.cargo/mutants.toml` must exclude a mutant of the workspace as cargo-mutants lists it, since an entry by line goes stale when an edit moves its mutant; `--exclusions` runs that check alone | `bedrock.gates` |
| `gates counts` | No test binary has lost a test it has on the PR's base, by name, or is gone, unless a commit's `Tests-Removed: <binary> <reason>` footer names it (L44, [measured, not stored](decisions/2026-11-14-test-counts-measured-not-stored.md)). CI gives the base, `--base origin/main`, and the gate builds it a second time; a run without one says it compared nothing | `bedrock.gates` |
| `gates plan` | A step PR carries its `Plan-Step:` footers, and `just next` agrees (L40) | `bedrock.gates` |
| `gates seam` | occam-geom's seam matches `ci/seam.txt`, every public signature of its modules; a change needs a newer decision, named in the record (L21) | `geometry.freeze` |
| `gates docs` | Links resolve, step and decision ids are unique, files in `docs/evidence/` are unchanged once merged (L41, L45) | `bedrock.gates` |
| `gates deps` | Dependency allow-list, `cargo deny`, `cargo shear`, REUSE | `bedrock.gates` |
| `gates errors` | Every code is in the catalogue; every code and remedy pair is registered by a followed-remedy test (L2); matches are exhaustive (L4) | `bedrock.report` |
| `gates determinism` | [The contract](DETERMINISM.md): output hashes match on x86-64 at 1 and N threads and on single-threaded wasm32, with NaNs canonicalised before hashing (L23) | `bedrock.maths` |
| `gates perf` | Native instruction counts per operation class, scalar and lanes. Browser timing is reported on every PR; only the same-run simd128-against-scalar ratio is gated (L30, L31) | `bedrock.perf` |

Nightly, not per PR: randomised boolean sequences, and the OCCT comparison (L16, L17), which
[runs the oracle over the corpus and then `occam-verify`'s comparison](decisions/2026-11-17-the-corpus-per-pr-the-oracle-nightly.md), so that no oracle output enters the repository. The workflow comes
from `bedrock.ci`, and jobs join it as the corpus and operations arrive.

Per PR, in `test`: the corpus cases that occam builds today and that record a closed-form answer are built, measured and
judged against it by `occam-verify`, and intersection's hits on every case's operands are held to the unculled scan;
each takes seconds. Both corpus tests pin what they leave out by reason and count,
and fail on a case the runner refuses, a count that moves or an empty corpus.

## 6. Milestones

Named, not numbered. They run in this order, except where a row says otherwise:
[`view` and `intersection` are open together](decisions/2026-11-15-view-and-intersection-together.md).

| Milestone | What it delivers | Detail |
|---|---|---|
| `bedrock` | Repo, all gates, the error model, units, one trivial operation through every layer | [plan/bedrock.md](plan/bedrock.md) |
| `census` | FreeCAD usage census, parity list, OCCT oracle, corpus, whole-operation baselines. Runs alongside `bedrock` | [plan/census.md](plan/census.md) |
| `proving-ground` | Throwaway prototypes, the parallel investigations, then the architecture | [plan/proving-ground.md](plan/proving-ground.md) |
| `maths` | The first milestone of the first release: lanes, dispatch, threads, robust decisions, owned `sin_cos` and `atan2` | [plan/maths.md](plan/maths.md) |
| `geometry` | Every curve and surface family through the two-layer seam, the new unit types, and report schema 2 | [plan/geometry.md](plan/geometry.md) |
| `topology` | The B-rep: records in copy-on-write chunks, stable ids, the edit whose commit runs the checker, and what changed | [plan/topology.md](plan/topology.md) |
| `construction` | An agent builds and inspects a solid from JSON alone: requests, wires, faces, primitives, sweeps, copies, mass properties, the native format and the `occam` command | [plan/construction.md](plan/construction.md) |
| `view` | A person prompts an agent and watches the model live: shared edges first, then faces, the watertight check on every output, the worker pool and the renderer contract; `occam view` redrawing on every run; faces and edges selected by the person and read by the agent | [plan/view.md](plan/view.md) |
| `intersection` | Where two surfaces meet and where a curve crosses a surface: closed forms, then the hybrid; failures in the result; loops below resolution reported; coincidence detected. Runs alongside `view` | [plan/intersection.md](plan/intersection.md) |
| `booleans` | Fuse, cut and common of an object and many tools in one pass, as pure phases over the sections; coincident faces answered; which input faces made which output faces; point classification; every failing pair reported, the model unchanged. Opens when `intersection` closes | [plan/booleans.md](plan/booleans.md) |
| first release | `exchange`, `blends`, `release` | [plan/first-release.md](plan/first-release.md) (outline) |
| after that | Named below; planned when reached | none yet |

[The review backlog of 2026-11-17](plan/review-2026-11-17.md) holds what a whole-codebase review claimed, in the order
the user gave. It is not a milestone and has no steps.

**After the first release**, in no fixed order and each needing the user's go-ahead: the 2D sketch solver; the document
layer (feature tree, persistent naming, incremental recompute);
[assemblies](decisions/2026-11-11-assemblies-after-release.md);
[view-dependent detail](decisions/2026-11-11-view-dependent-detail-after-release.md); shell, offset and draft; sweep
and loft; hidden-line drawings; the full web application; [the editing tools, GPU write-through
included](decisions/2026-11-14-write-through-after-the-first-release.md); [an in-memory session server, the in-page chat
and a web service with the prompt in the browser](decisions/2026-11-14-prompt-driven-building-shown-live.md);
server-side rendering; a C ABI and a side-by-side shim at
FreeCAD's `Part` layer (L39); an MCP server.

## 7. Decisions the user still owns

O1 to O4 are [answered](decisions/2026-11-08-the-plan-and-its-first-answers.md). O21, whether anything needs an exact derivative above the second, is answered by [the approved parity list](PARITY.md): rows 2.1 and 2.2 ask for evaluation to any derivative in `geometry`. O11, O12, O14, O16, O18 and O22 to O26 are [answered with the architecture](decisions/2026-11-11-the-architecture-and-its-answers.md), and [its decisions page](architecture/open-decisions.md) keeps the reasoning. O15 is [answered](decisions/2026-11-11-own-coefficients.md): occam fits its own coefficients. O8, O19, O20 and O28 to O30 are answered for `view`. As recommended: [WebGPU on current Chromium and Firefox](decisions/2026-11-13-webgpu-on-chromium-and-firefox.md), [continuous edits that re-run their request](decisions/2026-11-13-continuous-edits-rerun-the-request.md), and [the figures on real GPUs as goals](decisions/2026-11-13-gpu-figures-are-goals.md). Against the recommendations: [picking](decisions/2026-11-13-picking-in-the-first-release.md) in the first release, and [Preact with signals](decisions/2026-11-13-preact-with-signals.md) for the page now, which the full web application grows from. [GPU write-through](decisions/2026-11-13-write-through-in-the-first-release.md) was put in the first release against the recommendation, then [moved after it](decisions/2026-11-14-write-through-after-the-first-release.md), and continuous edits go with it. O31 is answered for `intersection`, against the recommendation: quadric pairs with no closed form are intersected [exactly, by Levin's pencil method](decisions/2026-11-13-quadric-pairs-exact-by-levins-method.md), in the first release. O32 is answered for `booleans`, against the recommendation: one boolean takes an object and [a list of tools](decisions/2026-11-14-a-boolean-takes-many-tools.md), in the first release.

| Id | Decision | Recommendation | Needed by |
|---|---|---|---|
| O5 | Cargo dependencies, asked one at a time when their step arrives. Answered: [`gungraun`](decisions/2026-11-09-gungraun.md) (test only, `bedrock.perf`), [`wasm-bindgen`](decisions/2026-11-09-wasm-bindgen.md) (`bedrock.thread`), [`fearless_simd`](decisions/2026-11-10-fearless-simd.md), [`rayon`](decisions/2026-11-10-rayon.md), [`postcard`](decisions/2026-11-12-binary-twin.md) (`construction`, the native format's binary twin) and [`clap`](decisions/2026-11-13-clap.md) (`construction.commands`, the command line). Still expected: a property-test crate (test only) | Decide each on the evidence | when reached |
| O6 | A lawyer's view of the provenance policy and oracle outputs | Worth having before the first public release | first release |
| O7 | Reserve `occam` on crates.io and an npm scope | Reserve once there is something real to publish | first release |
| O9 | A machine with more physical cores for the parallel investigations. This laptop has 4 | Start here, and decide after `proving-ground.p1` shows how far scaling matters | `proving-ground.p2` |
| O10 | A nightly toolchain for wasm threads, which need `-Zbuild-std`. [Spikes may use one already](decisions/2026-11-09-early-spikes.md) | P10 recommends keeping this open and not taking on nightly for real use. Nothing in the first release needs it. Reopen it if a real kernel has a model large enough that a copy per worker hurts, or tasks fine-grained enough that message passing dominates, or when `-Zbuild-std` stabilises | when one of those happens |
| O13 | A stored per-entity accuracy for imported geometry, fixed at import and never grown (S2). Foreign geometry was not measured | Decide when import is in scope | when import is in scope |
| O17 | x86-64-v3 code paths (S3/P3), and whether [`DETERMINISM.md`](DETERMINISM.md) rule 5, "No `target-cpu`", stays as written (P4). The NURBS gain exists only there. Having it means hand-written detection, a new dependency, or a build that excludes older CPUs | Leave this until a real kernel profile shows NURBS evaluation is worth it. Rule 5 needs no change if `dispatch!` is taken under O5 | when a kernel profile asks for it |
| O27 | Whether P1's comparison with OCCT should reach `main`. It holds family medians of the time ratio and counts of the cases where the prototype was faster, with no per-case OCCT number. [Kept out for now](decisions/2026-11-09-p1-occt-comparison.md) | Decide with O6's answer, because the figures are derived from oracle outputs | when O6 is answered |

O11 onward come from the spike findings named in each row, which give the detail.
