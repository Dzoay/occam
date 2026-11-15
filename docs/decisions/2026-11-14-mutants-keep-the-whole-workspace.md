# A PR's mutants keep the whole workspace's tests

**Decided by the user, 2026-11-14,** after a narrower scope was proposed, agreed, built and measured. The proposal was
one of seven changes to how work is checked, and the user's first answer was "lets do all 7": on a PR a mutant would
run the tests of its own crate and of the crates that depend on it directly, and the whole workspace's would run
nightly. The measurement below then showed that this was slower, not faster. The recommendation became to keep one
whole-workspace pass per PR, with no nightly mutants run, and the user agreed: "I agree with your assesment".

## What the proposal rested on

Measured on the build host, 12 to 14 November: mutation testing was 12 of the 21 hours its jobs ran; 242 mutants took
70 minutes at 4 jobs; a mutant in `occam-geom` could cost 100 to 600 seconds of rebuilding; and in CI the geometry
milestone's mutants shards took 4.5 to 6 hours per push. `.cargo/mutants.toml` has `test_workspace = true`, and the
proposal took the cost to be every mutant building and running the whole workspace.

## What was measured

The same 58 mutants, run three ways with 3 jobs on a laptop that other sessions were also building on, so the times
are rough. They are every 73rd mutant of the `construction` branch's diff against `main` (2,936 in all, in seven
crates), and 17 "delete field" mutants that cargo-mutants 27.1.0 runs whatever `--re` selects.

| Tests that judge a mutant | Passes | Wall time | Caught | Missed | Unviable | Building, summed over jobs | of which each job's first build | Testing |
|---|---|---|---|---|---|---|---|---|
| The whole workspace's (kept) | 1 | 26.8 min | 41 | 1 | 16 | 4,669 s | 2,393 s | 98 s |
| Its crate's and its direct dependants' (proposed) | 7 | 65.8 min | 41 | 1 | 16 | 9,980 s | 8,533 s | 40 s |
| Its crate's alone | 1 | 18.9 min | 40 | 2 | 16 | 3,354 s | 901 s | 26 s |

- **The tests are seconds; building is everything.** Tests took 26 to 98 seconds of each run.
- **Each pass starts cold.** cargo-mutants copies the tree once per job and builds each copy from nothing, 150 to 930
  seconds a copy here. A scope that differs by crate needs a pass per touched crate, and pays that for each.
- **Cargo already rebuilds only what depends on the mutated crate.** For the 40 mutants that were no job's first build
  in any run, building took 1,060 s, 1,418 s and 1,004 s. The crates the narrower scope leaves out are quick to build.
- **What the narrower scopes missed.** The crate and its direct dependants caught exactly what the whole workspace
  did. The crate alone let one more through, `occam-topo`'s `view.rs:47:9: replace View<'a, K>::of ->
  Option<View<'a, J>> with None`, which only a dependant's tests catch. That is why it was rejected, though it was
  the fastest.
- **A nightly whole-workspace run would add nothing** to a PR gate that already runs the whole workspace.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- `gates mutants` is as it was: one pass against the whole workspace's tests, and `occam-xtask`'s mutants against its
  own, since nothing depends on it. Zero survivors is the gate.
- Kept from the work: `just mutants --iterate`, which skips the mutants an earlier run in the same checkout caught and
  is never the gate; and CI's sixteen shards.
- The mutants build is no longer optimised, and the gate sets the timeout itself. Both come from the measurements
  below.

## Building unoptimised, which was adopted

Since building is everything, the same 58 mutants were run again with the `mutants` profile at opt-level 0, and then
in warm copies made by hand, on the same laptop with less other load than the first runs had.

| Run | Wall time | Building, summed over jobs | of which first builds | Testing | Longest tests of one mutant | Caught, missed, unviable |
|---|---|---|---|---|---|---|
| Opt-level 1, 3 jobs (as it was) | 26.8 min | 4,669 s | 2,393 s | 98 s | 30 s | 41, 1, 16 |
| **Opt-level 0, 3 jobs (adopted)** | 7.2 min | 986 s | 574 s | 300 s | 137 s | 41, 1, 16 |
| Warm in-place shards, opt-level 1 | 17.2 min, and 5.0 min to build first | 2,833 s | none cold | 126 s | 55 s | 41, 1, 16 |
| Warm in-place shards, opt-level 0 | 3.6 min, and 0.4 min to build first | 363 s | none cold | 204 s | 56 s | 42, 0, 16 |

- **Opt-level 0** gave the same outcome for every mutant in under a third of the time. The tests run slower, which
  matters only for a mutant no test catches, since the others stop at the first failure. The dependencies stay at
  opt-level 2.
- **The timeout follows from it.** With `--baseline=skip`, which the gate passes, cargo-mutants 27.1.0 gives a mutant's
  tests 300 seconds whatever `minimum_test_timeout` says, and a mutant that runs out of time counts as caught. At
  opt-level 0 a surviving mutant's tests took 137 seconds here, too near 300 for a slower machine. The gate now times
  its own unmutated run and passes `--timeout`: five times that, cargo-mutants' own default multiple of a baseline,
  and never less than 300 seconds. This rule is Claude's choice.

## Warm in-place shards, measured and left for later

The last two rows build the unmutated tests once, copy the whole tree three times with `cp -a --reflink=auto`, which
keeps file times so that cargo finds each copy already built, and run `cargo mutants --in-place --shard k/3` in each
copy at once. No job builds from nothing. At opt-level 0 that roughly halved the time again.

- Its one differing outcome was false: a surviving mutant counted as caught because two of `occam-cli`'s tests use
  the same scratch directory and one removed it under the other. That is the tests' fault, not the method's, and it
  can happen in any run.
- The risks are a mutated file left in a copy if a run is killed (never in the real tree, as long as only copies
  are mutated and they are removed), a real 1.2 GB or more per copy on a file system without reflinks, and CI's own
  `--shard` having to nest with the copies' shards. It needs more machinery in the gate than its gain over
  opt-level 0 justified now.

## `--copy-target`, measured, which does not work yet

Half of the first run's building was each job's first, cold build. cargo-mutants' `--copy-target true` is meant to
start each copy from an already built `target/`. Tried on the same tree, 2026-11-14:

- With `gitignore = true` in `.cargo/mutants.toml`, as the gate has it, the flag copies nothing: the copies held 5 MB
  and 1,108 files. With that line removed, each copy took the 1.2 GB, 6,593-file `target/` in about a second, by
  reflink on btrfs.
- The copies still built from nothing: 41 to 49 crates compiled for each job's first mutant, the registry's among
  them, and no mutant had finished after four minutes, where the same build in the tree it was copied from took one
  second. The copy does not keep file times (every copied file carried the time of the copy), and cargo judges a
  build fresh by them.

So it removes no cold build with this version. It is worth trying again when cargo-mutants keeps file times on copy.
