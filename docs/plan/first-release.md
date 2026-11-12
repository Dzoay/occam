# The first release (outline)

An outline only: each milestone gets its detailed file when it is next (L43). The design it builds is
[ARCHITECTURE.md](../ARCHITECTURE.md), and the rows are those [PARITY](../PARITY.md) places in each milestone.

**Every operation, from `construction` on, ships complete:** its request and result schema, `Report` codes with
followed-remedy tests, command-line and wasm exposure, invariant checks, a performance bench and browser timing.

**Gates:** every result returned as a success passes the invariant checker, in release builds too, and results are
deterministic. **Goals:** success rate and speed against OCCT on the corpus (L46). Until `exchange` exists, "the corpus"
means the generated parametric cases; the NIST STEP parts join when STEP reading lands.

**The type sketch:** each milestone deletes the modules of [docs/sketch](../sketch/src/lib.rs) that its crates now
hold.

| Milestone | Delivers | Parity rows | Exit |
|---|---|---|---|
| `maths` | [Detailed in its own file](maths.md) | 1.2, in part | [Detailed in its own file](maths.md) |
| `geometry` | [Detailed in its own file](geometry.md) | 1.2 to 1.5, 2.1 to 2.4, 2.8 | [Detailed in its own file](geometry.md) |
| `topology` | [Detailed in its own file](topology.md) | 1.1, 1.10 | [Detailed in its own file](topology.md) |
| `construction` | Requests validated before any geometry; primitives, wires, planar faces, extrude, revolve and transform; mass properties and bounds; the native format, which is its own dump; the describer; `occam run`, `list`, `adjacent`, `describe` and `check`; a listing of what is supported and what is refused | 1.6, 1.8, 1.12, 1.13, 2.7, 2.11, 2.13, 2.15, 2.17 | An agent builds a solid from JSON alone; the time to load a model of 10^5 faces, parsing and checking timed apart (goal) |
| `view` | P9's mesher, with its limits and the watertight check on every output; P10's worker pool; P12's renderer contract | 1.7 | Mesh buffers asserted; watertight; timed in the browser |
| `intersection` | The method list, closed forms then the hybrid; failures in the result; `BelowResolution`; coincidence detected; edge box tests checked by digest | none | Oracle and closed-form agreement across the corpus (goal) |
| `booleans` | P2's phases with frozen outputs; only touched faces prepared; a sort-and-sweep broad phase; every failing pair reported; coincident surfaces (O25); point classification; the decisions tally | 1.11, 2.14 | Invariants on all returned results (gate); randomised sequences nightly; S6's blind control rerun on boolean failures (goal) |
| `exchange` | STEP read and write; the table of [names and colours](../architecture/topology.md#names-and-colours) | 1.9, 2.9 | Round trip through the OCCT oracle, names and colours included; losses reported |
| `blends` | Constant-radius fillets with a radius per edge, the feasible radius computed first; chamfers; per-edge results | 1.14, 2.16 | Remedies followed by test, including the feasible radius |
| `release` | The bracket test, docs for agents, licence review | none | The bracket test passes and the user says release |
