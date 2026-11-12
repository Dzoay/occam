//! Source: original
//! The operations a request can ask for, by the names it gives them: the first release's (docs/architecture/
//! operations.md). Each says whether this build does it; everything else is refused by name (S6: an agent could not tell
//! a repairable input from an unsupported one).

use occam_report::{Code, Fault, Report};
use serde_json::{Value, json};

use crate::read::At;

/// An operation a request can ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Operation {
    /// A box at a frame (PARITY 1.12).
    Box,
    /// A cylinder at a frame (PARITY 1.12).
    Cylinder,
    /// A sphere (PARITY 2.13).
    Sphere,
    /// A cone (PARITY 2.13).
    Cone,
    /// A torus (PARITY 2.13).
    Torus,
    /// A wedge (PARITY 2.13).
    Wedge,
    /// A wire through points, lines and arcs (PARITY 1.6).
    Wire,
    /// A planar face bounded by a closed wire, with holes (PARITY 1.6).
    Face,
    /// A face extruded along a vector (PARITY 1.13).
    Extrude,
    /// A face or a wire revolved about an axis (PARITY 2.15).
    Revolve,
    /// A body moved, turned or scaled (PARITY 2.11).
    Transform,
    /// A copy of a body with new ids (PARITY 2.11).
    Copy,
    /// Volume, area, centre of mass and bounds (PARITY 1.8, 2.7).
    Measure,
    /// Fuse, cut or common of two bodies (PARITY 1.11).
    Boolean,
    /// Rounded edges, a radius per edge (PARITY 1.14).
    Fillet,
    /// Bevelled edges (PARITY 1.14).
    Chamfer,
}

impl Operation {
    /// Every operation, in the order the listing gives them.
    pub const ALL: [Self; 16] = [
        Self::Box,
        Self::Cylinder,
        Self::Sphere,
        Self::Cone,
        Self::Torus,
        Self::Wedge,
        Self::Wire,
        Self::Face,
        Self::Extrude,
        Self::Revolve,
        Self::Transform,
        Self::Copy,
        Self::Measure,
        Self::Boolean,
        Self::Fillet,
        Self::Chamfer,
    ];

    /// The name a request gives it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Box => "box",
            Self::Cylinder => "cylinder",
            Self::Sphere => "sphere",
            Self::Cone => "cone",
            Self::Torus => "torus",
            Self::Wedge => "wedge",
            Self::Wire => "wire",
            Self::Face => "face",
            Self::Extrude => "extrude",
            Self::Revolve => "revolve",
            Self::Transform => "transform",
            Self::Copy => "copy",
            Self::Measure => "measure",
            Self::Boolean => "boolean",
            Self::Fillet => "fillet",
            Self::Chamfer => "chamfer",
        }
    }

    /// The operation a request names `name`, if occam has one.
    #[must_use]
    pub fn named(name: &str) -> Option<Self> {
        Self::ALL.into_iter().find(|operation| operation.name() == name)
    }

    /// Whether this build does it. Each operation's step turns it on when it lands.
    #[must_use]
    pub const fn supported(self) -> bool {
        match self {
            Self::Wire
            | Self::Face
            | Self::Extrude
            | Self::Revolve
            | Self::Box
            | Self::Cylinder
            | Self::Sphere
            | Self::Cone
            | Self::Torus
            | Self::Wedge => true,
            Self::Transform | Self::Copy | Self::Measure | Self::Boolean | Self::Fillet | Self::Chamfer => false,
        }
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
        let made = ["box", "cylinder", "sphere", "cone", "torus", "wedge", "wire", "face", "extrude", "revolve"];
        assert_eq!(supported, made);
        assert_eq!(refused.first().map(String::as_str), Some("transform"));
        assert_eq!(refused.last().map(String::as_str), Some("chamfer"));
    }
}
