# Test counts are measured against the base, not stored

**Decided by the user, 2026-11-14,** asked while the `construction` branch was rebased onto `main`. The user asked
whether the stored test counts and similar state could be measured by tools instead, since keeping them current "seems
to slow us down". Two files hold measured state: `ci/test-counts.txt`, a floor of tests per test binary so that tests
cannot silently disappear (L44 in [the phasekit lessons](../evidence/2026-11-08-phasekit-lessons.md)), and
`ci/perf-baseline.txt`, the instruction counts per operation class. Every branch edits both, so they conflict on every
rebase. The recommendation was to measure both against `main` in CI and drop both files. **The user chose to drop only
the test counts: "Only drop test counts".** The perf record stays as it is.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- `ci/test-counts.txt`, its merge driver (`scripts/merge-counts.sh`, the `.gitattributes` line, `just setup`'s git
  config) and `just counts` go.
- The guard stays: tests still cannot silently disappear. `gates counts` lists the tests of the change's base and of the
  change itself, and fails when a test binary has fewer tests than on the base, or is gone.
  Since 2026-11-17 it compares the tests' names, not how many there are: a binary fails when a test it has on the base
  is not among its tests now, so a test swapped for another, or renamed, is a removal too. Nothing more is stored.
- A deliberate removal names itself in the change, with its reason, as a commit footer that the gate reads, as
  lowering a minimum by hand needed a reason before.
- Building the base costs a second build, so the comparison runs in CI. A local run without a base to compare against
  says so rather than passing silently.
- `ci/perf-baseline.txt` and `just perf --update` are unchanged.
