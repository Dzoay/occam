//! Source: original
//! The one way to change a model. An [`Edit`] works on its own copy of a snapshot's tables and is the only place ids
//! are given; [`Edit::commit`] checks it and is the only way to make a snapshot, so no unchecked model exists. A failed
//! or dropped edit leaves its input as it was (L5).

use core::num::NonZeroU32;
use std::collections::BTreeSet;

use occam_math::Exec;
use occam_report::{EntityKind, EntityRef, Report, StepRole};

use crate::check;
use crate::kind::{Id, Kind, Record};
use crate::records::Provenance;
use crate::snapshot::{Holds, Model, Snapshot, stale};

/// How a new entity came from an old one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Derivation {
    /// One piece of it.
    Split,
    /// It, made smaller.
    Trimmed,
    /// It, joined with others.
    Merged,
    /// A copy of it, moved or not.
    Copied,
}

/// Where a new entity came from (PARITY 1.11).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Lineage {
    /// The entity made.
    pub new: EntityRef,
    /// What it came from, in the snapshot the edit began from.
    pub old: EntityRef,
    /// How.
    pub how: Derivation,
}

/// What an edit created, changed and deleted, each entity once and in id order, and where each new entity came from in
/// the order the edit said (the brief).
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Changes {
    /// Entities that did not exist before, including any made in a slot the edit freed.
    pub created: Vec<EntityRef>,
    /// Entities that existed before and were given a new record, keeping their ids.
    pub changed: Vec<EntityRef>,
    /// Entities that existed before and are gone.
    pub deleted: Vec<EntityRef>,
    /// Where new entities came from.
    pub lineage: Vec<Lineage>,
}

/// A sequence of changes to a snapshot, applied on one thread, then checked and committed or dropped.
pub struct Edit {
    pub(crate) base: Snapshot,
    pub(crate) next: Model,
    step: u64,
    pub(crate) created: BTreeSet<EntityRef>,
    pub(crate) changed: BTreeSet<EntityRef>,
    pub(crate) deleted: BTreeSet<EntityRef>,
    lineage: Vec<Lineage>,
    /// The kinds whose tables had no slot left to give.
    pub(crate) full: BTreeSet<EntityKind>,
}

impl Snapshot {
    /// An edit of this snapshot, as the model's next step.
    #[must_use]
    pub fn edit(&self) -> Edit {
        Edit {
            base: self.clone(),
            next: self.model.clone(),
            step: self.model.step.saturating_add(1),
            created: BTreeSet::new(),
            changed: BTreeSet::new(),
            deleted: BTreeSet::new(),
            lineage: Vec::new(),
            full: BTreeSet::new(),
        }
    }
}

impl Edit {
    /// The model's step this edit is: one more than its snapshot's.
    #[must_use]
    pub const fn step(&self) -> u64 {
        self.step
    }

    /// The provenance of an entity this step makes in `role`.
    #[must_use]
    pub const fn provenance(&self, role: StepRole) -> Provenance {
        Provenance { step: self.step, role }
    }

    /// The snapshot it began from, unchanged.
    #[must_use]
    pub const fn base(&self) -> &Snapshot {
        &self.base
    }

    /// A copy of the record `id` has now, in this edit.
    #[must_use]
    pub fn record<K: Kind>(&self, id: Id<K>) -> Option<K::Record> {
        K::table(&self.next).live(id.slot(), id.generation()).cloned()
    }

    /// An id for a record written later in this edit, so that cycles, such as an edge and its uses, can be built.
    /// `commit` refuses a reserved id that was never written. If the kind's table has no slot left, the id names
    /// nothing, and `commit` refuses the edit.
    pub fn reserve<K: Kind>(&mut self) -> Id<K> {
        if let Some((slot, generation)) = K::table_mut(&mut self.next).reserve() {
            let id = Id::new(slot, generation);
            self.created.insert(id.entity());
            id
        } else {
            self.full.insert(K::ENTITY);
            Id::new(u32::MAX, NonZeroU32::MIN)
        }
    }

    /// A new entity with `record`, of the kind it is the record of.
    pub fn add<R: Record>(&mut self, record: R) -> Id<R::Kind> {
        self.write(record)
    }

    /// Writes `record` as a new entity of kind `K`.
    fn write<K: Kind>(&mut self, record: K::Record) -> Id<K> {
        let id = self.reserve::<K>();
        K::table_mut(&mut self.next).put(id.slot(), id.generation(), record);
        id
    }

    /// Gives `id` a new record. An entity changed in place keeps its id.
    ///
    /// # Errors
    ///
    /// `topo.stale-id` if `id` is not live, or reserved in this edit.
    pub fn set<K: Kind>(&mut self, id: Id<K>, record: K::Record) -> Result<(), Report> {
        let entity = id.entity();
        if !K::table_mut(&mut self.next).put(id.slot(), id.generation(), record) {
            return Err(stale(entity, self.next.live_generation(entity)));
        }
        if !self.created.contains(&entity) {
            self.changed.insert(entity);
        }
        Ok(())
    }

    /// Removes `id`, live or reserved. One created in this edit then appears in no list of the changes.
    ///
    /// # Errors
    ///
    /// `topo.stale-id` if `id` is neither live nor reserved in this edit.
    pub fn remove<K: Kind>(&mut self, id: Id<K>) -> Result<(), Report> {
        let entity = id.entity();
        if !K::table_mut(&mut self.next).free(id.slot(), id.generation()) {
            return Err(stale(entity, self.next.live_generation(entity)));
        }
        if self.created.remove(&entity) {
            self.lineage.retain(|lineage| lineage.new != entity);
        } else {
            self.changed.remove(&entity);
            self.deleted.insert(entity);
        }
        Ok(())
    }

    /// Records that `new` came from `old` by `how`. On commit a new entity takes the provenance of the first entity
    /// of its own kind it came from, over whatever the edit wrote, so a split face keeps the step that made it.
    ///
    /// # Errors
    ///
    /// `topo.stale-id` if `new` is neither live nor reserved in this edit, or `old` is not live in the snapshot the
    /// edit began from.
    pub fn derive(&mut self, new: EntityRef, old: EntityRef, how: Derivation) -> Result<(), Report> {
        if self.next.holds(new) == Holds::Nothing {
            return Err(stale(new, self.next.live_generation(new)));
        }
        if !self.base.model.is_live(old) {
            return Err(stale(old, self.base.model.live_generation(old)));
        }
        self.lineage.push(Lineage { new, old, how });
        Ok(())
    }

    /// Checks the edit and hands back the new snapshot and what changed. Nothing else makes a snapshot, so no
    /// unchecked model exists, in release builds too.
    ///
    /// # Errors
    ///
    /// One `check.invariant-failed` per failure, fault internal, its reason the invariant and its entities those
    /// involved. The snapshot the edit began from is unchanged.
    pub fn commit(mut self, _exec: &Exec) -> Result<(Snapshot, Changes), Vec<Report>> {
        let failures = check::edit(&self);
        if !failures.is_empty() {
            return Err(failures);
        }
        self.carry_provenance();
        self.next.step = self.step;
        let changes = Changes {
            created: self.created.into_iter().collect(),
            changed: self.changed.into_iter().collect(),
            deleted: self.deleted.into_iter().collect(),
            lineage: self.lineage,
        };
        Ok((Snapshot { model: self.next }, changes))
    }

    /// Gives each new entity the provenance of the first entity of its own kind it came from.
    fn carry_provenance(&mut self) {
        let mut carried = BTreeSet::new();
        for lineage in &self.lineage {
            let (new, old) = (lineage.new, lineage.old);
            if new.kind != old.kind || !self.created.contains(&new) || !carried.insert(new) {
                continue;
            }
            if let Some(born) = self.base.model.born(old) {
                self.next.set_born(new, born);
            }
        }
    }
}

#[cfg(test)]
mod tests;
