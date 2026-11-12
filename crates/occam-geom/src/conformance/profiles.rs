//! Source: original
//! Curves of every family to sweep, each placed skew to [`placement`]'s Z axis and away from it, and a comparison of two
//! surfaces over a window, for the swept families' tests.

use occam_units::{Angle, Direction, Length, Local, Placement, Point, UvPoint, Vec3, Vector, World};

use super::{distance, local, placement, tolerance, unit};
use crate::{BsplineCurve, Circle, Ellipse, Hyperbola, Line, Parabola, SplineCurve, Surface, UvBox};

/// A frame `offset` from [`placement`]'s origin, in its terms, and turned by `angle` about `about` first.
fn frame(offset: [f64; 3], about: [f64; 3], angle: f64) -> Placement<Local, World> {
    let [ox, oy, oz] = offset;
    let [ax, ay, az] = about;
    let axis = Direction::normalised(Vec3::new(ax, ay, az));
    let turn = Placement::<Local, Local>::rotation(axis, Angle::from_radians(angle));
    let shift = Placement::<Local, Local>::translation(Vector::from_vec3(Vec3::new(ox, oy, oz)));
    turn.then(&shift).then(&placement())
}

fn mm(x: f64) -> Length {
    Length::from_millimetres(x)
}

/// A line without end, skew to the axis.
pub(crate) fn endless() -> Line {
    let along = placement().direction(Direction::normalised(Vec3::new(0.2, 0.4, 1.0)));
    Line::new(Point::from_vec3(local(7.0, 1.0, -2.0)), along)
}

/// A segment of [`endless`].
pub(crate) fn segment() -> Line {
    endless().trimmed(-3.0, 5.0).unwrap()
}

/// A whole circle, in a tilted plane.
pub(crate) fn ring() -> Circle {
    Circle::placed(&frame([8.0, 0.0, 1.0], [1.0, 1.0, 0.0], 0.6), mm(2.5)).unwrap()
}

/// An arc of [`ring`].
pub(crate) fn arc() -> Circle {
    ring().trimmed(0.5, 4.0).unwrap()
}

/// An arc of an ellipse, tilted.
pub(crate) fn oval() -> Ellipse {
    let whole = Ellipse::placed(&frame([9.0, 1.0, 0.0], [0.0, 1.0, 1.0], 0.4), mm(3.0), mm(1.5)).unwrap();
    whole.trimmed(-1.0, 2.0).unwrap()
}

/// An arc of an ellipse in a plane a little off the axis's direction, so that turned about the axis it does not pass
/// through itself, as [`oval`] does.
pub(crate) fn upright_oval() -> Ellipse {
    let whole = Ellipse::placed(&frame([9.0, 1.0, 0.0], [1.0, 0.0, 0.0], 1.3), mm(3.0), mm(1.5)).unwrap();
    whole.trimmed(-1.0, 2.0).unwrap()
}

/// A hyperbola's branch, without end.
pub(crate) fn whole_branch() -> Hyperbola {
    Hyperbola::placed(&frame([6.0, 0.0, 0.0], [1.0, 0.0, 1.0], 0.3), mm(1.0), mm(1.5)).unwrap()
}

/// A part of [`whole_branch`].
pub(crate) fn branch() -> Hyperbola {
    whole_branch().trimmed(-1.2, 1.0).unwrap()
}

/// A parabola, without end.
pub(crate) fn whole_bow() -> Parabola {
    Parabola::placed(&frame([7.0, -1.0, 2.0], [0.0, 0.0, 1.0], 0.9), mm(1.5)).unwrap()
}

/// A parabola without end in a plane tilted to the axis, so that turned about it, unlike [`whole_bow`], whose plane is
/// square to the axis, it does not lie over itself.
pub(crate) fn lifted_bow() -> Parabola {
    Parabola::placed(&frame([7.0, -1.0, 2.0], [1.0, 0.0, 0.2], 0.8), mm(1.5)).unwrap()
}

/// A part of [`whole_bow`].
pub(crate) fn bow() -> Parabola {
    whole_bow().trimmed(-2.0, 2.5).unwrap()
}

/// A rational cubic B-spline that twists.
pub(crate) fn twist() -> SplineCurve {
    let poles = [[9.0, 0.0, 0.0], [10.0, 2.0, 1.0], [8.0, 3.0, 2.5], [7.0, 1.0, 4.0], [9.0, -1.0, 5.0]];
    let poles = poles.into_iter().map(|[x, y, z]| Point::from_vec3(local(x, y, z))).collect();
    let knots = vec![0.0, 0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0, 1.0];
    SplineCurve::new(&BsplineCurve { degree: 3, knots, poles, weights: Some(vec![1.0, 0.7, 1.5, 1.0, 0.8]) }).unwrap()
}

/// Where `a` and `b` first differ over `window`, sampled 7 by 7 with its edges: a point further apart than the band, or
/// a unit normal more than 10^-9 from the other's. `None` where they agree.
pub(crate) fn differ(a: &dyn Surface, b: &dyn Surface, window: UvBox) -> Option<String> {
    let band = tolerance().band().millimetres();
    let normal = |s: &dyn Surface, uv: UvPoint| {
        let jet = s.first(uv);
        unit(jet.du.rate().cross(jet.dv.rate()))
    };
    let at = |f: f64, lo: f64, hi: f64| lo + (hi - lo) * f;
    (0..=6).flat_map(|i| (0..=6).map(move |j| (f64::from(i) / 6.0, f64::from(j) / 6.0))).find_map(|(fu, fv)| {
        let uv = UvPoint::new(at(fu, window.lo.u(), window.hi.u()), at(fv, window.lo.v(), window.hi.v()));
        let (pa, pb, na, nb) = (a.point(uv), b.point(uv), normal(a, uv), normal(b, uv));
        let same = distance(pa, pb) <= band && (na - nb).norm() <= 1e-9;
        (!same).then(|| format!("at {uv:?}: {pa:?} and {pb:?}, facing {na:?} and {nb:?}"))
    })
}
