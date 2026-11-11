//! Source: original
//! Curves and surfaces behind one seam in two layers (S4, L21).
//!
//! A family implements [`Parametric`] or [`ParametricCurve`], generic over [`occam_math::Real`], so that one formula runs
//! as `f64` or in lanes. Everything else calls [`Surface`] or [`Curve`], which are dyn-compatible and joined to the family
//! traits by one blanket impl each. That impl is also where a call is checked: a parameter outside the domain, or a point
//! that is not finite, is refused there, so no family repeats it.
//!
//! All stored geometry is in `World`, so the traits carry no frame. Anything that depends only on a curve or surface is
//! computed when it is made, never inside an evaluation (S4's cone). A family is named only in its own module, and
//! nothing recovers a concrete type from the seam: what a surface can present, it is asked for.

#![expect(
    clippy::inline_always,
    reason = "a kernel compiles at a dispatched instruction level only through what is inlined into it (P4)"
)]

mod analytic;
mod batch;
#[cfg(test)]
mod conformance;
mod curve;
mod domain;
mod identity;
mod line;
mod plane;
#[cfg(test)]
mod source_check;
mod support;
mod surface;

pub use analytic::Analytic;
pub use batch::{Jet1, Jet2, Jets, Order, Scratch, eval_in_lanes};
pub use curve::{Curve, ParamCurve, ParametricCurve};
pub use domain::{Degeneracy, Domain, Param, ParamRange, UvBox, Wrap};
pub use identity::{BatchKey, Content, FamilyName, Shape};
pub use line::Line;
pub use plane::Plane;
pub use surface::{BsplineSurface, Closest, Parametric, Surface, TriangleView};
