//! Source: original
//! [`Bounds`]: a box that holds everything it bounds, rounding included. A box too small is a silent wrong answer (S1
//! item 13, P5, P8), so every bound here is rounded outward.

use crate::frame::{Point, World};
use crate::measure::Length;
use crate::tolerance::Tolerance;
use crate::vec3::Vec3;

/// An axis-aligned box in `World`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Bounds {
    low: Vec3,
    high: Vec3,
}

/// The smaller of two numbers, the first if they are equal, so the sign of a zero is fixed. Not `f64::min`, which may
/// return either zero (rule 2).
fn smaller(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// The larger of two numbers, the first if they are equal.
fn larger(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// Each component the smaller or the larger of the two.
fn each(a: Vec3, b: Vec3, pick: fn(f64, f64) -> f64) -> Vec3 {
    Vec3::new(pick(a.x, b.x), pick(a.y, b.y), pick(a.z, b.z))
}

/// Each component stepped one double down or up, so that a rounded sum bounds the exact one.
fn stepped(v: Vec3, step: fn(f64) -> f64) -> Vec3 {
    Vec3::new(step(v.x), step(v.y), step(v.z))
}

impl Bounds {
    /// The smallest box holding every point, which are stored and so exact: `None` for no points, or any that is not
    /// finite.
    #[must_use]
    pub fn of(points: impl IntoIterator<Item = Point<World>>) -> Option<Self> {
        let mut found: Option<Self> = None;
        for point in points {
            let p = point.to_vec3();
            if !(p.x.is_finite() && p.y.is_finite() && p.z.is_finite()) {
                return None;
            }
            found = Some(found.map_or(Self { low: p, high: p }, |b| Self {
                low: each(b.low, p, smaller),
                high: each(b.high, p, larger),
            }));
        }
        found
    }

    /// Every point within `radius` of `centre` in each coordinate, the bounds rounded outward.
    #[must_use]
    pub fn around(centre: Point<World>, radius: Length) -> Self {
        let (c, r) = (centre.to_vec3(), radius.millimetres().abs());
        let spread = Vec3::new(r, r, r);
        Self { low: stepped(c - spread, f64::next_down), high: stepped(c + spread, f64::next_up) }
    }

    /// The box holding both.
    #[must_use]
    pub fn union(&self, other: &Self) -> Self {
        Self { low: each(self.low, other.low, smaller), high: each(self.high, other.high, larger) }
    }

    /// The lowest corner.
    #[must_use]
    pub fn low(&self) -> Point<World> {
        Point::from_vec3(self.low)
    }

    /// The highest corner.
    #[must_use]
    pub fn high(&self) -> Point<World> {
        Point::from_vec3(self.high)
    }

    /// Whether `point` is inside, edges included.
    #[must_use]
    pub fn contains(&self, point: Point<World>) -> bool {
        let p = point.to_vec3();
        let inside = |low: f64, value: f64, high: f64| low <= value && value <= high;
        inside(self.low.x, p.x, self.high.x)
            && inside(self.low.y, p.y, self.high.y)
            && inside(self.low.z, p.z, self.high.z)
    }

    /// Whether the boxes come within the model's band of each other. It may say yes for boxes a little further apart,
    /// never no for boxes within it, so that a cull never drops a pair it should keep.
    #[must_use]
    pub fn overlaps(&self, other: &Self, tolerance: Tolerance) -> bool {
        let band = tolerance.band().millimetres();
        let reach = |low: f64, high: f64| (high + band).next_up() >= low;
        let axis = |a_low: f64, a_high: f64, b_low: f64, b_high: f64| reach(b_low, a_high) && reach(a_low, b_high);
        axis(self.low.x, self.high.x, other.low.x, other.high.x)
            && axis(self.low.y, self.high.y, other.low.y, other.high.y)
            && axis(self.low.z, self.high.z, other.low.z, other.high.z)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::measure::Angle;

    fn point(x: f64, y: f64, z: f64) -> Point<World> {
        Point::from_vec3(Vec3::new(x, y, z))
    }

    fn tolerance(length: f64) -> Tolerance {
        Tolerance::new(Length::from_millimetres(length), Angle::from_radians(1e-9)).unwrap()
    }

    #[test]
    fn the_box_of_points_is_their_extremes() {
        let b = Bounds::of([point(1.0, -2.0, 3.0), point(-4.0, 5.0, 0.5), point(0.0, 0.0, 9.0)]).unwrap();
        assert_eq!((b.low(), b.high()), (point(-4.0, -2.0, 0.5), point(1.0, 5.0, 9.0)));
        assert!(Bounds::of([]).is_none());
        assert!(Bounds::of([point(f64::NAN, 0.0, 0.0)]).is_none());
        assert!(Bounds::of([point(0.0, 0.0, 0.0), point(f64::INFINITY, 0.0, 0.0)]).is_none());
    }

    #[test]
    fn a_box_around_a_point_holds_the_exact_reach_of_its_radius() {
        use std::cmp::Ordering;

        use occam_math::Expansion;

        // c - r and c + r round at these sizes; the box must hold the exact values.
        for (centre, radius) in [(10_000.1, 0.1), (-3.3, 1e-9), (0.7, 0.3), (1e15, 0.01)] {
            let b = Bounds::around(point(centre, centre, centre), Length::from_millimetres(radius));
            let (c, r) = (Expansion::of(centre), Expansion::of(radius));
            let (low, high) = (Expansion::of(b.low().x().millimetres()), Expansion::of(b.high().x().millimetres()));
            assert_ne!(low.difference(&c.difference(&r)).sign(), Some(Ordering::Greater), "{centre} - {radius}");
            assert_ne!(high.difference(&c.sum(&r)).sign(), Some(Ordering::Less), "{centre} + {radius}");
        }
    }

    #[test]
    fn a_union_holds_both() {
        let a = Bounds::of([point(0.0, 0.0, 0.0), point(1.0, 1.0, 1.0)]).unwrap();
        let b = Bounds::of([point(-1.0, 0.5, 2.0)]).unwrap();
        let both = a.union(&b);
        assert_eq!((both.low(), both.high()), (point(-1.0, 0.0, 0.0), point(1.0, 1.0, 2.0)));
    }

    #[test]
    fn boxes_within_the_band_overlap_and_boxes_beyond_it_do_not() {
        let a = Bounds::of([point(0.0, 0.0, 0.0), point(1.0, 1.0, 1.0)]).unwrap();
        let touching = Bounds::of([point(1.0, 0.0, 0.0), point(2.0, 1.0, 1.0)]).unwrap();
        let near = Bounds::of([point(1.0 + 5e-8, 0.0, 0.0), point(2.0, 1.0, 1.0)]).unwrap();
        let far = Bounds::of([point(1.0 + 1e-6, 0.0, 0.0), point(2.0, 1.0, 1.0)]).unwrap();
        let above = Bounds::of([point(0.0, 0.0, 1.0 + 1e-3), point(1.0, 1.0, 2.0)]).unwrap();
        let tolerance = tolerance(1e-7);
        assert!(a.overlaps(&touching, tolerance) && touching.overlaps(&a, tolerance));
        assert!(a.overlaps(&near, tolerance) && near.overlaps(&a, tolerance));
        assert!(!a.overlaps(&far, tolerance) && !far.overlaps(&a, tolerance));
        assert!(!a.overlaps(&above, tolerance) && !above.overlaps(&a, tolerance));
    }

    #[test]
    fn smaller_and_larger_pick_without_min_or_max() {
        assert_eq!((smaller(1.0, 2.0), smaller(2.0, 1.0)), (1.0, 1.0));
        assert_eq!((larger(1.0, 2.0), larger(2.0, 1.0)), (2.0, 2.0));
        let bits = |x: f64| x.to_bits();
        assert_eq!((bits(smaller(0.0, -0.0)), bits(smaller(-0.0, 0.0))), (bits(0.0), bits(-0.0)), "the first zero");
        assert_eq!((bits(larger(0.0, -0.0)), bits(larger(-0.0, 0.0))), (bits(0.0), bits(-0.0)), "the first zero");
    }

    #[test]
    fn a_box_contains_what_is_inside_it_edges_included() {
        let b = Bounds::of([point(-1.0, 0.0, 2.0), point(1.0, 3.0, 5.0)]).unwrap();
        for inside in [point(0.0, 1.0, 3.0), point(-1.0, 0.0, 2.0), point(1.0, 3.0, 5.0), point(1.0, 0.0, 5.0)] {
            assert!(b.contains(inside), "{inside:?}");
        }
        for (x, y, z) in
            [(-1.5, 1.0, 3.0), (1.5, 1.0, 3.0), (0.0, -0.5, 3.0), (0.0, 3.5, 3.0), (0.0, 1.0, 1.5), (0.0, 1.0, 5.5)]
        {
            assert!(!b.contains(point(x, y, z)), "{x} {y} {z}");
        }
        assert!(!b.contains(point(f64::NAN, 1.0, 3.0)));
    }
}
