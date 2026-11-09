# Milestone: census

What "replacement" means, measured from outside. Runs alongside `bedrock`. Everything here obeys
[the provenance rules](../decisions/2026-11-08-licence-and-provenance.md). A session working on this milestone is a
survey session and never writes to `crates/`. Output goes in `docs/evidence/`, except `docs/PARITY.md`, which is a living
document, and the harness in `tools/oracle/`.

**Exit (gate):** the user has approved `docs/PARITY.md`, and the oracle harness reproduces its own results on a second
run.

**PR grouping:** one PR each.

## Steps

### `census.oracle`

`tools/oracle/`: OCCT 8.0.1 from the `cadquery-ocp` 8.0.1.1.0 wheel (CPython 3.11 or later), run through `uv` outside the
workspace, in a scrubbed environment with threading fixed (L16). The harness runs an operation and reports volume, area, bounding box, Euler characteristic and
validity. It compares geometrically, never by topology or bits (L12). A divergence register records disagreements, and
invariants decide them (L11). `arris` and `solidcore` at pinned commits give second opinions, driven only through their
documented interfaces.

`occam-verify` starts here as the Rust side: it reads oracle result files and compares them.

*Failing test first:* the harness rejects a deliberately wrong volume, and two runs give identical results.

### `census.corpus`

Test models with origin and licence recorded per case:

- At least 200 generated parametric cases, as operation lists with closed-form answers where they exist. These serve
  the spikes and every milestone before `exchange`.
- The NIST CAD test parts whose terms allow use. These join the tests once STEP reading exists.
- `arris` cases with attribution. A case whose origin or licence is unclear, or whose licence is
  [copyleft](../decisions/2026-11-08-no-copyleft.md), stays out.
- `solidcore` cases stay external until the user rules on them.

Small fixtures are committed. Large corpora are generated or fetched by a locked pipeline for the nightly run (L16).

*Output:* `docs/evidence/<date>-corpus-inventory.md`, and the pipeline.

### `census.baseline`

Whole-operation OCCT timings on the generated corpus: booleans, fillets, tessellation and thread scaling, plus the same
through `opencascade.js` (LGPL-2.1, outside the workspace) for the browser (L32), on a recorded machine. These are the numbers occam has to beat. Speed
targets that follow are goals, not gates, until they have been measured (L46).

*Output:* `docs/evidence/<date>-occt-baseline.md`.
