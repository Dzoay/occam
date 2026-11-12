//! Source: original
//! The composition crate: the one API every facade binds (L25). The command line and the browser build both reach the kernel
//! through here, so an operation is added once.

mod distance;
mod model;

pub use distance::{Answer, DistanceQuery, distance, distance_json};
pub use model::Model;
pub use occam_geom::workloads as spline_workloads;
pub use occam_geom::{Jets, SplineSurface};
pub use occam_math::{Lanes, Real, width, workloads};
pub use occam_report::{Code, DisplayUnits, Fault, NamedValue, Outcome, Remedy, RemedyKind, Report};
pub use occam_units::{Angle, Length, Point, Tolerance, UvPoint, World};
