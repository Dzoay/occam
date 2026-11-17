//! Source: bajaj-hoffmann-lynch-hopcroft-1988
//! An analytic surface in implicit form, for the trace's corrector (Bajaj, Hoffmann, Lynch and Hopcroft trace where an
//! implicit surface meets another surface through the implicit function and its gradient). Each form here is the
//! signed distance from its surface, or as near it as the surface's shape allows, so that its value is a gap in
//! millimetres and its gradient a unit normal: a plane's, a sphere's, a cylinder's and a torus's exactly, and a double
//! cone's away from its apex, both nappes included.

use occam_geom::Analytic;
use occam_math::{Interval, sin_cos};
use occam_units::Vec3;

use crate::decide::{dot, exact, less};

/// A surface as the zero set of a function of space.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Implicit {
    /// `n . (x - o)`.
    Plane { origin: Vec3, normal: Vec3 },
    /// `|x - c| - r`.
    Sphere { centre: Vec3, radius: f64 },
    /// The distance from the axis, less the radius.
    Cylinder { on_axis: Vec3, axis: Vec3, radius: f64 },
    /// `rho cos a - |h| sin a`, for `h` along the axis from the apex and `rho` across it: both nappes.
    Cone { apex: Vec3, axis: Vec3, slope: (f64, f64) },
    /// The distance from the circle of the tube's centres, less the tube's radius.
    Torus { centre: Vec3, axis: Vec3, major: f64, minor: f64 },
}

/// `x - origin` as its part along `axis`, its part across it, and the length of that.
fn split(x: Vec3, origin: Vec3, axis: Vec3) -> (f64, Vec3, f64) {
    let w = x - origin;
    let along = w.dot(axis);
    let across = w - axis * along;
    (along, across, across.norm())
}

/// `x - origin` as its part along `axis` and its part across it, enclosed.
fn parts(x: Vec3, origin: Vec3, axis: Vec3) -> (Interval, [Interval; 3]) {
    let (w, axis) = (less(exact(x), exact(origin)), exact(axis));
    let along = dot(w, axis);
    let [wx, wy, wz] = w;
    let [ax, ay, az] = axis;
    (along, [wx - ax * along, wy - ay * along, wz - az * along])
}

/// The size of every value `x` holds, enclosed: what it holds lies within its radius of its middle, and so its size
/// within that of its middle's size.
pub(crate) fn magnitude(x: Interval) -> Interval {
    Interval::new(x.mid().abs(), x.radius())
}

impl Implicit {
    /// The implicit form of `form`.
    pub(crate) fn of(form: Analytic) -> Self {
        match form {
            Analytic::Plane { origin, normal } => Self::Plane { origin: origin.to_vec3(), normal: normal.get() },
            Analytic::Sphere { centre, radius } => {
                Self::Sphere { centre: centre.to_vec3(), radius: radius.millimetres() }
            }
            Analytic::Cylinder { on_axis, axis, radius } => {
                Self::Cylinder { on_axis: on_axis.to_vec3(), axis: axis.get(), radius: radius.millimetres() }
            }
            Analytic::Cone { apex, axis, half_angle } => {
                Self::Cone { apex: apex.to_vec3(), axis: axis.get(), slope: sin_cos(half_angle.radians()) }
            }
            Analytic::Torus { centre, axis, major, minor } => Self::Torus {
                centre: centre.to_vec3(),
                axis: axis.get(),
                major: major.millimetres(),
                minor: minor.millimetres(),
            },
        }
    }

    /// The function's value at `x` and its gradient there, a unit vector wherever the surface has a normal. Where it
    /// has none, on a cylinder's or torus's axis or at a cone's apex, the gradient is not finite.
    pub(crate) fn at(&self, x: Vec3) -> (f64, Vec3) {
        match *self {
            Self::Plane { origin, normal } => ((x - origin).dot(normal), normal),
            Self::Sphere { centre, radius } => {
                let out = x - centre;
                let length = out.norm();
                (length - radius, out * (1.0 / length))
            }
            Self::Cylinder { on_axis, axis, radius } => {
                let (_, across, rho) = split(x, on_axis, axis);
                (rho - radius, across * (1.0 / rho))
            }
            Self::Cone { apex, axis, slope: (sin, cos) } => {
                let (along, across, rho) = split(x, apex, axis);
                let side = if along < 0.0 { -1.0 } else { 1.0 };
                (rho * cos - along * side * sin, across * (cos / rho) - axis * (side * sin))
            }
            Self::Torus { centre, axis, major, minor } => {
                let (along, across, rho) = split(x, centre, axis);
                let out = across * ((rho - major) / rho) + axis * along;
                let length = out.norm();
                (length - minor, out * (1.0 / length))
            }
        }
    }

    /// The function's value at `x`, enclosed: the formula of [`Implicit::at`] in interval arithmetic, on `x` and the
    /// form's numbers as they are held, so that the value's rounding is bounded. Every form here is a distance or, for
    /// the cone, changes no faster than one, so over a ball of radius `r` about `x` the function stays within `r` of it
    /// (P8's implicit test).
    pub(crate) fn enclosed(&self, x: Vec3) -> Interval {
        let length = |v: [Interval; 3]| dot(v, v).sqrt();
        match *self {
            Self::Plane { origin, normal } => dot(less(exact(x), exact(origin)), exact(normal)),
            Self::Sphere { centre, radius } => length(less(exact(x), exact(centre))) - Interval::exact(radius),
            Self::Cylinder { on_axis, axis, radius } => {
                let (_, across) = parts(x, on_axis, axis);
                length(across) - Interval::exact(radius)
            }
            Self::Cone { apex, axis, slope: (sin, cos) } => {
                let (along, across) = parts(x, apex, axis);
                length(across) * Interval::exact(cos) - magnitude(along) * Interval::exact(sin)
            }
            Self::Torus { centre, axis, major, minor } => {
                let (along, across) = parts(x, centre, axis);
                let out = length(across) - Interval::exact(major);
                (out * out + along * along).sqrt() - Interval::exact(minor)
            }
        }
    }

    /// `x` moved along the gradient onto the surface: exact for a distance, and the nearer foot otherwise.
    pub(crate) fn onto(&self, x: Vec3) -> Vec3 {
        let (value, gradient) = self.at(x);
        x - gradient * value
    }
}

#[cfg(test)]
mod tests {
    use occam_fixtures::point;
    use occam_units::{Angle, Direction, Length};

    use super::*;

    fn up() -> Direction<occam_units::World> {
        Direction::normalised(Vec3::new(0.0, 0.0, 1.0))
    }

    #[test]
    fn each_form_is_a_signed_distance_with_a_unit_gradient() {
        let mm = Length::from_millimetres;
        let forms = [
            (Analytic::Plane { origin: point(0.0, 0.0, 1.0), normal: up() }, Vec3::new(3.0, 4.0, 3.5), 2.5),
            (Analytic::Sphere { centre: point(1.0, 0.0, 0.0), radius: mm(2.0) }, Vec3::new(1.0, 0.0, 5.0), 3.0),
            (
                Analytic::Cylinder { on_axis: point(0.0, 0.0, 7.0), axis: up(), radius: mm(1.0) },
                Vec3::new(0.0, 3.0, -2.0),
                2.0,
            ),
            (
                Analytic::Torus { centre: point(0.0, 0.0, 0.0), axis: up(), major: mm(5.0), minor: mm(1.0) },
                Vec3::new(0.0, 5.0, 3.0),
                2.0,
            ),
        ];
        for (form, x, distance) in forms {
            let (value, gradient) = Implicit::of(form).at(x);
            assert!((value - distance).abs() < 1e-12 && (gradient.norm() - 1.0).abs() < 1e-12, "{form:?}");
            assert!(Implicit::of(form).at(Implicit::of(form).onto(x)).0.abs() < 1e-12, "{form:?}");
        }
    }

    #[test]
    fn each_forms_enclosed_value_holds_its_value_to_rounding() {
        let mm = Length::from_millimetres;
        let tilted = Direction::normalised(Vec3::new(0.3, -0.4, 0.866));
        let forms = [
            Analytic::Plane { origin: point(0.1, 0.0, 1.0), normal: tilted },
            Analytic::Sphere { centre: point(1.0, 0.2, 0.0), radius: mm(2.0) },
            Analytic::Cylinder { on_axis: point(0.0, 0.3, 7.0), axis: tilted, radius: mm(1.0) },
            Analytic::Cone { apex: point(0.0, 0.0, 0.5), axis: tilted, half_angle: Angle::from_radians(0.4) },
            Analytic::Torus { centre: point(0.0, 0.0, 0.0), axis: tilted, major: mm(5.0), minor: mm(1.0) },
        ];
        for form in forms {
            for x in [Vec3::new(3.0, 4.0, 3.5), Vec3::new(-0.7, 0.2, -2.5), Vec3::new(4.9, 0.1, 0.3)] {
                let (value, enclosed) = (Implicit::of(form).at(x).0, Implicit::of(form).enclosed(x));
                let holds = enclosed.lower() <= value && value <= enclosed.upper();
                assert!(holds && enclosed.radius() < 1e-13, "{form:?} at {x:?}: {value} in {enclosed:?}");
            }
        }
        let below = Implicit::of(Analytic::Cone {
            apex: point(0.0, 0.0, 0.0),
            axis: up(),
            half_angle: Angle::from_radians(0.4),
        });
        let (sin, cos) = sin_cos(0.4);
        let mid = below.enclosed(Vec3::new(2.0, 0.0, -3.0)).mid();
        assert!((mid - (2.0 * cos - 3.0 * sin)).abs() < 1e-14, "the lower nappe's height is taken by its size: {mid}");
    }

    #[test]
    fn a_cones_form_holds_both_nappes() {
        let cone = Implicit::of(Analytic::Cone {
            apex: point(0.0, 0.0, 0.0),
            axis: up(),
            half_angle: Angle::from_radians(std::f64::consts::FRAC_PI_4),
        });
        for height in [2.0, -2.0] {
            let (value, gradient) = cone.at(Vec3::new(2.0, 0.0, height));
            assert!(value.abs() < 1e-12, "{height}");
            let half = std::f64::consts::FRAC_1_SQRT_2;
            let want = Vec3::new(half, 0.0, if height > 0.0 { -half } else { half });
            assert!((gradient - want).norm() < 1e-12, "{height}: {gradient:?}");
        }
        let (outside, _) = cone.at(Vec3::new(3.0, 0.0, -1.0));
        assert!((outside - std::f64::consts::SQRT_2).abs() < 1e-12, "a distance below the apex too");
    }

    #[test]
    fn on_the_plane_of_a_cones_apex_its_form_is_the_upper_nappes() {
        // Where the part along the axis is nought both nappes' distances agree; the gradient is the upper nappe's.
        let cone = Implicit::of(Analytic::Cone {
            apex: point(0.0, 0.0, 0.0),
            axis: up(),
            half_angle: Angle::from_radians(std::f64::consts::FRAC_PI_4),
        });
        let (value, gradient) = cone.at(Vec3::new(2.0, 0.0, 0.0));
        let half = std::f64::consts::FRAC_1_SQRT_2;
        assert!((value - std::f64::consts::SQRT_2).abs() < 1e-12);
        assert!((gradient - Vec3::new(half, 0.0, -half)).norm() < 1e-12, "{gradient:?}");
    }
}
