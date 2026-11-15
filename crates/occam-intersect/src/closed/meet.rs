//! Source: original
//! Where two lines or circles of one plane meet: nowhere, at a touch, at two crossings, or everywhere, as the same
//! curve. Their numbers are intervals carrying their certified radii, and every choice among those is a decision
//! made through [`Decide`]. A meridian of a surface turned about an axis, and a cylinder's cross-section, are these.

use occam_math::{Interval, Side};

use super::decide::{Decide, Undecided};

/// A point of the plane.
#[derive(Clone, Copy, Debug)]
pub(super) struct Flat {
    pub(super) x: Interval,
    pub(super) y: Interval,
}

impl Flat {
    pub(super) const fn new(x: Interval, y: Interval) -> Self {
        Self { x, y }
    }

    /// The point at exactly `(x, y)`.
    pub(super) fn exact(x: f64, y: f64) -> Self {
        Self::new(Interval::exact(x), Interval::exact(y))
    }

    fn plus(self, other: Self, by: Interval) -> Self {
        Self::new(self.x + other.x * by, self.y + other.y * by)
    }

    fn minus(self, other: Self) -> Self {
        Self::new(self.x - other.x, self.y - other.y)
    }

    fn dot(self, other: Self) -> Interval {
        self.x * other.x + self.y * other.y
    }

    /// The cross product's one part, positive where `other` is anticlockwise of `self`.
    fn cross(self, other: Self) -> Interval {
        self.x * other.y - self.y * other.x
    }
}

/// A line or a circle of the plane.
#[derive(Clone, Copy, Debug)]
pub(super) enum Shape {
    /// The line through `at` along the unit `along`.
    Line { at: Flat, along: Flat },
    /// The circle about `centre` of `radius`.
    Circle { centre: Flat, radius: Interval },
}

/// How two shapes meet.
pub(super) enum Meeting {
    /// Nowhere.
    Apart,
    /// At these points, each a touch, where the two are tangent, or a crossing.
    At(Vec<(Flat, bool)>),
    /// Everywhere: they are one curve.
    Same,
}

/// How `a` and `b` meet, or [`Undecided`] where a decision could not be made.
pub(super) fn meet(a: &Shape, b: &Shape, decide: &mut Decide) -> Result<Meeting, Undecided> {
    match (*a, *b) {
        (Shape::Line { at, along }, Shape::Line { at: other, along: way }) => lines(at, along, other, way, decide),
        (Shape::Line { at, along }, Shape::Circle { centre, radius })
        | (Shape::Circle { centre, radius }, Shape::Line { at, along }) => {
            line_circle(at, along, centre, radius, decide)
        }
        (Shape::Circle { centre, radius }, Shape::Circle { centre: other, radius: size }) => {
            circles((centre, radius), (other, size), decide)
        }
    }
}

fn lines(at: Flat, along: Flat, other: Flat, way: Flat, decide: &mut Decide) -> Result<Meeting, Undecided> {
    let turn = along.cross(way);
    if decide.angle(turn)? == Side::Within {
        let gap = along.cross(other.minus(at));
        return Ok(if decide.length(gap)? == Side::Within { Meeting::Same } else { Meeting::Apart });
    }
    let by = other.minus(at).cross(way) / turn;
    Ok(Meeting::At(vec![(at.plus(along, by), false)]))
}

fn line_circle(
    at: Flat,
    along: Flat,
    centre: Flat,
    radius: Interval,
    decide: &mut Decide,
) -> Result<Meeting, Undecided> {
    let offset = centre.minus(at);
    let across = along.cross(offset);
    let foot = at.plus(along, along.dot(offset));
    let touch = Ok(Meeting::At(vec![(foot, true)]));
    match decide.length(across - radius)? {
        Side::Above => return Ok(Meeting::Apart),
        Side::Within => return touch,
        Side::Below => {}
    }
    match decide.length(across + radius)? {
        Side::Below => Ok(Meeting::Apart),
        Side::Within => touch,
        Side::Above => {
            let half = ((radius - across) * (radius + across)).sqrt();
            Ok(Meeting::At(vec![(foot.plus(along, -half), false), (foot.plus(along, half), false)]))
        }
    }
}

/// `x`, or its negation where its middle is below zero.
fn magnitude(x: Interval) -> Interval {
    if x.mid() < 0.0 { -x } else { x }
}

fn circles(
    (centre, radius): (Flat, Interval),
    (other, size): (Flat, Interval),
    decide: &mut Decide,
) -> Result<Meeting, Undecided> {
    let apart = other.minus(centre);
    let distance = apart.dot(apart).sqrt();
    if decide.length(distance)? == Side::Within {
        return Ok(if decide.length(radius - size)? == Side::Within { Meeting::Same } else { Meeting::Apart });
    }
    let toward = Flat::new(apart.x / distance, apart.y / distance);
    match decide.length(distance - (radius + size))? {
        Side::Above => return Ok(Meeting::Apart),
        Side::Within => return Ok(Meeting::At(vec![(centre.plus(toward, radius), true)])),
        Side::Below => {}
    }
    match decide.length(distance - magnitude(radius - size))? {
        Side::Below => Ok(Meeting::Apart),
        Side::Within if radius.mid() < size.mid() => Ok(Meeting::At(vec![(other.plus(toward, -size), true)])),
        Side::Within => Ok(Meeting::At(vec![(centre.plus(toward, radius), true)])),
        Side::Above => {
            let along = (distance * distance + radius * radius - size * size) / (Interval::exact(2.0) * distance);
            let half = ((radius - along) * (radius + along)).sqrt();
            let (base, normal) = (centre.plus(toward, along), Flat::new(-toward.y, toward.x));
            Ok(Meeting::At(vec![(base.plus(normal, -half), false), (base.plus(normal, half), false)]))
        }
    }
}
