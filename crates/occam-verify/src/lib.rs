//! Source: original
//! The Rust side of the oracle (L11, L12): finds the harness's result files and compares occam's measurements with them. The
//! file formats are defined in `tools/oracle/README.md`.

mod compare;
mod locate;
mod reading;

pub use compare::{Bounds, Closeness, Disagreement, Measure, Measured, Tolerances, compare};
pub use locate::{ARRIS, OPENCASCADE, Pin, default_root};
pub use reading::{NoResult, Reading, Verdict, judge, load, read};
