//! Source: rump-1999
//! How far a point lies from an analytic surface, signed, as an [`Interval`] over every point of a box: the implicit
//! function a curve's hits are isolated by. Each family's is its true distance, not a multiple of it, so that
//! `|d| <= band` is the band's own test (S2): a plane's offset along its normal; a sphere's, a cylinder's and a torus's
//! distance from their centre, axis or tube's circle, less the radius; and a double cone's offset across the nappe
//! nearer the point. Positive is the side a plane faces, and outside the others.
//!
//! The box's coordinates become intervals rounded outward, and the arithmetic is Rump's midpoint and radius, so the
//! interval holds the distance of every point in the box. Decisions on it go through occam-math's `decide_computed`.

use std::f64::consts::PI;
use std::sync::Arc;

use occam_geom::{Analytic, Curve, ParamMap};
use occam_math::{Interval, Predicate, Ring, Tally, Verdict, atan2, decide_computed, sin_cos};
use occam_units::{Affine, Angle, Bounds, Direction, Length, Placement, Point, Vec3, Vector, World};

/// How much of a coordinate an evaluated point may be off by: occam-geom's padding of a box for rounding, 64 machine
/// epsilons of its largest coordinate for each of four sums.
const EVALUATED: f64 = 256.0 * f64::EPSILON;

/// How much of each component a unit direction may be off its exact normalised value by.
const NORMALISED: f64 = 2.0 * f64::EPSILON;

/// Every value from `lo` to `hi`: the middle, and a radius rounded up past both ends. NaN, which decides nothing, where
/// either end is infinite.
fn between(lo: f64, hi: f64) -> Interval {
    let mid = lo * 0.5 + hi * 0.5;
    let (below, above) = ((mid - lo).next_up(), (hi - mid).next_up());
    Interval::new(mid, if above < below { below } else { above })
}

/// A stored number: exact.
fn exact(x: f64) -> Interval {
    Interval::exact(x)
}

/// A component of a unit direction, within [`NORMALISED`] of itself.
fn unit(x: f64) -> Interval {
    Interval::new(x, x.abs() * NORMALISED)
}

/// Every point of a box, a coordinate at a time.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Spread {
    x: Interval,
    y: Interval,
    z: Interval,
}

impl Spread {
    /// Every point of `bounds`.
    pub(crate) fn of(bounds: &Bounds) -> Self {
        let (low, high) = (bounds.low().to_vec3(), bounds.high().to_vec3());
        Self { x: between(low.x, high.x), y: between(low.y, high.y), z: between(low.z, high.z) }
    }

    /// Every point of the box an evaluated point is known to within ([`around`]).
    pub(crate) fn at(point: Point<World>) -> Self {
        Self::of(&around(point))
    }

    /// Each point less `origin`.
    fn from(self, origin: Point<World>) -> Self {
        let o = origin.to_vec3();
        Self { x: self.x - exact(o.x), y: self.y - exact(o.y), z: self.z - exact(o.z) }
    }

    /// The difference of a point of one and a point of the other.
    pub(crate) fn less(self, other: Self) -> Self {
        Self { x: self.x - other.x, y: self.y - other.y, z: self.z - other.z }
    }

    /// The component along the unit `axis`.
    fn along(self, axis: Vec3) -> Interval {
        self.x * unit(axis.x) + self.y * unit(axis.y) + self.z * unit(axis.z)
    }

    /// The squared length of the part across the unit `axis`: of the cross product with it.
    fn across(self, axis: Vec3) -> Interval {
        let (ax, ay, az) = (unit(axis.x), unit(axis.y), unit(axis.z));
        let (cx, cy, cz) = (self.y * az - self.z * ay, self.z * ax - self.x * az, self.x * ay - self.y * ax);
        cx * cx + cy * cy + cz * cz
    }

    /// The length.
    pub(crate) fn length(self) -> Interval {
        (self.x * self.x + self.y * self.y + self.z * self.z).sqrt()
    }
}

/// The box an evaluated point is known to within: [`EVALUATED`] of its largest coordinate about it. The point's own
/// rounding is not certified by the seam, so this is the padding occam-geom's own boxes take for it.
pub(crate) fn around(point: Point<World>) -> Bounds {
    let p = point.to_vec3();
    let size = [p.x, p.y, p.z].into_iter().fold(0.0, |a: f64, x| if x.abs() > a { x.abs() } else { a });
    Bounds::around(point, Length::from_millimetres(EVALUATED * size))
}

/// `bounds` grown by what an evaluated point of it may be off by ([`around`]), for a curve that was moved, whose own
/// box does not know the move's rounding.
pub(crate) fn padded(bounds: &Bounds) -> Bounds {
    around(bounds.low()).union(&around(bounds.high()))
}

/// `curve` moved rigidly into the frame of `form`, where the form's point is the origin and its axis or normal is Z,
/// and the form there, whose numbers are exact. `None` where the curve's family does not keep its form, or its
/// parameter, under the move. A box of the moved curve, [`padded`], holds the distance its original has: the move's
/// rounding is a few ulps of the coordinates, which the padding is many times.
pub(crate) fn canonical(curve: &dyn Curve, form: &Analytic) -> Option<(Arc<dyn Curve>, Analytic)> {
    let z = Vec3::new(0.0, 0.0, 1.0);
    let (origin, axis) = match *form {
        Analytic::Plane { origin, normal } => (origin, normal.get()),
        Analytic::Cylinder { on_axis, axis, .. } => (on_axis, axis.get()),
        Analytic::Cone { apex, axis, .. } | Analytic::Torus { centre: apex, axis, .. } => (apex, axis.get()),
        Analytic::Sphere { centre, .. } => (centre, z),
    };
    let across = axis.cross(z);
    let (off, along) = (across.norm(), axis.dot(z));
    let spin = if off > 0.0 {
        Placement::rotation(Direction::normalised(across), Angle::from_radians(atan2(off, along)))
    } else if along > 0.0 {
        Placement::identity()
    } else {
        Placement::rotation(Direction::normalised(Vec3::new(1.0, 0.0, 0.0)), Angle::from_radians(PI))
    };
    let shift = Placement::translation(Vector::from_vec3(Vec3::new(0.0, 0.0, 0.0) - origin.to_vec3()));
    let (moved, params) = curve.moved(&Affine::placed(shift.then(&spin)))?;
    let (at, up) = (Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)), Direction::normalised(z));
    let form = match *form {
        Analytic::Plane { .. } => Analytic::Plane { origin: at, normal: up },
        Analytic::Cylinder { radius, .. } => Analytic::Cylinder { on_axis: at, axis: up, radius },
        Analytic::Cone { half_angle, .. } => Analytic::Cone { apex: at, axis: up, half_angle },
        Analytic::Sphere { radius, .. } => Analytic::Sphere { centre: at, radius },
        Analytic::Torus { major, minor, .. } => Analytic::Torus { centre: at, axis: up, major, minor },
    };
    (params == ParamMap::IDENTITY).then_some((moved, form))
}

/// The signed distance from `form` of every point of `points`.
pub(crate) fn distance(form: &Analytic, points: Spread) -> Interval {
    match *form {
        Analytic::Plane { origin, normal } => points.from(origin).along(normal.get()),
        Analytic::Sphere { centre, radius } => points.from(centre).length() - exact(radius.millimetres()),
        Analytic::Cylinder { on_axis, axis, radius } => {
            points.from(on_axis).across(axis.get()).sqrt() - exact(radius.millimetres())
        }
        Analytic::Cone { apex, axis, half_angle } => {
            let (sin, cos) = sin_cos(half_angle.radians());
            let w = points.from(apex);
            let height = w.along(axis.get());
            // Within about 0.75 ulps (S5): two ulps' radius holds each.
            let (sin, cos) = (Interval::new(sin, sin * NORMALISED), Interval::new(cos, cos * NORMALISED));
            w.across(axis.get()).sqrt() * cos - (height * height).sqrt() * sin
        }
        Analytic::Torus { centre, axis, major, minor } => {
            let w = points.from(centre);
            let (height, off) = (w.along(axis.get()), w.across(axis.get()).sqrt() - exact(major.millimetres()));
            (off * off + height * height).sqrt() - exact(minor.millimetres())
        }
    }
}

/// The gradient of the signed distance from `form` at `p`, in plain arithmetic, for a search to follow: the unit normal
/// of the surface at the point's foot. NaN on an axis or at a centre, where it has none.
pub(crate) fn gradient(form: &Analytic, p: Point<World>) -> Vec3 {
    let unit = |w: Vec3| w * (1.0 / w.norm());
    let off = |w: Vec3, axis: Vec3| w - axis * w.dot(axis);
    match *form {
        Analytic::Plane { normal, .. } => normal.get(),
        Analytic::Sphere { centre, .. } => unit(p.to_vec3() - centre.to_vec3()),
        Analytic::Cylinder { on_axis, axis, .. } => unit(off(p.to_vec3() - on_axis.to_vec3(), axis.get())),
        Analytic::Cone { apex, axis, half_angle } => {
            let (sin, cos) = sin_cos(half_angle.radians());
            let (w, axis) = (p.to_vec3() - apex.to_vec3(), axis.get());
            let height = w.dot(axis);
            unit(off(w, axis)) * cos - axis * if height < 0.0 { -sin } else { sin }
        }
        Analytic::Torus { centre, axis, major, .. } => {
            let w = p.to_vec3() - centre.to_vec3();
            unit(w - unit(off(w, axis.get())) * major.millimetres())
        }
    }
}

/// A value computed elsewhere, with its certified radius, as the predicate decides it.
struct Value;

impl Predicate<1> for Value {
    const DEGREE: u32 = 1;

    fn value<T: Ring>(&self, [x]: &[T; 1]) -> T {
        x.clone()
    }
}

/// Where `value` lies against the band `[-eps, eps]`, and the decision counted. An undecided one is counted where it
/// is reported, so the tally's undecided are the reports.
pub(crate) fn judge(value: Interval, eps: f64) -> (Verdict, Tally) {
    let mut out = Vec::with_capacity(1);
    let tally = decide_computed(&Value, &[[value]], eps, &mut out);
    (out.first().copied().unwrap_or(Verdict::Undecided), Tally { undecided: 0, ..tally })
}
