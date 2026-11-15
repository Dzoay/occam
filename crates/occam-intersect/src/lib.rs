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
//! The crate works on surfaces only, through occam-geom's frozen seam, and knows nothing of faces or ids. Every decision
//! a method makes goes through occam-math's predicates.

mod methods;
mod section;
#[cfg(test)]
mod tests;

pub use methods::{Answer, Budget, Method, Methods, Spent, SurfacePair, Work, sections};
pub use section::{Coincidence, Contact, Section, SectionCurve};
