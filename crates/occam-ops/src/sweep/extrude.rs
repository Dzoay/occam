//! Source: original
//! The `extrude` operation (PARITY 1.13): a planar face swept along a vector into a solid. The face's copy at each end
//! closes it, and each piece of the face's edges sweeps one side: a plane from a line, a cylinder from an arc where the
//! vector runs along the arc's axis, and otherwise a surface of linear extrusion of the arc, which no analytic family
//! fits. Each point sweeps a straight edge. A whole circle is cut in two, and each side's surface is placed so that its
//! piece lies in the middle of its turn, so no face reaches its surface's seam.

use core::f64::consts::PI;
use std::sync::Arc;

use occam_geom::{Circle, Cylinder, Extrusion, Surface};
use occam_report::{Code, Input, NamedValue, Report};
use occam_topo::BodyKind;
use occam_units::{Direction, Length, UvPoint, Vec3};

use super::profile::{Piece, Profile};
use super::shape::{Flat, Shape, moved, planar};
use crate::construct::refused;
use crate::construct::space::{Bent, Flatland, frame, unit};
use crate::primitive::parts::uv_line;
use crate::primitive::solid::{LoopSpec, UseSpec};

/// Lays out `profile` swept along `vector`, at `at` in the request.
///
/// # Errors
///
/// [`Code::VectorInPlane`] for a vector that rises no more than `band` from the face's plane, and what a family
/// reports, which no piece of a face does.
pub(super) fn extrude(profile: Profile, vector: Vec3, (band, angle): (f64, f64), at: &Input) -> Result<Shape, Report> {
    let normal = profile.normal.unwrap_or(Vec3::new(0.0, 0.0, 0.0));
    let height = vector.dot(normal);
    if height.abs() <= band {
        return Err(refused(Code::VectorInPlane, at, band).with_value(NamedValue::length("height", height)));
    }
    let profile = if height.is_sign_negative() { profile.turned() } else { profile }.cut(|_| Vec::new());
    let mut shape = Shape::new(BodyKind::Solid, profile.body);
    let count = profile.spots.len();
    for (spot, lift) in profile.spots.iter().map(|s| (s, 0.0)).chain(profile.spots.iter().map(|s| (s, 1.0))) {
        shape.point(spot.at + vector * lift, spot.from);
    }
    let top = |k: usize| k.saturating_add(count);
    let mut sides = Vec::new();
    for (k, spot) in profile.spots.iter().enumerate() {
        sides.push(shape.edge(
            Bent::Line { origin: spot.at, along: unit(vector) },
            (0.0, vector.norm()),
            (k, top(k)),
            spot.from,
        )?);
    }
    let mut ends = Vec::new();
    for piece in profile.pieces() {
        let low = shape.edge(piece.bent, piece.range, piece.ends, piece.from)?;
        let high =
            shape.edge(moved(piece.bent, vector), piece.range, (top(piece.ends.0), top(piece.ends.1)), piece.from)?;
        ends.push((low, high));
    }
    for (piece, &(low, high)) in profile.pieces().zip(&ends) {
        let (from, to) = piece.run();
        let lateral = (sides.get(to).copied().unwrap_or(0), sides.get(from).copied().unwrap_or(0));
        side(&mut shape, piece, (low, high, lateral), (vector, angle))?;
    }
    let mut bottom = Vec::new();
    let mut top_loops = Vec::new();
    let mut edges = ends.iter();
    for ring in &profile.rings {
        let pairs: Vec<(&Piece, (usize, usize))> = ring.pieces.iter().zip(edges.by_ref().copied()).collect();
        let witness = ring.witness.unwrap_or(Vec3::new(0.0, 0.0, 0.0));
        let uses = pairs.iter().rev().map(|(p, (low, _))| (*low, !p.back, p.bent)).collect();
        bottom.push(Flat { uses, witness });
        let uses = pairs.iter().map(|(p, (_, high))| (*high, p.back, moved(p.bent, vector))).collect();
        top_loops.push(Flat { uses, witness: witness + vector });
    }
    let base = profile.spots.first().map_or(Vec3::new(0.0, 0.0, 0.0), |s| s.at);
    planar(&mut shape, &Flatland::new(base, normal), bottom, profile.of)?;
    planar(&mut shape, &Flatland::new(base + vector, normal), top_loops, profile.of)?;
    Ok(shape)
}

/// Adds the side `piece` sweeps along `vector`, between its edge at the bottom and at the top and the straight edges
/// from its last spot and its first, in that order.
fn side(
    shape: &mut Shape,
    piece: &Piece,
    (low, high, (last, first)): (usize, usize, (usize, usize)),
    (vector, angle): (Vec3, f64),
) -> Result<(), Report> {
    let edges = [(low, piece.back), (last, false), (high, !piece.back), (first, true)];
    let along = unit(vector);
    match piece.bent {
        Bent::Line { along: line, .. } => {
            let (a, b) = (piece.run().0, piece.run().1);
            let start = shape.at(a);
            let middle = (start + shape.at(b) + vector) * 0.5;
            let plane = Flatland::new(start, unit(line.cross(vector)));
            let lateral = |k: usize| Bent::Line { origin: shape.at(k), along };
            let bents = [piece.bent, lateral(b), moved(piece.bent, vector), lateral(a)];
            let uses = edges.iter().zip(bents).map(|(&(edge, reversed), bent)| (edge, reversed, bent)).collect();
            planar(shape, &plane, vec![Flat { uses, witness: middle }], piece.from)
        }
        Bent::Circle { centre, x, y, radius } => {
            let normal = x.cross(y);
            let delta = PI - f64::midpoint(piece.range.0, piece.range.1);
            let (sin, cos) = occam_math::sin_cos(delta);
            let turned = x * cos - y * sin;
            let placement = frame(centre, normal, Some(turned));
            let r = Length::from_millimetres(radius);
            let straight = along.cross(normal).norm() <= angle;
            let rise = if straight { vector.dot(normal) } else { vector.norm() };
            let surface: Arc<dyn Surface> = if straight {
                Arc::new(Cylinder::placed(&placement, r)?)
            } else {
                Arc::new(Extrusion::new(Circle::placed(&placement, r)?, Direction::normalised(vector))?)
            };
            let up = rise.signum();
            let (lo, hi) = piece.range;
            let at_end = if piece.back { lo } else { hi };
            let at_start = if piece.back { hi } else { lo };
            let pcurves = [
                uv_line((delta, 0.0), (1.0, 0.0))?,
                uv_line((at_end + delta, 0.0), (0.0, up))?,
                uv_line((delta, rise), (1.0, 0.0))?,
                uv_line((at_start + delta, 0.0), (0.0, up))?,
            ];
            let uses = edges.iter().zip(pcurves).map(|(&(edge, reversed), pcurve)| UseSpec { edge, reversed, pcurve });
            let ring = LoopSpec { uses: uses.collect(), witness: UvPoint::new(PI, rise * 0.5) };
            shape.face(surface, vec![ring], piece.from);
            Ok(())
        }
    }
}
