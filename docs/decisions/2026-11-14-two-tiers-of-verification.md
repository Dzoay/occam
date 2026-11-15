# Two tiers of verification: a worker's, and the integrated branch's

**Decided by the user, 2026-11-14,** in answer to a measured proposal of seven changes to how work is checked: "lets do
all 7". This is one of them. Until now every worker ran the full gates, the perf gate and the mutants over its own
change before each commit.

What was measured on the build host, 12 to 14 November:

- 68 jobs ran for 21 hours, and waited 15.6 hours for the lock that lets one job run at a time.
- 17 jobs failed, and cost 8.3 hours. Many failed on checks that take seconds, the complexity gate four times, which
  `just gates` ran last, after clippy, the tests and wasm.
- Mutation testing was 12 of the 21 hours.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- **A worker's tier, before each commit:** the tests of the crate it changed, clippy on that crate, and
  `just preflight`, which is formatting and the gates that only read the repository. None of them needs the
  one-job lock.
- **The integrated branch's tier, once:** `just gates`, `just perf` and `just mutants` run on the branch that brings
  the workers' commits together, and again in CI on its PR. They do not run once per worker.
- Perf is measured once on the base, and that measurement is shared: a worker does not measure the base again to
  compare against.
- A worker still shows its test red before implementing, and still reports what it could not check.
- `just gates` runs `just preflight` first, so a quick check that fails does so before anything slow starts.
- The gates themselves are unchanged and live in [the plan](../PLAN.md#5-gates). CI still runs all of them on every PR.
