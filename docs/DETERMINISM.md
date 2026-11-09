# The determinism contract

In a CAD kernel a flipped last bit changes topology, so occam promises the same bits, not merely close answers (L23).
`occam-math` is the one place numbers are computed, and [`gates determinism`](PLAN.md#5-gates) holds the promise.

## The promise

For the same inputs, every output has identical bits:

- on x86-64 and on single-threaded wasm32, in its baseline and simd128 builds;
- at any thread count;
- as `f64` or as `Lanes` of any width;
- in debug and in release builds.

## The rules

1. **One maths implementation.** The kernel computes through `occam_math::Real`, and its numbers are `f64`. `Real`'s
   operations (`+`, `-`, `*`, `/`, negation, `abs`, `sqrt`) are correctly rounded under IEEE 754, so they give the same
   bits everywhere. An operation joins only if it can keep that promise.
   [`proving-ground.s5`](plan/proving-ground.md) decides which of `sin`, `cos`, `atan2`, `acos` and the others occam
   implements itself.
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

## What holds it

`crates/occam-math/src/stream.rs` is the stream: blocks of pseudo-random operations over awkward inputs (NaN of every
pattern, the infinities, both zeros, subnormals) and ordinary ones, hashed with FNV-1a per operation. The hash is 64-bit
FNV-1a of the little-endian canonical bits of each output, and the blocks' hashes are folded in block order.

`gates determinism` builds `crates/occam-math/examples/determinism.rs` for release on x86-64 at one thread and at many,
and on wasm32-wasip2 baseline and simd128 under wasmtime. Each build runs the stream as scalar and as two, four and eight
lanes. Every hash must agree, and must equal the record in `ci/determinism-hashes.txt`.

The same stream is a test, so the suite checks it in debug builds on x86-64 and on wasm32 baseline and simd128 (`just
test-all` and `just wasm`).

## The recorded hashes

`ci/determinism-hashes.txt` comes from a separate implementation in another language, which shares no code with
`occam-math`. So a wrong answer that every build agrees on, such as a dropped NaN canonicalisation, still fails.

Changing a recorded hash changes the contract. Do it only when the stream's definition changes on purpose, derive the
new values from a separate implementation again, and say so in the PR.

## What it does not cover yet

- wasmtime on x86-64 runs the same floating-point instructions as native code, so comparing these builds cannot show a
  difference in NaN bits that only another CPU would produce. Rule 3 and the record are what guard against that.
- Windows is compile-checked only (`just wasm`), and aarch64 is not built.
- Browser engines are not run until `bedrock.thread` and `bedrock.perf`.
