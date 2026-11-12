//! `occam-report`: what changes after schema 2. Schema 2 itself (entity references, fault, reason, stage, named values
//! holding a point, `Partial`'s stage) is in the crate since `geometry.schema`, and its sketch is gone with
//! `geometry.freeze`. What is left is O26's descriptions and the codes and remedies of the milestones to come. It still
//! depends on nothing of occam's: a description is a form name and named values, never a geometry type.

use occam_report::{EntityRef, NamedValue};
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
    /// NEW codes of the milestones to come, joining the closed `Code` with wording and followed-remedy tests (L2).
    NewCode {
        InvalidParameter = "input.invalid-parameter",
        StaleId = "topo.stale-id",
        Undecided = "decide.undecided",
        Incomplete = "intersect.incomplete",
        BelowResolution = "intersect.below-resolution",
        CoincidentSurfaces = "boolean.coincident-surfaces",
        DanglingSection = "boolean.dangling-section",
        EmptyResult = "boolean.empty-result",
        RadiusTooLarge = "blend.radius-too-large",
        FaceFailed = "mesh.face-failed",
        InvariantFailed = "check.invariant-failed",
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
    /// A geometry family's form name (`cylinder`, `bspline`), as its `Shape` gives it, so a new family edits no enum
    /// here. For topology, the entity kind.
    pub form: String,
    /// Parameters, extent and position, shown in the caller's units (L9).
    pub values: Vec<NamedValue>,
    pub origin: Option<Origin>,
    /// Neighbours, in the same id form (PARITY 1.1).
    pub adjacent: Vec<EntityRef>,
}

/// CHANGED (O26): a report gains the descriptions of the entities it names, filled as the result leaves the kernel. The
/// rest is schema 2's, as the crate holds it.
#[derive(Clone, Debug, PartialEq)]
pub struct Details {
    pub schema_2: occam_report::Details,
    pub described: Vec<Description>,
}
