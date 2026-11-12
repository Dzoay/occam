//! Source: original
//! The pieces primitives are made of, laid out in a primitive's own coordinates and placed once: points, straight edges
//! and arcs, and curves in a face's parameters. A straight edge's parameter is the distance from its start, and an
//! arc's the angle about its centre, so on a plane, whose parameters are lengths, a straight edge's curve in the face's
//! parameters runs at the same rate.

use std::sync::Arc;

use occam_geom::{Circle, Curve, Line, ParamCurve, UvCurve};
use occam_report::Report;
use occam_units::{Direction, Length, Local, Placement, Point, Vec3, Vector, World};

use super::pose::axes;

/// A primitive's placement into the world.
#[derive(Clone, Copy)]
pub(crate) struct Parts {
    pub pose: Placement<Local, World>,
}

impl Parts {
    /// The point at `v` in the primitive's coordinates.
    pub(crate) fn point(self, v: Vec3) -> Point<World> {
        self.pose.point(Point::from_vec3(v))
    }

    /// The straight edge from `a` to `b`, its parameter the distance from `a`, and its range.
    pub(crate) fn line(self, a: Vec3, b: Vec3) -> (Arc<dyn Curve>, (f64, f64)) {
        let (from, to) = (self.point(a), self.point(b));
        let along = to.to_vec3() - from.to_vec3();
        (Arc::new(Line::new(from, Direction::normalised(along))), (0.0, along.norm()))
    }

    /// The arc of radius `radius` about `frame`'s origin in its `xy` plane, from angle `lo` to `hi` about its `z`.
    ///
    /// # Errors
    ///
    /// None for the arcs primitives make: a radius greater than zero and an angle range of at most one turn.
    pub(crate) fn arc(
        self,
        frame: Placement<Local, Local>,
        radius: f64,
        (lo, hi): (f64, f64),
    ) -> Result<Arc<dyn Curve>, Report> {
        let circle = Circle::placed(&frame.then(&self.pose), Length::from_millimetres(radius))?;
        Ok(Arc::new(circle.trimmed(lo, hi)?))
    }
}

/// The straight line in a face's parameters through `(u, v)` along `(du, dv)`, its parameter the distance along it.
///
/// # Errors
///
/// None: the line lies in the plane of the parameters.
pub(crate) fn uv_line((u, v): (f64, f64), (du, dv): (f64, f64)) -> Result<Arc<dyn ParamCurve>, Report> {
    let line = Line::new(Point::from_vec3(Vec3::new(u, v, 0.0)), Direction::normalised(Vec3::new(du, dv, 0.0)));
    Ok(Arc::new(UvCurve::new(line)?))
}

/// The line in a face's parameters along which the parameter `fixed` is held at `at` and the other runs with the
/// edge's: `(t, at)` if `fixed` is `v`, `(at, t)` if it is `u`.
pub(crate) fn uv_held(fixed: Held, at: f64) -> Result<Arc<dyn ParamCurve>, Report> {
    match fixed {
        Held::V => uv_line((0.0, at), (1.0, 0.0)),
        Held::U => uv_line((at, 0.0), (0.0, 1.0)),
    }
}

/// Which parameter a curve in a face's parameters holds fixed.
#[derive(Clone, Copy)]
pub(crate) enum Held {
    /// `u`, so it runs along `v`.
    U,
    /// `v`, so it runs along `u`.
    V,
}

/// The circle of radius `radius` in a face's parameters about `centre`, whose angle runs from `x` towards `y`, both
/// unit directions in the parameters' plane.
///
/// # Errors
///
/// None for a radius greater than zero.
pub(crate) fn uv_circle(centre: Vec3, x: Vec3, y: Vec3, radius: f64) -> Result<Arc<dyn ParamCurve>, Report> {
    let flat = Vec3::new(centre.x, centre.y, 0.0);
    let (x, y) = (Vec3::new(x.x, x.y, 0.0), Vec3::new(y.x, y.y, 0.0));
    let frame: Placement<Local, World> = axes(flat, x.cross(y) * (1.0 / x.cross(y).norm()), x * (1.0 / x.norm()));
    Ok(Arc::new(UvCurve::new(Circle::placed(&frame, Length::from_millimetres(radius))?)?))
}

/// `v`, in the coordinates `frame` places into the primitive's own.
pub(crate) fn within(frame: Placement<Local, Local>, v: Vec3) -> Vec3 {
    frame.inverse().point(Point::from_vec3(v)).to_vec3()
}

/// The direction `v`, in the coordinates `frame` places into the primitive's own.
pub(crate) fn turned(frame: Placement<Local, Local>, v: Vec3) -> Vec3 {
    frame.inverse().vector(Vector::from_vec3(v)).to_vec3()
}
