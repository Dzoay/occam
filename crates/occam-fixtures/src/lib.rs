//! Source: original
//! What occam's tests, benchmarks and determinism probes build with, so that none of it is compiled into a crate that
//! ships (docs/decisions/2026-11-17-occam-fixtures.md): the helpers every test wrote for itself, the models, the
//! workloads `gates perf` counts, what `gates determinism` hashes, and the runner's own [`step`], for tests that step a
//! model by hand. Never published; a crate names it in
//! `[dev-dependencies]` only, which `gates layers` holds.
//!
//! Everything here is built through the kernel crates' public APIs. A kernel crate's unit tests link a second build of
//! their own crate through this one, whose types are not theirs, so they may take from here only what is typed by the
//! crates above their own, as [`mm`] is. Where they need a model of their own crate's types, they compile the module
//! that builds it into their tests by `#[path]`: these quantities in occam-units, [`spline_workloads`] in occam-geom,
//! [`topo`]'s `models` and `testing` in occam-topo. Such a module names its kernel crate by its name, never `crate`,
//! its neighbours by `super`, and this crate by its name, and carries no tests of its own, which would run twice.
#![allow(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::panic,
    reason = "fixtures fail loudly, as the tests they were written in do: nothing here ships"
)]
#![allow(
    clippy::missing_panics_doc,
    clippy::must_use_candidate,
    reason = "a fixture that cannot be built panics, each of them, and a test has no use for one but to use it"
)]
#![allow(
    clippy::inline_always,
    reason = "a measured kernel compiles at a dispatched instruction level only through what is inlined into it (P4)"
)]

extern crate self as occam_fixtures;

pub mod lane_workloads;
pub mod mesh;
pub mod mesh_workloads;
pub mod operation_workloads;
pub mod spline_workloads;
#[cfg(test)]
mod spline_workloads_tests;
pub mod topo;
mod units;

pub use occam::step;
pub use units::{mm, point, rad, tol, tol_of};
