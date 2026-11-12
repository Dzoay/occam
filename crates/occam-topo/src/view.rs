//! Source: original
//! Reading a model: one view per entity, read field by field, so that how the records are laid out stays this crate's
//! alone (O22), and the adjacency queries of PARITY 1.1. Every walk stops where its chain ends, where it comes back to
//! its start, or after as many steps as its table has slots, so a corrupt model can make one short but never endless:
//! the checker is what finds the corruption.

use std::collections::BTreeSet;

use occam_geom::{Curve, ParamCurve, Surface};
use occam_units::{Length, Point, UvPoint, World};

use crate::kind::{
    Body, CurveGeom, Edge, EdgeUse, Face, Id, Kind, Loop, ParamCurveGeom, Shell, SurfaceGeom, Vertex, Wire,
};
use crate::records::{Around, BodyKind, EdgeRole, End, Provenance, Sheet, UseOwner};
use crate::snapshot::{Model, Snapshot};

/// An entity of kind `K`, read from one version of a model.
pub struct View<'a, K: Kind> {
    model: &'a Model,
    id: Id<K>,
    record: &'a K::Record,
}

#[expect(clippy::expl_impl_clone_on_copy, reason = "a derive would ask the record itself to be `Copy`")]
impl<K: Kind> Clone for View<'_, K> {
    fn clone(&self) -> Self {
        *self
    }
}

impl<K: Kind> Copy for View<'_, K> {}

impl<K: Kind> View<'_, K> {
    /// The entity's id.
    #[must_use]
    pub const fn id(self) -> Id<K> {
        self.id
    }
}

impl Snapshot {
    /// The entity `id` names, or `None` if it is not live here.
    #[must_use]
    pub fn view<K: Kind>(&self, id: Id<K>) -> Option<View<'_, K>> {
        view(&self.model, id)
    }
}

/// The entity `id` names in `model`, if it is live there.
pub(crate) fn view<K: Kind>(model: &Model, id: Id<K>) -> Option<View<'_, K>> {
    let record = K::table(model).live(id.slot(), id.generation())?;
    Some(View { model, id, record })
}

/// The chain from `first` by `next`, stopping where it ends, comes back to `first`, or reaches `most` items.
pub(crate) fn walk<T: Copy + Eq>(first: Option<T>, most: usize, mut next: impl FnMut(T) -> Option<T>) -> Vec<T> {
    let mut found = Vec::new();
    let mut at = first;
    while let Some(item) = at {
        if found.len() >= most || (!found.is_empty() && Some(item) == first) {
            break;
        }
        found.push(item);
        at = next(item);
    }
    found
}

/// `items` with each one kept only where it first appears.
fn once<T: Copy + Ord>(items: impl IntoIterator<Item = T>) -> Vec<T> {
    let mut seen = BTreeSet::new();
    items.into_iter().filter(|item| seen.insert(*item)).collect()
}

/// The most steps a walk over kind `K` can take in `model`.
pub(crate) fn most<K: Kind>(model: &Model) -> usize {
    K::table(model).len()
}

impl View<'_, Body> {
    /// What it is.
    #[must_use]
    pub const fn kind(self) -> BodyKind {
        self.record.kind
    }

    /// The step that made it.
    #[must_use]
    pub const fn born(self) -> Provenance {
        self.record.born
    }

    /// Its shells, in order.
    #[must_use]
    pub fn shells(self) -> Vec<Id<Shell>> {
        let model = self.model;
        walk(self.record.first_shell, most::<Shell>(model), |s| view(model, s).and_then(|v| v.record.next))
    }

    /// Its wires, in order.
    #[must_use]
    pub fn wires(self) -> Vec<Id<Wire>> {
        let model = self.model;
        walk(self.record.first_wire, most::<Wire>(model), |w| view(model, w).and_then(|v| v.record.next))
    }
}

impl View<'_, Shell> {
    /// The body it belongs to.
    #[must_use]
    pub const fn body(self) -> Id<Body> {
        self.record.body
    }

    /// The body's next shell.
    #[must_use]
    pub const fn next(self) -> Option<Id<Shell>> {
        self.record.next
    }

    /// Its faces, in order.
    #[must_use]
    pub fn faces(self) -> Vec<Id<Face>> {
        let model = self.model;
        walk(Some(self.record.first_face), most::<Face>(model), |f| view(model, f).and_then(|v| v.record.next))
    }
}

impl<'a> View<'a, Face> {
    /// The shell it belongs to.
    #[must_use]
    pub const fn shell(self) -> Id<Shell> {
        self.record.shell
    }

    /// The shell's next face.
    #[must_use]
    pub const fn next(self) -> Option<Id<Face>> {
        self.record.next
    }

    /// The id of the surface it lies on.
    #[must_use]
    pub const fn surface_id(self) -> Id<SurfaceGeom> {
        self.record.surface
    }

    /// The surface it lies on, borrowed from the snapshot, or `None` if the model is corrupt.
    #[must_use]
    pub fn surface(self) -> Option<&'a dyn Surface> {
        view(self.model, self.record.surface).map(|v| &*v.record.0)
    }

    /// Which period of each periodic parameter its loops lie in.
    #[must_use]
    pub const fn sheet(self) -> Sheet {
        self.record.sheet
    }

    /// Whether its outward normal is the surface's reversed.
    #[must_use]
    pub const fn reversed(self) -> bool {
        self.record.reversed
    }

    /// Its outer loop.
    #[must_use]
    pub const fn outer(self) -> Id<Loop> {
        self.record.outer
    }

    /// The step that made it.
    #[must_use]
    pub const fn born(self) -> Provenance {
        self.record.born
    }

    /// Its loops, the outer first.
    #[must_use]
    pub fn loops(self) -> Vec<Id<Loop>> {
        let model = self.model;
        walk(Some(self.record.outer), most::<Loop>(model), |l| view(model, l).and_then(|v| v.record.next))
    }

    /// Its edges, each once, in the order its loops use them (PARITY 1.1).
    #[must_use]
    pub fn edges(self) -> Vec<Id<Edge>> {
        let uses = self.loops().into_iter().filter_map(|l| view(self.model, l)).flat_map(View::<Loop>::uses);
        once(uses.filter_map(|u| view(self.model, u)).map(|u| u.record.edge))
    }
}

impl View<'_, Loop> {
    /// The face it bounds.
    #[must_use]
    pub const fn face(self) -> Id<Face> {
        self.record.face
    }

    /// The face's next loop.
    #[must_use]
    pub const fn next(self) -> Option<Id<Loop>> {
        self.record.next
    }

    /// A point strictly inside it, in the face's parameters.
    #[must_use]
    pub const fn witness(self) -> UvPoint {
        self.record.witness
    }

    /// Its uses, in order round it.
    #[must_use]
    pub fn uses(self) -> Vec<Id<EdgeUse>> {
        uses_from(self.model, self.record.first)
    }
}

/// The uses from `first` along their `next` links.
fn uses_from(model: &Model, first: Id<EdgeUse>) -> Vec<Id<EdgeUse>> {
    walk(Some(first), most::<EdgeUse>(model), |u| view(model, u).and_then(|v| v.record.next))
}

impl View<'_, Wire> {
    /// The body it belongs to.
    #[must_use]
    pub const fn body(self) -> Id<Body> {
        self.record.body
    }

    /// The body's next wire.
    #[must_use]
    pub const fn next(self) -> Option<Id<Wire>> {
        self.record.next
    }

    /// Its uses, in order along it.
    #[must_use]
    pub fn uses(self) -> Vec<Id<EdgeUse>> {
        uses_from(self.model, self.record.first)
    }
}

impl<'a> View<'a, EdgeUse> {
    /// The edge it uses.
    #[must_use]
    pub const fn edge(self) -> Id<Edge> {
        self.record.edge
    }

    /// The loop or wire it belongs to.
    #[must_use]
    pub const fn owner(self) -> UseOwner {
        self.record.owner
    }

    /// The next use round its loop or along its wire.
    #[must_use]
    pub const fn next(self) -> Option<Id<EdgeUse>> {
        self.record.next
    }

    /// The next use of the same edge.
    #[must_use]
    pub const fn radial(self) -> Id<EdgeUse> {
        self.record.radial
    }

    /// Whether it runs against the edge's curve.
    #[must_use]
    pub const fn reversed(self) -> bool {
        self.record.reversed
    }

    /// The id of the edge's image in the face's parameters, if it has one.
    #[must_use]
    pub const fn pcurve_id(self) -> Option<Id<ParamCurveGeom>> {
        self.record.pcurve
    }

    /// The edge's image in the face's parameters, if it has one.
    #[must_use]
    pub fn pcurve(self) -> Option<&'a dyn ParamCurve> {
        self.record.pcurve.and_then(|p| view(self.model, p)).map(|v| &*v.record.0)
    }

    /// The vertices it runs from and to, in its own direction.
    #[must_use]
    pub fn vertices(self) -> Option<(Id<Vertex>, Id<Vertex>)> {
        let (start, finish) = view(self.model, self.record.edge)?.record.ends;
        Some(if self.record.reversed { (finish, start) } else { (start, finish) })
    }
}

impl<'a> View<'a, Edge> {
    /// The id of the curve it lies on.
    #[must_use]
    pub const fn curve_id(self) -> Id<CurveGeom> {
        self.record.curve
    }

    /// The curve it lies on, borrowed from the snapshot, or `None` if the model is corrupt.
    #[must_use]
    pub fn curve(self) -> Option<&'a dyn Curve> {
        view(self.model, self.record.curve).map(|v| &*v.record.0)
    }

    /// The part of the curve's parameter it covers.
    #[must_use]
    pub const fn range(self) -> (f64, f64) {
        self.record.range
    }

    /// The vertices at the start and the finish of its range.
    #[must_use]
    pub const fn ends(self) -> (Id<Vertex>, Id<Vertex>) {
        self.record.ends
    }

    /// The next place round the star of the vertex at `end`.
    #[must_use]
    pub const fn around(self, end: End) -> Around {
        match end {
            End::Start => self.record.around.0,
            End::Finish => self.record.around.1,
        }
    }

    /// What it is for.
    #[must_use]
    pub const fn role(self) -> EdgeRole {
        self.record.role
    }

    /// How far it strays from the surface of any face that uses it.
    #[must_use]
    pub const fn deviation(self) -> Length {
        self.record.deviation
    }

    /// The step that made it.
    #[must_use]
    pub const fn born(self) -> Provenance {
        self.record.born
    }

    /// Its uses, round the radial cycle from its first.
    #[must_use]
    pub fn uses(self) -> Vec<Id<EdgeUse>> {
        let model = self.model;
        walk(self.record.first_use, most::<EdgeUse>(model), |u| view(model, u).map(|v| v.record.radial))
    }

    /// The faces round it, each once, in radial order (PARITY 1.1).
    #[must_use]
    pub fn faces(self) -> Vec<Id<Face>> {
        let owners = self.uses().into_iter().filter_map(|u| view(self.model, u)).map(|u| u.record.owner);
        let loops = owners.filter_map(|owner| match owner {
            UseOwner::Loop(l) => Some(l),
            UseOwner::Wire(_) => None,
        });
        once(loops.filter_map(|l| view(self.model, l)).map(|l| l.record.face))
    }
}

impl View<'_, Vertex> {
    /// Where it is.
    #[must_use]
    pub const fn point(self) -> Point<World> {
        self.record.point
    }

    /// The certified radius of its position.
    #[must_use]
    pub const fn radius(self) -> Length {
        self.record.radius
    }

    /// One place in its star.
    #[must_use]
    pub const fn star(self) -> Option<Around> {
        self.record.star
    }

    /// The edges that end at it, each once, in the order round its star (PARITY 1.1).
    #[must_use]
    pub fn edges(self) -> Vec<Id<Edge>> {
        let model = self.model;
        let bound = most::<Edge>(model).saturating_mul(2);
        let places = walk(self.record.star, bound, |place| view(model, place.edge).map(|e| e.around(place.end)));
        once(places.into_iter().map(|place| place.edge))
    }
}

impl<'a> View<'a, SurfaceGeom> {
    /// The surface.
    #[must_use]
    pub fn get(self) -> &'a dyn Surface {
        &*self.record.0
    }
}

impl<'a> View<'a, CurveGeom> {
    /// The curve.
    #[must_use]
    pub fn get(self) -> &'a dyn Curve {
        &*self.record.0
    }
}

impl<'a> View<'a, ParamCurveGeom> {
    /// The curve in a face's parameters.
    #[must_use]
    pub fn get(self) -> &'a dyn ParamCurve {
        &*self.record.0
    }
}

#[cfg(test)]
mod tests;
