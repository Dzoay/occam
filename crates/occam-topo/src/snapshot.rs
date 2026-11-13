//! Source: original
//! An immutable version of a model (S3-P3): every table shares its chunks with the version it came from, so taking one
//! costs a pointer per chunk.

use core::num::NonZeroU32;

use occam_report::{Code, EntityRef, Fault, NamedValue, Quantity, Report};
use occam_units::Tolerance;

use crate::kind::{Id, Kind, Visit, dispatch};
use crate::records::{
    BodyRecord, CurveRecord, EdgeRecord, EdgeUseRecord, FaceRecord, LoopRecord, ParamCurveRecord, Provenance,
    ShellRecord, SurfaceRecord, VertexRecord, WireRecord,
};
use crate::table::{State, Table};

/// The tables of a model and what is kept with them. `pub` only so that the sealed kind trait can name it.
#[derive(Clone, Debug, PartialEq)]
pub struct Model {
    pub(crate) tolerance: Tolerance,
    /// The last step committed: steps count over the model's life, so a step number is unique across requests.
    pub(crate) step: u64,
    pub(crate) bodies: Table<BodyRecord>,
    pub(crate) shells: Table<ShellRecord>,
    pub(crate) faces: Table<FaceRecord>,
    pub(crate) loops: Table<LoopRecord>,
    pub(crate) wires: Table<WireRecord>,
    pub(crate) uses: Table<EdgeUseRecord>,
    pub(crate) edges: Table<EdgeRecord>,
    pub(crate) vertices: Table<VertexRecord>,
    pub(crate) surfaces: Table<SurfaceRecord>,
    pub(crate) curves: Table<CurveRecord>,
    pub(crate) pcurves: Table<ParamCurveRecord>,
}

/// What a model's slot holds for an entity of any kind.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Holds {
    /// Nothing of that generation.
    Nothing,
    /// That generation, reserved and not yet written.
    Reserved,
    /// That generation, live.
    Live,
}

impl Model {
    /// A model with nothing in it, whose tables may each hold at most `most` slots.
    pub(crate) const fn empty(tolerance: Tolerance, most: u32) -> Self {
        Self {
            tolerance,
            step: 0,
            bodies: Table::with_most(most),
            shells: Table::with_most(most),
            faces: Table::with_most(most),
            loops: Table::with_most(most),
            wires: Table::with_most(most),
            uses: Table::with_most(most),
            edges: Table::with_most(most),
            vertices: Table::with_most(most),
            surfaces: Table::with_most(most),
            curves: Table::with_most(most),
            pcurves: Table::with_most(most),
        }
    }

    /// What `entity`'s slot holds of its generation.
    pub(crate) fn holds(&self, entity: EntityRef) -> Holds {
        dispatch(entity.kind, Holding { model: self, entity }).map_or(Holds::Nothing, |(holds, _)| holds)
    }

    /// Whether `entity` is live.
    pub(crate) fn is_live(&self, entity: EntityRef) -> bool {
        self.holds(entity) == Holds::Live
    }

    /// The generation of the live entity of `entity`'s kind in its slot, if there is one.
    pub(crate) fn live_generation(&self, entity: EntityRef) -> Option<u32> {
        dispatch(entity.kind, Holding { model: self, entity }).and_then(|(_, live)| live)
    }

    /// Appends every entity that `entity`'s record links to, if it is live.
    pub(crate) fn links(&self, entity: EntityRef, out: &mut Vec<EntityRef>) {
        dispatch(entity.kind, Linked { model: self, entity, out });
    }

    /// The step that made `entity`, if it is live and of a kind that keeps one.
    pub(crate) fn born(&self, entity: EntityRef) -> Option<Provenance> {
        dispatch(entity.kind, Born { model: self, entity }).flatten()
    }

    /// Sets the step that made `entity`, if it is live and of a kind that keeps one.
    pub(crate) fn set_born(&mut self, entity: EntityRef, born: Provenance) {
        dispatch(entity.kind, SetBorn { model: self, entity, born });
    }
}

/// The live record of `entity` in kind `K`'s table.
fn live<K: Kind>(model: &Model, entity: EntityRef) -> Option<&K::Record> {
    K::table(model).live(entity.slot, NonZeroU32::new(entity.generation)?)
}

/// Asks what an entity's slot holds, and the live generation there.
struct Holding<'a> {
    model: &'a Model,
    entity: EntityRef,
}

impl Visit for Holding<'_> {
    type Out = (Holds, Option<u32>);

    fn visit<K: Kind>(self) -> Self::Out {
        let table = K::table(self.model);
        let found = table.slot(self.entity.slot).filter(|slot| slot.generation() == self.entity.generation);
        let holds = found.map_or(Holds::Nothing, |slot| match slot.state() {
            State::Free => Holds::Nothing,
            State::Reserved => Holds::Reserved,
            State::Live(_) => Holds::Live,
        });
        (holds, table.live_generation(self.entity.slot))
    }
}

/// Lists what an entity's record links to.
struct Linked<'a> {
    model: &'a Model,
    entity: EntityRef,
    out: &'a mut Vec<EntityRef>,
}

impl Visit for Linked<'_> {
    type Out = ();

    fn visit<K: Kind>(self) {
        if let Some(record) = live::<K>(self.model, self.entity) {
            K::links(record, self.out);
        }
    }
}

/// Asks which step made an entity.
struct Born<'a> {
    model: &'a Model,
    entity: EntityRef,
}

impl Visit for Born<'_> {
    type Out = Option<Provenance>;

    fn visit<K: Kind>(self) -> Option<Provenance> {
        live::<K>(self.model, self.entity).and_then(K::born)
    }
}

/// Sets which step made an entity.
struct SetBorn<'a> {
    model: &'a mut Model,
    entity: EntityRef,
    born: Provenance,
}

impl Visit for SetBorn<'_> {
    type Out = ();

    fn visit<K: Kind>(self) {
        let generation = NonZeroU32::new(self.entity.generation);
        if let Some(record) = generation.and_then(|g| K::table_mut(self.model).live_mut(self.entity.slot, g)) {
            K::set_born(record, self.born);
        }
    }
}

/// An immutable version of a model. Cloning one shares every chunk of every table (S3-P3: 183 µs at a million faces,
/// against 31 ms for a plain arena).
#[derive(Clone, Debug, PartialEq)]
pub struct Snapshot {
    pub(crate) model: Model,
}

/// The `topo.stale-id` report for `entity`, whose slot holds `live`, the generation of the entity of the kind wanted
/// there now, if any.
pub(crate) fn stale(entity: EntityRef, live: Option<u32>) -> Report {
    let live = NamedValue::new("live", Quantity::Count, f64::from(live.unwrap_or(0)));
    Report::new(Code::StaleId, Fault::InvalidInput).with_entity(entity).with_value(live)
}

impl Snapshot {
    /// A model with nothing in it, built to `tolerance`.
    #[must_use]
    pub const fn empty(tolerance: Tolerance) -> Self {
        Self { model: Model::empty(tolerance, u32::MAX) }
    }

    /// The tolerance it is built to.
    #[must_use]
    pub const fn tolerance(&self) -> Tolerance {
        self.model.tolerance
    }

    /// The last step committed, counted over the model's life: 0 for a model no edit has changed.
    #[must_use]
    pub const fn step(&self) -> u64 {
        self.model.step
    }

    /// The live ids of a kind, in slot order: the order everything lists, iterates and hashes in (PARITY 1.1).
    #[must_use]
    pub fn ids<K: Kind>(&self) -> Vec<Id<K>> {
        K::table(&self.model).records().map(|(slot, generation, _)| Id::new(slot, generation)).collect()
    }

    /// An `EntityRef` from a report or a request, checked against this snapshot.
    ///
    /// # Errors
    ///
    /// [`Code::StaleId`] if no live entity of kind `K` has its slot and generation: it was removed, its slot holds a
    /// newer entity, or it names another kind.
    pub fn resolve<K: Kind>(&self, entity: EntityRef) -> Result<Id<K>, Report> {
        let table = K::table(&self.model);
        let generation = NonZeroU32::new(entity.generation).filter(|_| entity.kind == K::ENTITY);
        match generation.filter(|&g| table.live(entity.slot, g).is_some()) {
            Some(generation) => Ok(Id::new(entity.slot, generation)),
            None => Err(stale(entity, table.live_generation(entity.slot))),
        }
    }
}

#[cfg(test)]
mod tests {
    use occam_report::{DisplayUnits, EntityKind};

    use super::*;
    use crate::kind::sealed::Sealed;
    use crate::kind::{Edge, Face, Vertex};
    use crate::testing::{tolerance, vertex};

    /// Writes a vertex as an edit will, straight into the table.
    fn add(snapshot: &mut Snapshot, x: f64) -> Id<Vertex> {
        let table = Vertex::table_mut(&mut snapshot.model);
        let (slot, generation) = table.reserve().unwrap();
        assert!(table.put(slot, generation, vertex(x)));
        Id::new(slot, generation)
    }

    fn remove(snapshot: &mut Snapshot, id: Id<Vertex>) {
        assert!(Vertex::table_mut(&mut snapshot.model).free(id.slot(), id.generation()));
    }

    fn live_value(report: &Report) -> Option<f64> {
        report.value("live").map(|v| match v.value {
            occam_report::Value::Scalar(x) => x,
            occam_report::Value::Point(_) => f64::NAN,
        })
    }

    #[test]
    fn an_id_whose_slot_was_freed_and_reused_is_stale_never_the_new_entity() {
        let mut snapshot = Snapshot::empty(tolerance());
        let old = add(&mut snapshot, 1.0);
        remove(&mut snapshot, old);
        let freed = snapshot.resolve::<Vertex>(old.entity()).unwrap_err();
        assert_eq!(
            (freed.code, freed.fault, freed.entities.as_slice()),
            (Code::StaleId, Some(Fault::InvalidInput), &[old.entity()][..])
        );
        assert_eq!(live_value(&freed), Some(0.0));
        let new = add(&mut snapshot, 2.0);
        assert_eq!((new.slot(), new.generation().get()), (0, 2));
        let reused = snapshot.resolve::<Vertex>(old.entity()).unwrap_err();
        assert_eq!((reused.code, live_value(&reused)), (Code::StaleId, Some(2.0)));
        assert_eq!(
            reused.message(DisplayUnits::default()),
            "No live entity of the kind wanted has this slot and generation: the slot holds generation 2 now, or \
             nothing if that is 0."
        );
        assert_eq!(snapshot.resolve::<Vertex>(new.entity()), Ok(new));
    }

    #[test]
    fn an_id_of_another_kind_or_of_no_generation_or_slot_is_stale() {
        let mut snapshot = Snapshot::empty(tolerance());
        let v = add(&mut snapshot, 1.0);
        let as_edge = EntityRef::new(EntityKind::Edge, 0, 1);
        assert_eq!(
            snapshot.resolve::<Edge>(as_edge).map_err(|r| (r.code, live_value(&r))),
            Err((Code::StaleId, Some(0.0)))
        );
        let wrong = snapshot.resolve::<Face>(v.entity()).unwrap_err();
        assert_eq!((wrong.code, live_value(&wrong)), (Code::StaleId, Some(0.0)), "the face table's slot 0");
        let scratch = EntityRef::new(EntityKind::Vertex, 0, 0);
        assert_eq!(snapshot.resolve::<Vertex>(scratch).unwrap_err().code, Code::StaleId);
        let beyond = EntityRef::new(EntityKind::Vertex, 7, 1);
        assert_eq!(snapshot.resolve::<Vertex>(beyond).map_err(|r| live_value(&r)), Err(Some(0.0)));
        let older = EntityRef::new(EntityKind::Vertex, 0, 2);
        assert_eq!(snapshot.resolve::<Vertex>(older).map_err(|r| live_value(&r)), Err(Some(1.0)), "a newer one");
    }

    #[test]
    fn a_clone_shares_every_chunk_and_one_write_copies_exactly_one() {
        let mut snapshot = Snapshot::empty(tolerance());
        let ids: Vec<Id<Vertex>> = (0..2500).map(|k| add(&mut snapshot, f64::from(k))).collect();
        let mut copy = snapshot.clone();
        let table = |s: &Snapshot| Vertex::table(&s.model).clone();
        assert_eq!(table(&copy).shared_chunks(&table(&snapshot)), 3);
        remove(&mut copy, ids[1500]);
        assert_eq!(table(&copy).shared_chunks(&table(&snapshot)), 2);
        assert_eq!(snapshot.ids::<Vertex>(), ids, "the original keeps every vertex");
        assert_eq!(copy.ids::<Vertex>().len(), 2499);
        assert_eq!(snapshot.resolve::<Vertex>(ids[1500].entity()), Ok(ids[1500]));
    }

    #[test]
    fn ids_are_listed_once_each_in_slot_order_and_the_same_edits_give_the_same_ids() {
        let build = || {
            let mut snapshot = Snapshot::empty(tolerance());
            let ids: Vec<Id<Vertex>> = (0..1030).map(|k| add(&mut snapshot, f64::from(k))).collect();
            for id in [ids[1027], ids[3], ids[1024]] {
                remove(&mut snapshot, id);
            }
            let again: Vec<Id<Vertex>> = (0..4).map(|k| add(&mut snapshot, f64::from(k))).collect();
            (snapshot, again)
        };
        let ((first, again), (second, twice)) = (build(), build());
        assert_eq!(again, twice);
        let given: Vec<(u32, u32)> = again.iter().map(|id| (id.slot(), id.generation().get())).collect();
        assert_eq!(given, [(3, 2), (1024, 2), (1027, 2), (1030, 1)]);
        let listed = first.ids::<Vertex>();
        assert_eq!(listed, second.ids::<Vertex>());
        assert_eq!(listed.len(), 1031);
        assert!(listed.windows(2).all(|pair| pair[0].slot() < pair[1].slot()), "slot order, each once");
        assert_eq!(first.ids::<Edge>(), Vec::new());
        assert_eq!((first.step(), first.tolerance()), (0, tolerance()));
    }
}
