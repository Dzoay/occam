//! Source: original
//! The topology's records as the native format writes them: every field by name, links in their text form, numbers
//! exact, and nothing a record does not have, so a misspelt or extra field fails where it is written.

use occam_report::{EntityRef, StepRole};
use occam_topo::{
    Around, Body, BodyKind, BodyRecord, CurveGeom, Edge, EdgeRecord, EdgeRole, EdgeUse, EdgeUseRecord, End, Face,
    FaceRecord, Id, Loop, LoopRecord, ParamCurveGeom, Provenance, Sheet, Shell, ShellRecord, StepNote, SurfaceGeom,
    UseOwner, Vertex, VertexRecord, Wire, WireRecord,
};
use occam_units::{Length, Point, UvPoint, Vec3, World};
use serde::{Deserialize, Serialize};

use crate::exact::Exact;
use crate::link::{maybe, one, owner};

/// What a body is.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Kind {
    Solid,
    Sheet,
    Wire,
}

/// What an edge is for.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Role {
    Ordinary,
    Seam,
    Section,
}

/// Which end of an edge.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum Which {
    Start,
    Finish,
}

/// Which step made an entity, and its part in it.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Born {
    step: u64,
    role: StepRole,
}

/// What a step's request named it, kept with the model by the step's number.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields, rename_all = "kebab-case")]
pub(crate) struct NoteFile {
    step: u64,
    request_step: u32,
    operation: String,
    copy_of: Option<EntityRef>,
}

impl NoteFile {
    /// The note of the step `step`, as written.
    pub(crate) fn of(step: u64, note: &StepNote) -> Self {
        Self { step, request_step: note.request_step, operation: note.operation.clone(), copy_of: note.copy_of }
    }

    /// The step it is the note of, and the note.
    pub(crate) fn read(self) -> (u64, StepNote) {
        (self.step, StepNote { request_step: self.request_step, operation: self.operation, copy_of: self.copy_of })
    }
}

/// A place round a vertex's star.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Place {
    #[serde(with = "one")]
    edge: Id<Edge>,
    end: Which,
}

/// A body.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct BodyFile {
    kind: Kind,
    #[serde(with = "maybe")]
    first_shell: Option<Id<Shell>>,
    #[serde(with = "maybe")]
    first_wire: Option<Id<Wire>>,
    born: Born,
}

/// A shell.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct ShellFile {
    #[serde(with = "one")]
    body: Id<Body>,
    #[serde(with = "one")]
    face: Id<Face>,
    #[serde(with = "maybe")]
    next: Option<Id<Shell>>,
}

/// A face.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct FaceFile {
    #[serde(with = "one")]
    shell: Id<Shell>,
    #[serde(with = "one")]
    surface: Id<SurfaceGeom>,
    sheet: [i16; 2],
    reversed: bool,
    #[serde(with = "one")]
    outer: Id<Loop>,
    born: Born,
}

/// A loop.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct LoopFile {
    #[serde(with = "one")]
    face: Id<Face>,
    #[serde(with = "one")]
    first: Id<EdgeUse>,
    #[serde(with = "maybe")]
    next: Option<Id<Loop>>,
    witness: [Exact; 2],
}

/// A wire.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct WireFile {
    #[serde(with = "one")]
    body: Id<Body>,
    #[serde(with = "one")]
    first: Id<EdgeUse>,
    #[serde(with = "maybe")]
    next: Option<Id<Wire>>,
}

/// A use of an edge.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct UseFile {
    #[serde(with = "one")]
    edge: Id<Edge>,
    #[serde(with = "owner")]
    owner: UseOwner,
    #[serde(with = "maybe")]
    next: Option<Id<EdgeUse>>,
    #[serde(with = "one")]
    radial: Id<EdgeUse>,
    reversed: bool,
    #[serde(with = "maybe")]
    pcurve: Option<Id<ParamCurveGeom>>,
}

/// An edge.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct EdgeFile {
    #[serde(with = "one")]
    curve: Id<CurveGeom>,
    range: [Exact; 2],
    #[serde(with = "one")]
    start: Id<Vertex>,
    #[serde(with = "one")]
    finish: Id<Vertex>,
    around: [Place; 2],
    #[serde(with = "maybe")]
    first_use: Option<Id<EdgeUse>>,
    role: Role,
    deviation: Exact,
    born: Born,
}

/// A vertex.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct VertexFile {
    point: [Exact; 3],
    radius: Exact,
    star: Option<Place>,
}

const fn born(p: Provenance) -> Born {
    Born { step: p.step, role: p.role }
}

const fn provenance(b: Born) -> Provenance {
    Provenance { step: b.step, role: b.role }
}

const fn place(a: Around) -> Place {
    Place {
        edge: a.edge,
        end: match a.end {
            End::Start => Which::Start,
            End::Finish => Which::Finish,
        },
    }
}

const fn around(p: Place) -> Around {
    Around {
        edge: p.edge,
        end: match p.end {
            Which::Start => End::Start,
            Which::Finish => End::Finish,
        },
    }
}

fn point(p: Point<World>) -> [Exact; 3] {
    let v = p.to_vec3();
    [Exact(v.x), Exact(v.y), Exact(v.z)]
}

const fn mm(n: Exact) -> Length {
    Length::from_millimetres(n.0)
}

impl From<&BodyRecord> for BodyFile {
    fn from(r: &BodyRecord) -> Self {
        let kind = match r.kind {
            BodyKind::Solid => Kind::Solid,
            BodyKind::Sheet => Kind::Sheet,
            BodyKind::Wire => Kind::Wire,
        };
        Self { kind, first_shell: r.first_shell, first_wire: r.first_wire, born: born(r.born) }
    }
}

impl From<BodyFile> for BodyRecord {
    fn from(f: BodyFile) -> Self {
        let kind = match f.kind {
            Kind::Solid => BodyKind::Solid,
            Kind::Sheet => BodyKind::Sheet,
            Kind::Wire => BodyKind::Wire,
        };
        Self { kind, first_shell: f.first_shell, first_wire: f.first_wire, born: provenance(f.born) }
    }
}

impl From<&ShellRecord> for ShellFile {
    fn from(r: &ShellRecord) -> Self {
        Self { body: r.body, face: r.face, next: r.next }
    }
}

impl From<ShellFile> for ShellRecord {
    fn from(f: ShellFile) -> Self {
        Self { body: f.body, face: f.face, next: f.next }
    }
}

impl From<&FaceRecord> for FaceFile {
    fn from(r: &FaceRecord) -> Self {
        let (shell, surface, outer) = (r.shell, r.surface, r.outer);
        Self { shell, surface, sheet: [r.sheet.u, r.sheet.v], reversed: r.reversed, outer, born: born(r.born) }
    }
}

impl From<FaceFile> for FaceRecord {
    fn from(f: FaceFile) -> Self {
        let [u, v] = f.sheet;
        let (shell, surface, outer) = (f.shell, f.surface, f.outer);
        Self { shell, surface, sheet: Sheet { u, v }, reversed: f.reversed, outer, born: provenance(f.born) }
    }
}

impl From<&LoopRecord> for LoopFile {
    fn from(r: &LoopRecord) -> Self {
        let witness = [Exact(r.witness.u()), Exact(r.witness.v())];
        Self { face: r.face, first: r.first, next: r.next, witness }
    }
}

impl From<LoopFile> for LoopRecord {
    fn from(f: LoopFile) -> Self {
        let [u, v] = f.witness;
        Self { face: f.face, first: f.first, next: f.next, witness: UvPoint::new(u.0, v.0) }
    }
}

impl From<&WireRecord> for WireFile {
    fn from(r: &WireRecord) -> Self {
        Self { body: r.body, first: r.first, next: r.next }
    }
}

impl From<WireFile> for WireRecord {
    fn from(f: WireFile) -> Self {
        Self { body: f.body, first: f.first, next: f.next }
    }
}

impl From<&EdgeUseRecord> for UseFile {
    fn from(r: &EdgeUseRecord) -> Self {
        let (edge, owner, next, radial, pcurve) = (r.edge, r.owner, r.next, r.radial, r.pcurve);
        Self { edge, owner, next, radial, reversed: r.reversed, pcurve }
    }
}

impl From<UseFile> for EdgeUseRecord {
    fn from(f: UseFile) -> Self {
        let (edge, owner, next, radial, pcurve) = (f.edge, f.owner, f.next, f.radial, f.pcurve);
        Self { edge, owner, next, radial, reversed: f.reversed, pcurve }
    }
}

impl From<&EdgeRecord> for EdgeFile {
    fn from(r: &EdgeRecord) -> Self {
        let role = match r.role {
            EdgeRole::Ordinary => Role::Ordinary,
            EdgeRole::Seam => Role::Seam,
            EdgeRole::Section => Role::Section,
        };
        Self {
            curve: r.curve,
            range: [Exact(r.range.0), Exact(r.range.1)],
            start: r.ends.0,
            finish: r.ends.1,
            around: [place(r.around.0), place(r.around.1)],
            first_use: r.first_use,
            role,
            deviation: Exact(r.deviation.millimetres()),
            born: born(r.born),
        }
    }
}

impl From<EdgeFile> for EdgeRecord {
    fn from(f: EdgeFile) -> Self {
        let role = match f.role {
            Role::Ordinary => EdgeRole::Ordinary,
            Role::Seam => EdgeRole::Seam,
            Role::Section => EdgeRole::Section,
        };
        let [lo, hi] = f.range;
        let [first, second] = f.around;
        Self {
            curve: f.curve,
            range: (lo.0, hi.0),
            ends: (f.start, f.finish),
            around: (around(first), around(second)),
            first_use: f.first_use,
            role,
            deviation: mm(f.deviation),
            born: provenance(f.born),
        }
    }
}

impl From<&VertexRecord> for VertexFile {
    fn from(r: &VertexRecord) -> Self {
        Self { point: point(r.point), radius: Exact(r.radius.millimetres()), star: r.star.map(place) }
    }
}

impl From<VertexFile> for VertexRecord {
    fn from(f: VertexFile) -> Self {
        let [x, y, z] = f.point;
        let point = Point::from_vec3(Vec3::new(x.0, y.0, z.0));
        Self { point, radius: mm(f.radius), star: f.star.map(around) }
    }
}
