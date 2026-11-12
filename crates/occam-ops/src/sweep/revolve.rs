//! Source: original
//! The `revolve` operation (PARITY 2.15): a face or a wire turned about an axis in its plane, into a solid or a sheet.
//! The turn is cut into patches of at most half a turn. Each piece of the profile sweeps one face a patch, on the surface
//! [`around`](super::around) finds for it; each point off the axis sweeps an arc a patch, and a point on it stays where
//! it is, a pole where the faces meet. A face turned less than a whole turn is closed by its copies at each end.

use core::f64::consts::{PI, TAU};

use occam_report::{Code, NamedValue, Report};
use occam_topo::BodyKind;
use occam_units::{UvPoint, Vec3};

use super::around::{Axes, Family, Meridian, meridian, seams};
use super::given::RevolveArgs;
use super::profile::{Piece, Profile};
use super::shape::{Flat, Shape, planar};
use crate::construct::space::{Bent, Flatland};
use crate::construct::{refused, shown};
use crate::primitive::parts::uv_line;
use crate::primitive::solid::{LoopSpec, UseSpec};

/// Points along a circle's piece at which the profile is held to its plane and to one side of the axis.
const STEPS: u32 = 16;

/// How far past half a turn a revolution may go in one patch, in radians.
const HALF: f64 = 1e-9;

/// The first of `points` at which `measure` is greatest, and that greatest measure.
fn furthest(points: &[Vec3], measure: impl Fn(Vec3) -> f64) -> (Vec3, f64) {
    let start = (Vec3::new(f64::NAN, f64::NAN, f64::NAN), f64::NEG_INFINITY);
    points.iter().map(|&p| (p, measure(p))).fold(start, |best, next| if next.1 > best.1 { next } else { best })
}

/// Points along every piece of `profile`, the ends and, along an arc, between them.
fn samples(profile: &Profile) -> Vec<Vec3> {
    let mut points = Vec::new();
    for piece in profile.pieces() {
        let steps = if matches!(piece.bent, Bent::Line { .. }) { 1 } else { STEPS };
        let (lo, hi) = piece.range;
        points.extend((0..=steps).map(|k| piece.bent.point(lo + (hi - lo) * f64::from(k) / f64::from(steps))));
    }
    points
}

/// The axis `args` gives, with the direction across it to the profile's farthest point, and how far that is.
///
/// # Errors
///
/// [`Code::AxisOffPlane`] for a profile that leaves the plane of the axis and that point by more than `band`, and
/// [`Code::ProfileCrossesAxis`] for one that reaches more than `band` beyond the axis, on the other side; each names
/// the profile's point furthest out.
fn axes(profile: &Profile, args: &RevolveArgs, band: f64) -> Result<(Axes, f64), Report> {
    let (origin, axis) = (args.origin, args.axis);
    let points = samples(profile);
    let (far, reach) = furthest(&points, |p| ((p - origin) - axis * (p - origin).dot(axis)).norm());
    // A profile on the axis has no direction across it; it moves nothing, and is refused as such once read.
    let x = ((far - origin) - axis * (far - origin).dot(axis)) * (1.0 / reach);
    let (normal, at) = (axis.cross(x), &args.axis_at);
    let checks = [
        (Code::AxisOffPlane, furthest(&points, |p| (p - origin).dot(normal).abs())),
        (Code::ProfileCrossesAxis, furthest(&points, |p| -(p - origin).dot(x))),
    ];
    for (code, (p, distance)) in checks {
        if distance > band {
            let report = refused(code, at, band).with_value(shown("point", p));
            return Err(report.with_value(NamedValue::length("distance", distance)));
        }
    }
    Ok((Axes { origin, axis, x }, reach))
}

/// Lays out `profile` turned as `args` asks: a whole turn where the profile's farthest point would end within `band` of
/// where it began.
///
/// # Errors
///
/// What [`axes`] reports, [`Code::NoLength`] for a turn whose farthest point moves no more than `band`, and what a
/// family reports, which no piece of a profile on one side of the axis gives.
pub(super) fn revolve(profile: Profile, args: &RevolveArgs, band: f64) -> Result<Shape, Report> {
    let (axes, reach) = axes(&profile, args, band)?;
    if args.angle * reach <= band {
        let report = refused(Code::NoLength, &args.angle_at, band);
        return Err(report.with_value(NamedValue::length("length", args.angle * reach)));
    }
    let full = (TAU - args.angle) * reach <= band;
    let angle = if full { TAU } else { args.angle };
    let facing = profile.normal.map_or(1.0, |n| n.dot(axes.axis.cross(axes.x)));
    let profile =
        if facing.is_sign_negative() { profile.turned() } else { profile }.cut(|piece| seams(piece, axes, band));
    let patches: u32 = if angle <= PI + HALF { 1 } else { 2 };
    let angles: Vec<f64> = (0..=patches).map(|j| angle * f64::from(j) / f64::from(patches)).collect();
    let meridians: Vec<Meridian> = profile.pieces().map(|piece| meridian(piece, axes, band)).collect();
    let kind = if profile.normal.is_some() { BodyKind::Solid } else { BodyKind::Sheet };
    let mut sweep =
        Sweep { profile: &profile, axes, band, full, angles, meridians, shape: Shape::new(kind, profile.body) };
    let points = sweep.points();
    let meridian_edges = sweep.meridian_edges(&points)?;
    let rims = sweep.arcs(&points)?;
    sweep.sides(&meridian_edges, &rims)?;
    if !full && profile.normal.is_some() {
        sweep.caps(&meridian_edges)?;
    }
    Ok(sweep.shape)
}

/// A revolution being laid out.
struct Sweep<'a> {
    profile: &'a Profile,
    axes: Axes,
    band: f64,
    full: bool,
    /// The angles the patches begin and end at, from 0 to the whole angle.
    angles: Vec<f64>,
    /// Each piece's meridian, in the profile's order.
    meridians: Vec<Meridian>,
    shape: Shape,
}

/// For each spot or piece, what it gives at each angle the patches begin or end at, if anything.
type Columns = Vec<Vec<Option<usize>>>;

impl Sweep<'_> {
    /// The angle the patches begin or end at `j`.
    fn angle(&self, j: usize) -> f64 {
        self.angles.get(j).copied().unwrap_or(0.0)
    }

    /// How many patches the turn is cut into.
    fn patches(&self) -> usize {
        self.angles.len().saturating_sub(1)
    }

    /// The column of the angle `j`, which is the first again at the end of a whole turn.
    fn column(&self, j: usize) -> usize {
        if self.full && j == self.patches() { 0 } else { j }
    }

    /// Whether the spot at `p` is on the axis.
    fn on_axis(&self, p: Vec3) -> bool {
        self.axes.local(p).0.abs() <= self.band
    }

    /// Each spot's point at each angle: one point for all of them on the axis.
    fn points(&mut self) -> Columns {
        let mut columns = Vec::new();
        for spot in &self.profile.spots {
            let mut column: Vec<Option<usize>> = Vec::new();
            for j in 0..self.angles.len() {
                let first = column.first().copied().flatten();
                let point = match (self.on_axis(spot.at), first) {
                    (true, Some(pole)) => pole,
                    _ if self.column(j) != j => first.unwrap_or(0),
                    _ => {
                        let turned = self.axes.turn(self.angle(j)).point(spot.at);
                        self.shape.point(turned, spot.from)
                    }
                };
                column.push(Some(point));
            }
            columns.push(column);
        }
        columns
    }

    /// Each piece's edge at each angle; a piece on the axis has one edge, which a whole turn does not keep.
    fn meridian_edges(&mut self, points: &Columns) -> Result<Columns, Report> {
        let mut columns = Vec::new();
        let pieces: Vec<Piece> = self.profile.pieces().copied().collect();
        for (piece, meridian) in pieces.iter().zip(&self.meridians) {
            let mut column: Vec<Option<usize>> = Vec::new();
            for j in 0..self.angles.len() {
                let first = column.first().copied().flatten();
                let on_axis = matches!(meridian.family, Family::Axis);
                let edge = match (on_axis, first) {
                    (true, _) if self.full => None,
                    (true, Some(only)) => Some(only),
                    _ if self.column(j) != j => first,
                    _ => {
                        let ends = (at(points, piece.ends.0, j), at(points, piece.ends.1, j));
                        let bent = self.axes.turn(self.angle(j)).bent(piece.bent);
                        Some(self.shape.edge(bent, piece.range, ends, piece.from)?)
                    }
                };
                column.push(edge);
            }
            columns.push(column);
        }
        Ok(columns)
    }

    /// The circle about the axis that the spot at `p` turns on, its parameter the angle turned.
    fn circle(&self, p: Vec3) -> Bent {
        let (radius, height) = self.axes.local(p);
        let (axis, x) = (self.axes.axis, self.axes.x);
        Bent::Circle { centre: self.axes.origin + axis * height, x, y: axis.cross(x), radius }
    }

    /// Each spot's arc over each patch, none for a spot on the axis.
    fn arcs(&mut self, points: &Columns) -> Result<Columns, Report> {
        let mut columns = Vec::new();
        let spots = self.profile.spots.clone();
        for (k, spot) in spots.iter().enumerate() {
            let mut column = Vec::new();
            for j in 0..self.patches() {
                if self.on_axis(spot.at) {
                    column.push(None);
                    continue;
                }
                let range = (self.angle(j), self.angle(j.saturating_add(1)));
                let ends = (at(points, k, j), at(points, k, j.saturating_add(1)));
                column.push(Some(self.shape.edge(self.circle(spot.at), range, ends, spot.from)?));
            }
            columns.push(column);
        }
        Ok(columns)
    }
}

impl Sweep<'_> {
    /// Adds the face each piece sweeps over each patch.
    fn sides(&mut self, meridians: &Columns, arcs: &Columns) -> Result<(), Report> {
        let pieces: Vec<(Piece, Meridian)> = self.profile.pieces().copied().zip(self.meridians.clone()).collect();
        for (p, (piece, meridian)) in pieces.iter().enumerate() {
            if matches!(meridian.family, Family::Axis) {
                continue;
            }
            for j in 0..self.patches() {
                let next = j.saturating_add(1);
                let (from, to) = piece.run();
                let mut edges = vec![(at(meridians, p, j), piece.back, Use::Meridian(self.angle(j)))];
                if let Some(arc) = arcs.get(to).and_then(|c| c.get(j)).copied().flatten() {
                    edges.push((arc, false, Use::Arc(to)));
                }
                edges.push((at(meridians, p, next), !piece.back, Use::Meridian(self.angle(next))));
                if let Some(arc) = arcs.get(from).and_then(|c| c.get(j)).copied().flatten() {
                    edges.push((arc, true, Use::Arc(from)));
                }
                self.patch(piece, *meridian, (self.angle(j), self.angle(next)), edges)?;
            }
        }
        Ok(())
    }

    /// Adds the face `piece` sweeps between the angles `turn`, bounded by `edges`.
    fn patch(
        &mut self,
        piece: &Piece,
        meridian: Meridian,
        turn: (f64, f64),
        edges: Vec<(usize, bool, Use)>,
    ) -> Result<(), Report> {
        let middle = f64::midpoint(turn.0, turn.1);
        let spot = |k: usize| self.profile.spots.get(k).map_or(self.axes.origin, |s| s.at);
        let Some(surface) = meridian.surface(self.axes, middle, piece.range)? else {
            let (lo, hi) = (piece.bent.point(piece.range.0), piece.bent.point(piece.range.1));
            let ((r0, h), (r1, _)) = (self.axes.local(lo), self.axes.local(hi));
            let base = self.axes.origin + self.axes.axis * h;
            let witness = self.axes.turn(middle).point(base + self.axes.x * f64::midpoint(r0, r1));
            let bents = edges.iter().map(|&(edge, reversed, how)| {
                let bent = match how {
                    Use::Meridian(angle) => self.axes.turn(angle).bent(piece.bent),
                    Use::Arc(k) => self.circle(spot(k)),
                };
                (edge, reversed, bent)
            });
            let flat = Flat { uses: bents.collect(), witness };
            return planar(&mut self.shape, &Flatland::new(base, self.axes.axis), vec![flat], piece.from);
        };
        let turns = meridian.turns();
        let u = |angle: f64| PI + turns * (angle - middle);
        let (v0, v1) = meridian.v;
        let mut uses = Vec::new();
        for (edge, reversed, how) in edges {
            let pcurve = match how {
                Use::Meridian(angle) => uv_line((u(angle), v0), (0.0, v1))?,
                Use::Arc(k) => {
                    let t = if piece.ends.0 == k { piece.range.0 } else { piece.range.1 };
                    uv_line((PI - turns * middle, meridian.v(t)), (turns, 0.0))?
                }
            };
            uses.push(UseSpec { edge, reversed, pcurve });
        }
        let witness = UvPoint::new(PI, f64::midpoint(meridian.v(piece.range.0), meridian.v(piece.range.1)));
        self.shape.face(surface, vec![LoopSpec { uses, witness }], piece.from);
        Ok(())
    }

    /// Adds the face's copies at each end of a turn of less than a whole one.
    fn caps(&mut self, meridians: &Columns) -> Result<(), Report> {
        let last = self.patches();
        let end = self.axes.turn(self.angle(last));
        let (mut start_loops, mut end_loops) = (Vec::new(), Vec::new());
        let mut p = 0_usize;
        for ring in &self.profile.rings {
            let witness = ring.witness.unwrap_or(self.axes.origin);
            let (mut first, mut second) = (Vec::new(), Vec::new());
            for piece in &ring.pieces {
                first.push((at(meridians, p, 0), !piece.back, piece.bent));
                second.push((at(meridians, p, last), piece.back, end.bent(piece.bent)));
                p = p.saturating_add(1);
            }
            first.reverse();
            start_loops.push(Flat { uses: first, witness });
            end_loops.push(Flat { uses: second, witness: end.point(witness) });
        }
        let normal = self.profile.normal.unwrap_or(self.axes.axis);
        let base = self.profile.spots.first().map_or(self.axes.origin, |s| s.at);
        let of = self.profile.of;
        planar(&mut self.shape, &Flatland::new(base, normal), start_loops, of)?;
        planar(&mut self.shape, &Flatland::new(end.point(base), end.vector(normal)), end_loops, of)
    }
}

/// How a use of a patch's loop lies: a piece's edge at an angle, or a spot's arc.
#[derive(Clone, Copy)]
enum Use {
    Meridian(f64),
    Arc(usize),
}

/// The index at column `j` of item `k` of `columns`, which every column the layout reads has.
fn at(columns: &Columns, k: usize, j: usize) -> usize {
    columns.get(k).and_then(|column| column.get(j)).copied().flatten().unwrap_or(0)
}
