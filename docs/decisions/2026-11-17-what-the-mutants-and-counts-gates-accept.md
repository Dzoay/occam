# What the mutants and counts gates accept

**Decided by the user, 2026-11-17,** saying "I accept your 3 points" to the three choices put to them after the
review backlog of 2026-11-17's item 7 was fixed. Each was the recommendation.

1. **A renamed test needs a footer.** `gates counts` compares the tests' names in each binary with the base's
   ([measured, not stored](2026-11-14-test-counts-measured-not-stored.md)). A test that is renamed, or moved to another
   binary, is gone from where it was, and a rename cannot be told from a swap, so it needs a
   `Tests-Removed: <binary> <reason>` footer as a removal does.
2. **A mutant that runs out of time twice is caught.** `gates mutants` tests a mutant that timed out again, alone, and
   what that says stands: caught, missed, or out of time again, which counts as caught
   ([the mechanism](2026-11-14-mutants-keep-the-whole-workspace.md#what-a-copy-says-is-checked)). The timeout is not
   made longer for the number of copies running at once, since the retest alone answers what that would.
3. **A broken environment fails the whole pass.** When a copy that listed an unviable mutant no longer builds the
   unmutated tests, the gate fails with `mutants.environment-broke` and records nothing, whatever the other copies
   found.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- A branch in flight that renames a test fails `gates counts` in CI until its PR body carries the footer.
- A mutant that makes a test loop for ever costs the timeout twice, and needs no exclusion.
- After `mutants.environment-broke`, the run is repeated in full once the machine is mended; `--iterate` has nothing
  to go on from it.
