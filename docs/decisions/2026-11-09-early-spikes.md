# Six investigations start early, and what spikes may use

**Decided by the user, 2026-11-09,** asked whether six investigations that do not need census results may start before
`proving-ground` opens, and whether spikes may use `rayon`, `fearless_simd` and a nightly toolchain: "The six investigations
can start yes. for the spikes you can used rayon, fearless_simd and a nightly toolchain for wasm threads."

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- `proving-ground.s3` with `.p3`, `.s4`, `.s5`, `.p4`, `.p10` and `.p11` start now, because none of them needs the oracle or
  the corpus. The rest of the milestone waits for `census.oracle` and `census.corpus` as planned.
- Their code and written findings wait on the never-merged `spikes` branch. The findings PRs to `main` follow when
  `proving-ground` opens, so `just next` and `gates plan` are unaffected.
- `rayon`, `fearless_simd` and a nightly toolchain are allowed in spike code only, which is thrown away. Putting any of them
  in a crate, or using a nightly toolchain for real, is still its own question ([O5 and O10](../PLAN.md#7-decisions-the-user-still-owns)).
- Speed is measured one run at a time on the user's second machine, which has six physical cores and is otherwise quiet.
  Findings describe it by CPU, core count and governor, and never name it.
