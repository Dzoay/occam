//! Source: original
//! What the tests build with.

use std::sync::Arc;

use occam_geom::{Content, Domain, FamilyName, Jet1, Jet2, Line, Parametric, Plane, Scratch, Shape, UvBox};
use occam_math::{Exec, Real};
use occam_report::{Code, Fault, Report, Value};
use occam_units::{Angle, Bounds, Direction, Length, Point, Tangent, Tolerance, UvPoint, Vec3, World};

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

/// A plane on which no point can be found: what a surface whose search fails looks like to the checks.
pub(crate) struct Unfindable(pub(crate) Plane);

impl Parametric for Unfindable {
    fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R> {
        self.0.eval_point(uv)
    }

    fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R> {
        self.0.eval_first(uv)
    }

    fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R> {
        self.0.eval_second(uv)
    }

    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        self.0.derivatives(uv, order, out, scratch)
    }

    fn domain(&self) -> Domain {
        self.0.domain()
    }

    fn bounds(&self, patch: UvBox) -> Bounds {
        self.0.bounds(patch)
    }

    fn invert(&self, _: Point<World>, _: Option<UvPoint>, _: Tolerance, _: &mut Scratch) -> Result<UvPoint, Report> {
        Err(Report::new(Code::NotConverged, Fault::NumericallySensitive))
    }

    fn family(&self) -> FamilyName {
        FamilyName("unfindable")
    }

    fn content(&self, out: &mut Content) {
        self.0.content(out);
        out.push_family(self.family());
    }

    fn shape(&self) -> Shape {
        self.0.shape()
    }
}
