//! Source: original
//! The profile a sweep takes: a face, the only face of a sheet, or, for a revolution, a wire, the only wire of a wire
//! body. It is read back as rings of pieces, each a part of a line or a circle between two spots, and then removed with
//! its body and its geometry, as a face takes its wires: everything the sweep makes from it is new, and its lineage
//! names what it came from.

use std::collections::BTreeMap;

use occam_report::{Code, EntityKind, EntityRef, Fault, Input, Report};
use occam_topo::{
    Body, BodyKind, CurveGeom, Edge, EdgeUse, Edit, Face, Id, Kind, Loop, ParamCurveGeom, Shell, Snapshot, SurfaceGeom,
    Vertex, Wire,
};
use occam_units::Vec3;

use super::given::Named;
use crate::construct::space::{Bent, unit};
use crate::operand::{Labels, Operand};

/// A point of the profile: one of its vertices, or a point where one of its edges is cut.
#[derive(Clone, Copy, Debug)]
pub(super) struct Spot {
    /// Where it is, in millimetres.
    pub at: Vec3,
    /// The vertex, or the edge cut there.
    pub from: EntityRef,
}

/// A piece of a profile edge: the part `range` of its line or circle, between the spots at its ends.
#[derive(Clone, Copy, Debug)]
pub(super) struct Piece {
    /// The line or circle.
    pub bent: Bent,
    /// The part of it covered, lowest first.
    pub range: (f64, f64),
    /// Whether its ring runs against the curve.
    pub back: bool,
    /// The spots at the low and the high end of the range.
    pub ends: (usize, usize),
    /// The edge it is part of.
    pub from: EntityRef,
}

impl Piece {
    /// The spot its ring arrives at it from, and the spot it leaves for.
    pub(super) const fn run(&self) -> (usize, usize) {
        if self.back { (self.ends.1, self.ends.0) } else { (self.ends.0, self.ends.1) }
    }

    /// The same piece, its ring running the other way.
    const fn turned(self) -> Self {
        Self { back: !self.back, ..self }
    }
}

/// A run of pieces in order, round a loop or along a wire, with a point strictly inside it if it bounds a face.
#[derive(Clone, Debug)]
pub(super) struct Ring {
    /// The pieces, in order.
    pub pieces: Vec<Piece>,
    /// A point inside it, in millimetres, if it is a loop of a face.
    pub witness: Option<Vec3>,
}

impl Ring {
    /// The same ring, run the other way.
    fn turned(self) -> Self {
        Self { pieces: self.pieces.into_iter().rev().map(Piece::turned).collect(), witness: self.witness }
    }
}

/// A profile, read back and taken from the model.
#[derive(Clone, Debug)]
pub(super) struct Profile {
    /// Its spots.
    pub spots: Vec<Spot>,
    /// A face's loops, the outer first, or a wire's one run.
    pub rings: Vec<Ring>,
    /// A face's unit outward normal, about which its loops ran anticlockwise as they were read; `None` for a wire.
    pub normal: Option<Vec3>,
    /// Its body.
    pub body: EntityRef,
    /// Its face, or its wire.
    pub of: EntityRef,
}

impl Profile {
    /// The same profile with every ring run the other way.
    pub(super) fn turned(self) -> Self {
        Self { rings: self.rings.into_iter().map(Ring::turned).collect(), ..self }
    }

    /// Every piece, ring by ring.
    pub(super) fn pieces(&self) -> impl Iterator<Item = &Piece> {
        self.rings.iter().flat_map(|ring| ring.pieces.iter())
    }

    /// The same profile with each piece cut where `cuts` says, at parameters strictly inside its range, in order. A
    /// whole circle, which begins and ends at one spot, is always cut halfway round, so no piece comes back to its start.
    pub(super) fn cut(mut self, cuts: impl Fn(&Piece) -> Vec<f64>) -> Self {
        let mut rings = Vec::new();
        for ring in core::mem::take(&mut self.rings) {
            let mut pieces = Vec::new();
            for piece in ring.pieces {
                let (lo, hi) = piece.range;
                let mut at: Vec<f64> =
                    if piece.ends.0 == piece.ends.1 { vec![f64::midpoint(lo, hi)] } else { Vec::new() };
                at.extend(cuts(&piece));
                at.sort_by(f64::total_cmp);
                let split = self.split(piece, &at);
                pieces.extend(if piece.back { split.into_iter().rev().collect() } else { split });
            }
            rings.push(Ring { pieces, ..ring });
        }
        Self { rings, ..self }
    }

    /// `piece` cut at `at`, in order along its curve, each cut a new spot.
    fn split(&mut self, piece: Piece, at: &[f64]) -> Vec<Piece> {
        let (mut lo, mut from) = (piece.range.0, piece.ends.0);
        let mut out = Vec::new();
        for &t in at {
            let spot = self.spots.len();
            self.spots.push(Spot { at: piece.bent.point(t), from: piece.from });
            out.push(Piece { range: (lo, t), ends: (from, spot), ..piece });
            (lo, from) = (t, spot);
        }
        out.push(Piece { range: (lo, piece.range.1), ends: (from, piece.ends.1), ..piece });
        out
    }
}

/// What a sweep may take as its profile.
#[derive(Clone, Copy, PartialEq, Eq)]
pub(super) enum Takes {
    /// A face.
    Face,
    /// A face or a wire.
    FaceOrWire,
}

/// `at` names `given`, which is not a profile a sweep can take.
pub(super) fn not_a_profile(at: &Input, given: &str) -> Report {
    Report::new(Code::NotAProfile, Fault::InvalidInput).with_input(at.clone().given(given))
}

/// Reads the profile `named` names, with `labels` naming what earlier steps made, and takes it from `edit`.
///
/// # Errors
///
/// [`Code::InputUnknownLabel`] for a label no earlier step has, [`Code::StaleId`] for an id that names no live entity,
/// and [`Code::NotAProfile`] for anything but the only face of a sheet, or, where it `takes` one, the only wire of a
/// wire body, or for a face that is not planar or an edge that is not a line or a circle.
pub(super) fn take(edit: &mut Edit, named: &Named, labels: &Labels, takes: Takes) -> Result<Profile, Report> {
    let at = &named.at;
    let entity = match &named.operand {
        Operand::Label(label) => labels.get(label).ok_or_else(|| {
            Report::new(Code::InputUnknownLabel, Fault::InvalidInput).with_input(at.clone().given(label))
        })?,
        Operand::Entity(entity) => *entity,
    };
    let model = edit.base();
    let placed = |report: Report| report.with_input(at.clone());
    let found = match entity.kind {
        EntityKind::Face => Found::Face(model.resolve::<Face>(entity).map_err(placed)?),
        EntityKind::Wire if takes == Takes::FaceOrWire => Found::Wire(model.resolve::<Wire>(entity).map_err(placed)?),
        EntityKind::Body => body(model, model.resolve::<Body>(entity).map_err(placed)?, takes, at)?,
        _ => return Err(not_a_profile(at, &entity.text())),
    };
    let (profile, gone) = match found {
        Found::Face(face) => read_face(model, face, at)?,
        Found::Wire(wire) => read_wire(model, wire, at)?,
    };
    for entity in gone {
        remove(edit, entity)?;
    }
    Ok(profile)
}

/// A profile found by kind.
enum Found {
    Face(Id<Face>),
    Wire(Id<Wire>),
}

/// The only face of the sheet `id`, or the only wire of the wire body `id` where a wire is taken.
fn body(model: &Snapshot, id: Id<Body>, takes: Takes, at: &Input) -> Result<Found, Report> {
    let view = model.view(id).ok_or_else(|| not_a_profile(at, "a body no longer there"))?;
    match view.kind() {
        BodyKind::Sheet => {
            let faces: Vec<Id<Face>> = view
                .shells()
                .into_iter()
                .filter_map(|s| model.view(s))
                .flat_map(occam_topo::View::<Shell>::faces)
                .collect();
            faces.first().copied().map(Found::Face).ok_or_else(|| not_a_profile(at, "a sheet with no face"))
        }
        BodyKind::Wire if takes == Takes::FaceOrWire => {
            view.wires().first().copied().map(Found::Wire).ok_or_else(|| not_a_profile(at, "a wire body with no wire"))
        }
        BodyKind::Wire => Err(not_a_profile(at, "a wire body")),
        BodyKind::Solid => Err(not_a_profile(at, "a solid")),
    }
}

/// The spots and pieces of `uses`, in order, `spots` growing with each vertex first met.
fn pieces(
    model: &Snapshot,
    uses: &[Id<EdgeUse>],
    spots: &mut (Vec<Spot>, BTreeMap<Id<Vertex>, usize>),
    at: &Input,
) -> Result<Vec<Piece>, Report> {
    let mut out = Vec::new();
    for &u in uses {
        let link = model.view(u).ok_or_else(|| not_a_profile(at, "a use no longer there"))?;
        let edge = model.view(link.edge()).ok_or_else(|| not_a_profile(at, "an edge no longer there"))?;
        let curve = edge.curve().ok_or_else(|| not_a_profile(at, "an edge with no curve"))?;
        let bent = Bent::of(curve).ok_or_else(|| not_a_profile(at, curve.shape().form))?;
        let (start, finish) = edge.ends();
        let mut spot = |v: Id<Vertex>| -> Result<usize, Report> {
            if let Some(&k) = spots.1.get(&v) {
                return Ok(k);
            }
            let point = model.view(v).map(|p| p.point().to_vec3()).ok_or_else(|| not_a_profile(at, "a vertex"))?;
            spots.0.push(Spot { at: point, from: v.entity() });
            spots.1.insert(v, spots.0.len().saturating_sub(1));
            Ok(spots.0.len().saturating_sub(1))
        };
        let ends = (spot(start)?, spot(finish)?);
        out.push(Piece { bent, range: edge.range(), back: link.reversed(), ends, from: link.edge().entity() });
    }
    Ok(out)
}

/// What is taken with a profile: its body and everything of it, and the geometry those name.
type Gone = Vec<EntityRef>;

/// The face `id` as a profile, if it is the only face of a sheet and planar, and what goes with it.
fn read_face(model: &Snapshot, id: Id<Face>, at: &Input) -> Result<(Profile, Gone), Report> {
    let face = model.view(id).ok_or_else(|| not_a_profile(at, "a face no longer there"))?;
    let shell = model.view(face.shell()).ok_or_else(|| not_a_profile(at, "a face with no shell"))?;
    let body = model.view(shell.body()).ok_or_else(|| not_a_profile(at, "a face with no body"))?;
    match body.kind() {
        BodyKind::Sheet => {}
        BodyKind::Solid => return Err(not_a_profile(at, "a face of a solid")),
        BodyKind::Wire => return Err(not_a_profile(at, "a face of a wire body")),
    }
    if body.shells().len() != 1 || shell.faces().len() != 1 {
        return Err(not_a_profile(at, "a face of a sheet with others"));
    }
    let surface = face.surface().ok_or_else(|| not_a_profile(at, "a face with no surface"))?;
    if surface.shape().form != "plane" {
        return Err(not_a_profile(at, "a face that is not planar"));
    }
    let loops: Vec<Id<Loop>> = face.loops();
    let mut spots = (Vec::new(), BTreeMap::new());
    let (mut rings, mut gone) = (Vec::new(), vec![shell.body().entity(), face.shell().entity(), id.entity()]);
    gone.push(face.surface_id().entity());
    for &l in &loops {
        let ring = model.view(l).ok_or_else(|| not_a_profile(at, "a loop no longer there"))?;
        let uses = ring.uses();
        gone.push(l.entity());
        gone.extend(uses_and_edges(model, &uses));
        let witness = surface.point(ring.witness()).to_vec3();
        rings.push(Ring { pieces: pieces(model, &uses, &mut spots, at)?, witness: Some(witness) });
    }
    let outer = loops.first().and_then(|&l| model.view(l)).map(occam_topo::View::witness);
    let jet = surface.first(outer.ok_or_else(|| not_a_profile(at, "a face with no loop"))?);
    let normal = unit(jet.du.rate().cross(jet.dv.rate())) * if face.reversed() { -1.0 } else { 1.0 };
    gone.extend(spots.1.keys().map(|v| v.entity()));
    let body = shell.body().entity();
    let profile = Profile { spots: spots.0, rings, normal: Some(normal), body, of: id.entity() };
    Ok((profile, gone))
}

/// The wire `id` as a profile, if it is the only wire of a wire body, and what goes with it.
fn read_wire(model: &Snapshot, id: Id<Wire>, at: &Input) -> Result<(Profile, Gone), Report> {
    let wire = model.view(id).ok_or_else(|| not_a_profile(at, "a wire no longer there"))?;
    let body = model.view(wire.body()).ok_or_else(|| not_a_profile(at, "a wire with no body"))?;
    if body.wires().len() != 1 {
        return Err(not_a_profile(at, "a wire of a body with others"));
    }
    let uses = wire.uses();
    let mut spots = (Vec::new(), BTreeMap::new());
    let ring = Ring { pieces: pieces(model, &uses, &mut spots, at)?, witness: None };
    let mut gone = vec![wire.body().entity(), id.entity()];
    gone.extend(uses_and_edges(model, &uses));
    gone.extend(spots.1.keys().map(|v| v.entity()));
    let profile =
        Profile { spots: spots.0, rings: vec![ring], normal: None, body: wire.body().entity(), of: id.entity() };
    Ok((profile, gone))
}

/// `uses`, their curves in the face's parameters, their edges, and the edges' curves.
fn uses_and_edges(model: &Snapshot, uses: &[Id<EdgeUse>]) -> Gone {
    let mut gone = Vec::new();
    for &u in uses {
        let Some(link) = model.view(u) else { continue };
        gone.push(u.entity());
        gone.extend(link.pcurve_id().map(Id::entity));
        gone.push(link.edge().entity());
        gone.extend(model.view(link.edge()).map(|e| e.curve_id().entity()));
    }
    gone
}

/// Removes `entity` from `edit`, of whichever kind it is.
fn remove(edit: &mut Edit, entity: EntityRef) -> Result<(), Report> {
    match entity.kind {
        EntityKind::Body => gone::<Body>(edit, entity),
        EntityKind::Shell => gone::<Shell>(edit, entity),
        EntityKind::Face => gone::<Face>(edit, entity),
        EntityKind::Loop => gone::<Loop>(edit, entity),
        EntityKind::Wire => gone::<Wire>(edit, entity),
        EntityKind::EdgeUse => gone::<EdgeUse>(edit, entity),
        EntityKind::Edge => gone::<Edge>(edit, entity),
        EntityKind::Vertex => gone::<Vertex>(edit, entity),
        EntityKind::Surface => gone::<SurfaceGeom>(edit, entity),
        EntityKind::Curve => gone::<CurveGeom>(edit, entity),
        EntityKind::ParamCurve => gone::<ParamCurveGeom>(edit, entity),
        EntityKind::Model | EntityKind::Section | EntityKind::Region => Ok(()),
    }
}

/// Removes `entity`, of kind `K`, from `edit`.
fn gone<K: Kind>(edit: &mut Edit, entity: EntityRef) -> Result<(), Report> {
    let id = edit.base().resolve::<K>(entity)?;
    edit.remove(id)
}
