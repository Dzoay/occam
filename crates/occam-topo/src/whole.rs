//! Source: original
//! The whole model against every invariant (PARITY 1.10), for loading, importing and tests: `commit` checks only what
//! an edit touched, and this checks everything. Each task checks one chunk of one table, so the tasks follow the
//! storage and their results come back in task order through `Exec::map`: entity by entity, kind by kind in the order
//! [`EntityKind::ALL`] lists them, slot by slot, then body by body, the faces that name each shell found first by one
//! scan. The reports are the same at any thread count.

use core::num::NonZeroU32;
use std::sync::Arc;

use occam_math::{Exec, Grain};
use occam_report::{EntityKind, EntityRef, Report};

use crate::check::{self, Members};
use crate::kind::{Body, Id, Kind, Visit, dispatch};
use crate::snapshot::{Model, Snapshot};

/// The slots one task checks: one chunk of a table.
const TASK: NonZeroU32 = match NonZeroU32::new(1024) {
    Some(task) => task,
    None => NonZeroU32::MIN,
};

/// What one task checks: the entities of a kind in a run of slots, or the bodies in one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Task {
    /// Each live entity of the kind, in slots `from` to `from + TASK`.
    Entities(EntityKind, u32),
    /// Each live body as a whole, in slots `from` to `from + TASK`.
    Bodies(u32),
}

/// How many slots a kind's table has.
struct Slots<'a>(&'a Model);

impl Visit for Slots<'_> {
    type Out = u32;

    fn visit<K: Kind>(self) -> u32 {
        u32::try_from(K::table(self.0).len()).unwrap_or(u32::MAX)
    }
}

/// The live entities of a kind in a run of slots, in slot order.
struct Live<'a> {
    model: &'a Model,
    from: u32,
    to: u32,
}

impl Visit for Live<'_> {
    type Out = Vec<EntityRef>;

    fn visit<K: Kind>(self) -> Vec<EntityRef> {
        let table = K::table(self.model);
        let live = |slot| table.live_generation(slot).map(|generation| EntityRef::new(K::ENTITY, slot, generation));
        (self.from..self.to).filter_map(live).collect()
    }
}

/// The first slot of each run of `per` in a table of `len` slots.
fn runs(len: u32, per: NonZeroU32) -> impl Iterator<Item = u32> {
    (0..len.div_ceil(per.get())).map(move |run| run.saturating_mul(per.get()))
}

/// Every task, in the order their reports are given.
fn tasks(model: &Model, per: NonZeroU32) -> Vec<Task> {
    let mut tasks: Vec<Task> = EntityKind::ALL
        .iter()
        .filter_map(|&kind| dispatch(kind, Slots(model)).map(|len| (kind, len)))
        .flat_map(|(kind, len)| runs(len, per).map(move |from| Task::Entities(kind, from)))
        .collect();
    tasks.extend(runs(dispatch(EntityKind::Body, Slots(model)).unwrap_or(0), per).map(Task::Bodies));
    tasks
}

/// The reports of one task, the faces that name each shell being `members`.
fn run(model: &Model, members: &Members, task: Task, per: NonZeroU32) -> Vec<Report> {
    let mut out = Vec::new();
    match task {
        Task::Entities(kind, from) => {
            let live = Live { model, from, to: from.saturating_add(per.get()) };
            for entity in dispatch(kind, live).unwrap_or_default() {
                check::entity(model, entity, &mut out);
            }
        }
        Task::Bodies(from) => {
            let live = Live { model, from, to: from.saturating_add(per.get()) };
            for entity in live.visit::<Body>() {
                if let Some(generation) = NonZeroU32::new(entity.generation) {
                    check::body(model, members, Id::new(entity.slot, generation), &mut out);
                }
            }
        }
    }
    out
}

/// Every report of `model`, `per` slots a task, on `exec`.
fn checked(model: Model, exec: &Exec, per: NonZeroU32) -> Vec<Report> {
    let members = Arc::new(check::members(&model));
    let model = Arc::new(model);
    let tasks = Arc::new(tasks(&model, per));
    let count = tasks.len();
    let run = move |index: usize| tasks.get(index).map(|&task| run(&model, &members, task, per)).unwrap_or_default();
    let found = exec.map(count, Grain::Each, run);
    found.into_iter().flatten().collect()
}

impl Snapshot {
    /// The whole model against every invariant: for loading, importing and tests (PARITY 1.10). One report per failure,
    /// each naming its entities, entity by entity in the order of [`EntityKind::ALL`] and then of the slots, then body
    /// by body; the same list at any thread count.
    #[must_use]
    pub fn check_all(&self, exec: &Exec) -> Vec<Report> {
        checked(self.model.clone(), exec, TASK)
    }
}

#[cfg(test)]
mod tests;
