//! `occam-report`: what changes after schema 2. Schema 2 itself (entity references, fault, reason, stage, named values
//! holding a point, `Partial`'s stage) is in the crate since `geometry.schema`, and its sketch is gone with
//! `geometry.freeze`. O26's descriptions, their origin and a report's `described` list are in the crate since
//! `construction.describer`. What is left is the codes and remedies of the milestones to come, and the trait a failed
//! boolean's frozen outputs will answer through.

use occam_report::{Description, EntityRef};
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
    /// NEW codes of the milestones to come, joining the closed `Code` with wording and followed-remedy tests (L2).
    NewCode {
        InvalidParameter = "input.invalid-parameter",
        Undecided = "decide.undecided",
        BelowResolution = "intersect.below-resolution",
        CoincidentSurfaces = "boolean.coincident-surfaces",
        DanglingSection = "boolean.dangling-section",
        EmptyResult = "boolean.empty-result",
        RadiusTooLarge = "blend.radius-too-large",
        FaceFailed = "mesh.face-failed",
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

/// NEW, and the sketch's until the booleans give it a second source: anything that can say what an entity is. The
/// describer in `occam` reads a snapshot, or a refused step's edit, since `construction.describer`; a failed boolean's
/// frozen outputs will answer for the ids that exist only there (O26).
pub trait Describe: Sync {
    fn describe(&self, entity: EntityRef) -> Option<Description>;
}
