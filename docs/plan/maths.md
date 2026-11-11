# Milestone: maths

The first milestone of [the first release](first-release.md). `occam-math` gains what every later crate computes with:
comparison and selection in lanes, dispatch to the widest instruction level, the execution layer, robust decisions, and
its own `sin_cos` and `atan2`. The design is [the maths topic](../architecture/maths.md) and
[threads and lanes](../architecture/threads-and-lanes.md); the contract is [DETERMINISM.md](../DETERMINISM.md).

**Exit (gate):** `just gates`, `just mutants` and `just perf` are green; `gates determinism` gives the same bits across
targets, thread counts and dispatch levels, the owned functions included, against hashes from a separate
implementation; and the vectorisation check in `gates perf` holds every lane kernel.

**PR grouping:** every step shares one PR, with a commit each, so that CI runs once (the user's request, 2026-11-11).

**Throughout.** `f64::sin`, `f64::atan2` and the rest stay banned. The new dependencies are the two already approved,
[`fearless_simd`](../decisions/2026-11-10-fearless-simd.md) and [`rayon`](../decisions/2026-11-10-rayon.md), each named
only in `occam-math`. Randomised tests draw from the crate's own `SplitMix64`, so they draw the same cases on every
target. The determinism stream's separate implementation is `tools/determinism/`, in Python's standard library, and
every change to the stream takes its record from it first.

## Closing the milestone

Every step has merged, one commit each in one PR, whose body holds what was measured. The owned functions'
[coefficients are our own](../decisions/2026-11-11-own-coefficients.md), their accuracy is in
[DETERMINISM.md](../DETERMINISM.md), and the lane widths `gates perf` holds are in `occam_math::width`. The type
sketch's `math.rs` is gone: the sketch now uses the real crate.
