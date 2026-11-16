# The release binary is built for glibc, and the perf gate stays on musl

**Decided by the user, 2026-11-14:** the release binary targets `x86_64-unknown-linux-gnu`, while `gates perf` keeps
counting the instructions of a static `x86_64-unknown-linux-musl` build, so that its counts stay stable. It was one of
three options put after a measurement of what one commit allocates, and the user chose all three: this one; fixes in
the kernel's own code that allocate less; and trying allocator crates in a scratch copy, never committed, before asking
about any of them, one crate at a time.

## What was asked, and what it rested on

`gates perf` and CI's perf job build for static musl, so that no machine's libc changes the counts. No release target
was set: `cargo build --release -p occam-cli` gives the host's glibc build. The question was which the shipped binary
should be, after the topology's commit was measured on 13 November, on the topology milestone's branch shortly before
it merged (#49):

- **musl's allocator was over half of a commit's instructions.** Of one split's commit on a plate of 1,056 faces, the
  allocator was 1,002,721 of 1,853,157 instructions (54%), and 1,152,080 of 2,016,300 (57%) on 100,350 faces. It was
  92% of the growth between the two.
- **glibc's allocator is faster.** That commit's wall time, in microseconds, on musl and on glibc: 373–432 and 217–246
  at 1,056 faces; 453–490 and 263–284 at 100,350; 717–756 and 470–509 at 999,696. A hundred scattered edits at 999,696
  faces: 930–988 and 431–456. On both, time grows with the model while instructions barely do, so that growth is
  memory.
- **glibc could not be counted.** Valgrind stops with an illegal instruction in the measuring machine's glibc, so
  instruction counts there are not available to a gate.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- When the `release` milestone of [the first release](../plan/first-release.md) builds the binary that ships, it builds
  it for `x86_64-unknown-linux-gnu`, linked against the system's glibc. Other platforms are not decided here.
- `gates perf` is unchanged: instruction counts of a static musl build, against `ci/perf-baseline.txt`, whose header
  says how it was recorded. Its counts include musl's allocator, which costs more than glibc's, so the gate errs on the
  side of seeing an allocation the shipped binary would pay less for.
- Allocating less helps both builds. The kernel's own fixes came with this decision, in the same pull request, and
  were measured on the gate's musl build.
- An allocator crate is a new dependency, and none is added by this decision. Each is measured in a scratch copy on
  both libcs with the same workloads, and then put to the user with its numbers, one crate at a time.
