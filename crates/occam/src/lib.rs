//! Source: original
//! The composition crate: the one API every facade binds (L25). The command line and the browser build both reach the kernel
//! through here, so an operation is added once.

mod describe;
mod distance;
mod model;
pub mod operation_workloads;
mod run;
mod session;

pub use describe::describe;
pub use distance::{Answer, DistanceQuery, distance, distance_json};
pub use model::Model;
pub use occam_geom::workloads as spline_workloads;
pub use occam_geom::{Jets, SplineSurface};
pub use occam_math::{Exec, Lanes, Real, width, workloads};
pub use occam_ops::{Operation, guide, listing};
pub use occam_report::{
    Code, Description, DisplayUnits, EntityKind, EntityRef, Fault, Input, NamedValue, Origin, Outcome, Remedy,
    RemedyKind, Report,
};
pub use occam_units::{Angle, Length, Point, Tolerance, UvPoint, World};
pub use run::{Done, Failed, Ran, ToleranceFrom, ran_json, run};
pub use session::{Answered, Encoding, Listed, PerId, Session, ran_words};
