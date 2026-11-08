# The first release (outline)

An outline only. `proving-ground` will change it, and each milestone gets its detailed file when it is next (L43).

**Every operation, from `construction` on, ships complete:** its request and result schema, `Report` codes with
followed-remedy tests, command-line and wasm exposure, invariant checks, a performance bench and browser timing.

**Gates:** every result returned as a success passes the invariant checker, in release builds too, and results are
deterministic. **Goals:** success rate and speed against OCCT on the corpus (L46). Until `exchange` exists, "the corpus"
means the generated parametric cases; the NIST STEP parts join when STEP reading lands.

| Milestone | Delivers | Exit |
|---|---|---|
| `maths` | Lanes, exact predicates, intervals, the owned maths functions, thread and lane execution | Determinism across targets and thread counts |
| `geometry` | Analytic curves and surfaces, NURBS, mesh-backed surfaces, batch evaluation | One seam serves all three families; then the seam is frozen (L21) |
| `topology` | Arena, stable ids, history, immutable snapshots, `occam-check` | Euler and closure checks; seeded corruptions are caught |
| `construction` | Primitives, extrude and revolve; operations as data; `occam run ops.json --json`; a listing of what is supported and what is refused; native format with a readable dump | An agent builds a solid from JSON alone |
| `view` | Tessellation, a small browser viewer in a worker on typed arrays | Mesh buffers asserted; watertight; timed in the browser |
| `intersection` | Curve and surface intersection, with completeness reported | Oracle and closed-form agreement across the corpus (goal) |
| `booleans` | Fuse, cut and common, with history and per-pair failure reports | Invariants on all returned results (gate); randomised sequences nightly |
| `exchange` | STEP read and write | Round trip through the OCCT oracle; losses reported |
| `blends` | Constant-radius fillets and chamfers, per-edge results | Remedies followed by test, including the feasible radius |
| `release` | The bracket test, docs for agents, licence review | The bracket test passes and the user says release |
