//! Source: original
//! Where two surfaces meet, for the boolean's pairs (S4, P5). A [`Section`] holds the curves, each with its image on both
//! surfaces, the contacts, any coincidence, and beside them every failure met on the way and the [`occam_math::Tally`]
//! of the decisions made, so that what failed is never mistaken for what is not there (L5: P5's marching reported none
//! of its 13 wrong answers).
//!
//! [`Methods`] is an ordered list of [`Method`]s. A pair is put in canonical order first, by family, batch key and
//! content, and the section is mapped back, so swapping the two surfaces swaps the section's images and changes nothing
//! else. Each method in turn asks the surfaces what they can present and declines when it does not apply; the first
//! that applies answers, within a [`Budget`]. Running out of it is a failure that keeps what was found. [`sections`]
//! runs many pairs, one a task through [`occam_math::Exec`], one result each in pair order.
//!
//! A [`Tracer`] is the hybrid's building block: from a start point where two surfaces meet, the stations along the
//! curve they meet in, each with its parameters on both, until the curve closes, leaves a surface, ends in a touch, or
//! the trace stalls, which is reported with its stations (L5). It gives points, not curves. Whether it closes, whether
//! the surfaces touch or cross, what an edge is and whether it moved on are decided against the band through
//! occam-math's predicates and counted in its tally; one the band cannot make ends it undecided, reported as
//! `decide.undecided`. Whether a touch is a contact is decided later.
//!
//! The crate works on surfaces only, through occam-geom's frozen seam, and knows nothing of faces or ids. Every decision
//! a method makes goes through occam-math's predicates.

mod closed;
mod correct;
#[cfg(test)]
mod correct_tests;
mod decide;
mod edge;
mod implicit;
mod march;
#[cfg(test)]
mod march_tests;
mod methods;
mod project;
mod section;
#[cfg(test)]
mod tests;
mod trace;
#[cfg(test)]
mod trace_tests;

pub use closed::Closed;
pub use methods::{Answer, Budget, Method, Methods, Spent, SurfacePair, Work, sections};
pub use section::{Coincidence, Contact, Section, SectionCurve};
pub use trace::{Crossing, End, Jump, ROUND, Start, Station, Trace, Tracer, Uncertain, Which};
