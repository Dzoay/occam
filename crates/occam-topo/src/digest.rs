//! Source: original
//! A snapshot's digest (L23): 64-bit FNV-1a through occam-math's [`Digest`], over the tolerance, the step counter, and
//! every slot of every table, kind by kind in the order [`EntityKind::ALL`] lists them: each table's slot count, then
//! each slot's state and generation, and a live slot's record, with each curve and surface written as its content,
//! never its address. Free slots count because ids depend only on the table: two snapshots with the same digest give
//! the same ids to the same next edit. Every NaN is written as one pattern and the two zeros stay distinct, as
//! everywhere occam hashes. `gates determinism` compares digests across builds and thread counts.
//!
//! Every record is taken apart field by field with no `..`, so a field added later does not compile until it is
//! written here.

use occam_geom::Content;
use occam_math::Digest;
use occam_report::{EntityKind, StepRole};
use occam_units::{Length, Point, UvPoint, World};

use crate::kind::{Id, Kind};
use crate::records::{
    Around, BodyKind, BodyRecord, CurveRecord, EdgeRecord, EdgeRole, EdgeUseRecord, End, FaceRecord, LoopRecord,
    ParamCurveRecord, Provenance, Sheet, ShellRecord, SurfaceRecord, UseOwner, VertexRecord, WireRecord,
};
use crate::snapshot::{Model, Snapshot};
use crate::table::{State, Table};

/// What can be written to a digest.
trait Hashed {
    /// Writes it.
    fn hash(&self, digest: &mut Digest);
}

impl Hashed for u64 {
    fn hash(&self, digest: &mut Digest) {
        digest.write_u64(*self);
    }
}

impl Hashed for u32 {
    fn hash(&self, digest: &mut Digest) {
        digest.write_u64(u64::from(*self));
    }
}

impl Hashed for i16 {
    fn hash(&self, digest: &mut Digest) {
        digest.write_bytes(&self.to_le_bytes());
    }
}

impl Hashed for bool {
    fn hash(&self, digest: &mut Digest) {
        digest.write_bytes(&[u8::from(*self)]);
    }
}

impl Hashed for f64 {
    fn hash(&self, digest: &mut Digest) {
        digest.write_value(*self);
    }
}

impl Hashed for str {
    fn hash(&self, digest: &mut Digest) {
        u64::try_from(self.len()).unwrap_or(u64::MAX).hash(digest);
        digest.write_bytes(self.as_bytes());
    }
}

impl<T: Hashed> Hashed for Option<T> {
    fn hash(&self, digest: &mut Digest) {
        self.is_some().hash(digest);
        if let Some(value) = self {
            value.hash(digest);
        }
    }
}

impl<A: Hashed, B: Hashed> Hashed for (A, B) {
    fn hash(&self, digest: &mut Digest) {
        self.0.hash(digest);
        self.1.hash(digest);
    }
}

impl<K: Kind> Hashed for Id<K> {
    fn hash(&self, digest: &mut Digest) {
        self.slot().hash(digest);
        self.generation().get().hash(digest);
    }
}

impl Hashed for Length {
    fn hash(&self, digest: &mut Digest) {
        self.millimetres().hash(digest);
    }
}

impl Hashed for Point<World> {
    fn hash(&self, digest: &mut Digest) {
        let v = self.to_vec3();
        for x in [v.x, v.y, v.z] {
            x.hash(digest);
        }
    }
}

impl Hashed for UvPoint {
    fn hash(&self, digest: &mut Digest) {
        (self.u(), self.v()).hash(digest);
    }
}

/// Enums are written as their names, so reordering a declaration changes no digest.
macro_rules! named {
    ($($kind:ty { $($variant:ident => $name:literal),* $(,)? })*) => {
        $(
            impl Hashed for $kind {
                fn hash(&self, digest: &mut Digest) {
                    let name = match self {
                        $(Self::$variant => $name,)*
                    };
                    name.hash(digest);
                }
            }
        )*
    };
}

named! {
    BodyKind { Solid => "solid", Sheet => "sheet", Wire => "wire" }
    EdgeRole { Ordinary => "ordinary", Seam => "seam", Section => "section" }
    End { Start => "start", Finish => "finish" }
}

impl Hashed for StepRole {
    fn hash(&self, digest: &mut Digest) {
        self.text().hash(digest);
    }
}

impl Hashed for Provenance {
    fn hash(&self, digest: &mut Digest) {
        let Self { step, role } = self;
        (step, role).hash(digest);
    }
}

impl Hashed for Sheet {
    fn hash(&self, digest: &mut Digest) {
        let Self { u, v } = self;
        (u, v).hash(digest);
    }
}

impl Hashed for Around {
    fn hash(&self, digest: &mut Digest) {
        let Self { edge, end } = self;
        (edge, end).hash(digest);
    }
}

impl Hashed for UseOwner {
    fn hash(&self, digest: &mut Digest) {
        match self {
            Self::Loop(owner) => ("loop", owner).hash(digest),
            Self::Wire(owner) => ("wire", owner).hash(digest),
        }
    }
}

impl<T: Hashed + ?Sized> Hashed for &T {
    fn hash(&self, digest: &mut Digest) {
        (**self).hash(digest);
    }
}

impl Hashed for BodyRecord {
    fn hash(&self, digest: &mut Digest) {
        let Self { kind, first_shell, first_wire, born } = self;
        ((kind, first_shell), (first_wire, born)).hash(digest);
    }
}

impl Hashed for ShellRecord {
    fn hash(&self, digest: &mut Digest) {
        let Self { body, first_face, next } = self;
        ((body, first_face), next).hash(digest);
    }
}

impl Hashed for FaceRecord {
    fn hash(&self, digest: &mut Digest) {
        let Self { shell, next, surface, sheet, reversed, outer, born } = self;
        ((shell, next), (surface, sheet)).hash(digest);
        ((reversed, outer), born).hash(digest);
    }
}

impl Hashed for LoopRecord {
    fn hash(&self, digest: &mut Digest) {
        let Self { face, first, next, witness } = self;
        ((face, first), (next, witness)).hash(digest);
    }
}

impl Hashed for WireRecord {
    fn hash(&self, digest: &mut Digest) {
        let Self { body, first, next } = self;
        ((body, first), next).hash(digest);
    }
}

impl Hashed for EdgeUseRecord {
    fn hash(&self, digest: &mut Digest) {
        let Self { edge, owner, next, radial, reversed, pcurve } = self;
        ((edge, owner), (next, radial)).hash(digest);
        (reversed, pcurve).hash(digest);
    }
}

impl Hashed for EdgeRecord {
    fn hash(&self, digest: &mut Digest) {
        let Self { curve, range, ends, around, first_use, role, deviation, born } = self;
        ((curve, range), (ends, around)).hash(digest);
        ((first_use, role), (deviation, born)).hash(digest);
    }
}

impl Hashed for VertexRecord {
    fn hash(&self, digest: &mut Digest) {
        let Self { point, radius, star } = self;
        ((point, radius), star).hash(digest);
    }
}

impl Hashed for Content {
    fn hash(&self, digest: &mut Digest) {
        let words = self.words();
        u64::try_from(words.len()).unwrap_or(u64::MAX).hash(digest);
        for word in words {
            word.hash(digest);
        }
    }
}

/// The geometry records, written as their content.
macro_rules! by_content {
    ($($record:ident)*) => {
        $(
            impl Hashed for $record {
                fn hash(&self, digest: &mut Digest) {
                    let Self(geometry) = self;
                    let mut content = Content::default();
                    geometry.content(&mut content);
                    content.hash(digest);
                }
            }
        )*
    };
}

by_content!(SurfaceRecord CurveRecord ParamCurveRecord);

/// Writes `kind`'s name and how many slots `table` has, then each slot in order: its generation, its state, and a live
/// slot's record. A free slot's generation decides the id its next entity is given, so it counts as a record does.
fn table<R: Hashed + Clone>(digest: &mut Digest, kind: EntityKind, table: &Table<R>) {
    kind.text().hash(digest);
    u64::try_from(table.len()).unwrap_or(u64::MAX).hash(digest);
    for (_, slot) in table.slots() {
        slot.generation().hash(digest);
        match slot.state() {
            State::Free => "free".hash(digest),
            State::Reserved => "reserved".hash(digest),
            State::Live(record) => ("live", record).hash(digest),
        }
    }
}

impl Model {
    /// Writes everything that defines the model.
    fn hash(&self, digest: &mut Digest) {
        let Self {
            tolerance,
            step,
            bodies,
            shells,
            faces,
            loops,
            wires,
            uses,
            edges,
            vertices,
            surfaces,
            curves,
            pcurves,
        } = self;
        (tolerance.length(), tolerance.angle().radians()).hash(digest);
        step.hash(digest);
        table(digest, EntityKind::Vertex, vertices);
        table(digest, EntityKind::Edge, edges);
        table(digest, EntityKind::EdgeUse, uses);
        table(digest, EntityKind::Loop, loops);
        table(digest, EntityKind::Wire, wires);
        table(digest, EntityKind::Face, faces);
        table(digest, EntityKind::Shell, shells);
        table(digest, EntityKind::Body, bodies);
        table(digest, EntityKind::Surface, surfaces);
        table(digest, EntityKind::Curve, curves);
        table(digest, EntityKind::ParamCurve, pcurves);
    }
}

impl Snapshot {
    /// The digest of everything that defines the model: its tolerance, its step, and every slot of every table, its
    /// state and generation and a live slot's record, curves and surfaces by content, NaNs written as one pattern (L23).
    /// Two snapshots with the same digest hold the same model and give the same ids to the same next edit, whatever
    /// built them and on whatever thread count.
    #[must_use]
    pub fn digest(&self) -> u64 {
        let mut digest = Digest::EMPTY;
        self.model.hash(&mut digest);
        digest.finish()
    }
}

#[cfg(test)]
mod tests;
