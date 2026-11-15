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
