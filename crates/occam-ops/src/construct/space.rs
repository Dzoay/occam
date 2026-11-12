//! Source: original
//! The geometry of wires in space: placements that face a given way, the circle through three points, the line or
//! circle an edge lies on read back from its curve, and the plane a face's outer wire lies in, with each edge's image in
//! that plane's parameters. Every placement is built from rotations through the owned `sin_cos` and `atan2`, so it is the
//! same on every target.

use core::f64::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;

use occam_geom::{Circle, Curve, Line, UvCurve};
use occam_math::atan2;
use occam_report::Report;
use occam_topo::ParamCurveRecord;
use occam_units::{Angle, Direction, Length, Local, Placement, Point, Vec3, Vector, World};

use super::flat::Flat;

/// The unit vector along `v`, which is not zero.
pub(crate) fn unit(v: Vec3) -> Vec3 {
    v * (1.0 / v.norm())
}

/// The turn about `axis` (which need not be a unit vector) by `angle` radians.
fn turn(axis: Vec3, angle: f64) -> Placement<Local, Local> {
    Placement::rotation(Direction::<Local>::normalised(axis), Angle::from_radians(angle))
}

/// The shortest turn that takes the z axis to `normal`, a unit vector; half a turn about x if `normal` is minus z.
fn lift(normal: Vec3) -> Placement<Local, Local> {
    let z = Vec3::new(0.0, 0.0, 1.0);
    let axis = z.cross(normal);
    let (s, c) = (axis.norm(), z.dot(normal));
    if s > 1e-12 {
        turn(axis, atan2(s, c))
    } else if c > 0.0 {
        Placement::identity()
    } else {
        turn(Vec3::new(1.0, 0.0, 0.0), PI)
    }
}

/// The placement at `origin` whose z axis is `normal` and whose x axis is `x`, both unit and square to each other, or,
/// with no `x`, wherever the shortest turn from the z axis to `normal` takes the x axis.
pub(crate) fn frame(origin: Vec3, normal: Vec3, x: Option<Vec3>) -> Placement<Local, World> {
    let first = lift(normal);
    let spun = match x {
        Some(x) => {
            let carried = first.vector(Vector::from_vec3(Vec3::new(1.0, 0.0, 0.0))).to_vec3();
            first.then(&turn(normal, atan2(carried.cross(x).dot(normal), carried.dot(x))))
        }
        None => first,
    };
    spun.then(&Placement::identity()).then(&Placement::<World, World>::translation(Vector::from_vec3(origin)))
}

/// What an arc's three points say: the circle through them, or how far the middle one is from the line through the
/// others if they are in line.
pub(crate) struct Through {
    /// The centre.
    pub centre: Vec3,
    /// The unit normal about which the arc runs anticlockwise from its first point through its second to its third.
    pub normal: Vec3,
    /// The radius.
    pub radius: f64,
    /// The angle the arc turns through, above 0 and below a whole turn.
    pub sweep: f64,
}

/// The circle through `a`, `m` and `b`, or how far `m` is from the line through `a` and `b` if that is no more than
/// `band`, or `a` and `b` are no further apart than it.
#[expect(clippy::many_single_char_names, reason = "the three points and the triangle's sides, as geometry names them")]
pub(crate) fn through(a: Vec3, m: Vec3, b: Vec3, band: f64) -> Result<Through, f64> {
    let (u, v) = (m - a, b - a);
    let w = u.cross(v);
    let (chord, normal_length) = (v.norm(), w.norm());
    let off = if chord > 0.0 { normal_length / chord } else { (m - a).norm() };
    if chord <= band || off <= band {
        return Err(off);
    }
    let centre = a + (v.cross(w) * u.dot(u) + w.cross(u) * v.dot(v)) * (1.0 / (2.0 * w.dot(w)));
    let normal = unit(w);
    let (from, to) = (a - centre, b - centre);
    let angle = atan2(from.cross(to).dot(normal), from.dot(to));
    Ok(Through { centre, normal, radius: from.norm(), sweep: if angle > 0.0 { angle } else { angle + TAU } })
}

/// The line or circle an edge lies on, as its curve says when evaluated.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Bent {
    /// The line through `origin` along the unit `along`, its parameter the distance from `origin`.
    Line { origin: Vec3, along: Vec3 },
    /// The circle about `centre` of `radius`, at the angle `t` at `centre + radius (cos t x + sin t y)`.
    Circle { centre: Vec3, x: Vec3, y: Vec3, radius: f64 },
}

/// A point of `curve`, as a vector.
fn at(curve: &dyn Curve, t: f64) -> Vec3 {
    curve.point(t).to_vec3()
}

impl Bent {
    /// Its point at `t`.
    pub(crate) fn point(self, t: f64) -> Vec3 {
        match self {
            Self::Line { origin, along } => origin + along * t,
            Self::Circle { centre, x, y, radius } => {
                let (sin, cos) = occam_math::sin_cos(t);
                centre + (x * cos + y * sin) * radius
            }
        }
    }

    /// The line or circle `curve` is, if it is one: read from its points, so the same parameter gives the same point.
    pub(crate) fn of(curve: &dyn Curve) -> Option<Self> {
        match curve.shape().form {
            "line" => Some(Self::Line { origin: at(curve, 0.0), along: at(curve, 1.0) - at(curve, 0.0) }),
            "circle" => {
                let centre = (at(curve, 0.0) + at(curve, PI)) * 0.5;
                let (x, y) = (at(curve, 0.0) - centre, at(curve, FRAC_PI_2) - centre);
                let radius = x.norm();
                Some(Self::Circle { centre, x: x * (1.0 / radius), y: y * (1.0 / radius), radius })
            }
            _ => None,
        }
    }
}

/// A plane a face lies in: its placement, whose x and y are the face's parameters, in millimetres.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Flatland {
    /// From the plane's parameters, as x and y with z 0, to the world.
    pub placement: Placement<Local, World>,
    /// Its unit normal.
    pub normal: Vec3,
    /// Where its parameters are zero.
    origin: Vec3,
}

impl Flatland {
    /// The plane at `origin` facing along the unit `normal`.
    pub(crate) fn new(origin: Vec3, normal: Vec3) -> Self {
        Self { placement: frame(origin, normal, None), normal, origin }
    }

    /// The same plane facing the other way.
    pub(crate) fn flipped(&self) -> Self {
        Self::new(self.origin, self.normal * -1.0)
    }

    /// The parameters of `p`, and how far `p` is from the plane.
    pub(crate) fn uv(&self, p: Vec3) -> ([f64; 2], f64) {
        let local = self.placement.inverse().point(Point::from_vec3(p)).to_vec3();
        ([local.x, local.y], local.z)
    }

    /// The point at the parameters `uv`.
    pub(crate) fn point(&self, [u, v]: [f64; 2]) -> Vec3 {
        self.placement.point(Point::<Local>::from_vec3(Vec3::new(u, v, 0.0))).to_vec3()
    }

    /// The image of `v` in the plane's parameters.
    fn along(&self, v: Vec3) -> [f64; 2] {
        let local = self.placement.inverse().vector(Vector::from_vec3(v)).to_vec3();
        [local.x, local.y]
    }

    /// The piece `bent` makes over `range`, in the plane's parameters.
    pub(crate) fn flat(&self, bent: Bent, (lo, hi): (f64, f64)) -> Flat {
        match bent {
            Bent::Line { origin, along } => {
                Flat::Seg { a: self.uv(origin + along * lo).0, b: self.uv(origin + along * hi).0 }
            }
            Bent::Circle { centre, x, y, radius } => {
                let ([u, v], sign) = (self.along(x), x.cross(y).dot(self.normal).signum());
                let from = atan2(v, u) + sign * lo;
                Flat::Arc { c: self.uv(centre).0, r: radius, from, sweep: sign * (hi - lo) }
            }
        }
    }

    /// The curve in the plane's parameters that runs along `bent` with the same parameter.
    ///
    /// # Errors
    ///
    /// What the families report, which a line or circle in the plane z = 0 does not.
    pub(crate) fn pcurve(&self, bent: Bent) -> Result<ParamCurveRecord, Report> {
        let flat = |c: [f64; 2]| Point::from_vec3(Vec3::new(c[0], c[1], 0.0));
        match bent {
            Bent::Line { origin, along } => {
                let [u, v] = self.along(along);
                let line = Line::new(flat(self.uv(origin).0), Direction::normalised(Vec3::new(u, v, 0.0)));
                Ok(ParamCurveRecord(Arc::new(UvCurve::new(line)?)))
            }
            Bent::Circle { centre, x, y, radius } => {
                let [u, v] = self.along(x);
                let flip = if x.cross(y).dot(self.normal) < 0.0 { PI } else { 0.0 };
                let spun = turn(Vec3::new(1.0, 0.0, 0.0), flip).then(&turn(Vec3::new(0.0, 0.0, 1.0), atan2(v, u)));
                let [cu, cv] = self.uv(centre).0;
                let moved = Placement::<World, World>::translation(Vector::from_vec3(Vec3::new(cu, cv, 0.0)));
                let placement = spun.then(&Placement::identity()).then(&moved);
                let circle = Circle::placed(&placement, Length::from_millimetres(radius))?;
                Ok(ParamCurveRecord(Arc::new(UvCurve::new(circle)?)))
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use occam_geom::Circle as GeomCircle;
    use occam_units::Point;

    use super::*;

    fn close(a: Vec3, b: Vec3) -> bool {
        (a - b).norm() < 1e-12
    }

    #[test]
    fn a_frame_faces_its_normal_with_its_x_where_asked_or_by_the_shortest_turn() {
        let normal = unit(Vec3::new(1.0, 2.0, 2.0));
        let x = unit(Vec3::new(2.0, -1.0, 0.0));
        let placed = frame(Vec3::new(5.0, 0.0, 0.0), normal, Some(x));
        let axis = |v: Vec3| placed.vector(Vector::from_vec3(v)).to_vec3();
        assert!(close(axis(Vec3::new(0.0, 0.0, 1.0)), normal) && close(axis(Vec3::new(1.0, 0.0, 0.0)), x));
        assert!(close(placed.point(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0))).to_vec3(), Vec3::new(5.0, 0.0, 0.0)));
        let down = frame(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0), None);
        assert!(close(down.vector(Vector::from_vec3(Vec3::new(0.0, 0.0, 1.0))).to_vec3(), Vec3::new(0.0, 0.0, -1.0)));
        let up = frame(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0), None);
        assert!(close(up.vector(Vector::from_vec3(Vec3::new(1.0, 0.0, 0.0))).to_vec3(), Vec3::new(1.0, 0.0, 0.0)));
    }

    #[test]
    fn the_circle_through_three_points_runs_from_the_first_through_the_second() {
        let arc = through(Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(-1.0, 0.0, 0.0), 1e-7).unwrap();
        assert!(close(arc.centre, Vec3::new(0.0, 0.0, 0.0)) && close(arc.normal, Vec3::new(0.0, 0.0, 1.0)));
        assert!((arc.radius - 1.0).abs() < 1e-12 && (arc.sweep - PI).abs() < 1e-12);
        let long =
            through(Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, -1.0, 0.0), Vec3::new(0.0, 1.0, 0.0), 1e-7).unwrap();
        assert!(close(long.normal, Vec3::new(0.0, 0.0, -1.0)) && (long.sweep - 3.0 * FRAC_PI_2).abs() < 1e-12);
        let flat = through(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1e-9, 0.0), Vec3::new(2.0, 0.0, 0.0), 1e-7);
        assert!(flat.is_err_and(|off| (off - 1e-9).abs() < 1e-15));
        assert!(through(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 0.0), 1e-7).is_err());
    }

    #[test]
    fn a_curve_read_back_and_its_image_in_a_plane_agree_at_every_parameter() {
        let plane = Flatland::new(Vec3::new(1.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0));
        let line =
            Line::new(Point::from_vec3(Vec3::new(2.0, 3.0, 0.0)), Direction::normalised(Vec3::new(3.0, 4.0, 0.0)));
        let turned = frame(Vec3::new(4.0, 2.0, 0.0), Vec3::new(0.0, 0.0, -1.0), Some(unit(Vec3::new(1.0, 1.0, 0.0))));
        let circle = GeomCircle::placed(&turned, Length::from_millimetres(2.0)).unwrap();
        for curve in [&line as &dyn Curve, &circle] {
            let bent = Bent::of(curve).unwrap();
            let pcurve = plane.pcurve(bent).unwrap();
            for t in [0.0, 0.7, 2.5, 4.0] {
                assert!(close(bent.point(t), curve.point(t).to_vec3()), "{t}");
                let ([u, v], height) = plane.uv(curve.point(t).to_vec3());
                let image = pcurve.0.uv(t);
                assert!((image.u() - u).abs() < 1e-12 && (image.v() - v).abs() < 1e-12 && height.abs() < 1e-12, "{t}");
                assert!(close(plane.point([u, v]), curve.point(t).to_vec3()));
            }
            let flat = plane.flat(bent, (0.7, 2.5));
            assert!((flat.start()[0] - plane.uv(curve.point(0.7).to_vec3()).0[0]).abs() < 1e-12);
            assert!((flat.end()[1] - plane.uv(curve.point(2.5).to_vec3()).0[1]).abs() < 1e-12);
        }
        assert!(close(plane.flipped().normal, Vec3::new(0.0, 0.0, -1.0)));
    }
}
