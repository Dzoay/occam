//! `occam-report`: what changes. `SCHEMA` becomes 2. It still depends on nothing of occam's: a description is a form
//! name and named values, never a geometry type.

use occam_report::{Code, Quantity, Remedy};
use serde::{Deserialize, Serialize};

/// `occam_report`'s private `text_enum!`, repeated so that the sketch can declare closed enums the same way.
macro_rules! text_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$variant_meta:meta])* $variant:ident = $text:literal,)* }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
        pub enum $name {
            $($(#[$variant_meta])* #[serde(rename = $text)] $variant,)*
        }

        impl $name {
            pub const ALL: &'static [Self] = &[$(Self::$variant),*];

            #[must_use]
            pub const fn text(self) -> &'static str {
                match self {
                    $(Self::$variant => $text,)*
                }
            }
        }
    };
}

text_enum! {
    /// CHANGED: gains what S4 and S6 asked to name, and the kinds of `occam-topo`. `Solid` becomes `Body`, whose kind
    /// says solid, sheet or wire (PARITY 1.1, 1.6).
    EntityKind {
        Vertex = "vertex",
        Edge = "edge",
        EdgeUse = "edge-use",
        Loop = "loop",
        Wire = "wire",
        Face = "face",
        Shell = "shell",
        Body = "body",
        Model = "model",
        /// Geometry, named with no topology (S4, "What it decides" item 5).
        Surface = "surface",
        Curve = "curve",
        ParamCurve = "param-curve",
        /// Exists only in a failed operation's frozen output (generation 0).
        Section = "section",
        Region = "region",
    }
}

/// CHANGED: was `{ kind, id: u64 }`. Four of ten agents could not read a packed id (S6).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, Serialize, Deserialize)]
pub struct EntityRef {
    pub kind: EntityKind,
    pub slot: u32,
    /// 0 for an entity of a failed operation's frozen output, whose `slot` indexes that stage's output.
    pub generation: u32,
}

impl EntityRef {
    #[must_use]
    pub const fn new(kind: EntityKind, slot: u32, generation: u32) -> Self {
        Self { kind, slot, generation }
    }

    #[must_use]
    pub const fn is_scratch(self) -> bool {
        self.generation == 0
    }

    /// The one text form, `edge:41/1`, used alike by the command line, the JSON and the messages.
    #[must_use]
    pub fn text(self) -> String {
        format!("{}:{}/{}", self.kind.text(), self.slot, self.generation)
    }
}

text_enum! {
    /// NEW. Chosen where the failure is found, never defaulted from the code (S6: c07, and blind c12).
    Fault {
        InvalidInput = "invalid-input",
        NotImplemented = "not-implemented",
        NumericallySensitive = "numerically-sensitive",
        Internal = "internal",
    }
}

text_enum! {
    /// NEW. Closed reasons, not free text: S1's `reason` was a message's length, which agents read as a ratio (S6).
    Reason {
        NoSection = "no-section",
        OffsetAtBand = "offset-at-band",
        BelowResolution = "below-resolution",
        SurfacesCoincide = "surfaces-coincide",
        PairNotSupported = "pair-not-supported",
        RadiusNotPositive = "radius-not-positive",
        OperandsDisjoint = "operands-disjoint",
        BudgetSpent = "budget-spent",
    }
}

text_enum! {
    /// NEW. Where an operation was, by name (S6).
    Stage {
        Validate = "validate",
        Prepare = "prepare",
        Broad = "broad",
        Pairs = "pairs",
        EdgeSplit = "edge-split",
        FaceSplit = "face-split",
        Classify = "classify",
        Stitch = "stitch",
        Check = "check",
        Blend = "blend",
        MeshEdges = "mesh-edges",
        MeshFaces = "mesh-faces",
        Load = "load",
    }
}

text_enum! {
    /// NEW. An entity's part in the step that made it (S6: "the operation and operand it came from").
    StepRole {
        Profile = "profile",
        Target = "target",
        Tool = "tool",
        Input = "input",
        Made = "made",
    }
}

text_enum! {
    /// NEW codes, joining the closed `Code` with wording and followed-remedy tests (L2).
    NewCode {
        InvalidParameter = "input.invalid-parameter",
        StaleId = "topo.stale-id",
        Undecided = "decide.undecided",
        UndefinedParameter = "geom.undefined-parameter",
        DegenerateDirection = "geom.degenerate-direction",
        Incomplete = "intersect.incomplete",
        BelowResolution = "intersect.below-resolution",
        CoincidentSurfaces = "boolean.coincident-surfaces",
        DanglingSection = "boolean.dangling-section",
        EmptyResult = "boolean.empty-result",
        RadiusTooLarge = "blend.radius-too-large",
        FaceFailed = "mesh.face-failed",
        InvariantFailed = "check.invariant-failed",
        ThreadsUnavailable = "exec.threads-unavailable",
        Decisions = "measure.decisions",
    }
}

text_enum! {
    /// NEW remedies, each with its parameters in the catalogue (L3).
    NewRemedy {
        ReduceRadius = "reduce-radius",
        MoveApart = "move-apart",
        UseTolerance = "use-tolerance",
    }
}

/// CHANGED: `NamedValue.value` becomes this, so a position or an extent is one value. Each component is written as the
/// `number` module writes one today, so NaN and the infinities still round-trip (L5).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Value {
    Scalar(f64),
    Triple([f64; 3]),
}

/// `NamedValue`, with `value: Value`.
#[derive(Clone, Debug, PartialEq)]
pub struct Named {
    pub name: String,
    pub quantity: Quantity,
    pub value: Value,
}

/// NEW. Which step made an entity, what that step was, and the entity's part in it.
#[derive(Clone, Debug, PartialEq)]
pub struct Origin {
    /// Counted over the model's whole life, so unique across requests.
    pub step: u64,
    pub operation: String,
    pub role: StepRole,
}

/// NEW. What an entity is in the caller's terms (S6, O26). Filled by one resolver as the result leaves the kernel, never
/// at a failure site.
#[derive(Clone, Debug, PartialEq)]
pub struct Description {
    pub entity: EntityRef,
    /// A geometry family's form name (`cylinder`, `b-spline-surface`), held to the catalogue's list by a test, so a new
    /// family edits no enum here. For topology, the entity kind.
    pub form: String,
    /// Parameters, extent and position, shown in the caller's units (L9).
    pub values: Vec<Named>,
    pub origin: Option<Origin>,
    /// Neighbours, in the same id form (PARITY 1.1).
    pub adjacent: Vec<EntityRef>,
}

/// CHANGED: `last_valid` goes (S6: no agent used it, and the input is unchanged by construction); `stage` is new.
#[derive(Clone, Debug, PartialEq)]
pub struct Partial {
    pub stage: Stage,
    pub failing: Vec<EntityRef>,
    pub values: Vec<Named>,
}

/// CHANGED: what a `Report` says. `fault` is required at construction; `reason` and `described` are new. The tolerance
/// in force joins `values` whenever a tolerance decided the outcome (S6: blind c04).
#[derive(Clone, Debug, PartialEq)]
pub struct Details {
    pub code: Code,
    pub fault: Fault,
    pub reason: Option<Reason>,
    pub entities: Vec<EntityRef>,
    pub values: Vec<Named>,
    pub remedies: Vec<Remedy>,
    pub partial: Option<Partial>,
    pub described: Vec<Description>,
}
