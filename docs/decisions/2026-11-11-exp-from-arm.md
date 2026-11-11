# occam owns `exp`, adapted from phasekit's port of Arm's

**Decided by the user, 2026-11-11,** asked as [`geometry.curves`](../plan/geometry.md) began. A hyperbola, as FreeCAD
gives it, is `centre + a cosh(t) X + b sinh(t) Y`, and `occam-math` owned no `exp`, `cosh` or `sinh`. Asked whether to own
`exp`, to give the hyperbola a rational parameter instead, or to defer the hyperbola, the answer was "Own exp as you
recommend". Asked next whether to write it as [O15](2026-11-11-own-coefficients.md) wrote `sin_cos` and `atan2`, with
occam's own coefficients, or to adapt phasekit's `exp`, a port of Arm's optimized-routines with Arm's table and
coefficients, the answer was "Adapt phasekit's Arm exp".

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- `occam-math` owns `exp` and `expm1`, with `sinh` and `cosh` on them, and the hyperbola keeps FreeCAD's parameter.
- `exp` is phasekit's port of Arm's `math/exp.c` and its table from `math/exp_data.c` (optimized-routines v26.07,
  without FMA), and `expm1` is phasekit's own on that reduction. O15 still governs `sin_cos` and `atan2`; it does not
  cover `exp`.
- Arm's code is MIT OR Apache-2.0 WITH LLVM-exception, which is permissive. The file says where it came from,
  `REUSE.toml` names Arm's copyright and licence, `LICENSES/` holds the LLVM exception's text, and `docs/SOURCES.md` has
  its row.
- `exp` joins the determinism contract like the other owned functions: in the stream, in the separate implementation,
  and measured there against a high-precision reference.
