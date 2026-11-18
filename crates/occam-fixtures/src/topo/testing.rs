//! Source: original
//! What tests of the topology build with beside the models: a vertex, a line, a surface no point can be found on, and
//! faces turned over. occam-topo's unit tests compile this file too.

use std::sync::Arc;

use occam_fixtures::point;
use occam_geom::{Content, Domain, FamilyName, Jet1, Jet2, Line, Parametric, Plane, Scratch, Shape, UvBox};
use occam_math::Real;
use occam_report::{Code, Fault, Report, Value};
use occam_topo::{CurveRecord, EdgeUseRecord, Edit, Face, FaceRecord, Id, Loop, VertexRecord};
use occam_units::{Bounds, Direction, Length, Point, Tangent, Tolerance, UvPoint, Vec3, World};

/// A given vertex on the x axis, with no edges yet.
#[must_use]
pub fn vertex(x: f64) -> VertexRecord {
    VertexRecord { point: point(x, 0.0, 0.0), radius: Length::from_millimetres(0.0), star: None }
}

/// The x axis, as a curve.
#[must_use]
pub fn x_axis() -> CurveRecord {
    CurveRecord(Arc::new(Line::new(point(0.0, 0.0, 0.0), Direction::normalised(Vec3::new(1.0, 0.0, 0.0)))))
}

/// The number named `name` in `report`.
#[must_use]
pub fn number(report: &Report, name: &str) -> Option<f64> {
    report.value(name).and_then(|v| match v.value {
        Value::Scalar(x) => Some(x),
        Value::Point(_) => None,
    })
}

/// A plane on which no point can be found: what a surface whose search fails looks like to the checks.
pub struct Unfindable(pub Plane);

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

/// Turns loop `l` round in `edit`: each use runs the other way, and they come round in the other order.
pub fn turn_loop(edit: &mut Edit, l: Id<Loop>) {
    let uses = edit.view(l).unwrap().uses();
    let mut befores = uses.clone();
    befores.rotate_right(1);
    for (u, before) in uses.into_iter().zip(befores) {
        let record = edit.record(u).unwrap();
        edit.set(u, EdgeUseRecord { next: Some(before), reversed: !record.reversed, ..record }).unwrap();
    }
}

/// Turns `face` over in `edit`, its outward normal reversed.
pub fn turn_over(edit: &mut Edit, face: Id<Face>) {
    let record = edit.record(face).unwrap();
    edit.set(face, FaceRecord { reversed: !record.reversed, ..record }).unwrap();
}

/// Turns `face` inside out in `edit`: over, and each of its loops round, so they still agree.
pub fn turn_inside_out(edit: &mut Edit, face: Id<Face>) {
    for l in edit.view(face).unwrap().loops() {
        turn_loop(edit, l);
    }
    turn_over(edit, face);
}
