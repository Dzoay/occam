//! Source: original
//! The second way to make a snapshot: from what a file holds, slot by slot (PARITY 2.17). Nothing from outside is
//! trusted, so it is as strict as [`Edit::commit`](crate::Edit::commit): every edge's deviation from its faces is
//! measured again and stored (the deviation cap's decision), and the whole model is checked. Any failure refuses it.

use core::num::NonZeroU32;

use occam_math::Exec;
use occam_report::Report;
use occam_units::Tolerance;

use crate::geometric;
use crate::kind::Kind;
use crate::records::StepNote;
use crate::snapshot::{Model, Snapshot};
use crate::table::{State, Table};

/// One slot of a table as a file holds it: the generation of its latest entity, and that entity's record if it is live.
#[derive(Clone, Debug, PartialEq)]
pub struct Saved<R> {
    /// The generation of the slot's latest entity.
    pub generation: NonZeroU32,
    /// Its record, or `None` where the slot is free.
    pub record: Option<R>,
}

/// A model being read back from a file, table by table, then measured and checked by [`Restore::finish`].
#[derive(Clone, Debug)]
pub struct Restore {
    model: Model,
}

impl Restore {
    /// An empty model built to `tolerance`, whose last committed step was `step`.
    #[must_use]
    pub const fn new(tolerance: Tolerance, step: u64) -> Self {
        let mut model = Model::empty(tolerance, u32::MAX);
        model.step = step;
        Self { model }
    }

    /// Gives kind `K`'s table `slots`, in slot order, in place of what it had.
    pub fn table<K: Kind>(&mut self, slots: Vec<Saved<K::Record>>) {
        *K::table_mut(&mut self.model) = Table::restore(slots.into_iter().map(|s| (s.generation, s.record)));
    }

    /// Gives the step `step` the note its request named it by, in place of what it had.
    pub fn note(&mut self, step: u64, note: StepNote) {
        self.model.notes.insert(step, note);
    }

    /// The snapshot: every edge's deviation measured and stored, then the whole model checked on `exec`.
    ///
    /// # Errors
    ///
    /// Every failure: `check.invariant-failed` with its reason and entities, one for each edge whose distance from a
    /// face could not be measured and one for each invariant broken, as `commit` and `Snapshot::check_all` give them.
    pub fn finish(self, exec: &Exec) -> Result<Snapshot, Vec<Report>> {
        let mut model = self.model;
        let mut failures = geometric::measure_all(&mut model, exec);
        let snapshot = Snapshot { model };
        failures.extend(snapshot.check_all(exec));
        if failures.is_empty() { Ok(snapshot) } else { Err(failures) }
    }
}

impl Snapshot {
    /// Every slot of kind `K`'s table, in slot order, as a file saves it: the generation of its latest entity, and that
    /// entity's record if it is live.
    pub fn saved<K: Kind>(&self) -> impl Iterator<Item = Saved<&K::Record>> + '_ {
        K::table(&self.model).slots().filter_map(|(_, slot)| {
            let generation = NonZeroU32::new(slot.generation())?;
            let record = match slot.state() {
                State::Live(record) => Some(record),
                State::Free | State::Reserved => None,
            };
            Some(Saved { generation, record })
        })
    }
}

#[cfg(test)]
mod tests;
