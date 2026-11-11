//! `occam-ops`: operations as data. A request is validated and converted once, before any geometry, with a code for each
//! kind of invalid input (S6: blind on S1's report, an agent "repaired" a zero-radius hole by setting 0.0001 mm).

use std::sync::Arc;

use occam_report::{AngleUnit, Batch, LengthUnit, Outcome, Report};
use occam_units::{Length, Point, UvPoint, World};
use serde::Deserialize;

use occam_math::Exec;
use crate::report::{EntityRef, Stage};
use crate::topo::{Body, Changes, Describe, Edge, Face, Id, Snapshot};

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct Units {
    pub length: LengthUnit,
    pub angle: AngleUnit,
}

/// A new model's tolerance, in the request's units. Omitted, it is the default of O12, and the outcome says which was
/// used (`measure.tolerance-used`). On an existing model the model's own tolerance governs, and asking for a finer one is
/// `measure.tolerance-below-model`, as today.
#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct RequestTolerance {
    pub length: f64,
    pub angle: f64,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Request {
    pub schema: u32,
    pub units: Units,
    pub tolerance: Option<RequestTolerance>,
    pub steps: Vec<Step>,
}

#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Step {
    /// A name later steps in the same request can use as an operand.
    pub label: Option<String>,
    #[serde(flatten)]
    pub op: Op,
}

/// An earlier step's label, or an entity by id.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(untagged)]
pub enum Operand {
    Label(String),
    Entity(EntityRef),
}

#[derive(Clone, Copy, Debug, PartialEq, Deserialize)]
pub struct Frame3 {
    pub origin: [f64; 3],
    pub z: [f64; 3],
    pub x: [f64; 3],
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum BooleanKind {
    Fuse,
    Cut,
    Common,
}

/// Whether a step with many items keeps the input unless every item succeeds, or applies the items that succeed.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Commit {
    AllOrNothing,
    Feasible,
}

/// A radius per edge, so a `reduce-radius` remedy for one edge is followed by changing one number (L3, PARITY 1.14).
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct EdgeRadius {
    pub edge: EntityRef,
    pub radius: f64,
}

#[derive(Clone, Copy, Debug, PartialEq, Eq, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub enum Measurement {
    Volume,
    Area,
    CentreOfMass,
    Bounds,
}

/// The first release's operations (PARITY tiers 1 and 2 in its milestones); the rest are listed as refused.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(tag = "op", rename_all = "kebab-case")]
pub enum Op {
    #[serde(rename = "box")]
    Block {
        frame: Frame3,
        size: [f64; 3],
    },
    Cylinder {
        frame: Frame3,
        radius: f64,
        height: f64,
    },
    /// Edges through points, lines and arcs, joined into a wire; a gap is reported by location (PARITY 1.6).
    Wire {
        points: Vec<[f64; 3]>,
        closed: bool,
    },
    PlanarFace {
        outer: Operand,
        holes: Vec<Operand>,
    },
    Extrude {
        face: Operand,
        vector: [f64; 3],
    },
    Revolve {
        profile: Operand,
        origin: [f64; 3],
        axis: [f64; 3],
        angle: f64,
    },
    Boolean {
        kind: BooleanKind,
        target: Operand,
        tool: Operand,
    },
    Fillet {
        edges: Vec<EdgeRadius>,
        commit: Commit,
    },
    Transform {
        target: Operand,
        frame: Frame3,
        copy: bool,
    },
    Measure {
        target: Operand,
        what: Vec<Measurement>,
    },
}

/// A step's result. Steps stop at the first failure, and the model returned is the one after the last step that
/// succeeded, so nothing that failed changed the input (L5).
pub enum StepResult {
    Done(Outcome<Changes>),
    /// One result per item (L6). Under `AllOrNothing`, an item that would have succeeded says so and nothing was
    /// applied; under `Feasible`, the items that succeeded were.
    PerItem(Batch<Changes>),
    Failed(Failure),
}

pub struct Ran {
    pub model: Arc<Snapshot>,
    pub steps: Vec<StepResult>,
}

/// Everything that failed, in item order (P2: every failing pair, in candidate order), and the frozen outputs reached,
/// against which scratch ids are described (L5).
pub struct Failure {
    pub reports: Vec<Report>,
    pub frozen: Option<Arc<Frozen>>,
}

/// A boolean phase: an ordered map from frozen input to frozen output (P2). A pure function of content: its output
/// holds indices local to each item, never a model id or a number from this run, so caching items later changes no
/// phase (P8).
pub trait Phase: Send + Sync + 'static {
    const STAGE: Stage;
    type Input: Send + Sync + 'static;
    type Output: Send + Sync + 'static;

    /// # Errors
    ///
    /// Every failing item, in item order.
    fn run(exec: &Exec, input: Arc<Self::Input>) -> Result<Self::Output, Vec<Report>>;
}

/// Per touched face: trim polygons in its parameters, from stored parameter curves where there are some. Faces whose
/// bounds meet nothing of the other operand are not touched; they keep their ids and are labelled by propagation (P1).
pub struct Prepared {
    pub faces: Vec<Id<Face>>,
    pub trims: Vec<Vec<UvPoint>>,
}

/// Candidate face pairs, in stable order, from conservative boxes by sort and sweep (S1 item 13, P1, P8).
pub struct Candidates {
    pub pairs: Vec<(u32, u32)>,
}

/// Per distinct surface pair, its section; per edge against a face, its hits, in plan order.
pub struct Sections {
    pub pairs: Vec<crate::intersect::Section>,
}

/// Phase 3's three rounds, with a frozen vertex table between them (P2): per edge its stops, then pinch vertices merged,
/// then the clips.
pub struct Split {
    pub vertices: Vec<Point<World>>,
    pub pieces: Vec<(u32, u32)>,
}

pub struct Regions {
    pub loops: Vec<Vec<u32>>,
}

pub struct Labels {
    pub inside: Vec<bool>,
}

/// Every phase output reached before a failure.
#[derive(Default)]
pub struct Frozen {
    pub prepared: Option<Arc<Prepared>>,
    pub candidates: Option<Arc<Candidates>>,
    pub sections: Option<Arc<Sections>>,
    pub split: Option<Arc<Split>>,
    pub regions: Option<Arc<Regions>>,
    pub labels: Option<Arc<Labels>>,
}

impl Describe for Frozen {
    fn describe(&self, entity: EntityRef) -> Option<crate::report::Description> {
        unimplemented!()
    }
}

/// Fuse, cut or common (P2's shape (a)). Ids are given only in the final `Edit`, whose commit runs the checker.
///
/// # Errors
///
/// Every failing item of the phase that failed, with the frozen outputs reached.
pub fn boolean(
    exec: &Exec,
    model: &Arc<Snapshot>,
    kind: BooleanKind,
    target: Id<Body>,
    tool: Id<Body>,
) -> Result<Outcome<(Snapshot, Changes)>, Failure> {
    unimplemented!()
}

/// Constant-radius fillets, one result per edge. A radius too large is `blend.radius-too-large` naming the edge, with
/// `reduce-radius { edge, radius }` where `radius` is the largest the kernel certifies feasible, a band inside the
/// supremum, so that following it does not land on an undecided case.
///
/// # Errors
///
/// A failure that belongs to no one edge, such as the checker refusing the result.
pub fn fillet(
    exec: &Exec,
    model: &Arc<Snapshot>,
    edges: &[(Id<Edge>, Length)],
    commit: Commit,
) -> Result<(Snapshot, Batch<Changes>), Failure> {
    unimplemented!()
}

/// Volume, area and centre of mass, by the divergence theorem over the faces with adaptive quadrature (PARITY 1.8): the
/// bracket test's oracle compares the volume.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Mass {
    /// Cubic millimetres.
    pub volume: f64,
    /// Square millimetres.
    pub area: f64,
    pub centre: Point<World>,
}

/// # Errors
///
/// `check.invariant-failed` if the body is not closed.
pub fn mass(exec: &Exec, model: &Snapshot, body: Id<Body>) -> Result<Mass, Report> {
    unimplemented!()
}
