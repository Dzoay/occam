//! Source: original
//! [`Affine`]: a map of `World` to itself that may stretch, as a transform of a body asks (PARITY 2.11): a rigid
//! placement, a scale the same in every direction, a scale that differs along the axes of a frame, or these one after
//! another. It keeps its sense, so nothing is turned inside out. Where it is a similarity, a rigid placement after one
//! scale about the origin, it carries those two parts as they were given, so a curve or a surface that keeps its form
//! under it is placed by the turn it was given, not by a product of matrices.

use crate::frame::{Local, Point, World};
use crate::placement::Placement;
use crate::vec3::Vec3;

/// A similarity: `p` goes to `placement(scale p)`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Similar {
    /// The rigid part, after the scale.
    pub placement: Placement<World, World>,
    /// The scale about the origin, more than zero.
    pub scale: f64,
}

/// An affine map of `World` to itself that keeps its sense: `p` goes to `rows p + shift`, where the determinant of
/// `rows` is more than zero.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Affine {
    rows: [Vec3; 3],
    shift: Vec3,
    similar: Option<Similar>,
    most: f64,
}

/// The rows of the identity.
const UNIT: [Vec3; 3] = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];

/// `rows v`.
fn times(rows: &[Vec3; 3], v: Vec3) -> Vec3 {
    let [a, b, c] = rows;
    Vec3::new(a.dot(v), b.dot(v), c.dot(v))
}

impl Affine {
    /// The map that moves nothing.
    #[must_use]
    pub fn identity() -> Self {
        Self::placed(Placement::identity())
    }

    /// The rigid map `placement`.
    #[must_use]
    pub fn placed(placement: Placement<World, World>) -> Self {
        Self::similar_parts(placement, 1.0)
    }

    /// `placement` after a scale of `scale` about the origin, if `scale` is finite and more than zero.
    #[must_use]
    pub fn similarity(placement: Placement<World, World>, scale: f64) -> Option<Self> {
        (scale.is_finite() && scale > 0.0).then(|| Self::similar_parts(placement, scale))
    }

    /// The similarity of `placement` after `scale`, which is finite and more than zero.
    fn similar_parts(placement: Placement<World, World>, scale: f64) -> Self {
        let (rotation, shift) = placement.parts();
        let rows = rotation.map(|row| row * scale);
        Self { rows, shift, similar: Some(Similar { placement, scale }), most: scale }
    }

    /// A scale about the origin of `frame` by `factors` along its X, Y and Z axes, if each is finite and more than zero:
    /// a similarity where the three are equal, which fixes the frame's origin.
    #[must_use]
    #[expect(clippy::float_cmp, reason = "three factors as given, compared exactly: equal ones are one scale")]
    pub fn stretch(frame: &Placement<Local, World>, factors: [f64; 3]) -> Option<Self> {
        if !factors.iter().all(|f| f.is_finite() && *f > 0.0) {
            return None;
        }
        let (axes, centre) = frame.parts();
        let [a, b, c] = factors;
        if a == b && b == c {
            return Some(Self::similar_parts(Placement::from_parts(UNIT, centre * (1.0 - a)), a));
        }
        // `R S R^T`, where the columns of `R` are the frame's axes in `World`, which are the rows its placement holds
        // transposed: entry `(i, j)` is the sum over the axes of `R[i][k] factor[k] R[j][k]`.
        let entry = |i: &Vec3, j: &Vec3| i.x * a * j.x + i.y * b * j.y + i.z * c * j.z;
        let [r0, r1, r2] = axes;
        let rows = [r0, r1, r2].map(|i| Vec3::new(entry(&i, &r0), entry(&i, &r1), entry(&i, &r2)));
        let shift = centre - times(&rows, centre);
        let most = factors.into_iter().fold(0.0, |m: f64, f| if f > m { f } else { m });
        Some(Self { rows, shift, similar: None, most })
    }

    /// The map `p` to `rows p + shift`, if every number is finite and the determinant of `rows` is more than zero.
    #[must_use]
    pub fn linear(rows: [Vec3; 3], shift: Vec3) -> Option<Self> {
        let [a, b, c] = rows;
        let finite = [a, b, c, shift].iter().all(|v| v.x.is_finite() && v.y.is_finite() && v.z.is_finite());
        let most = (a.dot(a) + b.dot(b) + c.dot(c)).sqrt();
        (finite && a.dot(b.cross(c)) > 0.0).then_some(Self { rows, shift, similar: None, most })
    }

    /// This map, then `next`.
    #[must_use]
    pub fn then(&self, next: &Self) -> Self {
        let [n0, n1, n2] = next.rows;
        let [m0, m1, m2] = self.rows;
        let row = |n: Vec3| m0 * n.x + m1 * n.y + m2 * n.z;
        let similar = self.similar.zip(next.similar).map(|(first, second)| {
            let (rotation, shift) = first.placement.parts();
            let placement = Placement::from_parts(rotation, shift * second.scale).then(&second.placement);
            Similar { placement, scale: first.scale * second.scale }
        });
        Self {
            rows: [row(n0), row(n1), row(n2)],
            shift: times(&next.rows, self.shift) + next.shift,
            similar,
            most: self.most * next.most,
        }
    }

    /// Where `p` goes.
    #[must_use]
    pub fn point(&self, p: Point<World>) -> Point<World> {
        match self.similar {
            Some(Similar { placement, scale }) => placement.point(Point::from_vec3(p.to_vec3() * scale)),
            None => Point::from_vec3(times(&self.rows, p.to_vec3()) + self.shift),
        }
    }

    /// Where the vector `v` goes: by the map's linear part, never shifted.
    #[must_use]
    pub fn vector(&self, v: Vec3) -> Vec3 {
        match self.similar {
            Some(Similar { placement, scale }) => times(&placement.parts().0, v * scale),
            None => times(&self.rows, v),
        }
    }

    /// The map as a similarity, if it is one.
    #[must_use]
    pub const fn similar(&self) -> Option<Similar> {
        self.similar
    }

    /// A factor no length grows by more than under the map.
    #[must_use]
    pub const fn most(&self) -> f64 {
        self.most
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::direction::Direction;
    use crate::frame::Vector;
    use crate::measure::Angle;

    fn point(x: f64, y: f64, z: f64) -> Point<World> {
        Point::from_vec3(Vec3::new(x, y, z))
    }

    fn near(a: Point<World>, b: Point<World>) -> bool {
        (a - b).to_vec3().norm() <= 1e-12 * (1.0 + b.to_vec3().norm())
    }

    /// A quarter turn about Z, then a shift of (1, 2, 3).
    fn turned() -> Placement<World, World> {
        let quarter = Placement::rotation(Direction::normalised(Vec3::new(0.0, 0.0, 1.0)), Angle::from_radians(1.0));
        quarter.then(&Placement::translation(Vector::from_vec3(Vec3::new(1.0, 2.0, 3.0))))
    }

    #[test]
    fn a_similarity_scales_about_the_origin_then_places() {
        let map = Affine::similarity(turned(), 2.5).unwrap();
        let p = point(1.0, -2.0, 0.5);
        assert_eq!(map.point(p), turned().point(point(2.5, -5.0, 1.25)));
        assert_eq!(map.similar(), Some(Similar { placement: turned(), scale: 2.5 }));
        assert_eq!(map.most(), 2.5);
        let v = Vec3::new(0.0, 0.0, 2.0);
        assert!((map.vector(v) - Vec3::new(0.0, 0.0, 5.0)).norm() < 1e-15, "{:?}", map.vector(v));
        assert_eq!(Affine::identity().point(p), p);
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            assert_eq!(Affine::similarity(turned(), bad), None, "{bad}");
        }
    }

    /// A frame at (1, 1, 0) whose X axis is the world's Y and whose Z is the world's Z.
    fn sideways() -> Placement<Local, World> {
        let turn = Placement::<Local, Local>::rotation(
            Direction::normalised(Vec3::new(0.0, 0.0, 1.0)),
            Angle::from_radians(core::f64::consts::FRAC_PI_2),
        );
        turn.then(&Placement::identity()).then(&Placement::translation(Vector::from_vec3(Vec3::new(1.0, 1.0, 0.0))))
    }

    #[test]
    fn a_stretch_scales_along_its_frame_about_its_origin() {
        let map = Affine::stretch(&sideways(), [3.0, 1.0, 0.5]).unwrap();
        assert!(near(map.point(point(1.0, 1.0, 0.0)), point(1.0, 1.0, 0.0)), "its origin stays");
        assert!(near(map.point(point(1.0, 2.0, 0.0)), point(1.0, 4.0, 0.0)), "along its X, the world's Y, by 3");
        assert!(near(map.point(point(0.0, 1.0, 0.0)), point(0.0, 1.0, 0.0)), "along its Y by 1");
        assert!(near(map.point(point(1.0, 1.0, 4.0)), point(1.0, 1.0, 2.0)), "along Z by a half");
        assert_eq!((map.similar(), map.most()), (None, 3.0));
        assert!((map.vector(Vec3::new(0.0, 1.0, 1.0)) - Vec3::new(0.0, 3.0, 0.5)).norm() < 1e-15);
        let alike = Affine::stretch(&sideways(), [2.0, 2.0, 2.0]).unwrap();
        assert_eq!(alike.similar().map(|s| s.scale), Some(2.0), "equal factors are a similarity");
        assert!(near(alike.point(point(1.0, 1.0, 0.0)), point(1.0, 1.0, 0.0)), "about the frame's origin");
        assert!(near(alike.point(point(2.0, 1.0, 1.0)), point(3.0, 1.0, 2.0)));
        for bad in [[0.0, 1.0, 1.0], [1.0, -2.0, 1.0], [1.0, 1.0, f64::NAN]] {
            assert_eq!(Affine::stretch(&sideways(), bad), None, "{bad:?}");
        }
    }

    #[test]
    fn a_stretch_along_a_skew_frame_scales_each_of_its_axes_by_its_own_factor() {
        // Two factors equal and one not is a stretch still, not a similarity of the two.
        let turn = Placement::<Local, Local>::rotation(
            Direction::normalised(Vec3::new(1.0, 2.0, 3.0)),
            Angle::from_radians(0.7),
        );
        let frame = turn
            .then(&Placement::identity())
            .then(&Placement::translation(Vector::from_vec3(Vec3::new(4.0, -1.0, 2.0))));
        for factors in [[3.0, 3.0, 0.5], [2.0, 0.25, 5.0], [0.5, 4.0, 4.0]] {
            let map = Affine::stretch(&frame, factors).unwrap();
            assert_eq!(map.similar(), None, "{factors:?}");
            for local in [
                Vec3::new(1.0, 0.0, 0.0),
                Vec3::new(0.0, 1.0, 0.0),
                Vec3::new(0.0, 0.0, 1.0),
                Vec3::new(1.0, -2.0, 0.5),
            ] {
                let p = frame.point(Point::from_vec3(local));
                let [a, b, c] = factors;
                let want = frame.point(Point::from_vec3(Vec3::new(local.x * a, local.y * b, local.z * c)));
                assert!(near(map.point(p), want), "{factors:?} at {local:?}");
            }
        }
    }

    #[test]
    fn maps_one_after_another_compose_and_stay_similar_only_while_each_is() {
        let first = Affine::similarity(turned(), 2.0).unwrap();
        let second = Affine::similarity(turned().inverse(), 0.5).unwrap();
        let stretch = Affine::stretch(&sideways(), [3.0, 1.0, 0.5]).unwrap();
        let p = point(0.25, -1.5, 2.0);
        let both = first.then(&second);
        assert!(near(both.point(p), second.point(first.point(p))));
        assert_eq!(both.similar().map(|s| s.scale), Some(1.0));
        assert_eq!(both.most(), 1.0);
        let mixed = first.then(&stretch);
        assert!(near(mixed.point(p), stretch.point(first.point(p))));
        assert_eq!((mixed.similar(), mixed.most()), (None, 6.0));
        let v = Vec3::new(1.0, -1.0, 0.5);
        assert!((mixed.vector(v) - stretch.vector(first.vector(v))).norm() < 1e-12);
    }

    #[test]
    fn a_linear_map_must_keep_its_sense_and_be_finite() {
        let rows = [Vec3::new(2.0, 1.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
        let map = Affine::linear(rows, Vec3::new(0.0, 0.0, 1.0)).unwrap();
        assert_eq!(map.point(point(1.0, 1.0, 1.0)), point(3.0, 1.0, 2.0));
        assert!((map.most() - 7f64.sqrt()).abs() < 1e-15, "the rows' size bounds the stretch");
        let mirror = [Vec3::new(-1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
        assert_eq!(Affine::linear(mirror, Vec3::new(0.0, 0.0, 0.0)), None, "a mirror turns inside out");
        assert_eq!(Affine::linear(rows, Vec3::new(f64::NAN, 0.0, 0.0)), None);
        let flat = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 0.0)];
        assert_eq!(Affine::linear(flat, Vec3::new(0.0, 0.0, 0.0)), None, "a map that flattens has no sense to keep");
    }
}
