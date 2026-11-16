//! Source: original
//! The selection and the highlight (docs/architecture/live-view.md#the-selection-file): ids of a model, in the order
//! they were selected, with the digest of the model they were selected on, as the files beside the model hold them,
//! `part.occam.selection` and `part.occam.highlight`. The page posts what the person selects, `occam selection` reads
//! it, `occam highlight` writes the agent's ids, and `occam run` carries the selection over through its changes: an
//! entity changed keeps its id, one replaced, split, trimmed or merged into others, becomes those, and one deleted
//! drops out. A copy, or what a sweep made of an entity, is something new, so what was copied stays selected and what
//! a sweep took drops out. A selection whose digest is not the model's is stale and names nothing.

use occam_report::{Code, EntityRef, Fault, Input, Remedy, RemedyKind, Report, SCHEMA};
use occam_topo::{Changes, Derivation};
use serde_json::{Value, json};

use crate::run::Ran;
use crate::view::digest_text;

/// Ids of a model, in the order they were selected, and the digest of the model they were selected on.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Selection {
    /// The digest of the model the ids were selected on.
    pub digest: u64,
    /// The ids, in the order they were selected, each once.
    pub ids: Vec<EntityRef>,
}

/// What became of a selected id through a run: the ids it is now, none if it is gone.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Carried {
    /// The id as it was selected.
    pub was: EntityRef,
    /// What it is after the run, in the order the run made them: itself if it is still there, what replaced it, or
    /// nothing.
    pub now: Vec<EntityRef>,
}

impl Selection {
    /// `ids` of the model whose digest is `digest`, each kept where it first comes.
    #[must_use]
    pub fn new(digest: u64, ids: Vec<EntityRef>) -> Self {
        Self { digest, ids: once(ids) }
    }

    /// Its one JSON form, which its file holds: `{"schema", "digest", "ids"}`, the digest as sixteen hexadecimal
    /// digits.
    #[must_use]
    pub fn json(&self) -> String {
        json!({"schema": SCHEMA, "digest": digest_text(self.digest), "ids": self.ids}).to_string()
    }

    /// The JSON form of no selection, which the viewer sends its page where there is no file: no digest and no ids.
    #[must_use]
    pub fn none_json() -> String {
        json!({"schema": SCHEMA, "digest": Value::Null, "ids": []}).to_string()
    }

    /// The selection `text`, its one JSON form, holds, or `None` for text that is not one. An id given twice is kept
    /// where it was first given.
    #[must_use]
    pub fn from_json(text: &str) -> Option<Self> {
        let value: Value = serde_json::from_str(text).ok()?;
        value.get("schema")?.as_u64().filter(|&schema| schema == u64::from(SCHEMA))?;
        let digest = u64::from_str_radix(value.get("digest")?.as_str()?, 16).ok()?;
        let ids = value.get("ids")?.as_array()?.iter().map(|id| EntityRef::parse(id.as_str()?));
        Some(Self::new(digest, ids.collect::<Option<Vec<EntityRef>>>()?))
    }

    /// `view.selection-stale` for a selection made on another model than the one `model`, a file, holds now, with the
    /// remedy of selecting again.
    #[must_use]
    pub fn stale(model: &str) -> Report {
        Report::new(Code::SelectionStale, Fault::InvalidInput)
            .with_input(Input::at("--model").given(model))
            .with_remedy(Remedy::new(RemedyKind::SelectAgain))
    }

    /// The selection carried over through `ran`'s changes, step by step, onto the model whose digest is `digest`,
    /// and what became of each id.
    #[must_use]
    pub fn carried(&self, ran: &Ran, digest: u64) -> (Self, Vec<Carried>) {
        let each = |&was: &EntityRef| {
            let now = ran
                .done
                .iter()
                .fold(vec![was], |ids, done| once(ids.iter().flat_map(|&id| after(id, &done.changes)).collect()));
            Carried { was, now }
        };
        let carried: Vec<Carried> = self.ids.iter().map(each).collect();
        (Self::new(digest, carried.iter().flat_map(|c| c.now.iter().copied()).collect()), carried)
    }
}

/// What `id` is after a step that made `changes`: itself, unless the step deleted it, and then what replaced it.
fn after(id: EntityRef, changes: &Changes) -> Vec<EntityRef> {
    if !changes.deleted.contains(&id) {
        return vec![id];
    }
    let replaces = |how: Derivation| matches!(how, Derivation::Split | Derivation::Trimmed | Derivation::Merged);
    changes.lineage.iter().filter(|l| l.old == id && replaces(l.how)).map(|l| l.new).collect()
}

/// `ids`, each kept where it first comes.
fn once(ids: Vec<EntityRef>) -> Vec<EntityRef> {
    let mut seen = std::collections::BTreeSet::new();
    ids.into_iter().filter(|&id| seen.insert(id)).collect()
}

#[cfg(test)]
mod tests;
