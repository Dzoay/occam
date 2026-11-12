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
mod axes;
mod basis;
mod batch;
mod bspline;
mod cells;
mod circle;
mod cone;
#[cfg(test)]
mod conformance;
mod curve;
mod cylinder;
mod domain;
mod ellipse;
mod exact;
mod extrusion;
mod facet;
mod foot;
mod held;
mod hyperbola;
mod identity;
mod knots;
mod lanewise;
mod line;
mod meridian;
mod mesh;
mod nearest;
mod normal;
mod offset_curve;
mod offset_surface;
mod parabola;
mod plane;
mod revolution;
#[cfg(test)]
mod source_check;
mod sphere;
mod spline;
mod spline_curve;
mod spline_surface;
mod support;
mod surface;
mod swept;
mod torus;
mod trimmed_curve;
mod trimmed_surface;
mod turn;
pub mod workloads;

pub use analytic::Analytic;
pub use batch::{Jet1, Jet2, Jets, Order, Scratch, eval_in_lanes};
pub use bspline::{BsplineCurve, BsplineSurface};
pub use circle::Circle;
pub use cone::Cone;
pub use curve::{Curve, ParamCurve, ParametricCurve};
pub use cylinder::Cylinder;
pub use domain::{Degeneracy, Domain, Param, ParamRange, UvBox, Wrap};
pub use ellipse::Ellipse;
pub use extrusion::Extrusion;
pub use hyperbola::Hyperbola;
pub use identity::{BatchKey, Content, FamilyName, Shape};
pub use line::Line;
pub use mesh::MeshSurface;
pub use offset_curve::OffsetCurve;
pub use offset_surface::OffsetSurface;
pub use parabola::Parabola;
pub use plane::Plane;
pub use revolution::Revolution;
pub use sphere::Sphere;
pub use spline_curve::SplineCurve;
pub use spline_surface::SplineSurface;
pub use surface::{Closest, Parametric, Surface, TriangleView};
pub use torus::Torus;
pub use trimmed_curve::TrimmedCurve;
pub use trimmed_surface::TrimmedSurface;
