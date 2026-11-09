# Milestone: bedrock

The repository, every gate, and the types that cannot be retrofitted, proven by one trivial operation through every
layer. No geometry. The crates created here are `occam-report`, `occam-math`, `occam-units`, `occam`, `occam-cli`,
`occam-wasm` and `occam-xtask`.

**Exit (gate):** `just gates`, `just mutants` and `just perf` are green, every gate has caught its seeded violation, and
`bedrock.thread` works from Rust, the command line and wasm.

**PR grouping:** `repo` and `gates` share a PR. Every other step gets its own.

## Steps

### `bedrock.perf`

The instruction-count gate per operation class, browser timing reported on every PR with the simd128-against-scalar
ratio gated (L30), and baseline records stating the machine and governor. Hosted runners cannot pin a governor, so
wall-clock numbers are goals only (L31). Needs `gungraun` from O5.

*Failing test first:* a deliberately slowed `bedrock.thread` fails the gate.
