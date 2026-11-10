//! The compiled type sketch of occam's seams, for `docs/ARCHITECTURE.md` (L50).
//!
//! Each module stands for one crate of `docs/PLAN.md` section 4, in its order, and uses only the modules above it, as
//! the crates will. A module for an existing crate holds only what that crate gains or changes, and says which. Bodies
//! are `unimplemented!` unless a body shows something: this proves that the types and signatures fit together and fit
//! the real `occam-report`, `occam-math` and `occam-units`, not that anything works. Nothing here is copied into a crate;
//! each part is rewritten test-first when its milestone comes.
#![forbid(unsafe_code)]
#![allow(dead_code, unused_variables, clippy::unimplemented, missing_docs)]

pub mod report;

pub mod math;

pub mod units;

pub mod geom;

pub mod intersect;

pub mod topo;

pub mod mesh;

pub mod ops;

pub mod io;

pub mod compose;
