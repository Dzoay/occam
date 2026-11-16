//! Source: lane-riesenfeld-1980
//! A part of a surface's parameters as the hybrid's subdivision holds it, with a box that holds the surface over it:
//! the family's own `bounds`, which follows the part as it is halved, as Lane and Riesenfeld's subdivision closes the
//! hull of the control net in on its part; a B-spline's is the hull of the part's own poles. Where a box stops shrinking
//! as the part is halved, as an offset's does at twice its distance, the part's size is measured from points of the
//! surface over it instead, so that halving ends. The hybrid needs no B-spline form.

use occam_geom::{Surface, UvBox};
use occam_units::{Bounds, UvPoint, Vec3};

/// The most times a part is halved: a part this deep is `2^-24` of its surface's domain each way, at most.
pub(crate) const DEPTH: u32 = 48;

/// A box that is more than this part of its parent's has stopped shrinking. A box that follows its part is at most
/// 0.8 of its parent's when the part is halved across its wider side, as `sqrt(1.25 / 2)` is for a square.
const STALLED: f64 = 0.9;

/// A part of one surface's parameters and what holds the surface over it.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Piece {
    /// The part.
    pub patch: UvBox,
    /// A box that holds the surface over the part.
    pub bounds: Bounds,
    /// How large the part is: its box's diagonal, or where the box has stopped shrinking, twice the furthest a corner's
    /// point is from its middle's.
    pub size: f64,
    /// Whether the part is wider across its first parameter than its second, at its middle.
    pub wide_u: bool,
    /// How many times its surface's domain was halved to make it.
    pub depth: u32,
}

/// The length of a box's diagonal: not finite for a box that is not.
pub(crate) fn diagonal(bounds: &Bounds) -> f64 {
    (bounds.high().to_vec3() - bounds.low().to_vec3()).norm()
}

/// The middle of a part.
pub(crate) fn middle(patch: UvBox) -> UvPoint {
    let half = |lo: f64, hi: f64| lo * 0.5 + hi * 0.5;
    UvPoint::new(half(patch.lo.u(), patch.hi.u()), half(patch.lo.v(), patch.hi.v()))
}

/// Twice the furthest a corner of `patch` is from its middle, on `surface`.
fn spread(surface: &dyn Surface, patch: UvBox) -> f64 {
    let at = surface.point(middle(patch)).to_vec3();
    let far = |most: f64, corner: UvPoint| {
        let gap = (surface.point(corner).to_vec3() - at).norm();
        if gap > most { gap } else { most }
    };
    2.0 * patch.corners().into_iter().fold(0.0, far)
}

/// Whether `patch` is wider on `surface` across its first parameter than its second, at its middle.
fn wide_u(surface: &dyn Surface, patch: UvBox) -> bool {
    let jet = surface.first(middle(patch));
    let across = |rate: Vec3, lo: f64, hi: f64| rate.norm() * (hi - lo);
    across(jet.du.rate(), patch.lo.u(), patch.hi.u()) >= across(jet.dv.rate(), patch.lo.v(), patch.hi.v())
}

impl Piece {
    /// The piece held by `surface`'s own box over `patch`, its size measured from points where the box has stopped
    /// shrinking against `parent`'s size.
    fn of_bounds(surface: &dyn Surface, patch: UvBox, parent: Option<f64>) -> Self {
        let bounds = surface.bounds(patch);
        let own = diagonal(&bounds);
        let shrank = parent.is_none_or(|parent| own <= STALLED * parent);
        let size = if shrank { own } else { spread(surface, patch) };
        Self { patch, bounds, size, wide_u: wide_u(surface, patch), depth: 0 }
    }

    /// The piece of the whole of `surface`'s domain, `patch`.
    pub(crate) fn whole(surface: &dyn Surface, patch: UvBox) -> Self {
        Self::of_bounds(surface, patch, None)
    }

    /// The part `patch` of this piece, on `surface`, one halving deeper.
    pub(crate) fn part(&self, surface: &dyn Surface, patch: UvBox) -> Self {
        let part = Self::of_bounds(surface, patch, Some(diagonal(&self.bounds)));
        Self { depth: self.depth.saturating_add(1), ..part }
    }

    /// The two halves of the piece, across its wider side where both can be halved: `None` where neither can, or
    /// where it is [`DEPTH`] deep.
    pub(crate) fn halves(&self) -> Option<[UvBox; 2]> {
        if self.depth >= DEPTH {
            return None;
        }
        let (lo, hi, mid) = (self.patch.lo, self.patch.hi, middle(self.patch));
        let can_u = lo.u() < mid.u() && mid.u() < hi.u();
        let can_v = lo.v() < mid.v() && mid.v() < hi.v();
        if can_u && (self.wide_u || !can_v) {
            Some([UvBox::new(lo, UvPoint::new(mid.u(), hi.v())), UvBox::new(UvPoint::new(mid.u(), lo.v()), hi)])
        } else if can_v {
            Some([UvBox::new(lo, UvPoint::new(hi.u(), mid.v())), UvBox::new(UvPoint::new(lo.u(), mid.v()), hi)])
        } else {
            None
        }
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::PI;

    use occam_geom::Cylinder;
    use occam_units::Point;

    use super::*;
    use crate::trace_tests::{level, mm, moved};

    fn patch(u: (f64, f64), v: (f64, f64)) -> UvBox {
        UvBox::new(UvPoint::new(u.0, v.0), UvPoint::new(u.1, v.1))
    }

    /// The piece over `patch` that is wider across its first parameter where `wide_u`, its box and size made up.
    fn piece(patch: UvBox, wide_u: bool) -> Piece {
        let bounds = Bounds::of([Point::from_vec3(Vec3::new(0.0, 0.0, 0.0))]).unwrap();
        Piece { patch, bounds, size: 1.0, wide_u, depth: 0 }
    }

    #[test]
    fn a_boxs_diagonal_and_a_parts_spread_are_measured() {
        let corner = |x, y, z| Point::from_vec3(Vec3::new(x, y, z));
        assert_eq!(diagonal(&Bounds::of([corner(1.0, 2.0, 3.0), corner(4.0, 6.0, 15.0)]).unwrap()), 13.0);
        // On the plane z = 0, whose parameters are x and y, every corner of this part is sqrt(5) from its middle.
        let spread = spread(&level(0.0), patch((2.0, 4.0), (1.0, 5.0)));
        assert!((spread - 2.0 * 5.0_f64.sqrt()).abs() < 1e-12, "{spread}");
    }

    #[test]
    fn a_part_is_wider_across_the_parameter_its_surface_moves_further_along() {
        // On a cylinder of radius 10, a turn of 0.05 moves 0.5 and a rise of 0.9 moves 0.9; a turn of 0.2 moves 2.
        let cylinder = Cylinder::placed(&moved([0.0; 3]), mm(10.0)).unwrap();
        assert!(!wide_u(&cylinder, patch((1.0, 1.05), (2.0, 2.9))));
        assert!(wide_u(&cylinder, patch((1.0, 1.2), (2.0, 2.9))));
    }

    #[test]
    fn a_parts_size_is_its_boxs_diagonal_until_the_box_stops_shrinking() {
        // A half turn of a cylinder of radius 2: its box is 4 by 2, while its corners are 2.83 from its middle.
        let cylinder = Cylinder::placed(&moved([0.0; 3]), mm(2.0)).unwrap();
        let half = patch((0.0, PI), (0.0, 0.1));
        let own = diagonal(&cylinder.bounds(half));
        let spread = spread(&cylinder, half);
        assert!((own - spread).abs() > 1.0, "{own} {spread}");
        assert_eq!(Piece::of_bounds(&cylinder, half, None).size, own);
        assert_eq!(Piece::of_bounds(&cylinder, half, Some(2.0 * own)).size, own, "it shrank");
        assert_eq!(Piece::of_bounds(&cylinder, half, Some(own)).size, spread, "it did not");
        let whole = Piece::whole(&cylinder, half);
        assert_eq!((whole.depth, whole.part(&cylinder, patch((0.0, 1.0), (0.0, 0.1))).depth), (0, 1));
    }

    #[test]
    fn a_part_is_halved_across_its_wider_side_where_it_can_be() {
        let (u, v) = ((0.0, 2.0), (0.0, 4.0));
        let across_u = [patch((0.0, 1.0), v), patch((1.0, 2.0), v)];
        let across_v = [patch(u, (0.0, 2.0)), patch(u, (2.0, 4.0))];
        assert_eq!(piece(patch(u, v), true).halves(), Some(across_u));
        assert_eq!(piece(patch(u, v), false).halves(), Some(across_v));
        // A range one double wide has its middle at one end: at its low end, and with these, at its high end.
        let (above, below) = ((1.0, 1.0_f64.next_up()), (1.0_f64.next_down(), 1.0));
        assert_eq!((middle(patch(above, above)).u(), middle(patch(below, below)).u()), (1.0, 1.0));
        for narrow in [above, below] {
            let halves = piece(patch(narrow, v), true).halves().unwrap();
            assert_eq!(halves.map(|h| (h.lo.v(), h.hi.v())), [(0.0, 2.0), (2.0, 4.0)], "across v: {narrow:?}");
            let halves = piece(patch(u, narrow), false).halves().unwrap();
            assert_eq!(halves.map(|h| (h.lo.u(), h.hi.u())), [(0.0, 1.0), (1.0, 2.0)], "across u: {narrow:?}");
            assert_eq!(piece(patch(narrow, narrow), true).halves(), None);
        }
        let deep = Piece { depth: DEPTH, ..piece(patch(u, v), true) };
        assert_eq!(deep.halves(), None);
    }
}
