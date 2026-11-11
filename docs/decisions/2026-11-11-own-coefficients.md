# occam fits its own coefficients for the owned functions

**Decided by the user, 2026-11-11,** asked [O15](../PLAN.md#7-decisions-the-user-still-owns) as `maths` began: whether
the owned `sin_cos` and `atan2` should use fdlibm's published coefficients and carry Sun's notice, or occam's own. The
answer was "Generate our own", the recommended option.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- The polynomial coefficients, π/2 split for the reduction, and the bits of 2/π come from a script in `tools/`, in
  Python's standard library with exact fractions, committed so anyone can regenerate them. `tools-test` checks that the
  Rust file it writes is up to date. Step [`maths.coefficients`](../plan/maths.md) builds it.
- No fdlibm source or table is opened to write them. The methods are the published ones the spike cited: Cody and
  Waite (1980), Payne and Hanek (1983), and a minimax fit by Remez's exchange.
- No third-party notice joins `LICENSES/`, so the repository stays MIT OR Apache-2.0 alone.
- The accuracy S5 measured with fdlibm's kernels is the target: `sin` and `cos` within about 0.75 ulps, `atan2` within
  about 1.5. If our fit cannot reach it, that is reported, not hidden.
