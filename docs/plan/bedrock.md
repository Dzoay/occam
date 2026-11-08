# Milestone: bedrock

The repository, every gate, and the types that cannot be retrofitted, proven by one trivial operation through every
layer. No geometry. The crates created here are `occam-report`, `occam-math`, `occam-units`, `occam`, `occam-cli`,
`occam-wasm` and `occam-xtask`.

**Exit (gate):** `just gates`, `just mutants` and `just perf` are green, every gate has caught its seeded violation, and
`bedrock.thread` works from Rust, the command line and wasm.

**PR grouping:** `repo` and `gates` share a PR. Every other step gets its own.

## Steps

### `bedrock.ci`

GitHub Actions running the same commands: diff-based mutants in shards, wasm, the Windows check, and the nightly workflow.
Holds the 45-minute goal from PLAN §5. As in phasekit there is no `rust-toolchain.toml`: CI names one stable version, and
comparisons happen within one build.

*Check:* a deliberately failing PR is red for the right reason.

### `bedrock.report`

`occam-report`, std-only:

- `Report`: a stable `Code`, the entities involved, named values tagged by quantity, `Remedy` values with parameters
  (L3), and partial failure state (L5). Failure paths may allocate.
- `Outcome<T>`: a value plus diagnostics, so success reports too (L7).
- Batch results, one per item (L6).
- One JSON form with a schema version, written by hand (L4), and a listable catalogue of codes and remedies (L8).
- Wording lives here, with numbers displayed in the caller's units (L9).
- `followed_remedy!(code, remedy)`: a test registers the pair and retries using the parameters in the `Report` it received.
- `gates errors`, which compares that registry with the catalogue.

*Failing test first:* a `Report` round-trips through JSON unchanged, and a catalogued pair with no registered test fails
`gates errors`.

### `bedrock.maths`

`occam-math`'s skeleton: the determinism contract written down, the sealed `Real` trait with scalar and `[f64; W]` lane
implementations of basic arithmetic under `forbid(unsafe_code)`, a separate simd128 build for wasm, and
`gates determinism` (L23). `proving-ground.s5` decides which functions occam implements itself.

*Failing test first:* a stream of operations gives the same output hash on x86-64 and wasm32, scalar and lanes.

### `bedrock.units`

`occam-units`: `Length`, `Angle`, points and vectors typed by frame (world, local, UV), one conversion point at the API,
and a per-model `Tolerance` that is never global (L22). The internal length unit is the millimetre.

Compile-fail checks are `occam-xtask` fixtures: each runs `rustc --error-format=json`, asserts the expected error code, and
has a positive twin that must compile, so a fixture cannot pass just because the type does not exist yet.

*Failing test first:* compile-fail fixtures for adding a world point to a UV point and for passing a bare `f64` as a
`Length`; unit round-trip tests.

### `bedrock.thread`

One trivial operation through every crate listed above: measure the distance between two points, refusing a tolerance
below the model's. It gives a `Report` on failure, a followed remedy, and an `Outcome` on success, through the `occam`
composition crate, `occam-cli --json` and `occam-wasm`, under mutants. This proves the seams before there is any geometry
to hide behind. Needs `wasm-bindgen` from O5.

*Failing test first:* the command-line refusal is asserted as JSON, and following its remedy succeeds.

### `bedrock.perf`

The instruction-count gate per operation class, browser timing reported on every PR with the simd128-against-scalar
ratio gated (L30), and baseline records stating the machine and governor. Hosted runners cannot pin a governor, so
wall-clock numbers are goals only (L31). Needs `gungraun` from O5.

*Failing test first:* a deliberately slowed `bedrock.thread` fails the gate.
