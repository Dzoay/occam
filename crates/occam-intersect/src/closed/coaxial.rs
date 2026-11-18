//! Source: original
//! Surfaces turned about one axis: a plane across it, a sphere centred on it, and a cylinder, a cone or a torus about
//! it. Each is its meridians, what it cuts from a plane through the axis, written `(ρ, ζ)` across the axis and along it
//! and taken on both sides of it, so that a cone is its two lines and a torus its two circles. Where a meridian of one
//! meets a meridian of the other at `ρ > 0`, the surfaces meet in the circle that point turns through; where they meet
//! on the axis, in a point; and where two are one curve, the surfaces are one (O25).

use occam_geom::Analytic;
use occam_math::{Interval, Side, sin_cos};
use occam_units::Vec3;

use super::meet::{Flat, Meeting, Shape, meet};
use super::piece::Plan;
use super::{Found, Touch};
use crate::decide::{Decisions, Undecided};
use crate::interval3::{along, exact3};
use crate::resolution::{MARGIN, hidden};

/// An axis: a point on it and its unit direction.
#[derive(Clone, Copy, Debug)]
pub(super) struct Axis {
    pub(super) at: Vec3,
    pub(super) along: Vec3,
}

impl Axis {
    /// How far along the axis `p` is from its point.
    fn height(&self, p: Vec3) -> Interval {
        along(exact3(p), exact3(self.at), exact3(self.along))
    }
}

/// An interval holding `x`, a value the owned `sin_cos` computed within an ulp.
fn computed(x: f64) -> Interval {
    Interval::new(x, 4.0 * f64::EPSILON * x.abs())
}

/// The meridians of `form` about `axis`, which it is turned about.
fn meridians(form: Analytic, axis: &Axis) -> Vec<Shape> {
    let up = Flat::exact(0.0, 1.0);
    let line = |at: Flat, along: Flat| Shape::Line { at, along };
    match form {
        Analytic::Plane { origin, .. } => {
            vec![line(Flat::new(Interval::exact(0.0), axis.height(origin.to_vec3())), Flat::exact(1.0, 0.0))]
        }
        Analytic::Sphere { centre, radius } => {
            let centre = Flat::new(Interval::exact(0.0), axis.height(centre.to_vec3()));
            vec![Shape::Circle { centre, radius: Interval::exact(radius.millimetres()) }]
        }
        Analytic::Cylinder { radius, .. } => {
            let r = radius.millimetres();
            vec![line(Flat::exact(r, 0.0), up), line(Flat::exact(-r, 0.0), up)]
        }
        Analytic::Cone { apex, half_angle, .. } => {
            let apex = Flat::new(Interval::exact(0.0), axis.height(apex.to_vec3()));
            let (sin, cos) = sin_cos(half_angle.radians());
            let (sin, cos) = (computed(sin), computed(cos));
            vec![line(apex, Flat::new(sin, cos)), line(apex, Flat::new(-sin, cos))]
        }
        Analytic::Torus { centre, major, minor, .. } => {
            let (height, r) = (axis.height(centre.to_vec3()), Interval::exact(minor.millimetres()));
            let side = |x: f64| Shape::Circle { centre: Flat::new(Interval::exact(x), height), radius: r };
            vec![side(major.millimetres()), side(-major.millimetres())]
        }
    }
}

/// What a point where two meridians meet gives.
enum Turned {
    /// A circle about the axis at this height and radius.
    Circle(f64, f64),
    /// A point of the axis at this height.
    OnAxis(Interval),
    /// Nothing: it is the mirror, across the axis, of a point that gives one.
    Mirror,
}

/// How a meridian bends at `point` of it on the axis, towards greater heights: a line across the axis not at all, and
/// a circle centred on the axis by its curvature, towards its centre.
fn bend(shape: &Shape, point: Flat) -> f64 {
    match shape {
        Shape::Line { .. } => 0.0,
        Shape::Circle { centre, radius } => {
            let r = radius.mid();
            (centre.y.mid() - point.y.mid()) / (r * r)
        }
    }
}

/// How far from a point of the axis where `one` and `other` meet what the band hides there reaches. Where they touch,
/// both surfaces are as round there as their meridians, about the axis, and the gap between them is
/// `g₀ + ½ κ ρ²` at a distance `ρ` from it, `κ` the difference of the meridians' bends: twice `d = √(2 band / κ)`, as
/// [`resolution`](crate::resolution) derives. Where they cross, as a cone's side crosses the axis at its apex, it is no
/// tangency, and nought.
fn reach(one: &Shape, other: &Shape, (point, touch): (Flat, bool), band: f64) -> f64 {
    if !touch {
        return 0.0;
    }
    MARGIN * hidden(band, (bend(one, point) - bend(other, point)).abs())
}

fn turned(point: Flat, decide: &mut Decisions) -> Result<Turned, Undecided> {
    Ok(match decide.length(point.x)? {
        Side::Above => Turned::Circle(point.y.mid(), point.x.mid()),
        Side::Within => Turned::OnAxis(point.y),
        Side::Below => Turned::Mirror,
    })
}

/// The section of `a` and `b`, both turned about `axis`, or [`Undecided`] where a decision could not be made.
pub(super) fn coaxial(a: Analytic, b: Analytic, axis: &Axis, decide: &mut Decisions) -> Result<Found, Undecided> {
    let mut found = Found::default();
    let mut heights: Vec<Interval> = Vec::new();
    for one in meridians(a, axis) {
        for other in meridians(b, axis) {
            let points = match meet(&one, &other, decide)? {
                Meeting::Apart => continue,
                Meeting::Same => return Ok(Found::coincident()),
                Meeting::At(points) => points,
            };
            for (point, touch) in points {
                match turned(point, decide)? {
                    Turned::Circle(height, radius) => {
                        let centre = axis.at + axis.along * height;
                        found.curves.push(Plan::Circle { centre, normal: axis.along, radius });
                    }
                    Turned::OnAxis(height) => {
                        if !seen(&heights, height, decide)? {
                            heights.push(height);
                            let bound = reach(&one, &other, (point, touch), decide.band());
                            found.contacts.push(Touch { at: axis.at + axis.along * height.mid(), bound });
                        }
                    }
                    Turned::Mirror => {}
                }
            }
        }
    }
    Ok(found)
}

/// Whether a point of the axis at `height` is one already found, as a cone's two lines both find its apex.
fn seen(heights: &[Interval], height: Interval, decide: &mut Decisions) -> Result<bool, Undecided> {
    for other in heights {
        if decide.length(*other - height)? == Side::Within {
            return Ok(true);
        }
    }
    Ok(false)
}
