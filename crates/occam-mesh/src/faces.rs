//! Source: original
//! Each face meshed against its edges' samples (P9's phase 2), one face a task. Its boundary is drawn in its
//! parameters through those samples ([`crate::boundary`]), triangulated with every boundary segment an edge
//! ([`crate::triangulation`]), and refined until it is within the deflection measured to the triangle
//! ([`crate::refine`]). The triangulation's plane is the face's parameters with `u` scaled by the power of two at or
//! below the ratio of the surface's rates along them over the boundary, so that its Delaunay triangles are not
//! stretched by the parameters' units, and a power of two changes no decision.
//!
//! Each face's mesh is a vertex block of its own: a position, a unit normal and a place in the face's parameters at
//! each vertex. Its boundary vertices are its edges' samples, bit for bit, so the faces either side of an edge hold the
//! same positions along it without welding. The normals come from the surface: `Su x Sv` where its derivatives span a
//! plane; at a pole, the normal the surface declares there; and at an apex or a collapsed row, the sum of the surface's
//! normals at the middles of the triangles there. A face that will not mesh is a report naming it and why, and its
//! neighbours are meshed as if it were not.

use std::sync::Arc;

use occam_geom::{Degeneracy, Surface};
use occam_math::{Exec, Grain};
use occam_report::{Fault, Reason, Report};
use occam_topo::{Edge, Face, Id, Snapshot, View};
use occam_units::{Direction, Point, Tolerance, UvPoint, Vec3, World};

use crate::boundary::{self, Boundary, not_meshed};
use crate::deviation::between;
use crate::edges::{Edges, stale};
use crate::limits::{Limits, higher, lower};
use crate::refine::{Points, Work, refine};
use crate::triangulation::{Fail, Triangulation};

/// One use of an edge by a face, and the vertices of the face's mesh that are its edge's samples, in the use's
/// direction: first and last are its vertices.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Side {
    /// The edge.
    pub edge: Id<Edge>,
    /// Whether the use runs against the edge.
    pub reversed: bool,
    /// The vertices, in the use's direction.
    pub vertices: Vec<u32>,
}

/// One face's mesh: its own block of vertices, and triangles of them that face out of the face, anticlockwise about its
/// outward normal. Two vertices share a position only where the face's parameters collapse, at a pole, an apex or a
/// collapsed row, or where it uses an edge twice, as across a seam. No triangle has two corners at one position: where
/// the parameters collapse, the one between the two vertices there is left out, and the triangles either side of it
/// meet by position, not by index.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceMesh {
    /// Each vertex's position: along an edge, that edge's sample, bit for bit.
    pub points: Vec<Point<World>>,
    /// Each vertex's unit normal, out of the face, from its surface.
    pub normals: Vec<Direction<World>>,
    /// Each vertex's place in the face's parameters.
    pub uvs: Vec<UvPoint>,
    /// The triangles, three vertices each.
    pub triangles: Vec<[u32; 3]>,
    /// Each use of an edge, loop by loop, the outer loop first, each in order round it.
    pub sides: Vec<Side>,
    /// Whether refinement stopped at [`Limits::TRIANGLES_PER_FACE`], short of the deflection.
    pub capped: bool,
}

/// Every face of a model, each with its mesh or the report refusing it, in id order.
#[derive(Clone, Debug, PartialEq)]
pub struct Faces {
    list: Vec<(Id<Face>, Result<FaceMesh, Report>)>,
}

impl Faces {
    /// `list`, each face with its mesh or the report refusing it, in id order.
    pub(crate) const fn new(list: Vec<(Id<Face>, Result<FaceMesh, Report>)>) -> Self {
        Self { list }
    }

    /// The mesh of `face`, or the report refusing it; `None` for a face the model does not have.
    #[must_use]
    pub fn get(&self, face: Id<Face>) -> Option<Result<&FaceMesh, &Report>> {
        let at = self.list.binary_search_by_key(&face, |(id, _)| *id).ok()?;
        self.list.get(at).map(|(_, result)| result.as_ref())
    }

    /// Each face with its mesh or the report refusing it, in id order.
    pub fn iter(&self) -> impl Iterator<Item = (Id<Face>, Result<&FaceMesh, &Report>)> + '_ {
        self.list.iter().map(|(id, result)| (*id, result.as_ref()))
    }

    /// The reports refusing faces, in id order: none if every face meshed.
    pub fn refused(&self) -> impl Iterator<Item = &Report> + '_ {
        self.list.iter().filter_map(|(_, result)| result.as_ref().err())
    }
}

/// Meshes every face of `model` against `edges`, its edges' samples, to the limits they were sampled to, one face a
/// task on `exec`; the same bits at any thread count. A face that will not mesh is refused with
/// [`occam_report::Code::FaceNotMeshed`] and the reason, and the other faces are meshed as if it were not there.
#[must_use]
pub fn mesh_faces(exec: &Exec, model: &Snapshot, edges: &Edges) -> Faces {
    Faces { list: mesh_some(exec, model, &model.ids::<Face>(), edges) }
}

/// Each of `faces` meshed against `edges`, one face a task on `exec`, in the order given.
pub(crate) fn mesh_some(
    exec: &Exec,
    model: &Snapshot,
    faces: &[Id<Face>],
    edges: &Edges,
) -> Vec<(Id<Face>, Result<FaceMesh, Report>)> {
    let (model, edges) = (Arc::new(model.clone()), Arc::new(edges.clone()));
    let faces: Arc<[Id<Face>]> = faces.into();
    let of = Arc::clone(&faces);
    let meshed = exec.map(faces.len(), Grain::Each, move |k| of.get(k).map(|&f| (f, mesh_face(&model, f, &edges))));
    meshed.into_iter().flatten().collect()
}

/// Meshes `face` of `model` against `edges`, its edges' samples, to the limits they were sampled to.
///
/// # Errors
///
/// [`occam_report::Code::FaceNotMeshed`]: [`Reason::EdgeNotSampled`] where one of its edges was refused, with that
/// edge's remedy; [`Reason::NoParamCurve`] where a use of an edge has no curve in its parameters;
/// [`Reason::BoundaryCrosses`] where its boundary, through its edges' samples, crosses or touches itself, naming the
/// edges involved; and [`Reason::BelowResolution`] where a coordinate is beyond what can be decided exactly.
pub fn mesh_face(model: &Snapshot, face: Id<Face>, edges: &Edges) -> Result<FaceMesh, Report> {
    let cap = usize::try_from(Limits::TRIANGLES_PER_FACE).unwrap_or(usize::MAX);
    mesh_face_within(model, face, edges, cap)
}

/// [`mesh_face`], refinement stopping at `cap` triangles.
pub(crate) fn mesh_face_within(
    model: &Snapshot,
    face: Id<Face>,
    edges: &Edges,
    cap: usize,
) -> Result<FaceMesh, Report> {
    let view = model.view(face).ok_or_else(|| stale(model, face))?;
    let surface = view.surface().ok_or_else(|| stale(model, view.surface_id()))?;
    let boundary = boundary::of(model, view, edges)?;
    let scale = scale(surface, &boundary.uv);
    let plane: Vec<[f64; 2]> = boundary.uv.iter().map(|uv| [uv.u() * scale, uv.v()]).collect();
    let failed = |fail: Fail| refusal(face, &boundary, fail);
    let mut mesh = Triangulation::new(&plane, &boundary.segments).map_err(failed)?;
    let limits = edges.limits();
    let (deflection, floor, turn) =
        (limits.deflection().millimetres(), limits.floor().millimetres(), limits.half_turn_cosine());
    let square = model.tolerance().angle().radians();
    let work = Work { surface, deflection, turn, floor, square, cap, scale };
    let mut points = Points { uv: boundary.uv.clone(), at: boundary.at.clone() };
    let capped = refine(&mut mesh, &mut points, &work).map_err(failed)?;
    assemble((view, surface, model.tolerance()), &mesh, points, (boundary.sides, capped))
}

/// The report for `fail`, naming `face` and the edges whose points it involves.
fn refusal(face: Id<Face>, boundary: &Boundary, fail: Fail) -> Report {
    let (reason, fault, points) = match fail {
        Fail::Coincide(a, b) => (Reason::BoundaryCrosses, Fault::InvalidInput, vec![a, b]),
        Fail::Touches([a, b], c) => (Reason::BoundaryCrosses, Fault::InvalidInput, vec![a, b, c]),
        Fail::Crosses([a, b], [c, d]) => (Reason::BoundaryCrosses, Fault::InvalidInput, vec![a, b, c, d]),
        Fail::Undecided => (Reason::BelowResolution, Fault::NumericallySensitive, Vec::new()),
        Fail::Broken => (Reason::BudgetSpent, Fault::Internal, Vec::new()),
    };
    let mut edges: Vec<Id<Edge>> = points.into_iter().filter_map(|p| boundary.from.get(p).copied()).collect();
    edges.sort_unstable();
    edges.dedup();
    not_meshed(face, reason, fault, &edges)
}

/// The power of two at or below the ratio of `surface`'s rates along `u` and along `v`, summed over `places`, within
/// 2^64 of 1: what `u` is multiplied by in the plane the face is triangulated in. 1 where the ratio is not a normal
/// number, as where there are no places or one rate is nought at all of them.
fn scale(surface: &dyn Surface, places: &[UvPoint]) -> f64 {
    let jets: Vec<_> = places.iter().map(|&uv| surface.first(uv)).collect();
    let along_u: f64 = jets.iter().map(|jet| jet.du.rate().norm()).sum();
    let along_v: f64 = jets.iter().map(|jet| jet.dv.rate().norm()).sum();
    // Sums of lengths, so the ratio is never below nought.
    let ratio = along_u / along_v;
    if !ratio.is_normal() {
        return 1.0;
    }
    let exponent = i64::try_from((ratio.to_bits() >> 52) & 0x7ff).unwrap_or(1023).saturating_sub(1023);
    let exponent = exponent.clamp(-64, 64).saturating_add(1023);
    f64::from_bits(u64::try_from(exponent).unwrap_or(1023) << 52)
}

/// The unit normal of `surface` at `uv`, `Su x Sv` normalised, where its derivatives span a plane.
fn spanned(surface: &dyn Surface, uv: UvPoint, tol: Tolerance) -> Option<Vec3> {
    let jet = surface.first(uv);
    spans(jet.du.rate(), jet.dv.rate(), tol.angle().radians())
}

/// `su x sv` normalised, where `su` and `sv` span a plane: neither is a rounding against the other, within 64 machine
/// epsilons of it, as at a pole, and they are not parallel within `angle`.
fn spans(su: Vec3, sv: Vec3, angle: f64) -> Option<Vec3> {
    let (a, b) = (su.norm(), sv.norm());
    let across = su.cross(sv);
    let length = across.norm();
    let (short, long) = (lower(a, b), higher(a, b));
    let spanned = short > 64.0 * f64::EPSILON * long && length > angle * a * b && length.is_normal();
    spanned.then(|| across * (1.0 / length))
}

/// The normal `surface` declares at a pole within the band of `tol` of `p`, if `p` is one.
fn declared(surface: &dyn Surface, p: Point<World>, tol: Tolerance) -> Option<Vec3> {
    let near = |d: &&Degeneracy| (d.point - p).to_vec3().norm() <= tol.band().millimetres();
    surface.degeneracies().iter().find(near)?.normal.map(Direction::get)
}

/// The triangles of `mesh` whose corners are at three positions of `points`, numbered for the block, facing out of
/// the face if it is `reversed`; and at each point, the sum of the surface's normals at the middles of the triangles
/// at it, those with two corners at one position included.
#[expect(clippy::many_single_char_names, reason = "a triangle's corners, in the parameters and in space")]
fn triangles(
    mesh: &Triangulation,
    points: &Points,
    surface: &dyn Surface,
    (tol, reversed): (Tolerance, bool),
) -> Option<(Vec<[u32; 3]>, Vec<Vec3>)> {
    let mut summed = vec![Vec3::new(0.0, 0.0, 0.0); points.uv.len()];
    let mut triangles = Vec::new();
    for corners in mesh.triangles() {
        let places = corners.map(|k| points.uv.get(k).copied());
        let [Some(a), Some(b), Some(c)] = places else { return None };
        if let Some(n) = spanned(surface, between([a, b, c], [1, 1, 1], 3), tol) {
            for &k in &corners {
                if let Some(sum) = summed.get_mut(k) {
                    *sum = *sum + n;
                }
            }
        }
        let [p, q, r] = corners.map(|k| points.at.get(k).copied());
        if p == q || q == r || r == p {
            continue;
        }
        let [a, b, c] = corners.map(|k| u32::try_from(k).ok());
        let (a, b, c) = (a?, b?, c?);
        triangles.push(if reversed { [a, c, b] } else { [a, b, c] });
    }
    Some((triangles, summed))
}

/// The mesh of the face `view`, on `surface`, from its triangulation and points; with its uses and whether it was
/// capped. A vertex's normal is the surface's there; else, at a pole, the one the surface declares; else the sum of
/// the surface's at the middles of the triangles at it.
fn assemble(
    (view, surface, tol): (View<'_, Face>, &dyn Surface, Tolerance),
    mesh: &Triangulation,
    points: Points,
    (sides, capped): (Vec<Side>, bool),
) -> Result<FaceMesh, Report> {
    let unmeshed = |reason| not_meshed(view.id(), reason, Fault::NumericallySensitive, &[]);
    let (triangles, summed) =
        triangles(mesh, &points, surface, (tol, view.reversed())).ok_or_else(|| unmeshed(Reason::BudgetSpent))?;
    let mut normals = Vec::with_capacity(points.uv.len());
    for ((&uv, &p), &sum) in points.uv.iter().zip(&points.at).zip(&summed) {
        let found = spanned(surface, uv, tol).or_else(|| declared(surface, p, tol)).unwrap_or(sum);
        if !found.norm().is_normal() {
            return Err(unmeshed(Reason::BelowResolution));
        }
        let unit = Direction::normalised(found);
        normals.push(if view.reversed() { unit.reversed() } else { unit });
    }
    Ok(FaceMesh { points: points.at, normals, uvs: points.uv, triangles, sides, capped })
}

#[cfg(test)]
mod tests;
