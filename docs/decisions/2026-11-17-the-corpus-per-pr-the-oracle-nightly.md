# The corpus runs per PR, the oracle nightly

**Decided by the user, 2026-11-17,** saying "for these, I agree" to three recommendations made after the review backlog
of 2026-11-17's item 5 was fixed.

1. **The OCCT readings come from a nightly job.** It runs the oracle over the corpus and then `occam-verify`'s
   comparison on what the oracle wrote, as part of [`intersection.timing`](../plan/intersection.md). No oracle output
   enters the repository, so O6 is untouched.
2. **`corpus-hits` runs on every PR.** It was nightly because it took minutes in a debug build. It takes 3.5 seconds on
   the build host and about 10 on the laptop.
3. **A public validity check waits for booleans.** In the per-PR corpus test, "valid" means that the runner accepted
   the run, whose commit runs the checker. Whether occam exposes its checker to be called on a model is decided when
   [`booleans`](../plan/booleans.md) exist.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- `occam-verify`'s `load` keeps no caller until `intersection.timing` adds the nightly job.
- The nightly workflow's `corpus` job is gone; `just corpus-hits` runs the one test alone and prints its totals.
- The corpus test runs under the mutants gate too, after the unit tests, as the other tests seconds long do.
