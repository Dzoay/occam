//! Source: original
//! What the tests build with.

use std::sync::Arc;

use occam_geom::Line;
use occam_math::Exec;
use occam_report::{Report, Value};
use occam_units::{Angle, Direction, Length, Point, Tolerance, Vec3, World};

use crate::kind::{Id, Kind};
use crate::records::{CurveRecord, VertexRecord};
use crate::snapshot::Snapshot;

/// The model tolerance the tests work to.
pub(crate) fn tolerance() -> Tolerance {
    Tolerance::new(Length::from_millimetres(1e-7), Angle::from_radians(1e-9)).unwrap()
}

/// The point at these coordinates, in millimetres.
pub(crate) fn at(x: f64, y: f64, z: f64) -> Point<World> {
    Point::new(Length::from_millimetres(x), Length::from_millimetres(y), Length::from_millimetres(z))
}

/// A given vertex on the x axis, with no edges yet.
pub(crate) fn vertex(x: f64) -> VertexRecord {
    VertexRecord { point: at(x, 0.0, 0.0), radius: Length::from_millimetres(0.0), star: None }
}

/// The x axis, as a curve.
pub(crate) fn x_axis() -> CurveRecord {
    CurveRecord(Arc::new(Line::new(at(0.0, 0.0, 0.0), Direction::normalised(Vec3::new(1.0, 0.0, 0.0)))))
}

/// One thread.
pub(crate) const fn serial() -> Exec {
    Exec::serial()
}

/// The live record of `id` in `snapshot`.
pub(crate) fn record<K: Kind>(snapshot: &Snapshot, id: Id<K>) -> Option<&K::Record> {
    K::table(&snapshot.model).live(id.slot(), id.generation())
}

/// The number named `name` in `report`.
pub(crate) fn number(report: &Report, name: &str) -> Option<f64> {
    report.value(name).and_then(|v| match v.value {
        Value::Scalar(x) => Some(x),
        Value::Point(_) => None,
    })
}
