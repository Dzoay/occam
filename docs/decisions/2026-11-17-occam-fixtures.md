# occam-fixtures holds what tests, benchmarks and probes build with

**Decided by the user, 2026-11-17,** on item 9 of that day's review backlog. The review found test models, benchmark
workloads and determinism probes inside the kernel crates: compiled into occam-topo and occam-mesh behind the cargo
features `workloads` and `determinism`, exported from occam-math, occam-geom and `occam` as public modules that the
command line and the browser build carried, and copied where a crate could not reach another's.

Two things were asked.

- A name for one unpublished crate that takes them out of the kernel crates, which adds a line to
  [the layers](../PLAN.md#4-layers). The user chose **"occam-fixtures"**, as recommended.
- Whether to move in stages or at once. The user chose **"All at once"**, in one PR, against the recommendation of
  staging.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

The design:

- **Nothing that exists for tests, benchmarks or probes is compiled into a library.** No kernel crate has a
  `workloads` or `determinism` feature, a public `workloads` module, or a module that relaxes the panic lints outside
  `#[cfg(test)]`.
- **`occam-fixtures` holds everything that is built through public APIs:** `mm`, `rad`, `point`, `tol` and `tol_of`;
  the lane and spline workloads from occam-math and occam-geom; the topology's models, test helpers and plates; the
  mesher's layouts and its determinism probe; `occam`'s mesh and operation workloads; and the runner's step.
- **A crate's unit tests cannot take a fixture typed by their own crate from it.** They link a second build of their
  crate through occam-fixtures, whose types are not theirs. So they take from it only what is typed by crates above
  their own. Where they need a fixture of their own crate's types, they compile occam-fixtures' source file into their
  tests with `#[path]`, under `#[cfg(test)]`: one source, built twice, never in a library. occam-geom does this for the
  measured spline, occam-topo for its models and helpers. Such a file names its kernel crate by its name, which the
  crate's tests make resolve with `extern crate self`, and carries no tests of its own, which would run twice.
- **What needs private access stays in its crate under `#[cfg(test)]`.** The topology's determinism probe corrupts
  models through occam-topo's own tables, as no edit could, so it is a test of occam-topo: `gates determinism` runs
  that test as every build and reads the hashes it prints, where it runs the other probes' examples. occam-topo's look
  inside a snapshot and occam-math's exact-value helpers stay too.
- **One step for the runner and the tests.** `occam::step` names the step in an edit, does the work and commits it.
  The runner calls it, and occam-ops' unit tests call it through occam-fixtures, each with its own build's `execute`,
  so a test's model is stepped as the runner steps it. They cannot call the runner whole: it runs the other build of
  occam-ops, not the one under test.

The rules:

- `occam-fixtures` is never published. It sits in the tooling layer of [§4](../PLAN.md#4-layers), which is the home
  of the rule `gates layers` holds: it binds any crate above it, and any crate may name it, in `[dev-dependencies]`
  only. The gate's test shows a dev-dependency allowed and the same dependency refused in `[dependencies]` and
  `[build-dependencies]`.
- A new fixture goes in `occam-fixtures` if it can be built through public APIs, and in its crate's `#[cfg(test)]`
  code only if it cannot.
- The records of `gates determinism` and `gates perf` did not change with the move: the workloads and probes were
  moved, not rewritten.
- A test that lived in a moved file runs in `occam-fixtures`' test binary. The PR lists each.
