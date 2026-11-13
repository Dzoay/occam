//! Source: original
//! A body's moved records written through the edit. In place, every entity keeps its id, and geometry the body's
//! entities share stays shared where it moved as one; a curve that became an edge's own B-spline, or a parameter curve,
//! which is each use's own, gets a record of its own where another entity named it first. A copy has every entity new
//! and derived as `copied` from the one it came from, its links among its own entities as the body's were among its, and
//! its geometry its own: a surface for each the body's faces had, a curve for each edge and a parameter curve for each
//! use. Vertex radii grow by the most the map stretches anything; each edge's deviation `commit` measures again.

use std::collections::{BTreeMap, BTreeSet};

use occam_report::{Code, EntityRef, Fault, Report};
use occam_topo::{
    Around, Body, CurveRecord, Derivation, Edge, EdgeUse, Edit, Face, Id, Kind, Loop, ParamCurveRecord, Record, Shell,
    SurfaceRecord, UseOwner, Vertex, Wire,
};
use occam_units::Length;

use super::body::Parts;
use super::moved::Moves;

/// The record `id` has in `edit`.
fn record<K: Kind>(edit: &Edit, id: Id<K>) -> Result<K::Record, Report> {
    edit.record(id).ok_or_else(|| Report::new(Code::StaleId, Fault::InvalidInput).with_entity(id.entity()))
}

/// Where geometry is written: to the entity it replaces the first time that is met, and after that to that entity
/// again where it is shared, or to a new one where it is `own`.
struct Places {
    written: BTreeSet<EntityRef>,
}

impl Places {
    /// Writes `geometry` for an entity that named `id`, and gives the id it is written to.
    fn place<R: Record>(
        &mut self,
        edit: &mut Edit,
        id: Id<R::Kind>,
        geometry: R,
        own: bool,
    ) -> Result<Id<R::Kind>, Report> {
        if self.written.insert(id.entity()) {
            edit.set(id, geometry)?;
            Ok(id)
        } else if own {
            Ok(edit.add(geometry))
        } else {
            Ok(id)
        }
    }
}

/// `length` grown by the most a map stretches anything.
fn grown(length: Length, most: f64) -> Length {
    Length::from_millimetres(length.millimetres() * most)
}

/// Writes the moved body in place: every entity keeps its id, and each geometry record is replaced by its moved form.
///
/// # Errors
///
/// What the edit reports.
pub(super) fn in_place(edit: &mut Edit, parts: &Parts, moves: &Moves) -> Result<(), Report> {
    for (&id, surface) in &moves.surfaces {
        edit.set(id, SurfaceRecord(surface.clone()))?;
    }
    for (&face, &sheet) in &moves.sheets {
        let mut r = record(edit, face)?;
        r.sheet = sheet;
        edit.set(face, r)?;
    }
    for (&l, &witness) in &moves.witnesses {
        let mut r = record(edit, l)?;
        r.witness = witness;
        edit.set(l, r)?;
    }
    let mut places = Places { written: BTreeSet::new() };
    for &e in &parts.edges {
        let mut r = record(edit, e)?;
        if let Some(m) = moves.edges.get(&e) {
            r.curve = places.place(edit, r.curve, CurveRecord(m.curve.clone()), m.own)?;
            r.range = m.range;
        }
        edit.set(e, r)?;
    }
    for &u in &parts.uses {
        let mut r = record(edit, u)?;
        if let (Some(id), Some(pcurve)) = (r.pcurve, moves.pcurves.get(&u)) {
            r.pcurve = Some(places.place(edit, id, ParamCurveRecord(pcurve.clone()), true)?);
            edit.set(u, r)?;
        }
    }
    for &v in &parts.vertices {
        let mut r = record(edit, v)?;
        if let Some(&point) = moves.points.get(&v) {
            r.point = point;
        }
        r.radius = grown(r.radius, moves.most);
        edit.set(v, r)?;
    }
    Ok(())
}

/// New ids for each entity of a kind, in the order given.
fn fresh<K: Kind>(edit: &mut Edit, old: &[Id<K>]) -> BTreeMap<Id<K>, Id<K>> {
    old.iter().map(|&id| (id, edit.reserve())).collect()
}

/// The new id for `id`, or `id` itself where the body did not hold it.
fn to<K: Kind>(map: &BTreeMap<Id<K>, Id<K>>, id: Id<K>) -> Id<K> {
    map.get(&id).copied().unwrap_or(id)
}

/// The copy's ids for the body's entities.
struct Ids {
    body: (Id<Body>, Id<Body>),
    shells: BTreeMap<Id<Shell>, Id<Shell>>,
    faces: BTreeMap<Id<Face>, Id<Face>>,
    loops: BTreeMap<Id<Loop>, Id<Loop>>,
    wires: BTreeMap<Id<Wire>, Id<Wire>>,
    uses: BTreeMap<Id<EdgeUse>, Id<EdgeUse>>,
    edges: BTreeMap<Id<Edge>, Id<Edge>>,
    vertices: BTreeMap<Id<Vertex>, Id<Vertex>>,
}

impl Ids {
    /// The copy's `around`.
    fn around(&self, around: Around) -> Around {
        Around { edge: to(&self.edges, around.edge), ..around }
    }

    /// Every pair of new and old entity, the body first.
    fn pairs(&self) -> Vec<(EntityRef, EntityRef)> {
        fn each<K: Kind>(map: &BTreeMap<Id<K>, Id<K>>) -> impl Iterator<Item = (EntityRef, EntityRef)> + '_ {
            map.iter().map(|(old, new)| (new.entity(), old.entity()))
        }
        let mut out = vec![(self.body.1.entity(), self.body.0.entity())];
        out.extend(each(&self.shells).chain(each(&self.faces)).chain(each(&self.loops)).chain(each(&self.wires)));
        out.extend(each(&self.uses).chain(each(&self.edges)).chain(each(&self.vertices)));
        out
    }
}

/// A copy being written: its ids, the moves, and the lineage of what it has written so far, new then old.
struct Copier<'a> {
    ids: Ids,
    moves: &'a Moves,
    lineage: Vec<(EntityRef, EntityRef)>,
}

impl Copier<'_> {
    /// The copy's body, shells and faces, each face on its surface moved, shared by the faces that shared it.
    fn body_shells_faces(&mut self, edit: &mut Edit) -> Result<(), Report> {
        let ids = &self.ids;
        let mut r = record(edit, ids.body.0)?;
        r.first_shell = r.first_shell.map(|s| to(&ids.shells, s));
        r.first_wire = r.first_wire.map(|w| to(&ids.wires, w));
        edit.set(ids.body.1, r)?;
        for (&old, &new) in &ids.shells {
            let mut r = record(edit, old)?;
            (r.body, r.face, r.next) = (ids.body.1, to(&ids.faces, r.face), r.next.map(|s| to(&ids.shells, s)));
            edit.set(new, r)?;
        }
        let mut surfaces = BTreeMap::new();
        for (&old, &new) in &ids.faces {
            let mut r = record(edit, old)?;
            if let Some(surface) = self.moves.surfaces.get(&r.surface).filter(|_| !surfaces.contains_key(&r.surface)) {
                let id = edit.add(SurfaceRecord(surface.clone()));
                self.lineage.push((id.entity(), r.surface.entity()));
                surfaces.insert(r.surface, id);
            }
            r.surface = to(&surfaces, r.surface);
            r.sheet = self.moves.sheets.get(&old).copied().unwrap_or(r.sheet);
            (r.shell, r.outer) = (to(&ids.shells, r.shell), to(&ids.loops, r.outer));
            edit.set(new, r)?;
        }
        Ok(())
    }

    /// The copy's loops, wires and uses, each use with its parameter curve moved, its own.
    fn loops_wires_uses(&mut self, edit: &mut Edit) -> Result<(), Report> {
        let ids = &self.ids;
        for (&old, &new) in &ids.loops {
            let mut r = record(edit, old)?;
            r.witness = self.moves.witnesses.get(&old).copied().unwrap_or(r.witness);
            (r.face, r.first) = (to(&ids.faces, r.face), to(&ids.uses, r.first));
            r.next = r.next.map(|l| to(&ids.loops, l));
            edit.set(new, r)?;
        }
        for (&old, &new) in &ids.wires {
            let mut r = record(edit, old)?;
            (r.body, r.first, r.next) = (ids.body.1, to(&ids.uses, r.first), r.next.map(|w| to(&ids.wires, w)));
            edit.set(new, r)?;
        }
        for (&old, &new) in &ids.uses {
            let mut r = record(edit, old)?;
            if let (Some(id), Some(pcurve)) = (r.pcurve, self.moves.pcurves.get(&old)) {
                let added = edit.add(ParamCurveRecord(pcurve.clone()));
                self.lineage.push((added.entity(), id.entity()));
                r.pcurve = Some(added);
            }
            r.owner = match r.owner {
                UseOwner::Loop(l) => UseOwner::Loop(to(&ids.loops, l)),
                UseOwner::Wire(w) => UseOwner::Wire(to(&ids.wires, w)),
            };
            (r.edge, r.radial) = (to(&ids.edges, r.edge), to(&ids.uses, r.radial));
            r.next = r.next.map(|u| to(&ids.uses, u));
            edit.set(new, r)?;
        }
        Ok(())
    }

    /// The copy's edges and vertices, each edge on its curve moved, its own.
    fn edges_vertices(&mut self, edit: &mut Edit) -> Result<(), Report> {
        let (ids, moves) = (&self.ids, self.moves);
        for (&old, &new) in &ids.edges {
            let mut r = record(edit, old)?;
            if let Some(m) = moves.edges.get(&old) {
                let id = edit.add(CurveRecord(m.curve.clone()));
                self.lineage.push((id.entity(), r.curve.entity()));
                (r.curve, r.range) = (id, m.range);
            }
            r.ends = (to(&ids.vertices, r.ends.0), to(&ids.vertices, r.ends.1));
            r.around = (ids.around(r.around.0), ids.around(r.around.1));
            r.first_use = r.first_use.map(|u| to(&ids.uses, u));
            edit.set(new, r)?;
        }
        for (&old, &new) in &ids.vertices {
            let mut r = record(edit, old)?;
            r.point = moves.points.get(&old).copied().unwrap_or(r.point);
            r.radius = grown(r.radius, moves.most);
            r.star = r.star.map(|a| ids.around(a));
            edit.set(new, r)?;
        }
        Ok(())
    }
}

/// Writes a moved copy of the body `body`, every entity new and derived as copied from the one it came from, and gives
/// the copy.
///
/// # Errors
///
/// What the edit reports.
pub(super) fn copied(edit: &mut Edit, body: Id<Body>, parts: &Parts, moves: &Moves) -> Result<Id<Body>, Report> {
    let ids = Ids {
        body: (body, edit.reserve()),
        shells: fresh(edit, &parts.shells),
        faces: fresh(edit, &parts.faces),
        loops: fresh(edit, &parts.loops),
        wires: fresh(edit, &parts.wires),
        uses: fresh(edit, &parts.uses),
        edges: fresh(edit, &parts.edges),
        vertices: fresh(edit, &parts.vertices),
    };
    let mut copier = Copier { lineage: ids.pairs(), ids, moves };
    copier.body_shells_faces(edit)?;
    copier.loops_wires_uses(edit)?;
    copier.edges_vertices(edit)?;
    for (new, old) in copier.lineage {
        edit.derive(new, old, Derivation::Copied)?;
    }
    Ok(copier.ids.body.1)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_vertex_radius_grows_by_the_most_the_map_stretches_anything() {
        let grown = grown(Length::from_millimetres(2.0), 3.0);
        assert_eq!(grown.millimetres(), 6.0);
    }
}
