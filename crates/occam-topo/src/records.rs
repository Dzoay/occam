//! Source: original
//! What each entity holds (S1): the fields the topology topic lists, and the links between entities. An edit writes
//! whole records; everything else reads them through views, so that how they are laid out stays this crate's (O22).

use std::sync::Arc;

use occam_geom::{Content, Curve, ParamCurve, Surface};
use occam_report::EntityRef;
use occam_units::{Length, Point, UvPoint, World};

use crate::kind::{
    Body, CurveGeom, Edge, EdgeUse, Face, Id, Kind, Loop, ParamCurveGeom, Shell, SurfaceGeom, Vertex, Wire,
};

/// What a body is (PARITY 1.1, 1.6).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum BodyKind {
    /// Closed shells bounding a volume.
    Solid,
    /// Faces bounding no volume.
    Sheet,
    /// Edges alone.
    Wire,
}

/// Which period of each periodic parameter a face's loops lie in (S1 item 2: without it, 152 more failures and 20 wrong
/// answers, the most damaging removal measured).
#[derive(Clone, Copy, Debug, PartialEq, Eq, Default)]
pub struct Sheet {
    /// The period of `u`, counted from the domain's own.
    pub u: i16,
    /// The period of `v`.
    pub v: i16,
}

/// What an edge is for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum EdgeRole {
    /// Any edge that is neither of the others.
    Ordinary,
    /// Between two patches of one periodic surface, so that no face crosses its own seam (S1 item 5).
    Seam,
    /// Made by a boolean where two faces cross, so that its two sides are expected to differ (S1).
    Section,
}

/// Which end of an edge.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum End {
    /// Where its range begins.
    Start,
    /// Where its range ends.
    Finish,
}

/// One end of an edge, as a place in the star of the vertex there.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Around {
    /// The edge.
    pub edge: Id<Edge>,
    /// Its end at the vertex.
    pub end: End,
}

/// What owns an edge use.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum UseOwner {
    /// A loop bounding a face.
    Loop(Id<Loop>),
    /// A wire bounding nothing.
    Wire(Id<Wire>),
}

/// A body.
#[derive(Clone, Debug, PartialEq)]
pub struct BodyRecord {
    /// What it is.
    pub kind: BodyKind,
    /// Its first shell; the rest follow from it.
    pub first_shell: Option<Id<Shell>>,
    /// Its first wire; the rest follow from it.
    pub first_wire: Option<Id<Wire>>,
}

/// A shell: faces joined along their edges.
#[derive(Clone, Debug, PartialEq)]
pub struct ShellRecord {
    /// The body it belongs to.
    pub body: Id<Body>,
    /// Its first face; the rest follow from it.
    pub first_face: Id<Face>,
    /// The body's next shell.
    pub next: Option<Id<Shell>>,
}

/// A face: a piece of one surface bounded by loops.
#[derive(Clone, Debug, PartialEq)]
pub struct FaceRecord {
    /// The shell it belongs to.
    pub shell: Id<Shell>,
    /// The shell's next face.
    pub next: Option<Id<Face>>,
    /// The surface it lies on.
    pub surface: Id<SurfaceGeom>,
    /// Which period of each periodic parameter its loops lie in.
    pub sheet: Sheet,
    /// Whether its outward normal is the surface's normal reversed.
    pub reversed: bool,
    /// Its outer loop, which the rest follow.
    pub outer: Id<Loop>,
}

/// A loop of edge uses bounding a face. Which loop is outer is decided by a strict interior point, kept as a witness,
/// not by area (S1 item 3: assuming otherwise broke 52 cases).
#[derive(Clone, Debug, PartialEq)]
pub struct LoopRecord {
    /// The face it bounds.
    pub face: Id<Face>,
    /// Its first use; the rest follow around it.
    pub first: Id<EdgeUse>,
    /// The face's next loop.
    pub next: Option<Id<Loop>>,
    /// A point strictly inside it, in the face's parameters.
    pub witness: UvPoint,
}

/// A chain of edge uses that bounds no face: a profile, or the edges of a wire body (PARITY 1.1, 1.6).
#[derive(Clone, Debug, PartialEq)]
pub struct WireRecord {
    /// The body it belongs to.
    pub body: Id<Body>,
    /// Its first use; the rest follow along it.
    pub first: Id<EdgeUse>,
    /// The body's next wire.
    pub next: Option<Id<Wire>>,
}

/// A use of an edge by a loop or a wire, with its direction (S1 item 4: the two-uses check caught a 52-case bug before
/// any geometry).
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeUseRecord {
    /// The edge used.
    pub edge: Id<Edge>,
    /// The loop or wire it belongs to.
    pub owner: UseOwner,
    /// The next use around the loop or along the wire; `None` only at the end of an open wire.
    pub next: Option<Id<EdgeUse>>,
    /// The next use of the same edge, round a cycle that comes back to this one, so an edge can have any number of
    /// uses and the checker decides which counts are valid for its body (PARITY 1.10).
    pub radial: Id<EdgeUse>,
    /// Whether it runs against the edge's curve.
    pub reversed: bool,
    /// The edge's image in the face's parameters (S1 item 7: without it, the same answers 3.45 times slower).
    pub pcurve: Option<Id<ParamCurveGeom>>,
}

/// An edge: a piece of one curve between two vertices.
#[derive(Clone, Debug, PartialEq)]
pub struct EdgeRecord {
    /// The curve it lies on.
    pub curve: Id<CurveGeom>,
    /// The part of the curve's parameter it covers, lowest first.
    pub range: (f64, f64),
    /// The vertices at the start and the finish of its range; the same vertex twice for a closed edge.
    pub ends: (Id<Vertex>, Id<Vertex>),
    /// The next place round the star of each end's vertex, in the same order as `ends`, so that the edges at a vertex
    /// are found by walking, whatever meets there: a pinch, a sheet's boundary or a wire (S1 item 6).
    pub around: (Around, Around),
    /// One of its uses; the rest follow round the radial cycle. `None` for an edge no loop or wire uses.
    pub first_use: Option<Id<EdgeUse>>,
    /// What it is for.
    pub role: EdgeRole,
    /// How far it strays from the surface of any face that uses it, measured and never asserted (S1 item 9: up to 140
    /// times the tolerance with every volume right).
    pub deviation: Length,
}

/// A vertex, shared by identity (S1 item 1). Nothing limits how many edges meet at one, so a pinch is a vertex like any
/// other (S1 item 6).
#[derive(Clone, Debug, PartialEq)]
pub struct VertexRecord {
    /// Where it is.
    pub point: Point<World>,
    /// The certified radius of a computed position, zero for a given one (S2). Not the near-tangent shift, which goes
    /// in reports.
    pub radius: Length,
    /// One place in its star, from which the rest follow; `None` for a vertex no edge ends at.
    pub star: Option<Around>,
}

/// A surface faces lie on. Two records are the same if their surfaces' contents are, never by address (P8).
#[derive(Clone)]
pub struct SurfaceRecord(pub Arc<dyn Surface>);

/// A curve edges lie on. Two records are the same if their curves' contents are.
#[derive(Clone)]
pub struct CurveRecord(pub Arc<dyn Curve>);

/// A curve in a face's parameters. Two records are the same if their contents are.
#[derive(Clone)]
pub struct ParamCurveRecord(pub Arc<dyn ParamCurve>);

/// Appends `id` as a report names it.
fn link<K: Kind>(out: &mut Vec<EntityRef>, id: Id<K>) {
    out.push(id.entity());
}

/// Appends `id`, if there is one.
fn link_some<K: Kind>(out: &mut Vec<EntityRef>, id: Option<Id<K>>) {
    out.extend(id.map(Id::entity));
}

impl BodyRecord {
    pub(crate) fn links(&self, out: &mut Vec<EntityRef>) {
        link_some(out, self.first_shell);
        link_some(out, self.first_wire);
    }
}

impl ShellRecord {
    pub(crate) fn links(&self, out: &mut Vec<EntityRef>) {
        link(out, self.body);
        link(out, self.first_face);
        link_some(out, self.next);
    }
}

impl FaceRecord {
    pub(crate) fn links(&self, out: &mut Vec<EntityRef>) {
        link(out, self.shell);
        link_some(out, self.next);
        link(out, self.surface);
        link(out, self.outer);
    }
}

impl LoopRecord {
    pub(crate) fn links(&self, out: &mut Vec<EntityRef>) {
        link(out, self.face);
        link(out, self.first);
        link_some(out, self.next);
    }
}

impl WireRecord {
    pub(crate) fn links(&self, out: &mut Vec<EntityRef>) {
        link(out, self.body);
        link(out, self.first);
        link_some(out, self.next);
    }
}

impl EdgeUseRecord {
    pub(crate) fn links(&self, out: &mut Vec<EntityRef>) {
        link(out, self.edge);
        match self.owner {
            UseOwner::Loop(owner) => link(out, owner),
            UseOwner::Wire(owner) => link(out, owner),
        }
        link_some(out, self.next);
        link(out, self.radial);
        link_some(out, self.pcurve);
    }
}

impl EdgeRecord {
    pub(crate) fn links(&self, out: &mut Vec<EntityRef>) {
        link(out, self.curve);
        link(out, self.ends.0);
        link(out, self.ends.1);
        link(out, self.around.0.edge);
        link(out, self.around.1.edge);
        link_some(out, self.first_use);
    }
}

impl VertexRecord {
    pub(crate) fn links(&self, out: &mut Vec<EntityRef>) {
        link_some(out, self.star.map(|place| place.edge));
    }
}

/// The geometry records, which link to nothing and compare by content.
macro_rules! geometry {
    ($($record:ident: $name:literal;)*) => {
        $(
            impl $record {
                pub(crate) const fn links(&self, _out: &mut Vec<EntityRef>) {}
            }

            impl PartialEq for $record {
                fn eq(&self, other: &Self) -> bool {
                    let (mut mine, mut theirs) = (Content::default(), Content::default());
                    self.0.content(&mut mine);
                    other.0.content(&mut theirs);
                    mine == theirs
                }
            }

            impl core::fmt::Debug for $record {
                fn fmt(&self, f: &mut core::fmt::Formatter<'_>) -> core::fmt::Result {
                    write!(f, "{}({})", $name, self.0.family().0)
                }
            }
        )*
    };
}

geometry! {
    SurfaceRecord: "SurfaceRecord";
    CurveRecord: "CurveRecord";
    ParamCurveRecord: "ParamCurveRecord";
}

#[cfg(test)]
mod tests {
    use super::*;
    use occam_geom::{Line, Plane, UvCurve};
    use occam_units::{Direction, Local, Placement, Vec3, Vector};

    fn line(x: f64) -> Line {
        Line::new(Point::from_vec3(Vec3::new(x, 0.0, 0.0)), Direction::normalised(Vec3::new(0.0, 1.0, 0.0)))
    }

    fn plane(z: f64) -> Plane {
        let lift = Placement::<World, World>::translation(Vector::from_vec3(Vec3::new(0.0, 0.0, z)));
        Plane::placed(&Placement::<Local, World>::identity().then(&lift))
    }

    #[test]
    fn geometry_records_are_equal_when_their_contents_are_never_by_address() {
        let (a, b) = (SurfaceRecord(Arc::new(plane(1.0))), SurfaceRecord(Arc::new(plane(1.0))));
        assert_eq!(a, b, "two surfaces made apart with the same content");
        assert_ne!(a, SurfaceRecord(Arc::new(plane(2.0))));
        assert_eq!(CurveRecord(Arc::new(line(1.0))), CurveRecord(Arc::new(line(1.0))));
        assert_ne!(CurveRecord(Arc::new(line(1.0))), CurveRecord(Arc::new(line(3.0))));
        let uv = |x| ParamCurveRecord(Arc::new(UvCurve::new(line(x)).unwrap()));
        assert_eq!(uv(1.0), uv(1.0));
        assert_ne!(uv(1.0), uv(3.0));
    }
}
