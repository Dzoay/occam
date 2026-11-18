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

/// Whether `child`, the box of a half of what `parent` boxed, shrank: by a sixteenth of its size at least. A family's
/// box halves with its piece, or close to it; one that keeps more, as a B-spline's keeps a knot span's whole height
/// over a patch of no width across it, tells no more when its piece is halved again.
pub(crate) fn shrank(parent: &Bounds, child: &Bounds) -> bool {
    let size = |b: &Bounds| (b.high().to_vec3() - b.low().to_vec3()).norm();
    size(child) <= size(parent) * (15.0 / 16.0)
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

#[cfg(test)]
mod tests {
    use occam_fixtures::point as at;
    use occam_geom::{Circle, Line};
    use occam_units::Local;

    use super::*;

    /// One point, exactly: a box of nothing but it.
    fn only(p: Point<World>) -> Spread {
        Spread::of(&Bounds::of([p]).unwrap())
    }

    /// A unit direction with every component non-zero and none one.
    fn slanted() -> Direction<World> {
        Direction::normalised(Vec3::new(1.0, 2.0, 2.0))
    }

    /// Every family placed off the origin about [`slanted`], with a sphere about the same centre.
    fn forms() -> [Analytic; 5] {
        let (o, a) = (at(1.0, -2.0, 3.0), slanted());
        [
            Analytic::Plane { origin: o, normal: a },
            Analytic::Sphere { centre: o, radius: Length::from_millimetres(1.5) },
            Analytic::Cylinder { on_axis: o, axis: a, radius: Length::from_millimetres(1.5) },
            Analytic::Cone { apex: o, axis: a, half_angle: Angle::from_radians(0.4) },
            Analytic::Torus {
                centre: o,
                axis: a,
                major: Length::from_millimetres(3.0),
                minor: Length::from_millimetres(1.0),
            },
        ]
    }

    /// The signed distance from `form` of `p`, by another route than [`distance`]'s: the component along the axis and
    /// the rest by subtraction, in plain arithmetic.
    fn plainly(form: &Analytic, p: Point<World>) -> f64 {
        let parts = |o: Point<World>, axis: Direction<World>| {
            let w = p.to_vec3() - o.to_vec3();
            let h = w.dot(axis.get());
            (h, (w - axis.get() * h).norm())
        };
        match *form {
            Analytic::Plane { origin, normal } => parts(origin, normal).0,
            Analytic::Sphere { centre, radius } => (p.to_vec3() - centre.to_vec3()).norm() - radius.millimetres(),
            Analytic::Cylinder { on_axis, axis, radius } => parts(on_axis, axis).1 - radius.millimetres(),
            Analytic::Cone { apex, axis, half_angle } => {
                let ((h, rho), (sin, cos)) = (parts(apex, axis), sin_cos(half_angle.radians()));
                rho * cos - h.abs() * sin
            }
            Analytic::Torus { centre, axis, major, minor } => {
                let (h, rho) = parts(centre, axis);
                ((rho - major.millimetres()) * (rho - major.millimetres()) + h * h).sqrt() - minor.millimetres()
            }
        }
    }

    /// Points around the forms, above and below the cone's apex and inside and outside each.
    fn points() -> [Point<World>; 4] {
        [at(4.0, 1.5, -1.5), at(-1.0, 0.5, 7.0), at(2.5, -4.0, 1.0), at(0.2, -0.3, 6.5)]
    }

    #[test]
    fn a_box_of_two_ends_holds_both_whichever_end_its_middle_rounds_towards() {
        let unit = f64::EPSILON;
        for (lo, hi) in [(1.0, 1.0 + 3.0 * unit), (1.0 - 1.5 * unit, 1.0), (-2.0, 5.0)] {
            let held = between(lo, hi);
            assert!(held.radius() >= held.mid() - lo && held.radius() >= hi - held.mid(), "{lo} {hi}: {held:?}");
            assert!(held.radius() <= (hi - lo).next_up(), "{lo} {hi}: {held:?}");
        }
    }

    #[test]
    fn an_evaluated_points_box_follows_its_largest_coordinate() {
        let b = around(at(1.0, -100.0, 1000.0));
        let reach = EVALUATED * 1000.0;
        for (low, high) in [(b.low().x(), b.high().x()), (b.low().z(), b.high().z())] {
            let width = high.millimetres() - low.millimetres();
            assert!(width >= 2.0 * reach && width < 2.1 * reach, "{b:?}");
        }
    }

    #[test]
    fn each_familys_distance_is_its_true_signed_distance_off_the_origin_and_axes() {
        for form in forms() {
            for p in points() {
                let (got, want) = (distance(&form, only(p)), plainly(&form, p));
                assert!((got.mid() - want).abs() < 1e-12 && got.radius() < 1e-12, "{form:?} at {p:?}: {got:?}, {want}");
            }
        }
    }

    #[test]
    fn each_familys_gradient_is_the_slope_of_its_distance() {
        for form in forms() {
            for p in points() {
                let step = 1e-6;
                let slope = |e: Vec3| {
                    let (ahead, behind) = (p.to_vec3() + e * step, p.to_vec3() - e * step);
                    (plainly(&form, Point::from_vec3(ahead)) - plainly(&form, Point::from_vec3(behind))) / (2.0 * step)
                };
                let want = Vec3::new(
                    slope(Vec3::new(1.0, 0.0, 0.0)),
                    slope(Vec3::new(0.0, 1.0, 0.0)),
                    slope(Vec3::new(0.0, 0.0, 1.0)),
                );
                let got = gradient(&form, p);
                assert!((got - want).norm() < 1e-6, "{form:?} at {p:?}: {got:?}, not {want:?}");
            }
        }
    }

    #[test]
    fn on_a_cones_apex_plane_the_gradient_is_the_upper_nappes() {
        let cone = Analytic::Cone {
            apex: at(0.0, 0.0, 0.0),
            axis: Direction::normalised(Vec3::new(0.0, 0.0, 1.0)),
            half_angle: Angle::from_radians(0.4),
        };
        let (sin, cos) = sin_cos(0.4);
        assert_eq!(gradient(&cone, at(2.0, 0.0, 0.0)), Vec3::new(cos, 0.0, -sin));
    }

    #[test]
    fn a_curve_moved_into_a_forms_frame_keeps_its_distance_from_it() {
        let line = Line::new(at(4.0, -1.0, 0.5), Direction::normalised(Vec3::new(1.0, -3.0, 2.0)));
        let spin = Placement::rotation(Direction::normalised(Vec3::new(0.3, 1.0, -0.2)), Angle::from_radians(1.1));
        let circle =
            Circle::placed(&Placement::<Local, World>::identity().then(&spin), Length::from_millimetres(2.0)).unwrap();
        let (up, down) =
            (Direction::normalised(Vec3::new(0.0, 0.0, 1.0)), Direction::normalised(Vec3::new(0.0, 0.0, -1.0)));
        let along = [slanted(), up, down];
        for (curve, axis) in [&line as &dyn Curve, &circle].into_iter().flat_map(|c| along.map(|a| (c, a))) {
            let form = Analytic::Cylinder { on_axis: at(1.0, -2.0, 3.0), axis, radius: Length::from_millimetres(1.5) };
            let (moved, there) = canonical(curve, &form).unwrap();
            assert_eq!(moved.range(), curve.range());
            for t in [0.0, 0.7, 2.5, -1.3] {
                let (got, want) = (distance(&there, Spread::at(moved.point(t))).mid(), plainly(&form, curve.point(t)));
                assert!((got - want).abs() < 1e-12, "{axis:?} at {t}: {got} against {want}");
            }
        }
    }
}
