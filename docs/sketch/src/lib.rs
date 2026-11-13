//! The compiled type sketch of occam's seams, for `docs/ARCHITECTURE.md` (L50).
//!
//! Each module stands for one crate of `docs/PLAN.md` section 4, in its order, and uses only the modules above it, as
//! the crates will. A module for an existing crate holds only what that crate gains or changes, and says which. Bodies
//! are `unimplemented!` unless a body shows something: this proves that the types and signatures fit together and fit
//! the real `occam-report`, `occam-math`, `occam-units`, `occam-geom` and `occam-topo`, not that anything works. Nothing
//! here is copied into a crate; each part is rewritten test-first when its milestone comes. `geom.rs` and `units.rs` went
//! with `geometry.freeze`, when occam-geom's seam was frozen and recorded in `ci/seam.txt`, and `topo.rs` with
//! `topology.scale`, when occam-topo held all of it but the describer.
#![forbid(unsafe_code)]
#![allow(dead_code, unused_variables, clippy::unimplemented, missing_docs)]

pub mod report;

pub mod intersect;

pub mod mesh;

pub mod ops;

pub mod io;

pub mod compose;
