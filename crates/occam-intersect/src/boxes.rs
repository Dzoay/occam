//! Source: original
//! The boxes hits are decided over: a curve's over part of its span and a surface's over part of its patch. Each is the
//! family's own box, and where that has stopped shrinking as its piece is halved, as a B-spline's does inside a knot
//! span, where it holds the hull of its poles, it is cut down to the hull of the piece's own B-spline form. Both boxes
//! hold the piece, so what they share does.

use occam_geom::{Curve, Surface, UvBox};
use occam_units::{Bounds, Length, Point, Vec3, World};

/// The length of a box's diagonal: NaN or infinity for a box that is not finite.
pub(crate) fn size(bounds: &Bounds) -> f64 {
    (bounds.high().to_vec3() - bounds.low().to_vec3()).norm()
}

/// Whether a piece's box is at most three quarters of its parent's, as a box that follows its piece is when the piece
/// is halved.
fn shrank(own: &Bounds, parent: Option<&Bounds>) -> bool {
    parent.is_none_or(|parent| size(own) <= 0.75 * size(parent))
}

/// The box of `poles`, weighed by positive `weights` if any, padded for the rounding of the sums of `terms` terms that
/// evaluate a point of it: 64 machine epsilons of its largest coordinate for each. `None` for a weight that is not
/// positive, which voids the hull.
fn hull(poles: &[Point<World>], weights: Option<&[f64]>, terms: u16) -> Option<Bounds> {
    if weights.is_some_and(|w| w.iter().any(|w| !(*w > 0.0 && w.is_finite()))) {
        return None;
    }
    let found = Bounds::of(poles.iter().copied())?;
    let (low, high) = (found.low(), found.high());
    let corners = [low.to_vec3(), high.to_vec3()];
    let largest =
        corners.iter().flat_map(|c| [c.x, c.y, c.z]).fold(0.0, |a: f64, x| if x.abs() > a { x.abs() } else { a });
    let margin = Length::from_millimetres(64.0 * f64::from(terms) * f64::EPSILON * largest);
    Some(Bounds::around(low, margin).union(&Bounds::around(high, margin)))
}

/// What both boxes hold, or the first where they share nothing, which two boxes of one piece cannot do.
fn meet(a: &Bounds, b: &Bounds) -> Bounds {
    let pick = |x: f64, y: f64, larger: bool| if (y > x) == larger { y } else { x };
    let each =
        |p: Vec3, q: Vec3, larger| Vec3::new(pick(p.x, q.x, larger), pick(p.y, q.y, larger), pick(p.z, q.z, larger));
    let low = each(a.low().to_vec3(), b.low().to_vec3(), true);
    let high = each(a.high().to_vec3(), b.high().to_vec3(), false);
    if low.x <= high.x && low.y <= high.y && low.z <= high.z {
        Bounds::of([Point::from_vec3(low), Point::from_vec3(high)]).unwrap_or(*a)
    } else {
        *a
    }
}

/// A box holding `curve` from `lo` to `hi`, the half of a piece whose box was `parent`.
pub(crate) fn curve(curve: &dyn Curve, lo: f64, hi: f64, parent: Option<&Bounds>) -> Bounds {
    let own = curve.bounds(lo, hi);
    if shrank(&own, parent) {
        return own;
    }
    let form = curve.to_bspline(lo, hi);
    let tight = form.and_then(|f| hull(&f.poles, f.weights.as_deref(), f.degree.saturating_add(2)));
    tight.map_or(own, |tight| meet(&own, &tight))
}

/// A box holding `surface` over `patch`, a part of a patch whose box was `parent`.
pub(crate) fn surface(surface: &dyn Surface, patch: UvBox, parent: Option<&Bounds>) -> Bounds {
    let own = surface.bounds(patch);
    if shrank(&own, parent) {
        return own;
    }
    let form = surface.to_bspline(patch);
    let terms = |f: &occam_geom::BsplineSurface| f.degree.0.saturating_add(f.degree.1).saturating_add(2);
    let tight = form.and_then(|f| hull(&f.poles, f.weights.as_deref(), terms(&f)));
    tight.map_or(own, |tight| meet(&own, &tight))
}
