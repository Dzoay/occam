//! Source: original
//! Reports: occam's errors as an API for agents (L1).
//!
//! Every failure, and every success worth remarking on, is a [`Report`]: a stable [`Code`], the entities involved, numbers
//! tagged by quantity, [`Remedy`] values with parameters to act on, and what a solver had achieved when it stopped. Every
//! code and remedy is listed by [`catalogue_json`], and each pair is proven by a test using [`followed_remedy!`].

/// Declares a closed enum with a stable text form, which is also its JSON form, and a list of all its values.
macro_rules! text_enum {
    ($(#[$meta:meta])* $name:ident { $($(#[$variant_meta:meta])* $variant:ident = $text:literal,)* }) => {
        $(#[$meta])*
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord, serde::Serialize, serde::Deserialize)]
        pub enum $name {
            $($(#[$variant_meta])* #[serde(rename = $text)] $variant,)*
        }

        impl $name {
            /// Every value, in declaration order.
            pub const ALL: &'static [Self] = &[$(Self::$variant),*];

            /// The stable text form.
            #[must_use]
            pub const fn text(self) -> &'static str {
                match self {
                    $(Self::$variant => $text,)*
                }
            }

            /// The variant's name in Rust.
            #[must_use]
            pub const fn name(self) -> &'static str {
                match self {
                    $(Self::$variant => stringify!($variant),)*
                }
            }
        }
    };
}

/// Declares the closed list of codes with everything the catalogue says about each, so each code is written once and the
/// catalogue's matches, generated here, name every code (L4).
macro_rules! catalogue {
    ($(
        $(#[$meta:meta])*
        $variant:ident = $text:literal {
            $severity:ident, [$($remedy:ident),*],
            $summary:literal,
            $template:literal $(,)?
        }
    )*) => {
        text_enum! {
            /// A stable identifier for everything occam can report.
            Code {
                $($(#[$meta])* $variant = $text,)*
            }
        }

        impl Code {
            /// How serious it is.
            const fn severity(self) -> Severity {
                match self {
                    $(Self::$variant => Severity::$severity,)*
                }
            }

            /// One sentence on what it means.
            const fn summary(self) -> &'static str {
                match self {
                    $(Self::$variant => $summary,)*
                }
            }

            /// The wording, with `{name}` for each value it shows.
            const fn template(self) -> &'static str {
                match self {
                    $(Self::$variant => $template,)*
                }
            }

            /// The kinds of fix it can suggest.
            const fn remedies(self) -> &'static [RemedyKind] {
                match self {
                    $(Self::$variant => &[$(RemedyKind::$remedy),*],)*
                }
            }
        }
    };
}

mod code;
mod description;
mod display;
mod fault;
mod follow;
mod json;
mod number;
mod origin;
mod outcome;
mod report;
mod value;

pub use code::{Code, Entry, RemedyKind, Severity, catalogue_json};
pub use description::{Description, Origin, Shown, ValueShown};
pub use display::{AngleUnit, DisplayUnits, LengthUnit};
pub use fault::{Fault, Reason, Stage};
pub use follow::follow_remedy;
pub use json::SCHEMA;
pub use number::Number;
pub use origin::StepRole;
pub use outcome::{Batch, Outcome};
pub use report::{Details, Input, Partial, Remedy, Report};
pub use value::{EntityKind, EntityRef, NamedValue, Quantity, Value, figure};
