//! Source: original
//! The operations a request can ask for, by the names it gives them: the first release's (docs/architecture/
//! operations.md). Each says whether this build does it; everything else is refused by name (S6: an agent could not tell
//! a repairable input from an unsupported one).
//!
//! An operation has one home, its row in [`operations!`] below: its name, the fields its step takes, what they hold, and
//! the reader and runner its module gives. The listing, the guide, the request's reader and `execute` all go by the row.

use occam_report::{Code, EntityRef, Fault, Report};
use occam_topo::Edit;
use serde_json::{Map, Value, json};

use crate::args::Args;
use crate::operand::Labels;
use crate::read::At;
use crate::request::Units;
use crate::{construct, measure, primitive, sweep, transform};

/// What the step at a place, asking for an operation, gives in its fields, already held to those the operation takes,
/// in the request's units, if every input is good; every bad input goes in the list.
pub(crate) type Reader = fn(&At, Operation, &Map<String, Value>, Units, &mut Vec<Report>) -> Option<Args>;

/// What a step made, if it made one thing a later step can name by its label, or why it made nothing.
pub(crate) type Made = Result<Option<EntityRef>, Report>;

/// Does a step in the edit from what its reader read, with the labels of the steps before it; nothing, given what
/// another operation's reader read.
pub(crate) type Runner = fn(&mut Edit, &Args, &Labels) -> Option<Made>;

/// How this build does an operation: its step's reader and its runner, which go together.
#[derive(Clone, Copy)]
pub(crate) struct Does {
    /// Reads its step.
    pub(crate) read: Reader,
    /// Runs what `read` read.
    pub(crate) run: Runner,
}

/// All there is to say of an operation, in one place: the row [`operations!`] takes for it.
struct Row {
    /// The name a request gives it.
    name: &'static str,
    /// The fields its step takes beside `op` and `label`.
    fields: &'static [&'static str],
    /// What those fields hold, as the guide gives it.
    takes: &'static str,
    /// How this build does it, if it does.
    does: Option<Does>,
}

impl Row {
    /// An operation this build does: its name, its fields, what they hold, and its reader and runner.
    const fn done(name: &'static str, fields: &'static [&'static str], takes: &'static str, does: Does) -> Self {
        Self { name, fields, takes, does: Some(does) }
    }

    /// An operation this build does not do yet, which so takes no fields and has no reader or runner.
    const fn later(name: &'static str) -> Self {
        Self { name, fields: &[], takes: "", does: None }
    }
}

/// The fields a `transform` and a `copy` step take.
const MOVED: &[&str] = &["target", "scale", "about", "frame"];

/// Makes [`Operation`], [`Operation::ALL`] and each operation's row from one list, so that an operation is added, or
/// turned on, in one place, and none can be without its name, its fields, or a reader and a runner together.
macro_rules! operations {
    ($($(#[$doc:meta])+ $operation:ident: $row:expr,)+) => {
        /// An operation a request can ask for.
        #[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
        pub enum Operation {
            $($(#[$doc])+ $operation,)+
        }

        impl Operation {
            /// Every operation, in the order the listing gives them.
            pub const ALL: [Self; [$(Operation::$operation),+].len()] = [$(Self::$operation),+];

            /// Its row.
            const fn row(self) -> &'static Row {
                match self {
                    $(Self::$operation => {
                        const ROW: Row = $row;
                        &ROW
                    })+
                }
            }
        }
    };
}

operations! {
    /// A box at a frame (PARITY 1.12).
    Box: Row::done("box", &["size", "frame"], "size [x, y, z]; frame", primitive::SOLID),
    /// A cylinder at a frame (PARITY 1.12).
    Cylinder: Row::done(
        "cylinder",
        &["radius", "height", "frame"],
        "radius; height, along the frame's z; frame",
        primitive::SOLID,
    ),
    /// A sphere (PARITY 2.13).
    Sphere: Row::done("sphere", &["radius", "frame"], "radius; frame", primitive::SOLID),
    /// A cone (PARITY 2.13).
    Cone: Row::done(
        "cone",
        &["radius1", "radius2", "height", "frame"],
        "radius1 at the base and radius2 at the top, either but not both 0; height; frame",
        primitive::SOLID,
    ),
    /// A torus (PARITY 2.13).
    Torus: Row::done(
        "torus",
        &["major", "minor", "frame"],
        "major, the radius its tube's centre runs round; minor, the tube's, less than major; frame",
        primitive::SOLID,
    ),
    /// A wedge (PARITY 2.13).
    Wedge: Row::done(
        "wedge",
        &["size", "top", "frame"],
        r#"size [x, y, z]; top {"start": where it begins along x, "width": along x}; frame"#,
        primitive::SOLID,
    ),
    /// A wire through points, lines and arcs (PARITY 1.6).
    Wire: Row::done(
        "wire",
        &["points", "closed", "edges"],
        concat!(
            "points [point, ...], with straight edges between, or edges [",
            r#"{"line": [from, to]} or {"arc": [from, through, to]} or "#,
            r#"{"circle": {"centre": point, "radius": length, "normal": direction}}, ...]; "#,
            "and closed: true to join the last to the first by a straight edge, if they do not meet",
        ),
        construct::WIRE,
    ),
    /// A planar face bounded by a closed wire, with holes (PARITY 1.6).
    Face: Row::done(
        "face",
        &["outer", "holes"],
        "outer, a closed planar wire, and holes [wire, ...] inside it, if any; a wire is a label, an id \
         or a wire written in place",
        construct::FACE,
    ),
    /// A face extruded along a vector (PARITY 1.13).
    Extrude: Row::done(
        "extrude",
        &["face", "vector"],
        "face, a planar face; vector [x, y, z], the way and length it is swept",
        sweep::EXTRUDE,
    ),
    /// A face or a wire revolved about an axis (PARITY 2.15).
    Revolve: Row::done(
        "revolve",
        &["profile", "origin", "axis", "angle"],
        "profile, a face or a wire; origin, a point on the axis; axis, a direction; angle",
        sweep::REVOLVE,
    ),
    /// A body moved, turned or scaled (PARITY 2.11).
    Transform: Row::done(
        "transform",
        MOVED,
        "target, a body; scale, a number or three along about's axes, if any; about, a frame; \
         frame, where the world's frame goes, after the scale. The body keeps its ids",
        transform::TRANSFORM,
    ),
    /// A copy of a body with new ids (PARITY 2.11).
    Copy: Row::done(
        "copy",
        MOVED,
        "target, scale, about and frame, as for transform; the copy has new ids",
        transform::COPY,
    ),
    /// Volume, area, centre of mass and bounds (PARITY 1.8, 2.7).
    Measure: Row::done(
        "measure",
        &["target", "what"],
        "target, an entity; what [measurement, ...], or all the target has if left out",
        measure::MEASURE,
    ),
    /// Fuse, cut or common of two bodies (PARITY 1.11).
    Boolean: Row::later("boolean"),
    /// Rounded edges, a radius per edge (PARITY 1.14).
    Fillet: Row::later("fillet"),
    /// Bevelled edges (PARITY 1.14).
    Chamfer: Row::later("chamfer"),
}

impl Operation {
    /// The name a request gives it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        self.row().name
    }

    /// The fields its step takes beside `op` and `label`: the one list its reader checks a step against, and the guide
    /// names each. An operation this build does not do takes none yet.
    #[must_use]
    pub const fn fields(self) -> &'static [&'static str] {
        self.row().fields
    }

    /// What its step's fields hold, as the guide gives it: each of [`Self::fields`], by name.
    #[must_use]
    pub const fn takes(self) -> &'static str {
        self.row().takes
    }

    /// Whether this build does it. Each operation's step turns it on when it lands, by giving its row a reader and a
    /// runner.
    #[must_use]
    pub const fn supported(self) -> bool {
        self.row().does.is_some()
    }

    /// Its reader and its runner, if this build does it.
    pub(crate) const fn does(self) -> Option<&'static Does> {
        self.row().does.as_ref()
    }

    /// The operation a request names `name`, if occam has one.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|operation| operation.name() == name)
    }

    /// The operation the step at `at` names `name`, if this build does it.
    ///
    /// # Errors
    ///
    /// [`Code::UnknownOperation`] if occam has no operation of that name, and [`Code::NotSupported`], fault not
    /// implemented, if it has one this build does not do yet.
    pub(crate) fn asked(at: &At, name: &str) -> Result<Self, Report> {
        let Some(operation) = Self::named(name) else {
            return Err(at.report(Code::UnknownOperation).with_input(at.input().given(name)));
        };
        if operation.supported() {
            Ok(operation)
        } else {
            Err(Report::new(Code::NotSupported, Fault::NotImplemented).with_input(at.input()))
        }
    }
}

/// What this build does and what it refuses, by name, as JSON: the listing an agent reads before it writes a request.
#[must_use]
pub fn listing() -> Value {
    let names = |supported: bool| -> Vec<&str> {
        Operation::ALL.into_iter().filter(|op| op.supported() == supported).map(Operation::name).collect()
    };
    json!({"schema": crate::request::SCHEMA, "supported": names(true), "refused": names(false)})
}

#[cfg(test)]
mod tests {
    use occam_report::Input;

    use super::*;

    #[test]
    fn every_operation_has_one_name_that_reads_back() {
        for operation in Operation::ALL {
            assert_eq!(Operation::named(operation.name()), Some(operation));
        }
        let mut names: Vec<&str> = Operation::ALL.map(Operation::name).to_vec();
        names.sort_unstable();
        names.dedup();
        assert_eq!(names.len(), Operation::ALL.len());
        assert_eq!(Operation::named("loft"), None);
        assert_eq!(Operation::named("Box"), None, "names are as the listing gives them");
    }

    #[test]
    fn an_unknown_operation_is_refused_by_name_and_a_known_one_not_done_yet_as_not_implemented() {
        let at = At::root().field("steps").item(0).field("op").in_step(0, Some("loft"));
        let unknown = Operation::asked(&at, "loft").unwrap_err();
        assert_eq!((unknown.code, unknown.fault), (Code::UnknownOperation, Some(Fault::InvalidInput)));
        assert_eq!(unknown.input, Some(Input::at("steps[0].op").in_step(0, Some("loft")).given("loft")));
        let at = At::root().field("steps").item(1).field("op").in_step(1, Some("fillet"));
        let later = Operation::asked(&at, "fillet").unwrap_err();
        assert_eq!((later.code, later.fault), (Code::NotSupported, Some(Fault::NotImplemented)));
        assert_eq!(later.input, Some(Input::at("steps[1].op").in_step(1, Some("fillet"))));
        let units = occam_report::DisplayUnits::default();
        assert_eq!(later.message(units), "steps[1].op is fillet, which occam does not do yet.");
    }

    #[test]
    fn each_operation_s_reader_holds_its_step_to_the_fields_its_own_row_gives() {
        for operation in Operation::ALL.into_iter().filter(|op| op.supported()) {
            let name = operation.name();
            let step = json!({"op": name, "zzz": 1});
            let request = json!({"schema": 1, "units": {"length": "mm", "angle": "rad"}, "steps": [step]});
            let reports = crate::request::Request::parse(&request.to_string()).unwrap_err();
            let unknown: Vec<_> = reports.iter().filter(|r| r.code == Code::InputUnknownField).collect();
            let taken = operation.fields().join(", ");
            let at = Input::at("steps[0].zzz").in_step(0, Some(name)).given("zzz").expected(&taken);
            assert_eq!(unknown.iter().map(|r| r.input.clone()).collect::<Vec<_>>(), [Some(at)], "{name}");
            assert!(!operation.fields().is_empty() && !operation.takes().is_empty(), "{name}");
        }
        for operation in Operation::ALL.into_iter().filter(|op| !op.supported()) {
            assert!(operation.does().is_none(), "{operation:?} has no reader or runner");
            assert_eq!((operation.fields().len(), operation.takes()), (0, ""), "{operation:?} takes nothing yet");
        }
    }

    #[test]
    fn the_listing_says_what_is_supported_and_what_is_refused_each_operation_once() {
        let listing = listing();
        let names = |key: &str| -> Vec<String> {
            listing[key].as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect()
        };
        let (supported, refused) = (names("supported"), names("refused"));
        assert_eq!(listing["schema"], json!(1));
        assert_eq!(supported.len() + refused.len(), Operation::ALL.len());
        for operation in Operation::ALL {
            let listed = if operation.supported() { &supported } else { &refused };
            assert!(listed.contains(&operation.name().to_string()), "{operation:?}");
        }
        let done: Vec<&str> = Operation::ALL.into_iter().filter(|op| op.supported()).map(Operation::name).collect();
        assert_eq!(supported, done, "in the rows' order");
        assert_eq!(refused.first().map(String::as_str), Some("boolean"));
        assert_eq!(refused.last().map(String::as_str), Some("chamfer"));
    }
}
