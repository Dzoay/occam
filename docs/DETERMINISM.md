# The determinism contract

A last-bit difference can change what a kernel decides, and identical bits make every result reproducible and every
parallel run testable (L23). `occam-math` is the one place numbers are computed, and
[`gates determinism`](PLAN.md#5-gates) holds the promise.

## The promise

For the same inputs, every output has identical bits:

- on x86-64 and on single-threaded wasm32, in its baseline and simd128 builds;
- at any thread count;
- as `f64` or as `Lanes` of any width;
- at every instruction level `dispatch` compiles a kernel for;
- in debug and in release builds.

That holds for everything `Real` computes, and for the functions `occam-math` owns: `sin_cos`, with `sin` and `cos`, its
two halves, `atan2`, `exp`, `expm1`, and `sinh_cosh` with `sinh` and `cosh`. [S5](evidence/2026-11-09-s5.md) measured what else would need to be identical. Computing one
model in the browser and on a server is the case that would need more
([the decision](decisions/2026-11-09-one-maths-choke-point.md), O16), and the promise grows only with what is owned.

## The rules

1. **One maths implementation.** The kernel computes through `occam_math::Real`, and its numbers are `f64`. `Real`'s
   operations (`+`, `-`, `*`, `/`, negation, `abs`, `sqrt`, `next_up`, `next_down` and `copysign`) are correctly
   rounded or exact under IEEE 754, so they give the same bits everywhere. Anything added later goes through this crate
   too, so that what it does can change in one place.
   - **`occam_math` owns `sin_cos` and `atan2`** (S5, O14), written over `Real` with `+ - * /` and integer operations
     only, so the promise covers them at every lane width, thread count, level and target, in debug and in release. The
     record holds their hashes, from the separate implementation.
   - **`occam_math` owns `exp` and `expm1`** as well, Arm's `exp` by way of phasekit
     ([the decision](decisions/2026-11-11-exp-from-arm.md)), and `sinh_cosh` on them, written over `Real`. `exp` and
     `expm1` read a table and branch, so they are written for one `f64`, with integer operations on its bits, and in
     lanes each lane runs them alone through `Real::map`: the same bits as a single call, by construction.
   - **They are not correctly rounded.** Their accuracy is stated and tested: `sin` and `cos` within 0.75 ulps, and
     `atan2` within 1.5, on S5's sets; `exp` within 0.51 (Arm's bound), `expm1` within 0.65, and `sinh` and `cosh`
     within 1.1 below 709 in magnitude and within 1.6 from 709 to where they overflow. Each is measured by
     the separate implementation against a reference of at least 120 digits (`tools/determinism/accuracy.py`). No caller
     may rely on more.
   - **Every other transcendental stays banned until `occam-math` owns it,** by rule 2. The ban holds for literal
     arguments too: the compiler folds `f64::sin` of a literal with the libm of the machine that builds it, so one wasm
     build gave different bits in release and debug (S5).
2. **Nothing else computes.** Clippy's `disallowed-methods` bans every `f64` method whose result Rust does not pin down:
   the transcendental functions and powers it documents as non-deterministic, with `to_degrees` and `to_radians`; `min`,
   `max` and `clamp`, which may return either zero; `algebraic_*`, which the optimiser may rewrite; and `mul_add`, which
   rounds once and so differs from `a * b + c`.
3. **A NaN's sign and payload mean nothing.** Rust leaves both unspecified for the result of arithmetic. Whether a value
   is NaN is deterministic, so code may test `is_nan`, but nothing may depend on the sign or payload (`is_sign_negative`,
   `copysign`, `total_cmp`, `to_bits`). Every NaN is replaced by one pattern, `canonical_bits`, before it is hashed. The
   two zeros are different bits and stay different.
4. **Order comes from the data.** Parallel work is divided and merged by item index, never by which thread finished
   first, and the kernel has no hashed collections (Clippy's `disallowed-types`).
5. **One build of the numbers.** No `target-cpu`, and no floating-point flags. Fused operations and reordering are never
   turned on: Rust performs neither unless asked to.
6. **No decision on a computed value compares it with exactly zero** (S5). It uses a band, through `decide_computed`,
   which answers `Undecided` rather than guess, or a predicate on stored inputs, through `decide_stored`, which is
   exact. Identical bits make a decision repeatable, not right: a last-bit difference between libraries flips decisions
   made with no band. A band moves the boundary rather than removing it, so the side of a periodic seam and a count
   that lands on an integer need rules of their own ([the determinism topic](architecture/determinism.md#the-rules)).

## What holds it

`crates/occam-math/src/stream.rs` is the stream: blocks of pseudo-random operations over awkward inputs (NaN of every
pattern, the infinities, both zeros, subnormals) and ordinary ones, hashed with FNV-1a per operation. The hash is 64-bit
FNV-1a of the little-endian canonical bits of each output, and the blocks' hashes are folded in block order.

`gates determinism` builds `crates/occam-math/examples/determinism.rs` for release on x86-64, on one thread and on a
pool of many through `Exec`, and on wasm32-wasip2 baseline and simd128 under wasmtime. The native build also runs at
every narrower instruction level the machine has, each block of the stream dispatched to it. Each run computes the
stream as scalar and as two, four and eight lanes. Every hash must agree, and must equal the record in
`ci/determinism-hashes.txt`.

The same stream is a test, so the suite checks it in debug builds on x86-64 and on wasm32 baseline and simd128 (`just
test-all` and `just wasm`).

The topology has a probe of its own, `crates/occam-topo/src/probe.rs`, built for the gate by
`crates/occam-topo/examples/determinism.rs` under occam-topo's `determinism` feature, which nothing that ships turns on.
It hashes the digests of models built through `Edit`, and the reports `Snapshot::check_all` gives on models corrupted
as no edit could, each report as its JSON. The gate runs it on x86-64 on one thread and on many, and on wasm32-wasip2
baseline and simd128, but not at the narrower instruction levels, because the topology passes no level to anything.
Every hash must agree, and must equal the record in `ci/determinism-topology.txt`. Its test checks the same in debug
builds, on one thread and on four.

## The recorded hashes

`ci/determinism-hashes.txt` comes from a separate implementation in another language, which shares no code with
`occam-math`: [`tools/determinism/stream.py`](../tools/determinism/stream.py), on CPython's IEEE doubles. So a wrong
answer that every build agrees on, such as a dropped NaN canonicalisation, still fails. `just tools-test` checks that
the record is what it computes, and `just determinism-record` prints it.

Changing a recorded hash changes the contract. Do it only when the stream's definition changes on purpose: change the
separate implementation first, take the new values from it, and say so in the PR.

`ci/determinism-topology.txt` has no separate implementation to come from, because there is no second topology: it is
what the x86-64 build prints on one thread (`just determinism-topology`). It holds every other build and thread count
to that one, and holds the topology to its answers, so a change to a model, a check or the digest shows as a changed
hash, to be explained in the PR. Unlike the maths record, it cannot catch an answer that every build gets wrong alike.

## What it does not cover yet

- wasmtime on x86-64 runs the same floating-point instructions as native code, so comparing these builds cannot show a
  difference in NaN bits that only another CPU would produce. Rule 3 and the record are what guard against that.
- The topology's record comes from one of the builds it judges, not from a separate implementation (above).
- Windows is compile-checked only (`just wasm`), and aarch64 is not built.
- One browser engine is run: `gates perf` times Chromium and requires the simd128 and scalar builds to compute the same bits. Other
  engines are not run.
