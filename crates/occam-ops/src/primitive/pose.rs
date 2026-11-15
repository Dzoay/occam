//! Source: original
//! Where a primitive is: its frame, an origin and two axes, `z` and `x`, turned into a placement once. Only `x`'s part
//! across `z` counts, so the two need not be given perpendicular; neither may be zero, and `x` may not lie along `z`.
//! The placement is two turns and a shift, made with the owned `sin_cos` and `atan2` (O14), so a frame along the
//! world's axes places every point exactly where it is given.

use core::f64::consts::PI;

use occam_math::atan2;
use occam_units::{Angle, Direction, Frame, Local, Placement, Vec3, Vector};

/// A primitive's frame: its origin in millimetres, and the unit directions of its `z` axis and of its `x` axis, which
/// is across `z`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pose {
    /// Where its own origin is.
    pub origin: Vec3,
    /// Its `z` axis.
    pub z: Vec3,
    /// Its `x` axis, across `z`.
    pub x: Vec3,
}

impl Pose {
    /// The world's own frame.
    pub(crate) const STANDARD: Self =
        Self { origin: Vec3::new(0.0, 0.0, 0.0), z: Vec3::new(0.0, 0.0, 1.0), x: Vec3::new(1.0, 0.0, 0.0) };

    /// The placement from the primitive's own coordinates into `B`.
    pub(crate) fn placement<B: Frame>(self) -> Placement<Local, B> {
        axes(self.origin, self.z, self.x)
    }
}

/// The turn about the unit `axis` by `angle` radians.
fn turn(axis: Vec3, angle: f64) -> Placement<Local, Local> {
    Placement::rotation(Direction::normalised(axis), Angle::from_radians(angle))
}

/// A turn that takes the `z` axis to the unit `z`: half a turn about the direction halfway between them, after half a
/// turn about `x` where `z` points down, so the halfway direction is never short.
fn tilt(z: Vec3) -> Placement<Local, Local> {
    let up = Vec3::new(0.0, 0.0, 1.0);
    if z.z >= 0.0 { turn(up + z, PI) } else { turn(Vec3::new(1.0, 0.0, 0.0), PI).then(&turn(z - up, PI)) }
}

/// The placement whose origin is `origin`, its `z` axis along the unit `z` and its `x` axis along the unit `x`, which
/// is across `z`: the tilt that takes `z` there, then the turn about it that takes `x` there, then the shift.
pub(crate) fn axes<B: Frame>(origin: Vec3, z: Vec3, x: Vec3) -> Placement<Local, B> {
    let tilt = tilt(z);
    let tilted = tilt.vector(Vector::from_vec3(Vec3::new(1.0, 0.0, 0.0))).to_vec3();
    let spin = turn(z, atan2(tilted.cross(x).dot(z), tilted.dot(x)));
    tilt.then(&spin).then(&Placement::identity()).then(&Placement::<B, B>::translation(Vector::from_vec3(origin)))
}

#[cfg(test)]
mod tests {
    use occam_units::{Point, World};

    use super::*;

    /// Within a few roundings of a point `size` from the origin.
    fn close(a: Vec3, b: Vec3, size: f64) -> bool {
        (a - b).norm() <= 16.0 * f64::EPSILON * (1.0 + size)
    }

    fn unit(x: f64, y: f64, z: f64) -> Vec3 {
        let v = Vec3::new(x, y, z);
        v * (1.0 / v.norm())
    }

    #[test]
    fn the_standard_frame_places_every_point_where_it_is() {
        let p: Placement<Local, World> = Pose::STANDARD.placement();
        let at = Vec3::new(3.25, -7.5, 11.0);
        assert_eq!(p.point(Point::from_vec3(at)).to_vec3(), at, "exactly");
    }

    #[test]
    fn a_frame_takes_its_axes_and_origin_where_they_are_given() {
        let frames = [
            (Vec3::new(1.0, 2.0, 3.0), unit(0.0, 0.0, -1.0), unit(0.0, 1.0, 0.0)),
            (Vec3::new(-4.0, 0.5, 0.0), unit(1.0, 1.0, 1.0), unit(1.0, -1.0, 0.0)),
            (Vec3::new(0.0, 0.0, 0.0), unit(0.0, -1.0, -0.2), unit(1.0, 0.0, 0.0)),
            (Vec3::new(9.0, 9.0, 9.0), unit(0.3, -0.1, 0.9), unit(-0.1, 0.9, 0.2)),
        ];
        for (origin, z, x) in frames {
            let x = (x - z * x.dot(z)) * (1.0 / (x - z * x.dot(z)).norm());
            let p: Placement<Local, World> = axes(origin, z, x);
            let place = |v: Vec3| p.point(Point::from_vec3(v)).to_vec3();
            assert!(close(place(Vec3::new(0.0, 0.0, 0.0)), origin, origin.norm()), "{origin:?}");
            assert!(close(place(Vec3::new(0.0, 0.0, 1.0)) - origin, z, origin.norm()), "{z:?}");
            assert!(close(place(Vec3::new(1.0, 0.0, 0.0)) - origin, x, origin.norm()), "{x:?}");
            assert!(close(place(Vec3::new(0.0, 1.0, 0.0)) - origin, z.cross(x), origin.norm()), "right-handed");
        }
    }
}
