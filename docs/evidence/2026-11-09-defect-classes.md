# Defect classes in public issue reports

Evidence for the plan step `census.defects` in [the census milestone](../plan/census.md), measured on 2026-11-09. It
ranks the most common failure classes in FreeCAD's and OCCT's public trackers, says which of them occam will build by
[the plan](../PLAN.md) and [its first release](../plan/first-release.md), and names the proof that would show each built
class is designed out (L37, L39).

Obeys [the provenance decision](../decisions/2026-11-08-licence-and-provenance.md): a script fetched public issues and
stripped code and patches, and only the stripped text was read, for symptoms. This file holds classes, counts and my own
summaries. It holds no issue text, stack trace or name.

## What was fetched

Anonymous GitHub API, with the client name `occam-census` and no token or contact parameter. 25 requests in total: 3 for
labels and 22 for pages of issues, under a stop at 20 remaining and a limit of 40 an hour. Comments were not fetched.

| Source | Label(s) | Sort | Pages | Issues kept | Pull requests dropped | Walk |
|---|---|---|---:|---:|---:|---|
| FreeCAD | `Mod: Part` and `Type: Bug` | newest first | 4 | 160 | 240 | capped by page limit |
| FreeCAD | `Mod: Part Design` and `Type: Bug` | newest first | 4 | 174 | 226 | capped by page limit |
| FreeCAD | `3rd party: OCC` | newest first | 3 | 299 | 1 | capped by page limit |
| FreeCAD | `Topic: Toponaming` | newest first | 2 | 131 | 69 | capped by page limit |
| FreeCAD | `Topic: Fillets` | newest first | 2 | 126 | 0 | complete |
| FreeCAD | `File format: STEP` | newest first | 1 | 99 | 0 | complete |
| FreeCAD | `Topic: Performance` | newest first | 2 | 137 | 3 | complete |
| OCCT | `2. Bug` | newest first | 4 | 232 | 136 | complete |

1,157 unique issues after removing 201 duplicates across labels: 925 from FreeCAD, 232 from OCCT.

**Not reached:** OCCT's Mantis tracker at `tracker.dev.opencascade.org`. Its hostname did not resolve at either attempt,
while other `opencascade.org` hosts did. Reports held only in that tracker are not in this measurement.

## Method

Everything is in `tools/census/`, standard library only: `defects_fetch.py`, `defects_strip.py` and
`defects_classify.py`, developed test-first on synthetic fixtures.

**Stripping** removes fenced and indented code, inline code spans, `<pre>` and similar HTML, stack traces, diff hunks,
unfenced code lines, links to patches, commits, pull requests and diffs, attachments, images, every other URL (link text
is kept for ordinary pages only), `@mentions`, email addresses, sign-offs and author trailers, user paths, and quoted
text. Library identifiers left in prose are reduced to plain words. Sentences that describe code rather than symptoms
are dropped as well. The fetcher stores no author, assignee or date beyond the creation day.

**Classifying** uses keyword rules, deterministic and with no model. An issue joins a class if a topic word appears in
its title, or if a topic word and a failure word appear in one sentence of its body. Some classes also require a
failure word in the title, or exclude sentences that mention a file format, so that an incidental mention does not
count. Issues labelled as features, questions or documentation, or titled as feature requests, are excluded (24 of the
broad sample). The rules were tuned on small samples of stripped text, using matched sentences only, and each change was
test-first.

**The ranking uses the broad sample only**: issues reached without searching for a symptom, meaning FreeCAD's
`Mod: Part` and `Mod: Part Design` bugs and `3rd party: OCC` reports, and OCCT's `2. Bug` reports. Topical label pages
(toponaming, fillets, STEP, performance) would inflate the classes they were fetched for, so they are shown separately
as "all fetched". Counts are multi-label: one report can be in several classes, so columns do not sum to totals. Of 823
defects in the broad sample (593 FreeCAD, 230 OCCT), 151 had no class.

## Limits

- **Sample, not census.** FreeCAD's walks stopped at their page limit, newest first, so this is mainly recent reports
  against recent versions. Only OCCT's GitHub bug label was walked to the end.
- **The broad sample is kernel-leaning by design.** FreeCAD's application-level defects (interface, Sketcher, FEM, CAM)
  are undercounted because they are not what was asked for.
- **Tracker populations differ.** FreeCAD is user reports; OCCT is mostly developers and downstream integrators. The
  per-source columns matter more than the total.
- **Keyword rules are approximate.** Precision below comes from my own reading of small samples (7 to 12 matched
  excerpts per class) and is a judgement, not a measurement. Recall is cross-checked against FreeCAD's own labels where
  a label exists:

  | FreeCAD label | Class | Recall |
  |---|---|---:|
  | `Topic: Thickness` | offset-shell | 20/20 (100%) |
  | `Topic: Fillets` | fillet-chamfer | 105/109 (96%) |
  | `Type: Crash` | crash | 70/75 (93%) |
  | `File format: STEP` | exchange | 83/90 (92%) |
  | `Topic: Performance` | performance | 66/115 (57%) |
  | `Topic: Toponaming` | topo-naming | 65/118 (55%) |

  So fillet, crash and exchange counts are nearly complete where labelled, while topo-naming and performance are
  undercounted by about half, because many labelled reports say neither word.
- **Names in prose are not removed reliably.** The stripper drops the common patterns, but free-text names can survive
  in stripped text. Nothing derived from reading them appears here.

## The ranking

Broad sample, ranked by total. `Scope` says whether occam builds the class: *first release* cites the milestone that
delivers it, *after* means the plan names it for after the first release (§6) and each needs the user's go-ahead,
*outside* means it is not a modelling-kernel concern or not in the plan. The proofs follow in the next section. Sampled
precision is my estimate from reading matched excerpts.

| # | Class | Total | FreeCAD | OCCT | All fetched | Sampled precision | Scope |
|---:|---|---:|---:|---:|---:|---|---|
| 1 | Fillet and chamfer failures | 164 | 132 | 32 | 198 | about 90% | first release, `blends` (constant radius) |
| 2 | Crashes (segfault, abort, access violation) | 120 | 77 | 43 | 153 | about 95% | first release, cross-cutting |
| 3 | Extrude, revolve, pad and pocket | 98 | 92 | 6 | 110 | about 75% | first release, `construction` |
| 4 | Sweep, pipe and loft | 75 | 49 | 26 | 78 | about 75% | after |
| 5 | Boolean operations fail or give wrong results | 74 | 50 | 24 | 79 | about 85% | first release, `booleans` |
| 6 | STEP, IGES and BREP import and export | 63 | 21 | 42 | 127 | about 85% | first release, `exchange` (STEP only) |
| 7 | 3D view and rendering | 46 | 37 | 9 | 70 | about 75% | outside |
| 8 | Curve and surface intersection, section and projection | 45 | 32 | 13 | 46 | about 65% | first release, `intersection` |
| 9 | Application interface: dialogs, panels, menus | 42 | 41 | 1 | 72 | not sampled | outside |
| 10 | Operations return invalid shapes | 41 | 24 | 17 | 49 | about 95% | first release, `topology` and the release gate |
| 11 | Pattern, mirror and transformation features | 40 | 39 | 1 | 44 | about 75% | outside the first release |
| 12 | Offset, shell and thickness | 38 | 33 | 5 | 39 | about 80% | after |
| 13 | Topological naming: references break after an edit | 35 | 35 | 0 | 71 | about 75% | partly: `topology` for stable ids and history; document layer after |
| 14 | Build, compile and packaging | 31 | 5 | 26 | 33 | about 75% | outside |
| 15 | Hangs, freezes and infinite loops | 26 | 16 | 10 | 50 | about 75% | first release, cross-cutting |
| 16 | Placement and attachment of features | 24 | 22 | 2 | 28 | not sampled | outside the first release |
| 17 | Tolerance, gap and near-coincidence trouble | 21 | 10 | 11 | 26 | about 60% | first release, `occam-units` and `occam-check` |
| 18 | Hidden-line drawings and TechDraw | 20 | 16 | 4 | 24 | not sampled | after |
| 19 | Curves, B-splines and NURBS | 19 | 10 | 9 | 19 | about 85% | first release, `geometry` |
| 20 | Other formats (STL, OBJ, DXF, 3MF, SVG, glTF, IFC), tied at 19 | 19 | 7 | 12 | 27 | not sampled | outside the first release |
| 20 | Python bindings, macros and the scripting API, tied at 19 | 19 | 15 | 4 | 23 | not sampled | outside |

Below the twenty, each with its scope, because the plan names them or they back a commitment:

| Class | Total | FreeCAD | OCCT | All fetched | Scope |
|---|---:|---:|---:|---:|---|
| Wrong measurements: volume, area, inertia, bounding box | 17 | 7 | 10 | 20 | first release, oracle comparison |
| Tessellation and meshing defects | 16 | 6 | 10 | 24 | first release, `view` |
| Silent or unhelpful failure: empty result, no reason given | 15 | 10 | 5 | 17 | first release, `Report` |
| Performance: slow operations | 14 | 12 | 2 | 72 | first release, measured (`gates perf`, a goal) |
| Sketcher and constraint solving | 14 | 13 | 1 | 42 | after |
| Memory leaks and exhaustion | 10 | 3 | 7 | 15 | first release, partly |
| Assembly | 10 | 10 | 0 | 24 | outside |
| Shape healing and repair | 9 | 6 | 3 | 9 | outside the first release |
| Units and unit conversion | 7 | 3 | 4 | 10 | first release, `occam-units` |
| Finite element analysis | 6 | 6 | 0 | 9 | outside |
| Feature-tree recompute failures | 5 | 5 | 0 | 7 | after, document layer |
| Non-deterministic results and thread-safety | 4 | 2 | 2 | 4 | first release, `gates determinism` |
| Architecture and BIM | 3 | 2 | 1 | 9 | outside |
| CAM and toolpaths | 3 | 3 | 0 | 14 | outside |
| Native file save and load failures | 2 | 2 | 0 | 6 | first release, native format |
| Draft angles | 1 | 1 | 0 | 1 | after |

## Proofs for the classes occam will build

Each row is the invariant or test that would show the class is designed out, in behaviour terms. These are inputs to
each milestone's plan, not instructions (see [evidence README](README.md)).

| Class | Proof or invariant |
|---|---|
| Fillet and chamfer | The bracket test: too large a radius returns a report naming the edge and the largest feasible radius, and a followed-remedy test confirms following it succeeds. Per-edge results, so one bad edge never fails the others. A property test over randomised edges with a closed-form maximum radius, checking volume against the closed form and passing the invariant checker. |
| Crashes | No panics, no unchecked indexing or arithmetic in kernel crates (the lint gate). Every operation returns a report, and the input is unchanged on failure (L5). A nightly randomised operation-list run and a malformed-STEP fuzz that assert no process ever aborts. |
| Extrude, revolve, pad and pocket | Randomised profiles with closed-form volume and area (prism, Pappus); tangent and seam-edge inputs succeed and pass the checker, or return a report naming the input edge. |
| Boolean operations | The volume identity vol(A union B) + vol(A intersect B) = vol(A) + vol(B) over randomised near-tangent, coplanar and coaxial pairs, nightly. Every returned result passes the invariant checker. A failure report carries the failing pair, the curves found and the residual gap (L5). Deterministic across thread counts. |
| STEP import and export | Round trip through the pinned OCCT oracle with matching volume and validity; losses reported per entity; inch-unit and non-ASCII label round trips; malformed input returns a report and never aborts. |
| Intersection | Closed-form agreement on analytic pairs, with completeness reported: the count found equals the closed-form count, no repeated points beyond `Tolerance`, and tangent contacts classified as such. |
| Invalid shapes | The first-release gate itself: every result returned as a success passes the invariant checker, in release builds too. Seeded-corruption tests that the checker catches each invariant, and a corpus run counting zero successes that fail it. |
| Topological naming (kernel half) | Stable ids and a full created, changed and deleted history from every operation (`occam-topo`). After randomised parameter edits and replay, every result entity traces through history to its inputs; a reference that cannot be carried returns a report rather than binding to another face. The document-level naming layer is after the first release. |
| Hangs | Every iterative solver has an iteration budget, and exhausting it returns a report with the residual (L5). A nightly randomised run asserts no operation exceeds its budget. The plan does not name this yet. |
| Tolerance and gaps | One per-model `Tolerance`, never silently grown. On randomised near-coincident inputs, any returned shape satisfies the checker's gap bounds, and any failure report names the pair and the measured gap. |
| Curves and NURBS | A NURBS arc evaluates as its circle, finite-difference derivatives agree, and parameter ranges stay within their domain through every operation (a checker invariant). |
| Measurements | Volume, area, inertia and bounding box equal the closed form on primitives; the box contains every sampled point and transforms with the shape. |
| Tessellation | Asserted on the mesh buffers: watertight (every mesh edge shared by exactly two triangles), seam edges agree across faces, no zero-area triangles, and deviation within the requested bound. |
| Silent failure | `gates errors`: every code has a remedy and a followed-remedy test (L2). A success cannot carry a null or empty shape unless empty is the declared answer. |
| Performance | `gates perf` instruction counts per operation class, browser timing per pull request, compared with the `census.baseline` numbers. A goal, not a gate, until measured (L46). |
| Memory | Rust ownership and arenas remove leaks and overruns by construction; a corpus-wide peak-allocation measure reports regressions, as a goal. |
| Units | Typed lengths and angles, converted once at the API, with compile-fail fixtures; a STEP round trip with inch input keeps its dimensions. |
| Non-determinism | `gates determinism`: identical bits at any thread count, and on x86-64 and single-threaded wasm32 for what `Real` computes (L23). |
| Native save and load | Save, load and compare round trips; the readable dump matches; a corrupted file returns a report. |

## Outside scope

- **After the first release (§6), each needing the user's go-ahead:** sweep and loft, offset and shell, draft angles, the
  sketch solver, hidden-line drawings, recompute and the document layer.
- **Not in the first release and not on the parity list until `census.parity` says otherwise:** patterns and mirror,
  placement and attachment, shape healing, other exchange formats.
- **Not a modelling-kernel concern:** 3D view and rendering (occam's `view` is a small wasm viewer over tessellation),
  the application interface, build and packaging, Python and scripting bindings, assembly, FEM, CAM and BIM.
