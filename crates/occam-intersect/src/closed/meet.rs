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

#[cfg(test)]
mod tests {
    use occam_units::{Angle, Length, Tolerance};

    use super::*;

    fn decide() -> Decide {
        Decide::new(Tolerance::new(Length::from_millimetres(1e-9), Angle::from_radians(1e-9)).unwrap())
    }

    fn circle(x: f64, y: f64, r: f64) -> Shape {
        Shape::Circle { centre: Flat::exact(x, y), radius: Interval::exact(r) }
    }

    fn line(at: (f64, f64), along: (f64, f64)) -> Shape {
        let norm = (along.0 * along.0 + along.1 * along.1).sqrt();
        Shape::Line { at: Flat::exact(at.0, at.1), along: Flat::exact(along.0 / norm, along.1 / norm) }
    }

    /// What `points` gives where the shapes do not meet.
    const APART: Vec<((f64, f64), bool)> = Vec::new();

    /// The points where `a` and `b` meet, each a point and whether it is a touch.
    fn points(a: &Shape, b: &Shape) -> Vec<((f64, f64), bool)> {
        match meet(a, b, &mut decide()).unwrap() {
            Meeting::At(found) => found.into_iter().map(|(p, touch)| ((p.x.mid(), p.y.mid()), touch)).collect(),
            Meeting::Apart => Vec::new(),
            Meeting::Same => vec![((f64::NAN, f64::NAN), true)],
        }
    }

    fn near(got: (f64, f64), want: (f64, f64)) -> bool {
        (got.0 - want.0).abs() + (got.1 - want.1).abs() <= 1e-12
    }

    #[test]
    fn circles_off_the_origin_meet_where_both_are_the_lesser_inside_touching_on_its_far_side() {
        // The lesser circle first, inside the greater and touching it: the touch is on the far side of the lesser.
        let found = points(&circle(1.0, 2.0, 0.5), &circle(1.5, 2.0, 1.0));
        assert_eq!(found.len(), 1);
        assert!(near(found[0].0, (0.5, 2.0)) && found[0].1, "{found:?}");
        // The greater first, the same touch.
        let found = points(&circle(1.5, 2.0, 1.0), &circle(1.0, 2.0, 0.5));
        assert!(found.len() == 1 && near(found[0].0, (0.5, 2.0)), "{found:?}");
        // The lesser well inside: no meeting, either way round.
        assert_eq!(points(&circle(1.0, 2.0, 0.3), &circle(1.2, 2.0, 1.0)), APART);
        assert_eq!(points(&circle(1.2, 2.0, 1.0), &circle(1.0, 2.0, 0.3)), APART);
        // Crossing, off the origin: (1, 2) and (2, 2), both of radius 1, cross at x = 1.5.
        let found = points(&circle(1.0, 2.0, 1.0), &circle(2.0, 2.0, 1.0));
        let half = 0.75_f64.sqrt();
        assert!(
            found.len() == 2
                && found
                    .iter()
                    .all(|(p, touch)| !touch && (near(*p, (1.5, 2.0 - half)) || near(*p, (1.5, 2.0 + half)))),
            "{found:?}"
        );
    }

    #[test]
    fn lines_and_circles_off_the_origin_meet_where_both_are() {
        // The line y = 3 against the circle about (1, 2) of radius 1: a touch at (1, 3); y = 2.5 crosses it twice.
        let touch = points(&line((-4.0, 3.0), (1.0, 0.0)), &circle(1.0, 2.0, 1.0));
        assert!(touch.len() == 1 && near(touch[0].0, (1.0, 3.0)) && touch[0].1, "{touch:?}");
        let cross = points(&line((-4.0, 2.5), (1.0, 0.0)), &circle(1.0, 2.0, 1.0));
        let half = 0.75_f64.sqrt();
        assert!(cross.len() == 2 && cross.iter().any(|(p, _)| near(*p, (1.0 + half, 2.5))), "{cross:?}");
        // Two lines off the origin: through (1, 2) along x, and through (3, -1) along y, meet at (3, 2).
        let found = points(&line((1.0, 2.0), (1.0, 0.0)), &line((3.0, -1.0), (0.0, 1.0)));
        assert!(found.len() == 1 && near(found[0].0, (3.0, 2.0)), "{found:?}");
        // Parallel and apart, or the same line.
        assert_eq!(points(&line((1.0, 2.0), (1.0, 1.0)), &line((1.0, 3.0), (1.0, 1.0))), APART);
        let same = |a: &Shape, b: &Shape| matches!(meet(a, b, &mut decide()), Ok(Meeting::Same));
        assert!(same(&line((1.0, 2.0), (1.0, 1.0)), &line((3.0, 4.0), (1.0, 1.0))), "a line is itself");
        assert!(same(&circle(1.0, 2.0, 0.7), &circle(1.0, 2.0, 0.7)), "a circle is itself");
    }
}
