//! Source: original
//! A model between commands (docs/architecture/command-line.md): opened from the native format in either encoding,
//! which measures and checks it whole, so a model a session holds is sound; changed only by a request that goes through
//! whole; asked what it holds; and meshed, after a run only where the run touched it. Every command takes many ids and
//! gives one answer per id, so an id that names nothing is refused alone, `topo.stale-id`, and the others are answered
//! (L6). The command line and the browser build both wrap this, and its answers' one JSON form is [`Answered::json`].
//! The browser build alone also opens, without checking it again, the twin a checked session saved, for its mesh
//! workers.

mod wire;

use std::borrow::Cow;
use std::ops::Range;

use occam_math::Exec;
use occam_mesh::Mesh;
use occam_ops::default_tolerance;
use occam_report::{Description, EntityKind, EntityRef, Report};
use occam_topo::{Changes, Face, Snapshot};

pub use wire::ran_words;
pub(crate) use wire::{answer, line};

use crate::describe::{self, Source};
use crate::mesh::{Again, Meshed, limits};
use crate::pick::{Pick, Picked, Picking};
use crate::run::{Failed, Ran};

/// Which of the native format's two encodings a model is saved in.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Encoding {
    /// The readable JSON, which is also the dump.
    Json,
    /// The binary twin, which holds the same records and loads faster.
    Binary,
}

/// The live ids of one kind.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Listed {
    /// The kind asked for.
    pub kind: EntityKind,
    /// Its live ids, in slot order.
    pub entities: Vec<EntityRef>,
}

/// One id asked about, and its answer or why it has none.
pub type PerId<T> = (EntityRef, Result<T, Report>);

/// What a command answered, each answer in the order asked.
#[derive(Clone, Debug, PartialEq)]
pub enum Answered {
    /// The live ids of each kind asked for.
    Listed(Vec<Listed>),
    /// What each id names.
    Described(Vec<PerId<Description>>),
    /// The neighbours of what each id names.
    Adjacent(Vec<PerId<Vec<EntityRef>>>),
    /// What checking the model found, by opening it: nothing, for a sound model.
    Checked(Vec<Report>),
    /// Why the command could not answer at all: the model would not open, or a file would not read.
    Refused(Vec<Report>),
}

/// A model, how it is saved, and its last mesh.
#[derive(Clone, Debug)]
pub struct Session {
    /// The model, once a request has made one or one was opened.
    model: Option<Snapshot>,
    /// How it is saved.
    encoding: Encoding,
    /// The model's last mesh, and what every run kept since it changed: what the next mesh starts from.
    meshed: Option<(Mesh, Vec<Changes>)>,
    /// The model's mesh at the limits last picked at, made ready to pick, until the model changes.
    picking: Option<Box<Picking>>,
}

impl Session {
    /// A session with no model yet, whose first request makes one, saved in `encoding`.
    #[must_use]
    pub const fn new(encoding: Encoding) -> Self {
        Self { model: None, encoding, meshed: None, picking: None }
    }

    /// The model `bytes` hold in either encoding, which it keeps, measured and checked whole on `exec` as `commit`
    /// would: nothing from outside is trusted.
    ///
    /// # Errors
    ///
    /// As `occam_io::load`: bytes that are not the format or not this version, a damaged binary file, and every check
    /// the model fails, each naming its entities.
    pub fn open(bytes: &[u8], exec: &Exec) -> Result<Self, Vec<Report>> {
        Ok(Self::holding(occam_io::load(bytes, exec)?, bytes))
    }

    /// The model `bytes` hold in either encoding, which a session of this program measured and checked before it saved
    /// them: read as [`Session::open`] reads them, refusing bytes that are not the format, but neither measured nor
    /// checked again. Only the browser build has it, through the `trusted` feature, for its mesh workers, which open
    /// the twin the editing worker checked (docs/decisions/2026-11-16-mesh-workers-trust-the-editing-worker.md); the
    /// `occam` command, which opens files, is built without it.
    ///
    /// # Errors
    ///
    /// As `occam_io::parse`: bytes that are not the format or not this version, a damaged binary file, and a curve or
    /// surface its family would not make, naming it.
    #[cfg(feature = "trusted")]
    pub fn open_trusted(bytes: &[u8]) -> Result<Self, Vec<Report>> {
        Ok(Self::holding(occam_io::parse(bytes)?.trusted(), bytes))
    }

    /// A session holding `model`, read from `bytes`, saved in their encoding from now on.
    fn holding(model: Snapshot, bytes: &[u8]) -> Self {
        let encoding = if bytes.starts_with(occam_io::MAGIC) { Encoding::Binary } else { Encoding::Json };
        Self { model: Some(model), encoding, meshed: None, picking: None }
    }

    /// How the model is saved.
    #[must_use]
    pub const fn encoding(&self) -> Encoding {
        self.encoding
    }

    /// Saves the model in `encoding` from now on.
    pub const fn save_as(&mut self, encoding: Encoding) {
        self.encoding = encoding;
    }

    /// The model, if there is one yet.
    #[must_use]
    pub const fn model(&self) -> Option<&Snapshot> {
        self.model.as_ref()
    }

    /// Runs `request` on the model. A run that goes through and changes the model is kept, and its bytes in the
    /// session's encoding come with it. A run that stops short leaves the model as it was, so the whole request,
    /// mended, can be run again; so does one whose model the format cannot write yet, which the run's failure names
    /// (`io.not-saved`, with no step).
    pub fn run(&mut self, request: &str) -> (Ran, Option<Vec<u8>>) {
        let mut ran = crate::run(self.model.as_ref(), request);
        let before = self.model.as_ref().map_or(0, Snapshot::step);
        let Some(next) = ran.model.as_ref().filter(|next| ran.failed.is_none() && next.step() != before) else {
            return (ran, None);
        };
        match self.encode(next) {
            Ok(bytes) => {
                self.model = Some(next.clone());
                self.picking = None;
                if let Some((_, since)) = &mut self.meshed {
                    since.extend(ran.done.iter().map(|done| done.changes.clone()));
                }
                (ran, Some(bytes))
            }
            Err(report) => {
                let report = describe::report(report, &[Source::Model(next)]);
                ran.failed = Some(Failed { step: None, reports: vec![report] });
                (ran, None)
            }
        }
    }

    /// `model` in the session's encoding.
    fn encode(&self, model: &Snapshot) -> Result<Vec<u8>, Report> {
        encode(model, self.encoding)
    }

    /// The model's bytes in `encoding`, once there is a model: as the binary twin, what `occam view` sends its page,
    /// whichever encoding the model came in.
    ///
    /// # Errors
    ///
    /// `io.not-saved`, as a run's failure names it, for a model the format cannot write yet.
    #[must_use]
    pub fn encoded(&self, encoding: Encoding) -> Option<Result<Vec<u8>, Report>> {
        let model = self.model.as_ref()?;
        Some(encode(model, encoding).map_err(|report| describe::report(report, &[Source::Model(model)])))
    }

    /// The model's digest (L23), once there is a model: two models with the same digest are the same model, so it is
    /// what names the model a page shows.
    #[must_use]
    pub fn digest(&self) -> Option<u64> {
        self.model.as_ref().map(digest_of)
    }

    /// The model to answer from: an empty one before any request has made one.
    fn answering(&self) -> Result<Cow<'_, Snapshot>, Report> {
        answering(self.model.as_ref())
    }

    /// The model meshed on `exec` to a chordal `deflection` in millimetres and an `angle` in radians, with its buffers
    /// and one answer per face. After a run, at the same limits, only the faces the runs since the last mesh touched,
    /// and those along an edge whose samples changed, are meshed again; the rest keep their blocks bit for bit. A
    /// session with no model yet meshes an empty one.
    ///
    /// # Errors
    ///
    /// `mesh.limit-not-positive` for a deflection or an angle that is not a finite number greater than zero.
    pub fn mesh(&mut self, exec: &Exec, deflection: f64, angle: f64) -> Result<Meshed, Vec<Report>> {
        let limits = limits(deflection, angle).map_err(|report| vec![report])?;
        let model = self.answering().map_err(|report| vec![report])?.into_owned();
        let (mesh, meshed) = match self.meshed.take() {
            Some((before, since)) if before.edges.limits() == limits => {
                let again = occam_mesh::remesh(exec, &model, &before, &since);
                (again.mesh, again.meshed)
            }
            _ => (occam_mesh::mesh(exec, &model, limits), model.ids()),
        };
        let answer = Meshed::new(limits, &model, &mesh, &meshed);
        self.meshed = Some((mesh, Vec::new()));
        Ok(answer)
    }

    /// How many faces the model has, which [`Session::mesh_part`] counts its ranges in: none before a request has made
    /// a model.
    #[must_use]
    pub fn face_count(&self) -> usize {
        self.model.as_ref().map_or(0, |model| model.ids::<Face>().len())
    }

    /// The model's faces from `faces.start` up to `faces.end`, counted in id order, meshed on `exec` to a chordal
    /// `deflection` in millimetres and an `angle` in radians, with one answer per face: a part of the mesh
    /// [`Session::mesh`] gives, as a mesh worker meshes a range of faces. Its buffers are numbered from its own start,
    /// and each face's block holds the bits the whole mesh's does, so the parts of the model's faces, laid end to end in
    /// id order, are the whole mesh's buffers. Its reports refuse the edges its faces use and say what checking the part
    /// watertight as far as it reaches found. A range past the last face stops at it. The session's last mesh is
    /// neither used nor kept, since a mesh worker's session never changes its model.
    ///
    /// # Errors
    ///
    /// `mesh.limit-not-positive` for a deflection or an angle that is not a finite number greater than zero.
    pub fn mesh_part(
        &self,
        exec: &Exec,
        deflection: f64,
        angle: f64,
        faces: Range<usize>,
    ) -> Result<Meshed, Vec<Report>> {
        let limits = limits(deflection, angle).map_err(|report| vec![report])?;
        let model = self.answering().map_err(|report| vec![report])?;
        let ids = model.ids::<Face>();
        let end = faces.end.min(ids.len());
        let part = ids.get(faces.start.min(end)..end).unwrap_or_default();
        let mesh = occam_mesh::mesh_part(exec, &model, limits, part);
        Ok(Meshed::new(limits, &model, &mesh, part))
    }

    /// What a page that drew the model of `before`, meshed to a chordal `deflection` in millimetres and an `angle` in
    /// radians, must mesh again on `exec` to draw this one meshed to them: the faces whose blocks or refusals may
    /// differ, counted in id order as [`Session::mesh_part`] counts them, and what keeps its mesh. A session with no
    /// model yet holds an empty one, so after it every face is meshed again.
    ///
    /// # Errors
    ///
    /// `mesh.limit-not-positive` for a deflection or an angle that is not a finite number greater than zero.
    pub fn again(&self, before: &Self, exec: &Exec, deflection: f64, angle: f64) -> Result<Again, Vec<Report>> {
        let limits = limits(deflection, angle).map_err(|report| vec![report])?;
        let (was, now) = (before.answering(), self.answering());
        let (was, now) = (was.map_err(|report| vec![report])?, now.map_err(|report| vec![report])?);
        Ok(Again::new(exec, &was, &now, limits))
    }

    /// What the ray `asked` hits first in the model's mesh, made on `exec` to a chordal `deflection` in millimetres
    /// and an `angle` in radians: the id of the face it crosses first, or of an edge within its tolerance that wins
    /// over that face, the point hit and how far along the ray; or `mesh.ray-missed`, with a point to aim at. The mesh
    /// is the one [`Session::mesh`] gives, the same bits at any thread count, and the session keeps it, with its box
    /// trees, for the next pick at the same limits until the model changes. A session with no model yet picks an
    /// empty one.
    ///
    /// # Errors
    ///
    /// `mesh.limit-not-positive` for a deflection or an angle that is not a finite number greater than zero;
    /// `units.not-finite` for a start or a tolerance that is not finite, `input.no-direction` for a direction that is
    /// zero or not finite, and `input.negative` for a tolerance below zero.
    pub fn pick(&mut self, exec: &Exec, deflection: f64, angle: f64, asked: Pick) -> Result<Picked, Vec<Report>> {
        let (model, picking) = self.picking(exec, deflection, angle)?;
        picking.pick(&model, asked).map_err(|report| vec![report])
    }

    /// The samples of `edge` in the model's mesh made on `exec` to a chordal `deflection` in millimetres and an
    /// `angle` in radians, kept as [`Session::pick`] keeps it: what the page draws a selected or highlighted edge
    /// through. `None` for an id that is not an edge a face uses, or whose samples were refused.
    ///
    /// # Errors
    ///
    /// `mesh.limit-not-positive` for a deflection or an angle that is not a finite number greater than zero.
    pub fn samples(
        &mut self,
        exec: &Exec,
        deflection: f64,
        angle: f64,
        edge: EntityRef,
    ) -> Result<Option<Vec<[f64; 3]>>, Vec<Report>> {
        let (_, picking) = self.picking(exec, deflection, angle)?;
        Ok(picking.samples(edge))
    }

    /// The model, and its mesh at the limits made ready to pick: the one kept if it was made to them, else one made
    /// now with the model's digest, which is computed only then. What is kept is dropped when a run changes the model.
    fn picking(
        &mut self,
        exec: &Exec,
        deflection: f64,
        angle: f64,
    ) -> Result<(Cow<'_, Snapshot>, &Picking), Vec<Report>> {
        let limits = limits(deflection, angle).map_err(|report| vec![report])?;
        let Self { model: held, picking, .. } = self;
        let held = &*held;
        let model = answering(held.as_ref()).map_err(|report| vec![report])?;
        let kept = picking.take().filter(|picking| picking.limits == limits);
        let made =
            || Box::new(Picking::new(limits, held.as_ref().map(digest_of), occam_mesh::mesh(exec, &model, limits)));
        let picking = picking.insert(kept.unwrap_or_else(made));
        Ok((model, picking))
    }

    /// The live ids of each of `kinds`, in the order asked; none of a kind no model holds.
    #[must_use]
    pub fn list(&self, kinds: &[EntityKind]) -> Vec<Listed> {
        let listed = |kind| self.model.as_ref().map_or_else(Vec::new, |model| model.entities(kind));
        kinds.iter().map(|&kind| Listed { kind, entities: listed(kind) }).collect()
    }

    /// What each of `ids` names, as the one resolver describes it, or `topo.stale-id` for one that names nothing live.
    #[must_use]
    pub fn describe(&self, ids: &[EntityRef]) -> Vec<PerId<Description>> {
        let model = match self.answering() {
            Ok(model) => model,
            Err(report) => return ids.iter().map(|&id| (id, Err(report.clone()))).collect(),
        };
        let one = |id: EntityRef| -> Result<Description, Report> {
            model.resolve_entity(id).map_err(|report| describe::report(report, &[Source::Model(&model)]))?;
            let mut described = describe::describe(&model, &[id]);
            Ok(described.pop().unwrap_or_else(|| Description::none(id)))
        };
        ids.iter().map(|&id| (id, one(id))).collect()
    }

    /// The neighbours of what each of `ids` names, as its description lists them (PARITY 1.1), or `topo.stale-id` for
    /// one that names nothing live.
    #[must_use]
    pub fn adjacent(&self, ids: &[EntityRef]) -> Vec<PerId<Vec<EntityRef>>> {
        self.describe(ids).into_iter().map(|(id, described)| (id, described.map(|d| d.adjacent))).collect()
    }
}

/// `model`, or an empty one where there is none yet.
fn answering(model: Option<&Snapshot>) -> Result<Cow<'_, Snapshot>, Report> {
    match model {
        Some(model) => Ok(Cow::Borrowed(model)),
        None => default_tolerance().map(|tolerance| Cow::Owned(Snapshot::empty(tolerance))),
    }
}

#[cfg(test)]
thread_local! {
    static DIGESTS: core::cell::Cell<usize> = const { core::cell::Cell::new(0) };
}

/// `model`'s digest, counted in tests, so that a test can show which calls compute one: on a large model it takes as
/// long as many picks.
fn digest_of(model: &Snapshot) -> u64 {
    #[cfg(test)]
    DIGESTS.with(|digests| digests.set(digests.get().saturating_add(1)));
    model.digest()
}

/// How many digests sessions have computed on this thread.
#[cfg(test)]
fn digests() -> usize {
    DIGESTS.with(core::cell::Cell::get)
}

/// `model` in `encoding`.
fn encode(model: &Snapshot, encoding: Encoding) -> Result<Vec<u8>, Report> {
    match encoding {
        Encoding::Json => occam_io::save(model),
        Encoding::Binary => occam_io::save_binary(model),
    }
}

#[cfg(test)]
mod tests;
