//! Source: original
//! Refinement, by chordal error measured to the triangle (P9). Each triangle inside a face is read at the points of a
//! grid of six steps a side on it in the face's parameters, its corners left out, and the points along a fixed edge
//! too, whose chord its edge's samples answer for. At each it is held to two things: how far the surface there is from
//! the triangle, against 15/16 of the deflection, which leaves room for how far the surface rises between the grid's
//! points (on a sphere, 2% beyond them); and how far the surface's normal there turns from its normal at the
//! triangle's middle, against half the angle, as an edge's tangent is held. P9 compared the triangle's own normal with
//! the surface's, which a sliver along an edge has none of to speak of, so refinement chased slivers. A triangle that
//! strays beyond either gets a new point, at the grid point strictly inside it that strays most, and every triangle
//! that changes is read again.
//!
//! A triangle turned over against the surface, its normal at a right angle or more to the surface's at its middle, as
//! one whose corners lie far apart round an apex or a pole can be, is refined whatever its size.
//!
//! A triangle none of whose sides is longer than twice the floor is held to the deflection alone, as an edge's piece is
//! never cut below it, so that at a conical point, where the normal turns however small the triangles round it,
//! refinement stops (P9); the distance from the surface falls as triangles shrink, there too. A triangle with two
//! corners at one place, where the face's parameters collapse, is not refined: it is left out of the mesh, and its
//! neighbours answer for the surface there. Refinement stops, and says so, when one more point would take the face past
//! the cap on triangles.

use std::collections::VecDeque;

use occam_geom::Surface;
use occam_units::{Point, UvPoint, Vec3, World};

use crate::deviation::{between, distance, grid};
use crate::limits::higher;
use crate::triangulation::{Fail, Triangulation};

/// How many steps a side the grid refinement reads on each triangle has.
const STEPS: u32 = 6;

/// What share of the deflection refinement holds a triangle to at its grid's points.
const SHARE: f64 = 15.0 / 16.0;

/// What refinement holds a face's triangles to.
pub(crate) struct Work<'a> {
    pub surface: &'a dyn Surface,
    pub deflection: f64,
    /// The cosine of half the angle.
    pub turn: f64,
    pub floor: f64,
    /// The model's angular tolerance, within which a triangle at a right angle to the surface is turned over.
    pub square: f64,
    pub cap: usize,
    /// What `u` is multiplied by in the triangulation's plane: a power of two, so that it changes no decision.
    pub scale: f64,
}

/// A face's points as refinement adds to them: each one's place in the face's parameters, and its position.
pub(crate) struct Points {
    pub uv: Vec<UvPoint>,
    pub at: Vec<Point<World>>,
}

/// The unit normal of `surface` at `uv`, where its derivatives span one.
fn normal(surface: &dyn Surface, uv: UvPoint) -> Option<Vec3> {
    let jet = surface.first(uv);
    let across = jet.du.rate().cross(jet.dv.rate());
    let length = across.norm();
    length.is_normal().then(|| across * (1.0 / length))
}

/// How far the surface at `uv` strays from the triangle at `flat`, and its normal from `middle`, the surface's normal
/// at the triangle's middle, as a fraction of what each may: above 1 where it strays too far.
fn strays(work: &Work<'_>, uv: UvPoint, flat: [Vec3; 3], middle: Option<Vec3>) -> f64 {
    let jet = work.surface.first(uv);
    let off = distance(jet.point.to_vec3(), flat) / (SHARE * work.deflection);
    let across = jet.du.rate().cross(jet.dv.rate());
    let (Some(middle), true) = (middle, across.norm().is_normal()) else { return off };
    higher(off, (1.0 - across.dot(middle) / across.norm()) / (1.0 - work.turn))
}

/// Where triangle `t` of `mesh` takes a new point, if it strays too far or is turned over.
fn assess(mesh: &Triangulation, points: &Points, work: &Work<'_>, t: usize) -> Option<UvPoint> {
    let corners = mesh.corners(t)?;
    let [Some(a), Some(b), Some(c)] = corners.map(|k| points.uv.get(k).copied()) else { return None };
    let [Some(pa), Some(pb), Some(pc)] = corners.map(|k| points.at.get(k).map(|p| p.to_vec3())) else { return None };
    wants(work, [a, b, c], [pa, pb, pc], mesh.fixed(t))
}

/// Where the triangle with corners at `[a, b, c]` in the face's parameters and at `[pa, pb, pc]` in space, whose edges
/// opposite them are fixed as `fixed` says, takes a new point, if it strays too far or is turned over.
#[expect(clippy::many_single_char_names, reason = "a triangle's corners, in the parameters and in space")]
fn wants(work: &Work<'_>, [a, b, c]: [UvPoint; 3], [pa, pb, pc]: [Vec3; 3], fixed: [bool; 3]) -> Option<UvPoint> {
    if pa == pb || pb == pc || pc == pa {
        return None;
    }
    let longest = higher(higher((pb - pa).norm(), (pc - pb).norm()), (pa - pc).norm());
    let at_middle = normal(work.surface, between([a, b, c], [1, 1, 1], 3));
    let across = (pb - pa).cross(pc - pa);
    let over = at_middle.is_some_and(|n| across.dot(n) <= work.square * across.norm());
    let middle = at_middle.filter(|_| longest > 2.0 * work.floor);
    let (mut worst, mut best): (f64, Option<(f64, UvPoint)>) = (0.0, None);
    for at in grid(STEPS) {
        let [i, j, k] = at;
        let on_fixed = (i == 0 && fixed[0]) || (j == 0 && fixed[1]) || (k == 0 && fixed[2]);
        if i == STEPS || j == STEPS || k == STEPS || on_fixed {
            continue;
        }
        let place = between([a, b, c], at, STEPS);
        let score = strays(work, place, [pa, pb, pc], middle);
        worst = higher(worst, score);
        if i > 0 && j > 0 && k > 0 && best.is_none_or(|(most, _)| score > most) {
            best = Some((score, place));
        }
    }
    best.filter(|_| worst > 1.0 || over).map(|(_, place)| place)
}

/// Refines the inside triangles of `mesh`, whose points are `points`, to `work`; gives whether it stopped at the cap.
///
/// # Errors
///
/// What placing a point in the triangulation reports.
pub(crate) fn refine(mesh: &mut Triangulation, points: &mut Points, work: &Work<'_>) -> Result<bool, Fail> {
    let mut queue: VecDeque<usize> = mesh.inside().into();
    let mut queued = vec![false; mesh.slots()];
    for &t in &queue {
        if let Some(q) = queued.get_mut(t) {
            *q = true;
        }
    }
    while let Some(t) = queue.pop_front() {
        if let Some(q) = queued.get_mut(t) {
            *q = false;
        }
        let Some(uv) = assess(mesh, points, work, t) else { continue };
        if mesh.count().saturating_add(2) > work.cap {
            return Ok(true);
        }
        let Some(index) = mesh.place_in(t, [uv.u() * work.scale, uv.v()])? else { continue };
        if index != points.uv.len() {
            return Err(Fail::Broken);
        }
        points.uv.push(uv);
        points.at.push(work.surface.point(uv));
        queued.resize(mesh.slots(), false);
        for changed in mesh.take_touched() {
            // Only triangles inside change: no flip crosses a fixed edge, and the inside ends only at fixed edges.
            if let Some(q) = queued.get_mut(changed).filter(|q| !**q) {
                *q = true;
                queue.push_back(changed);
            }
        }
    }
    Ok(false)
}

#[cfg(test)]
mod tests;
