//! `occam-topo`: the B-rep, with the invariant checker as a module (it was `occam-check`). Records in copy-on-write
//! chunks (S3-P3), read through per-kind views so the layout is this module's alone (O22). Ids are given only in `Edit`,
//! on one thread (S3-P3, P2).

use std::marker::PhantomData;
use std::num::NonZeroU32;
use std::sync::Arc;

use occam_report::Report;
use occam_units::{Length, Point, Tolerance, UvPoint, World};
use serde::{Deserialize, Serialize};

use occam_geom::{Curve, ParamCurve, Surface};
use occam_math::Exec;
use occam_report::{EntityKind, EntityRef};

use crate::report::{Description, StepRole};

mod sealed {
    pub trait Sealed {}
}

/// An entity kind as a type, so a face id cannot go where an edge id is wanted.
pub trait Kind: sealed::Sealed + Copy + Eq + Ord + core::fmt::Debug + Send + Sync + 'static {
    const ENTITY: EntityKind;
    /// What `Edit` writes and the native file stores.
    type Record: Clone + Send + Sync + 'static;

    fn table(snapshot: &Snapshot) -> &Table<Self>;
}

/// Slot and generation (S3-P3); the kind is the type. The generation changes only when a slot is reused, so an entity
/// changed in place keeps its id and is listed in `Changes::changed`.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct Id<K: Kind> {
    slot: u32,
    generation: NonZeroU32,
    kind: PhantomData<K>,
}

impl<K: Kind> Id<K> {
    #[must_use]
    pub const fn entity(self) -> EntityRef {
        EntityRef::new(K::ENTITY, self.slot, self.generation.get())
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BodyKind {
    Solid,
    Sheet,
    Wire,
}

/// Which step made an entity, and its part in it (S6: 7 of 10 agents asked first). Stored on bodies, faces and edges and
/// carried forward by `Edit::derive`. `step` counts over the model's life, so it is unique across requests.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Provenance {
    pub step: u64,
    pub role: StepRole,
}

/// Which period of each periodic parameter a face's loops lie in (S1 item 2: without it, 152 more failures and 20
/// wrong answers, the most damaging removal measured).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Sheet {
    pub u: i16,
    pub v: i16,
}

/// `Seam`: between two chart-sized patches of one periodic surface, so no face crosses its own seam (S1 item 5).
/// `Section`: made by a boolean, where S1's label check expects the two sides to differ.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeRole {
    Ordinary,
    Seam,
    Section,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseOwner {
    Loop(Id<Loop>),
    Wire(Id<Wire>),
}

#[derive(Clone, Debug, PartialEq)]
pub struct BodyRecord {
    pub kind: BodyKind,
    pub first_shell: Option<Id<Shell>>,
    pub first_wire: Option<Id<Wire>>,
    pub born: Provenance,
}

#[derive(Clone, Debug, PartialEq)]
pub struct ShellRecord {
    pub body: Id<Body>,
    pub first_face: Id<Face>,
    pub next: Option<Id<Shell>>,
    pub closed: bool,
}

#[derive(Clone, Debug, PartialEq)]
pub struct FaceRecord {
    pub shell: Id<Shell>,
    pub next: Option<Id<Face>>,
    pub surface: Id<SurfaceGeom>,
    pub sheet: Sheet,
    pub reversed: bool,
    pub outer: Id<Loop>,
    pub born: Provenance,
}

/// Outer or not is decided by a strict interior point, kept as a witness, not by area (S1 item 3: the bug from
/// assuming otherwise broke 52 cases).
#[derive(Clone, Debug, PartialEq)]
pub struct LoopRecord {
    pub face: Id<Face>,
    pub first: Id<EdgeUse>,
    pub next: Option<Id<Loop>>,
    pub witness: UvPoint,
}

/// A wire that bounds no face: a profile, or the edges of a wire body (PARITY 1.1, 1.6).
#[derive(Clone, Debug, PartialEq)]
pub struct WireRecord {
    pub body: Id<Body>,
    pub first: Id<EdgeUse>,
    pub next: Option<Id<Wire>>,
    pub closed: bool,
}

/// A use of an edge, with direction (S1 item 4: the two-uses check caught a 52-case bug before any geometry).
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeUseRecord {
    pub edge: Id<Edge>,
    pub owner: UseOwner,
    /// The next use around the loop or along the wire; `None` only at the end of an open wire.
    pub next: Option<Id<EdgeUse>>,
    /// The next use of the same edge: none for a free edge, one for a sheet's boundary or a wire, two in a closed solid.
    /// The checker decides which counts are valid for the body, so none is impossible to store (PARITY 1.10, 2.12).
    pub radial: Option<Id<EdgeUse>>,
    pub reversed: bool,
    /// The image in the face's parameters (S1 item 7).
    pub pcurve: Option<Id<ParamCurveGeom>>,
}

#[derive(Clone, Debug, PartialEq)]
pub struct EdgeRecord {
    pub curve: Id<CurveGeom>,
    pub range: (f64, f64),
    pub ends: (Id<Vertex>, Id<Vertex>),
    pub first_use: Option<Id<EdgeUse>>,
    pub role: EdgeRole,
    /// Measured to every face that uses it, never asserted (S1 item 9: up to 140 times the tolerance with every volume
    /// right).
    pub deviation: Length,
    pub born: Provenance,
}

/// Shared by identity, merged by the band as they are made (S1 item 1). Four-valent at a pinch (S1 item 6): nothing
/// limits a vertex's edges, and the checker counts shells, the one check that sees a missing pinch.
#[derive(Clone, Debug, PartialEq)]
pub struct VertexRecord {
    pub point: Point<World>,
    /// The certified radius of a computed position, zero for a given one (S2: 32 bytes a vertex). Not the near-tangent
    /// shift, which goes in reports.
    pub radius: Length,
    /// One edge of the star, so a fillet walks it without indexing the model (critique of design B, point 3).
    pub first_edge: Option<Id<Edge>>,
}

#[derive(Clone)]
pub struct SurfaceRecord(pub Arc<dyn Surface>);

#[derive(Clone)]
pub struct CurveRecord(pub Arc<dyn Curve>);

#[derive(Clone)]
pub struct ParamCurveRecord(pub Arc<dyn ParamCurve>);

/// Records a chunk holds: 1,024 suits edits that scatter, as a boolean's do (S3-P3: 14 against 41 µs an edit).
pub const CHUNK: usize = 1024;

/// One kind's records in copy-on-write chunks. Cloning shares every chunk; the first write to a chunk copies it.
#[derive(Clone)]
pub struct Table<K: Kind> {
    chunks: Vec<Arc<Vec<Slot<K::Record>>>>,
}

#[derive(Clone)]
struct Slot<R> {
    generation: u32,
    record: Option<R>,
}

/// A colour as STEP gives it: red, green and blue, each from 0 to 1.
#[derive(Clone, Copy, Debug, PartialEq, Serialize, Deserialize)]
pub struct Colour {
    pub red: f64,
    pub green: f64,
    pub blue: f64,
}

/// An entity's name and colour (PARITY 2.9). Either may be absent.
#[derive(Clone, Debug, Default, PartialEq, Serialize, Deserialize)]
pub struct Attributes {
    pub name: Option<String>,
    pub colour: Option<Colour>,
}

type AttributeChunk = Arc<Vec<Option<Attributes>>>;

/// Names and colours by entity: beside the records and never in them, so no geometric code reads them. Kept per kind in
/// copy-on-write chunks of `CHUNK` slots, as the records are, so taking a snapshot costs what S3-P3 measured. A chunk
/// with no entries is never allocated.
#[derive(Clone, Default)]
pub struct AttributeTable {
    /// By `EntityKind`, then by chunk.
    chunks: Vec<Vec<Option<AttributeChunk>>>,
}

/// An immutable version of a model. Cloning shares every chunk (S3-P3: 183 µs at a million faces, against 31 ms for a
/// plain arena).
#[derive(Clone)]
pub struct Snapshot {
    tolerance: Tolerance,
    /// The model's step counter.
    steps: u64,
    bodies: Table<Body>,
    shells: Table<Shell>,
    faces: Table<Face>,
    loops: Table<Loop>,
    wires: Table<Wire>,
    uses: Table<EdgeUse>,
    edges: Table<Edge>,
    vertices: Table<Vertex>,
    surfaces: Table<SurfaceGeom>,
    curves: Table<CurveGeom>,
    pcurves: Table<ParamCurveGeom>,
    attributes: AttributeTable,
}

macro_rules! kinds {
    ($($marker:ident => $entity:ident, $record:ty, $field:ident;)*) => {
        $(
            #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
            pub struct $marker;

            impl sealed::Sealed for $marker {}

            impl Kind for $marker {
                const ENTITY: EntityKind = EntityKind::$entity;
                type Record = $record;

                fn table(snapshot: &Snapshot) -> &Table<Self> {
                    &snapshot.$field
                }
            }
        )*
    };
}

kinds! {
    Body => Body, BodyRecord, bodies;
    Shell => Shell, ShellRecord, shells;
    Face => Face, FaceRecord, faces;
    Loop => Loop, LoopRecord, loops;
    Wire => Wire, WireRecord, wires;
    EdgeUse => EdgeUse, EdgeUseRecord, uses;
    Edge => Edge, EdgeRecord, edges;
    Vertex => Vertex, VertexRecord, vertices;
    SurfaceGeom => Surface, SurfaceRecord, surfaces;
    CurveGeom => Curve, CurveRecord, curves;
    ParamCurveGeom => ParamCurve, ParamCurveRecord, pcurves;
}

/// A face, read field by field. Under records each method reads a field of one record; under struct-of-arrays it would
/// read one column. Callers cannot tell, so changing O22's answer changes this module alone.
#[derive(Clone, Copy)]
pub struct FaceView<'a> {
    snapshot: &'a Snapshot,
    id: Id<Face>,
}

impl<'a> FaceView<'a> {
    #[must_use]
    pub const fn id(self) -> Id<Face> {
        self.id
    }

    /// Borrowed from the snapshot: no reference count is touched on a read.
    #[must_use]
    pub fn surface(self) -> &'a dyn Surface {
        unimplemented!()
    }

    #[must_use]
    pub fn sheet(self) -> Sheet {
        unimplemented!()
    }

    #[must_use]
    pub fn reversed(self) -> bool {
        unimplemented!()
    }

    #[must_use]
    pub fn outer(self) -> Id<Loop> {
        unimplemented!()
    }

    pub fn loops(self) -> impl Iterator<Item = Id<Loop>> + 'a {
        std::iter::empty()
    }

    /// Adjacency (PARITY 1.1).
    pub fn edges(self) -> impl Iterator<Item = Id<Edge>> + 'a {
        std::iter::empty()
    }

    #[must_use]
    pub fn born(self) -> Provenance {
        unimplemented!()
    }
}

#[derive(Clone, Copy)]
pub struct EdgeView<'a> {
    snapshot: &'a Snapshot,
    id: Id<Edge>,
}

impl<'a> EdgeView<'a> {
    #[must_use]
    pub fn curve(self) -> &'a dyn Curve {
        unimplemented!()
    }

    #[must_use]
    pub fn range(self) -> (f64, f64) {
        unimplemented!()
    }

    #[must_use]
    pub fn ends(self) -> (Id<Vertex>, Id<Vertex>) {
        unimplemented!()
    }

    #[must_use]
    pub fn deviation(self) -> Length {
        unimplemented!()
    }

    /// The faces around it, in radial order (PARITY 1.1).
    pub fn faces(self) -> impl Iterator<Item = Id<Face>> + 'a {
        std::iter::empty()
    }

    pub fn uses(self) -> impl Iterator<Item = Id<EdgeUse>> + 'a {
        std::iter::empty()
    }
}

#[derive(Clone, Copy)]
pub struct VertexView<'a> {
    snapshot: &'a Snapshot,
    id: Id<Vertex>,
}

impl<'a> VertexView<'a> {
    #[must_use]
    pub fn point(self) -> Point<World> {
        unimplemented!()
    }

    #[must_use]
    pub fn radius(self) -> Length {
        unimplemented!()
    }

    pub fn edges(self) -> impl Iterator<Item = Id<Edge>> + 'a {
        std::iter::empty()
    }
}

impl Snapshot {
    #[must_use]
    pub fn empty(tolerance: Tolerance) -> Self {
        unimplemented!()
    }

    #[must_use]
    pub const fn tolerance(&self) -> Tolerance {
        self.tolerance
    }

    #[must_use]
    pub fn face(&self, id: Id<Face>) -> Option<FaceView<'_>> {
        unimplemented!()
    }

    #[must_use]
    pub fn edge(&self, id: Id<Edge>) -> Option<EdgeView<'_>> {
        unimplemented!()
    }

    #[must_use]
    pub fn vertex(&self, id: Id<Vertex>) -> Option<VertexView<'_>> {
        unimplemented!()
    }

    /// The entity's name and colour, if it has either.
    #[must_use]
    pub fn attributes(&self, entity: EntityRef) -> Option<&Attributes> {
        unimplemented!()
    }

    /// Live ids of a kind in slot order: the order everything iterates, hashes and lists in (PARITY 1.1).
    #[must_use]
    pub fn ids<K: Kind>(&self) -> Vec<Id<K>> {
        let table = K::table(self);
        unimplemented!()
    }

    /// An `EntityRef` from a report or a request, checked against this snapshot.
    ///
    /// # Errors
    ///
    /// `topo.stale-id` if no live entity has that kind, slot and generation.
    pub fn resolve<K: Kind>(&self, entity: EntityRef) -> Result<Id<K>, Report> {
        unimplemented!()
    }

    #[must_use]
    pub fn edit(&self, step_role: StepRole) -> Edit {
        unimplemented!()
    }

    /// Every live record in slot order, every geometry's `Content`, the names and colours, and the tolerance, NaNs
    /// canonicalised (L23).
    #[must_use]
    pub fn digest(&self) -> u64 {
        unimplemented!()
    }

    /// The whole model against every invariant: on load, on import, and in tests (PARITY 1.10). One report per failure,
    /// each naming its entities.
    #[must_use]
    pub fn check_all(&self, exec: &Exec) -> Vec<Report> {
        unimplemented!()
    }
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Derivation {
    Split,
    Trimmed,
    Merged,
    Copied,
}

/// What an operation created, changed and deleted (the brief), and what each new entity came from (PARITY 1.11).
#[derive(Clone, Debug, Default, PartialEq, Serialize)]
pub struct Changes {
    pub created: Vec<EntityRef>,
    pub changed: Vec<EntityRef>,
    pub deleted: Vec<EntityRef>,
    pub lineage: Vec<(EntityRef, EntityRef, Derivation)>,
}

/// The sequential apply: the only place ids are given. Dropping it leaves the input as it was (L5).
pub struct Edit {
    base: Snapshot,
    next: Snapshot,
    changes: Changes,
    step: u64,
}

impl Edit {
    /// An id for a record written later in this edit, so that cycles (an edge and its uses, a face and its outer loop)
    /// can be built from an empty model. `commit` refuses a reserved id that was never set.
    pub fn reserve<K: Kind>(&mut self) -> Id<K> {
        unimplemented!()
    }

    pub fn add<K: Kind>(&mut self, record: K::Record) -> Id<K> {
        unimplemented!()
    }

    /// # Errors
    ///
    /// `topo.stale-id` if `id` is not live or reserved here.
    pub fn set<K: Kind>(&mut self, id: Id<K>, record: K::Record) -> Result<(), Report> {
        unimplemented!()
    }

    /// Removes the record, and its name and colour with it.
    ///
    /// # Errors
    ///
    /// `topo.stale-id` if `id` is not live.
    pub fn remove<K: Kind>(&mut self, id: Id<K>) -> Result<(), Report> {
        unimplemented!()
    }

    /// Sets or clears an entity's name and colour. What is set here wins over what `commit` carries along lineage.
    ///
    /// # Errors
    ///
    /// `topo.stale-id` if `entity` is not live or reserved here.
    pub fn set_attributes(&mut self, entity: EntityRef, attributes: Option<Attributes>) -> Result<(), Report> {
        unimplemented!()
    }

    /// Records where `new` came from, and carries `old`'s provenance to it.
    pub fn derive(&mut self, new: EntityRef, old: EntityRef, how: Derivation) {
        unimplemented!()
    }

    /// Checks what was created or changed and its neighbours, plus the global counts (Euler characteristic, shells per
    /// body), then hands back the new snapshot. Nothing else makes a snapshot, so no unchecked model exists, in release
    /// builds too. Checking only what changed keeps the check's cost following the edit (P1's plate ladder).
    ///
    /// It also carries names and colours along the lineage `derive` recorded: a split, trimmed or copied entity takes
    /// its source's, and a merged one keeps only what all its sources share.
    ///
    /// # Errors
    ///
    /// One `check.invariant-failed` per failure, each naming its entities, with fault `internal`.
    pub fn commit(self, exec: &Exec) -> Result<(Snapshot, Changes), Vec<Report>> {
        unimplemented!()
    }
}

/// Anything that can say what an entity is: a snapshot for the model's entities, and a failed operation's frozen outputs
/// for the ones that exist only there (O26).
pub trait Describe: Sync {
    fn describe(&self, entity: EntityRef) -> Option<Description>;
}

impl Describe for Snapshot {
    fn describe(&self, entity: EntityRef) -> Option<Description> {
        unimplemented!()
    }
}
