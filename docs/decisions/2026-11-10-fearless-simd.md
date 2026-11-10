# fearless_simd for lane dispatch

**Decided by the user, 2026-11-10,** asked about `fearless_simd` and `rayon` under
[O5](../PLAN.md#7-decisions-the-user-still-owns) before `proving-ground.design`: "fearless_simd and rayon are fine to
use." The kernel may depend on `fearless_simd`.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- It is for `dispatch!` over `occam_math::Lanes<W>`, as [P4](../evidence/2026-11-09-p4.md) recommended. Under
  `forbid(unsafe_code)` and without a `target-cpu` build, `dispatch!` is the only way to reach AVX2, which gave the mask
  kernels a factor of 1.5 to 2 with the same bits. Kernels stay written over plain `Lanes<W>`. The crate's explicit SIMD
  types are left alone unless a later measurement asks for them.
- Only `occam-math` names it, so the rest of the kernel sees `Lanes<W>`. [`DETERMINISM.md`](../DETERMINISM.md) rule 5,
  "No `target-cpu`", stands as written, and `gates determinism` holds every dispatched path to the same bits.
- It enters a `Cargo.toml` with the first step that uses it, and its line in `ci/dependencies.txt` cites this decision.
  Version 1.2.0, which the spikes used, has no dependencies of its own and is MIT OR Apache-2.0, so
  [the no-copyleft decision](2026-11-08-no-copyleft.md) holds.
