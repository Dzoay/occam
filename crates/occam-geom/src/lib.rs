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

/// This crate by its name, for the fixtures its tests compile from occam-fixtures' source.
#[cfg(test)]
extern crate self as occam_geom;

mod analytic;
mod arc;
mod axes;
mod basis;
mod batch;
mod bspline;
mod cells;
mod chart;
mod circle;
mod cone;
#[cfg(test)]
mod conformance;
mod continuity;
mod curve;
mod cylinder;
mod domain;
mod ellipse;
#[cfg(test)]
mod every_family;
mod exact;
mod extrusion;
mod facet;
mod field;
mod fitted_curve;
mod foot;
mod held;
mod hyperbola;
mod identity;
mod image;
mod interpolate;
mod join;
mod knots;
mod lanewise;
mod laps;
mod line;
mod local;
mod meridian;
mod mesh;
mod nearest;
mod normal;
mod offset_curve;
mod offset_surface;
mod parabola;
mod param_curve;
mod plane;
mod quartic;
mod restore;
#[cfg(test)]
mod restore_families;
mod revolution;
mod series;
#[cfg(test)]
mod source_check;
mod spacing;
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
mod uv_curve;
/// The spline `gates perf` measures, which the families' tests are run on too: occam-fixtures' source, compiled into
/// the tests alone, where it names this crate as any caller does.
#[cfg(test)]
#[path = "../../occam-fixtures/src/spline_workloads.rs"]
#[allow(dead_code, reason = "the tests use the spline, not what is measured on it")]
mod workloads;

pub use analytic::Analytic;
pub use arc::{length, rule as gauss_legendre};
pub use batch::{Jet1, Jet2, Jets, Order, Scratch, eval_in_lanes};
pub use bspline::{BsplineCurve, BsplineSurface};
pub use chart::Chart;
pub use circle::Circle;
pub use cone::Cone;
pub use continuity::{Continuity, Smoothness, curve_join, require_curve_join, require_surface_join, surface_join};
pub use curve::{Curve, ParametricCurve};
pub use cylinder::Cylinder;
pub use domain::{Degeneracy, Domain, Param, ParamMap, ParamRange, UvBox, UvMap, Wrap};
pub use ellipse::Ellipse;
pub use extrusion::Extrusion;
pub use fitted_curve::FittedCurve;
pub use hyperbola::Hyperbola;
pub use identity::{BatchKey, Content, FamilyName, Shape};
pub use image::Image;
pub use interpolate::interpolate;
pub use join::join_edge;
pub use line::Line;
pub use local::{CurveLocal, SurfaceLocal, curve_local, surface_local};
pub use mesh::MeshSurface;
pub use offset_curve::OffsetCurve;
pub use offset_surface::OffsetSurface;
pub use parabola::Parabola;
pub use param_curve::{ParamCurve, ParametricParamCurve, UvClosest};
pub use plane::Plane;
pub use quartic::Quartic;
pub use revolution::Revolution;
pub use spacing::{by_count, by_deflection, by_spacing};
pub use sphere::Sphere;
pub use spline_curve::SplineCurve;
pub use spline_surface::SplineSurface;
pub use surface::{Closest, Parametric, Surface, TriangleView};
pub use torus::Torus;
pub use trimmed_curve::TrimmedCurve;
pub use trimmed_surface::TrimmedSurface;
pub use uv_curve::UvCurve;
