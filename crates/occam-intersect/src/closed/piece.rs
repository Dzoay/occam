//! Source: original
//! The curves the closed forms find: each made in one of occam-geom's exact families from exact numbers, a circle cut
//! where it passes through a sphere's pole, and each given its image on both surfaces through their charts, in
//! occam-geom's image family. On a cone, a ruling's image is a line of constant `u`, which passes through the apex
//! where the angle about the axis is undefined.

use std::f64::consts::TAU;
use std::sync::Arc;

use occam_geom::{
    Chart, Circle, Curve, Ellipse, Hyperbola, Image, Line, Parabola, ParamCurve, ParametricCurve, UvCurve,
};
use occam_math::{Side, atan2};
use occam_units::{Direction, Point, Vec3};

use super::{exact3, minus, norm3, unit};
use crate::decide::{Decisions, Undecided};

/// A curve a closed form found, before it is made.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Plan {
    /// The line through `at` along `along`.
    Line { at: Vec3, along: Vec3 },
    /// The line through a cone's `apex` along `along`, its parameter the distance from the apex.
    Ruling { apex: Vec3, along: Vec3 },
    /// The circle about `centre` facing `normal`.
    Circle { centre: Vec3, normal: Vec3, radius: f64 },
    /// `centre + a cos t x + b sin t y`.
    Ellipse { centre: Vec3, x: Vec3, y: Vec3, radii: (f64, f64) },
    /// `vertex + t²/4f x + t y`.
    Parabola { vertex: Vec3, x: Vec3, y: Vec3, focal: f64 },
    /// `centre + a cosh t x + b sinh t y`.
    Hyperbola { centre: Vec3, x: Vec3, y: Vec3, radii: (f64, f64) },
}

/// A closed conic's centre, unit axes and radii along them, where it is cut.
#[derive(Clone, Copy, Debug)]
struct Ring {
    centre: Vec3,
    x: Vec3,
    y: Vec3,
    radii: (f64, f64),
}

impl Ring {
    /// The angle of `p`, a point of the conic's plane, as the conic's parameter: the eccentric angle of an ellipse.
    fn angle(&self, p: Vec3) -> f64 {
        let q = p - self.centre;
        atan2(q.dot(self.y) / self.radii.1, q.dot(self.x) / self.radii.0)
    }

    /// The conic's point at `t`.
    fn at(&self, t: f64) -> Vec3 {
        let (sin, cos) = occam_math::sin_cos(t);
        self.centre + self.x * (self.radii.0 * cos) + self.y * (self.radii.1 * sin)
    }
}

/// A curve made, with what its images need.
enum Piece {
    Line(Line),
    Ruling(Line, Vec3),
    /// A circle or an arc of one, and whether it was cut at a pole of each surface of the pair.
    Circle(Circle, [bool; 2]),
    Ellipse(Ellipse),
    Parabola(Parabola),
    Hyperbola(Hyperbola),
}

/// A unit axis square to the unit `normal`: the world axis least along it, with its part along it taken away.
pub(crate) fn square_to(normal: Vec3) -> Vec3 {
    let world = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
    let least = world.into_iter().fold((world[0], f64::INFINITY), |best, axis| {
        let size = axis.dot(normal).abs();
        if size < best.1 { (axis, size) } else { best }
    });
    unit(least.0 - normal * least.0.dot(normal))
}

/// The numbers a conic's content holds after its family's name: its axes, `sizes`, and its whole range.
fn numbers(centre: Vec3, x: Vec3, y: Vec3, sizes: &[f64], closed: bool) -> Vec<f64> {
    let n = x.cross(y);
    let mut out = vec![centre.x, centre.y, centre.z, x.x, x.y, x.z, y.x, y.y, y.z, n.x, n.y, n.z];
    out.extend_from_slice(sizes);
    out.extend(if closed { [0.0, TAU, 2.0] } else { [f64::NEG_INFINITY, f64::INFINITY, 0.0] });
    out
}

fn made(plan: Plan) -> Option<Piece> {
    Some(match plan {
        Plan::Line { at, along } => Piece::Line(Line::new(Point::from_vec3(at), Direction::normalised(along))),
        Plan::Ruling { apex, along } => {
            Piece::Ruling(Line::new(Point::from_vec3(apex), Direction::normalised(along)), unit(along))
        }
        Plan::Circle { centre, normal, radius } => {
            let x = square_to(normal);
            let y = normal.cross(x);
            Piece::Circle(Circle::restore(&numbers(centre, x, y, &[radius], true)).ok()?, [false; 2])
        }
        Plan::Ellipse { centre, x, y, radii } => {
            Piece::Ellipse(Ellipse::restore(&numbers(centre, x, y, &[radii.0, radii.1], true)).ok()?)
        }
        Plan::Parabola { vertex, x, y, focal } => {
            Piece::Parabola(Parabola::restore(&numbers(vertex, x, y, &[focal], false)).ok()?)
        }
        Plan::Hyperbola { centre, x, y, radii } => {
            Piece::Hyperbola(Hyperbola::restore(&numbers(centre, x, y, &[radii.0, radii.1], false)).ok()?)
        }
    })
}

impl Plan {
    /// The curve it is made as, whole: `None` where that cannot be made of its numbers.
    pub(crate) fn curve(self) -> Option<Arc<dyn Curve>> {
        Some(made(self)?.curve())
    }
}

/// The conic `plan` is, where it is a closed one, as a ring.
fn ring_of(plan: Plan) -> Option<Ring> {
    match plan {
        Plan::Circle { centre, normal, radius } => {
            let x = square_to(normal);
            Some(Ring { centre, x, y: normal.cross(x), radii: (radius, radius) })
        }
        Plan::Ellipse { centre, x, y, radii } => Some(Ring { centre, x, y, radii }),
        _ => None,
    }
}

/// Whether `p` is on `ring`, within the band.
fn on_ring(ring: &Ring, p: Vec3, decide: &mut Decisions) -> Result<bool, Undecided> {
    let off = ring.at(ring.angle(p));
    Ok(decide.length(norm3(minus(exact3(p), exact3(off))))? == Side::Within)
}

/// Where a closed conic is cut: at the angle of each pole of either surface it passes through, with which surface's it
/// is, and of each point of `through` it passes through, as where it crosses another conic, each decided, in angle
/// order and each place once.
fn cuts(
    ring: &Ring,
    charts: [&Chart; 2],
    through: &[Vec3],
    decide: &mut Decisions,
) -> Result<Vec<(f64, Option<usize>, Vec3)>, Undecided> {
    let mut found: Vec<(f64, Option<usize>, Vec3)> = Vec::new();
    let poles = charts
        .iter()
        .enumerate()
        .flat_map(|(which, chart)| chart.poles().into_iter().map(move |p| (p.to_vec3(), Some(which))));
    for (p, which) in poles.chain(through.iter().map(|p| (*p, None))) {
        if !on_ring(ring, p, decide)? {
            continue;
        }
        let mut again = false;
        for (_, other, at) in &mut found {
            if decide.length(norm3(minus(exact3(p), exact3(*at))))? == Side::Within {
                *other = other.or(which);
                again = true;
            }
        }
        if !again {
            found.push((ring.angle(p), which, p));
        }
    }
    found.sort_by(|a, b| a.0.total_cmp(&b.0));
    Ok(found)
}

/// The curves `plan` is made as: one, or a closed conic's arcs between the places it is cut, the poles of either
/// surface it passes through and the points of `through`, each decided. `None` where a curve cannot be made of its
/// numbers.
pub(super) fn pieces(
    plan: Plan,
    charts: [&Chart; 2],
    through: &[Vec3],
    decide: &mut Decisions,
) -> Result<Option<Vec<Made>>, Undecided> {
    let Some(piece) = made(plan) else { return Ok(None) };
    let Some(ring) = ring_of(plan) else { return Ok(made_all(vec![piece], charts)) };
    let cuts = cuts(&ring, charts, through, decide)?;
    if cuts.is_empty() {
        return Ok(made_all(vec![piece], charts));
    }
    let mut arcs = Vec::with_capacity(cuts.len());
    let round = cuts.first().map(|(at, which, _)| (at + TAU, *which));
    let rest = cuts.iter().skip(1).map(|(at, which, _)| (*at, *which));
    for ((start, which, _), (end, other)) in cuts.iter().zip(rest.chain(round)) {
        let mut cut = [false; 2];
        for w in [*which, other].into_iter().flatten() {
            if let Some(slot) = cut.get_mut(w) {
                *slot = true;
            }
        }
        let arc = match &piece {
            Piece::Circle(circle, _) => circle.trimmed(*start, end).map(|arc| Piece::Circle(arc, cut)),
            Piece::Ellipse(ellipse) => ellipse.trimmed(*start, end).map(Piece::Ellipse),
            _ => return Ok(made_all(vec![piece], charts)),
        };
        let Ok(arc) = arc else { return Ok(None) };
        arcs.push(arc);
    }
    Ok(made_all(arcs, charts))
}

/// A curve made, with its images on the pair's two surfaces.
pub(super) struct Made {
    pub(super) curve: Arc<dyn Curve>,
    pub(super) on: [Arc<dyn ParamCurve>; 2],
}

fn made_all(pieces: Vec<Piece>, charts: [&Chart; 2]) -> Option<Vec<Made>> {
    pieces
        .into_iter()
        .map(|piece| {
            let on = [image(&piece, charts[0], 0)?, image(&piece, charts[1], 1)?];
            Some(Made { curve: piece.curve(), on })
        })
        .collect()
}

impl Piece {
    fn curve(&self) -> Arc<dyn Curve> {
        match self {
            Self::Line(c) | Self::Ruling(c, _) => Arc::new(*c),
            Self::Circle(c, ..) => Arc::new(*c),
            Self::Ellipse(c) => Arc::new(*c),
            Self::Parabola(c) => Arc::new(*c),
            Self::Hyperbola(c) => Arc::new(*c),
        }
    }
}

/// An open curve's image: beside the axis, toward its point at 0, its vertex.
fn open<C: ParametricCurve + Clone>(curve: C, chart: &Chart) -> Arc<dyn ParamCurve> {
    let toward = curve.eval_point(0.0);
    Arc::new(Image::beside(curve, *chart, toward))
}

/// A closed conic's image, or an arc's, as a conic; but an arc cut at one of this surface's poles keeps beside the
/// axis, toward its middle.
fn closed<C: ParametricCurve + Clone>(curve: C, chart: &Chart, cut: bool) -> Option<Arc<dyn ParamCurve>> {
    if cut {
        let range = curve.range();
        let middle = curve.eval_point(0.5 * range.lo + 0.5 * range.hi);
        return Some(Arc::new(Image::beside(curve, *chart, middle)));
    }
    Some(Arc::new(Image::conic(curve, *chart)?))
}

/// A ruling's image: on a cone, the line of the ruling's angle from the apex's parameters, `v` running as the
/// chart says; on any other surface, as any open curve's.
fn ruling(line: Line, along: Vec3, chart: &Chart) -> Option<Arc<dyn ParamCurve>> {
    let Some((apex, up)) = chart.ruling(Direction::normalised(along)) else { return Some(open(line, chart)) };
    let start = Point::from_vec3(Vec3::new(apex.u(), apex.v(), 0.0));
    let image = Line::new(start, Direction::normalised(Vec3::new(0.0, up, 0.0)));
    Some(Arc::new(UvCurve::new(image).ok()?))
}

fn image(piece: &Piece, chart: &Chart, which: usize) -> Option<Arc<dyn ParamCurve>> {
    match piece {
        Piece::Ruling(line, along) => ruling(*line, *along, chart),
        Piece::Line(c) => Some(open(*c, chart)),
        Piece::Parabola(c) => Some(open(*c, chart)),
        Piece::Hyperbola(c) => Some(open(*c, chart)),
        Piece::Circle(c, cut) => closed(*c, chart, cut.get(which).copied().unwrap_or(false)),
        Piece::Ellipse(c) => closed(*c, chart, false),
    }
}
