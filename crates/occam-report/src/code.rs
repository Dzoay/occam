//! Source: original
//! The catalogue: every code occam can report and every remedy it can suggest (L8). Both are closed, so a match on either
//! is exhaustive and a new code cannot be forgotten (L4). `gates errors` holds every code and remedy pair to a test that
//! follows it (L2).

use serde::Serialize;

use crate::Report;
use crate::json::{SCHEMA, encode};

text_enum! {
    /// How serious a report is.
    Severity {
        /// The operation failed.
        Error = "error",
        /// The operation succeeded, with something to be careful of.
        Warning = "warning",
        /// The operation succeeded, and this is worth knowing.
        Note = "note",
    }
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

catalogue! {
    /// The text given to [`Report::from_json`] is not a report.
    Malformed = "report.malformed" {
        Error, [],
        "The text is not a report.",
        "The text is not a valid report: the problem is at line {line}, column {column}.",
    }

    /// The text given to [`Report::from_json`] uses a schema version this build cannot read.
    UnsupportedSchema = "report.unsupported-schema" {
        Error, [UseSchema],
        "The report uses a schema version this build cannot read.",
        "Schema version {found} is not supported. This build reads version {supported}.",
    }

    /// A number given as a length or an angle is NaN or infinite, or becomes infinite when converted.
    NotFinite = "units.not-finite" {
        Error, [],
        "A number given as a length or an angle is not finite.",
        "The number given is {value}, which is not finite in occam's internal units.",
    }

    /// A tolerance is not a finite number greater than zero.
    ToleranceOutOfRange = "units.tolerance-out-of-range" {
        Error, [],
        "A tolerance must be a finite number greater than zero.",
        "The tolerance {tolerance} is not a finite number greater than zero.",
    }

    /// An operation was asked for a finer tolerance than the model is built to.
    ToleranceBelowModel = "measure.tolerance-below-model" {
        Error, [UseModelTolerance],
        "An operation cannot work to a finer tolerance than its model is built to.",
        "The tolerance asked for, {requested}, is finer than the model's tolerance, {model}.",
    }

    /// What tolerance an operation worked to.
    ToleranceUsed = "measure.tolerance-used" {
        Note, [],
        "The tolerance an operation worked to.",
        "The operation worked to a tolerance of {tolerance}.",
    }

    /// A command-line argument is missing or is not understood.
    BadArgument = "cli.bad-argument" {
        Error, [],
        "A command-line argument is missing or is not understood.",
        "Argument {position} is not understood. Expected: occam distance --from X,Y,Z --to X,Y,Z \
        --model-tolerance LENGTH,ANGLE --tolerance LENGTH [--json], in millimetres and radians.",
    }

    /// The threads asked for could not be started.
    ThreadsUnavailable = "exec.threads-unavailable" {
        Error, [UseThreads],
        "The threads asked for could not be started.",
        "{threads} threads could not be started on this machine.",
    }

    /// A direction was asked of a vector too short to have one.
    DegenerateDirection = "geom.degenerate-direction" {
        Error, [],
        "A vector no longer than the model's tolerance has no direction.",
        "The vector is {length} long, no longer than the tolerance of {tolerance}, so it has no direction.",
    }

    /// A curve or surface was asked for a parameter where it is not defined.
    UndefinedParameter = "geom.undefined-parameter" {
        Error, [],
        "A curve or surface is not defined at the parameter asked for.",
        "The parameter {parameter} is outside the domain from {low} to {high}.",
    }

    /// A curve or surface was given a size that is not a finite number greater than zero.
    SizeNotPositive = "geom.size-not-positive" {
        Error, [],
        "A curve or surface was given a size that is not a finite number greater than zero.",
        "The size given, {size}, is not a finite number greater than zero.",
    }

    /// A curve or surface was given a size or an angle outside the range its form allows.
    OutOfRange = "geom.out-of-range" {
        Error, [],
        "A curve or surface was given a size or an angle outside the range its form allows.",
        "The value given, {value}, is not strictly between {low} and {high}.",
    }

    /// A curve or surface was given the wrong number of knots, poles or weights.
    WrongCount = "geom.wrong-count" {
        Error, [],
        "A curve or surface was given the wrong number of knots, poles or weights.",
        "{found} were given where {needed} are needed.",
    }

    /// A B-spline's knots decrease, or repeat more often than its degree allows.
    KnotsNotValid = "geom.knots-not-valid" {
        Error, [],
        "A B-spline's knots must not decrease, and may repeat at most its degree times inside and once more at an \
        end.",
        "Knot {index}, {knot}, is out of order or repeated more often than the degree allows.",
    }

    /// A rational curve or surface was given a weight that is not a finite number greater than zero.
    WeightNotPositive = "geom.weight-not-positive" {
        Error, [],
        "A rational curve or surface was given a weight that is not a finite number greater than zero.",
        "Weight {index} is {weight}, which is not a finite number greater than zero.",
    }

    /// A surface was asked for a derivative of an order it does not have, as a mesh has none above the first.
    NoDerivative = "geom.no-derivative" {
        Error, [],
        "A surface was asked for a derivative of an order it does not have.",
        "A derivative of order {order} was asked for, and this surface has them to order {highest}.",
    }

    /// A mesh's triangle names a vertex it does not have, does not run anticlockwise with some area in the surface's
    /// parameters, or overlaps another there.
    TriangleNotValid = "geom.triangle-not-valid" {
        Error, [],
        "A mesh's triangle must name three of its vertices, run anticlockwise with some area in the parameters, \
        and overlap no other there.",
        "Triangle {index} names a vertex there is not, does not run anticlockwise with some area in the \
        parameters, or overlaps another there.",
    }

    /// A curve was swept along a direction it runs along: a line along its own direction, which makes no surface,
    /// or a hyperbola along an asymptote without end, which makes one whose edge is at infinity.
    SweptAlongItself = "geom.swept-along-itself" {
        Error, [],
        "A curve swept along a direction it runs along makes no surface, or one with an edge at infinity.",
        "The curve runs along {direction}, the direction it is swept along: trim it, or sweep it another way.",
    }

    /// A surface was offset whose normal is not continuous: it has no second derivative, as a mesh has none.
    NotSmooth = "geom.not-smooth" {
        Error, [],
        "An offset needs a surface with second derivatives, so that its normal is continuous.",
        "The surface has derivatives to order {highest}, and an offset needs them to order {needed}.",
    }

    /// A curve in a face's parameters does not lie along its edge's curve within the band.
    EdgeOffSurface = "geom.edge-off-surface" {
        Error, [],
        "A curve in a face's parameters does not lie along its edge's curve within the band.",
        "The edge's curve on the face is {gap} from its own at {point}, more than the band of {band}.",
    }

    /// A curve given as a parameter curve leaves the plane `z = 0`, whose `x` and `y` are a face's parameters.
    NotInPlane = "geom.not-in-plane" {
        Error, [],
        "A parameter curve must be drawn in the plane z = 0, whose x and y are the face's parameters.",
        "The curve reaches {height} from the plane z = 0, in which a parameter curve is drawn.",
    }

    /// A curve has no tangent, or a surface no normal, at the parameter asked: its derivatives there span no
    /// direction.
    SingularPoint = "geom.singular-point" {
        Error, [],
        "A curve has no tangent, or a surface no normal, where its derivatives span no direction.",
        "The derivatives at the parameter asked span {span}, too little to give a direction there.",
    }

    /// A join between two curves or two surfaces is less smooth than the continuity required of it.
    JoinBelowContinuity = "geom.join-below-continuity" {
        Error, [],
        "A join is less smooth than the continuity required of it.",
        "The join first fails at order {failed}, short of the order {required} required, by {gap}.",
    }

    /// A measure did not reach the accuracy asked within the work it is allowed.
    NotConverged = "measure.not-converged" {
        Error, [],
        "A measure did not reach the accuracy asked within the work it is allowed.",
        "The measure reached an accuracy of {reached} where {asked} was asked for.",
    }

    /// An id names no live entity of the kind wanted: it was removed, or its slot holds a newer one.
    StaleId = "topo.stale-id" {
        Error, [],
        "An id names no live entity of the kind wanted: it was removed, or its slot holds a newer one.",
        "No live entity of the kind wanted has this slot and generation: the slot holds generation {live} now, or \
        nothing if that is 0.",
    }

    /// An edit would leave the model breaking an invariant of the B-rep, so it was not committed.
    InvariantFailed = "check.invariant-failed" {
        Error, [],
        "An edit would leave the model breaking an invariant of the B-rep, so it was not committed.",
        "The edit would break the invariant given as the reason, at the entities named, so the model is unchanged.",
    }

    /// The text given as a request is not JSON.
    RequestMalformed = "request.malformed" {
        Error, [],
        "The request is not JSON.",
        "The request is not JSON: the problem is at line {line}, column {column}.",
    }

    /// The request uses a schema version this build cannot read.
    RequestUnsupportedSchema = "request.unsupported-schema" {
        Error, [UseSchema],
        "The request uses a schema version this build cannot read.",
        "Request schema version {found} is not supported. This build reads version {supported}.",
    }

    /// A step asks for an operation occam does not have.
    UnknownOperation = "request.unknown-operation" {
        Error, [],
        "A step asks for an operation occam does not have.",
        "{parameter} is {given}, which is not an operation occam has.",
    }

    /// A step asks for an operation occam has, and does not do yet.
    NotSupported = "request.not-supported" {
        Error, [],
        "A step asks for an operation occam does not do yet.",
        "{parameter} is {operation}, which occam does not do yet.",
    }

    /// A request leaves out something it needs.
    InputMissing = "input.missing" {
        Error, [],
        "A request leaves out something it needs.",
        "{parameter} is missing.",
    }

    /// A request gives something of the wrong kind: text where a number is wanted, or a number where a list is.
    InputWrongType = "input.wrong-type" {
        Error, [],
        "A request gives something of the wrong kind where it gives it.",
        "{parameter} is a {given}, where a {expected} is wanted.",
    }

    /// A request gives a name outside the closed list that place takes, such as a unit.
    InputUnknownName = "input.unknown-name" {
        Error, [],
        "A request gives a name that is not one of those allowed there.",
        "{parameter} is {given}, which is not one of {expected}.",
    }

    /// A request gives a field the place it is in does not take, as a misspelt one.
    InputUnknownField = "input.unknown-field" {
        Error, [],
        "A request gives a field that is not taken there.",
        "{parameter} is not a field the request takes there.",
    }

    /// Two steps of a request have the same label, so a later step could not tell them apart.
    InputDuplicateLabel = "input.duplicate-label" {
        Error, [],
        "Two steps of a request have the same label.",
        "{parameter} is {given}, which step {first} is already labelled.",
    }

    /// A step's label is empty, or reads as an entity's id, as `edge:41/1`, so an operand naming it would be read as that id.
    InputLabelNotValid = "input.label-not-valid" {
        Error, [],
        "A label must be text that is not empty and is not an entity's id.",
        "{parameter} is {given}: a label is text that is not empty and is not an entity's id.",
    }

    /// An operand names a label no earlier step of the request has.
    InputUnknownLabel = "input.unknown-label" {
        Error, [],
        "An operand names a label no earlier step has.",
        "{parameter} names {given}, which no earlier step is labelled.",
    }

    /// A request gives none, or more than one, of the fields that are alternatives at one place.
    InputOneOf = "input.one-of" {
        Error, [],
        "A request gives none, or more than one, of the fields that are alternatives there.",
        "{parameter} gives {given}, where exactly one of {expected} is wanted.",
    }

    /// An edge would have no length: its ends, or its circle's radius, are within the band.
    NoLength = "construct.no-length" {
        Error, [],
        "An edge would have no length: its ends, or its circle's radius, are within the band.",
        "{parameter} gives an edge {length} long, no longer than the band of {band}.",
    }

    /// An arc's three points lie on one line within the band, so no circle passes through them.
    PointsInLine = "construct.points-in-line" {
        Error, [],
        "An arc's three points lie on one line, so no circle passes through them.",
        "{parameter} gives an arc whose middle point is {offset} from the line through its ends, no more than the band of {band}.",
    }

    /// An edge of a wire begins further than the band from where the edge before it ends.
    WireGap = "construct.wire-gap" {
        Error, [MeetAt],
        "An edge of a wire does not begin where the one before it ends.",
        "{parameter} begins {gap} from where the edge before it ends, at {end}, more than the band of {band}.",
    }

    /// A face needs a wire that comes back to where it begins, and this one does not.
    WireOpen = "construct.wire-open" {
        Error, [MeetAt],
        "A face needs a closed wire, and this one does not come back to where it begins.",
        "{parameter} ends {gap} from where it begins, at {start}, more than the band of {band}.",
    }

    /// A wire touches or crosses itself other than where consecutive edges meet.
    WireCrossesItself = "construct.wire-crosses-itself" {
        Error, [],
        "A wire touches or crosses itself.",
        "{parameter} touches or crosses itself at {point}, where its edges {first} and {second}, counted from 0, come within the band of {band}.",
    }

    /// Two wires of a face touch or cross.
    WiresCross = "construct.wires-cross" {
        Error, [],
        "Two wires of a face touch or cross.",
        "{parameter} touches or crosses the face's wire {other}, counted from 0 with the outer one first, at {point}, within the band of {band}.",
    }

    /// A hole of a face does not lie inside its outer wire.
    HoleOutside = "construct.hole-outside" {
        Error, [],
        "A hole of a face is not inside its outer wire.",
        "{parameter} is a hole that does not lie inside the face's outer wire: it begins at {point}.",
    }

    /// A hole of a face lies inside another of its holes.
    HolesNested = "construct.holes-nested" {
        Error, [],
        "A hole of a face lies inside another of its holes.",
        "{parameter} lies inside the face's wire {other}, another hole.",
    }

    /// A face's wires do not lie in one plane within the band.
    NotPlanar = "construct.not-planar" {
        Error, [],
        "A face's wires do not lie in one plane.",
        "{parameter} reaches {point}, {distance} from the plane of the face's outer wire, more than the band of {band}.",
    }

    /// A face's wire bounds no point clear of its edges, so it is too thin to be a face.
    NoInterior = "construct.no-interior" {
        Error, [],
        "A face's wire bounds no point clear of its edges.",
        "{parameter} bounds no point further than {clearance} from the face's wires.",
    }

    /// A face names something that is not a wire it can be made from: not a wire or a wire body, or a wire of edges that are not lines or circles.
    NotAWire = "construct.not-a-wire" {
        Error, [],
        "A face names something that is not a wire it can be made from.",
        "{parameter} names {given}, which is not a wire a face can be made from.",
    }

    /// The bytes given to load are not a model in the native format.
    NotReadable = "io.not-readable" {
        Error, [],
        "The bytes given to load are not a model in the native format.",
        "The file is not a native model: it fails at line {line}, column {column}.",
    }

    /// A native model file is of a format version this build does not read.
    OtherVersion = "io.other-version" {
        Error, [],
        "A native model file is of a format version this build does not read.",
        "The file is format version {version}; this build reads version {supported}.",
    }

    /// A model holds a curve or surface of a family the native format does not write yet, so it was not saved.
    NotSaved = "io.not-saved" {
        Error, [],
        "A model holds a curve or surface of a family the native format does not write yet, so it was not saved.",
        "The geometry named is of a family the native format does not write yet, so nothing was saved.",
    }

    /// A size that may be zero but not negative, as a cone's radius or a wedge's top width, is negative.
    InputNegative = "input.negative" {
        Error, [],
        "A request gives a negative value where none may be.",
        "{parameter} is {value}, which is less than zero.",
    }

    /// A cone's two radii are equal, which would make it a cylinder, or nothing if both are zero.
    InputConeRadiiEqual = "input.cone-radii-equal" {
        Error, [],
        "A cone's two radii must differ.",
        "{parameter} is {value}, as is the cone's other radius: a cone's radii differ, and equal ones make a cylinder.",
    }

    /// A direction a request gives is zero or not finite, so it points nowhere.
    InputNoDirection = "input.no-direction" {
        Error, [],
        "A request gives a direction that is zero or not finite.",
        "{parameter} is zero or not finite, so it has no direction.",
    }

    /// A frame's x axis lies along its z axis, so it gives no direction across it.
    InputAxesParallel = "input.axes-parallel" {
        Error, [],
        "A frame's x axis lies along its z axis.",
        "{parameter} lies along the frame's z axis, so it gives no x direction across it.",
    }

    /// An extrusion's vector lies in its face's plane within the band, so it would sweep no volume.
    VectorInPlane = "construct.vector-in-plane" {
        Error, [],
        "An extrusion's vector lies in its face's plane, so it sweeps no volume.",
        "{parameter} rises {height} from the face's plane, no more than the band of {band}.",
    }

    /// A sweep names something that is not a profile it can sweep: not the only face of a sheet, or for a revolution the only wire of a wire body, or a face that is not planar, or one with an edge that is not a line or a circle.
    NotAProfile = "construct.not-a-profile" {
        Error, [],
        "A sweep names something that is not a profile it can sweep.",
        "{parameter} names {given}, which is not a profile a sweep can be made from.",
    }

    /// A revolution's profile does not lie in one plane with its axis, within the band.
    AxisOffPlane = "construct.axis-off-plane" {
        Error, [],
        "A revolution's profile does not lie in one plane with its axis.",
        "The profile reaches {point}, {distance} from the plane through {parameter} and the profile's farthest point, more than the band of {band}.",
    }

    /// A revolution's profile lies on both sides of its axis, beyond the band.
    ProfileCrossesAxis = "construct.profile-crosses-axis" {
        Error, [],
        "A revolution's profile lies on both sides of its axis.",
        "The profile reaches {point}, {distance} beyond {parameter} from its farthest point, more than the band of {band}.",
    }
}

text_enum! {
    /// A kind of fix occam can suggest. The values to apply it with are in the [`crate::Remedy`].
    RemedyKind {
        /// Read or write the report with the schema version in the `version` parameter.
        UseSchema = "use-schema",
        /// Ask for the tolerance in the `tolerance` parameter, which is the model's.
        UseModelTolerance = "use-model-tolerance",
        /// Run on the number of threads in the `threads` parameter.
        UseThreads = "use-threads",
        /// Move the end the report names to the point in the `point` parameter, so that two ends meet.
        MeetAt = "meet-at",
    }
}

/// What the catalogue says about a code.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Entry {
    /// How serious it is.
    pub severity: Severity,
    /// One sentence on what it means.
    pub summary: &'static str,
    /// The wording, where `{name}` is replaced by the report's value of that name, in the caller's units (L9).
    pub template: &'static str,
    /// The kinds of fix it can suggest.
    pub remedies: &'static [RemedyKind],
}

impl Code {
    /// What the catalogue says about this code.
    #[must_use]
    pub const fn entry(self) -> Entry {
        Entry {
            severity: self.severity(),
            summary: self.summary(),
            template: self.template(),
            remedies: self.remedies(),
        }
    }
}

impl RemedyKind {
    /// One sentence on what following it means.
    #[must_use]
    pub const fn summary(self) -> &'static str {
        match self {
            Self::UseSchema => "Use the schema version given.",
            Self::UseModelTolerance => "Ask for the model's tolerance, or for a coarser one.",
            Self::UseThreads => {
                "Run on the number of threads given. One thread needs none started, so it always works."
            }
            Self::MeetAt => "Move the end the report names to the point given, so that the two ends meet.",
        }
    }

    /// The names of the values it carries.
    #[must_use]
    pub const fn parameters(self) -> &'static [&'static str] {
        match self {
            Self::UseSchema => &["version"],
            Self::UseModelTolerance => &["tolerance"],
            Self::UseThreads => &["threads"],
            Self::MeetAt => &["point"],
        }
    }
}

#[derive(Serialize)]
struct CodeRow {
    name: &'static str,
    code: Code,
    severity: Severity,
    summary: &'static str,
    template: &'static str,
    remedies: &'static [RemedyKind],
}

#[derive(Serialize)]
struct RemedyRow {
    name: &'static str,
    kind: RemedyKind,
    summary: &'static str,
    parameters: &'static [&'static str],
}

#[derive(Serialize)]
struct Catalogue {
    schema: u32,
    codes: Vec<CodeRow>,
    remedies: Vec<RemedyRow>,
}

/// Every code and remedy as JSON (L8).
///
/// # Errors
///
/// A [`Report`] if the catalogue cannot be written, which would be a bug.
pub fn catalogue_json() -> Result<String, Report> {
    let code_row = |code: &Code| {
        let entry = code.entry();
        CodeRow {
            name: code.name(),
            code: *code,
            severity: entry.severity,
            summary: entry.summary,
            template: entry.template,
            remedies: entry.remedies,
        }
    };
    let remedy_row = |kind: &RemedyKind| RemedyRow {
        name: kind.name(),
        kind: *kind,
        summary: kind.summary(),
        parameters: kind.parameters(),
    };
    encode(&Catalogue {
        schema: SCHEMA,
        codes: Code::ALL.iter().map(code_row).collect(),
        remedies: RemedyKind::ALL.iter().map(remedy_row).collect(),
    })
}

#[cfg(test)]
mod tests {
    use serde_json::{Value, json};

    use super::*;

    fn report_and_units_codes() -> Value {
        json!([
            {
                "name": "Malformed",
                "code": "report.malformed",
                "severity": "error",
                "summary": "The text is not a report.",
                "template": "The text is not a valid report: the problem is at line {line}, column {column}.",
                "remedies": [],
            },
            {
                "name": "UnsupportedSchema",
                "code": "report.unsupported-schema",
                "severity": "error",
                "summary": "The report uses a schema version this build cannot read.",
                "template": "Schema version {found} is not supported. This build reads version {supported}.",
                "remedies": ["use-schema"],
            },
            {
                "name": "NotFinite",
                "code": "units.not-finite",
                "severity": "error",
                "summary": "A number given as a length or an angle is not finite.",
                "template": "The number given is {value}, which is not finite in occam's internal units.",
                "remedies": [],
            },
            {
                "name": "ToleranceOutOfRange",
                "code": "units.tolerance-out-of-range",
                "severity": "error",
                "summary": "A tolerance must be a finite number greater than zero.",
                "template": "The tolerance {tolerance} is not a finite number greater than zero.",
                "remedies": [],
            },
        ])
    }

    fn measure_command_line_and_exec_codes() -> Value {
        json!([
            {
                "name": "ToleranceBelowModel",
                "code": "measure.tolerance-below-model",
                "severity": "error",
                "summary": "An operation cannot work to a finer tolerance than its model is built to.",
                "template": "The tolerance asked for, {requested}, is finer than the model's tolerance, {model}.",
                "remedies": ["use-model-tolerance"],
            },
            {
                "name": "ToleranceUsed",
                "code": "measure.tolerance-used",
                "severity": "note",
                "summary": "The tolerance an operation worked to.",
                "template": "The operation worked to a tolerance of {tolerance}.",
                "remedies": [],
            },
            {
                "name": "BadArgument",
                "code": "cli.bad-argument",
                "severity": "error",
                "summary": "A command-line argument is missing or is not understood.",
                "template": "Argument {position} is not understood. Expected: occam distance --from X,Y,Z --to X,Y,Z --model-tolerance LENGTH,ANGLE --tolerance LENGTH [--json], in millimetres and radians.",
                "remedies": [],
            },
            {
                "name": "ThreadsUnavailable",
                "code": "exec.threads-unavailable",
                "severity": "error",
                "summary": "The threads asked for could not be started.",
                "template": "{threads} threads could not be started on this machine.",
                "remedies": ["use-threads"],
            },
        ])
    }

    fn geom_codes() -> Value {
        json!([
            {
                "name": "DegenerateDirection",
                "code": "geom.degenerate-direction",
                "severity": "error",
                "summary": "A vector no longer than the model's tolerance has no direction.",
                "template": "The vector is {length} long, no longer than the tolerance of {tolerance}, so it has no direction.",
                "remedies": [],
            },
            {
                "name": "UndefinedParameter",
                "code": "geom.undefined-parameter",
                "severity": "error",
                "summary": "A curve or surface is not defined at the parameter asked for.",
                "template": "The parameter {parameter} is outside the domain from {low} to {high}.",
                "remedies": [],
            },
            {
                "name": "SizeNotPositive",
                "code": "geom.size-not-positive",
                "severity": "error",
                "summary": "A curve or surface was given a size that is not a finite number greater than zero.",
                "template": "The size given, {size}, is not a finite number greater than zero.",
                "remedies": [],
            },
            {
                "name": "OutOfRange",
                "code": "geom.out-of-range",
                "severity": "error",
                "summary": "A curve or surface was given a size or an angle outside the range its form allows.",
                "template": "The value given, {value}, is not strictly between {low} and {high}.",
                "remedies": [],
            },
        ])
    }

    fn spline_and_mesh_codes() -> Value {
        json!([
            {
                "name": "WrongCount",
                "code": "geom.wrong-count",
                "severity": "error",
                "summary": "A curve or surface was given the wrong number of knots, poles or weights.",
                "template": "{found} were given where {needed} are needed.",
                "remedies": [],
            },
            {
                "name": "KnotsNotValid",
                "code": "geom.knots-not-valid",
                "severity": "error",
                "summary": "A B-spline's knots must not decrease, and may repeat at most its degree times inside and once more at an end.",
                "template": "Knot {index}, {knot}, is out of order or repeated more often than the degree allows.",
                "remedies": [],
            },
            {
                "name": "WeightNotPositive",
                "code": "geom.weight-not-positive",
                "severity": "error",
                "summary": "A rational curve or surface was given a weight that is not a finite number greater than zero.",
                "template": "Weight {index} is {weight}, which is not a finite number greater than zero.",
                "remedies": [],
            },
            {
                "name": "NoDerivative",
                "code": "geom.no-derivative",
                "severity": "error",
                "summary": "A surface was asked for a derivative of an order it does not have.",
                "template": "A derivative of order {order} was asked for, and this surface has them to order {highest}.",
                "remedies": [],
            },
            {
                "name": "TriangleNotValid",
                "code": "geom.triangle-not-valid",
                "severity": "error",
                "summary": "A mesh's triangle must name three of its vertices, run anticlockwise with some area in the parameters, and overlap no other there.",
                "template": "Triangle {index} names a vertex there is not, does not run anticlockwise with some area in the parameters, or overlaps another there.",
                "remedies": [],
            },
            {
                "name": "SweptAlongItself",
                "code": "geom.swept-along-itself",
                "severity": "error",
                "summary": "A curve swept along a direction it runs along makes no surface, or one with an edge at infinity.",
                "template": "The curve runs along {direction}, the direction it is swept along: trim it, or sweep it another way.",
                "remedies": [],
            },
            {
                "name": "NotSmooth",
                "code": "geom.not-smooth",
                "severity": "error",
                "summary": "An offset needs a surface with second derivatives, so that its normal is continuous.",
                "template": "The surface has derivatives to order {highest}, and an offset needs them to order {needed}.",
                "remedies": [],
            },
        ])
    }

    fn edge_codes() -> Value {
        json!([
            {
                "name": "EdgeOffSurface",
                "code": "geom.edge-off-surface",
                "severity": "error",
                "summary": "A curve in a face's parameters does not lie along its edge's curve within the band.",
                "template": "The edge's curve on the face is {gap} from its own at {point}, more than the band of {band}.",
                "remedies": [],
            },
            {
                "name": "NotInPlane",
                "code": "geom.not-in-plane",
                "severity": "error",
                "summary": "A parameter curve must be drawn in the plane z = 0, whose x and y are the face's parameters.",
                "template": "The curve reaches {height} from the plane z = 0, in which a parameter curve is drawn.",
                "remedies": [],
            },
        ])
    }

    fn measure_codes() -> Value {
        json!([
            {
                "name": "SingularPoint",
                "code": "geom.singular-point",
                "severity": "error",
                "summary": "A curve has no tangent, or a surface no normal, where its derivatives span no direction.",
                "template": "The derivatives at the parameter asked span {span}, too little to give a direction there.",
                "remedies": [],
            },
            {
                "name": "JoinBelowContinuity",
                "code": "geom.join-below-continuity",
                "severity": "error",
                "summary": "A join is less smooth than the continuity required of it.",
                "template": "The join first fails at order {failed}, short of the order {required} required, by {gap}.",
                "remedies": [],
            },
            {
                "name": "NotConverged",
                "code": "measure.not-converged",
                "severity": "error",
                "summary": "A measure did not reach the accuracy asked within the work it is allowed.",
                "template": "The measure reached an accuracy of {reached} where {asked} was asked for.",
                "remedies": [],
            },
        ])
    }

    fn topology_codes() -> Value {
        json!([
            {
                "name": "StaleId",
                "code": "topo.stale-id",
                "severity": "error",
                "summary": "An id names no live entity of the kind wanted: it was removed, or its slot holds a newer one.",
                "template": "No live entity of the kind wanted has this slot and generation: the slot holds generation {live} now, or nothing if that is 0.",
                "remedies": [],
            },
            {
                "name": "InvariantFailed",
                "code": "check.invariant-failed",
                "severity": "error",
                "summary": "An edit would leave the model breaking an invariant of the B-rep, so it was not committed.",
                "template": "The edit would break the invariant given as the reason, at the entities named, so the model is unchanged.",
                "remedies": [],
            },
        ])
    }

    fn request_codes() -> Value {
        json!([
            {
                "name": "RequestMalformed",
                "code": "request.malformed",
                "severity": "error",
                "summary": "The request is not JSON.",
                "template": "The request is not JSON: the problem is at line {line}, column {column}.",
                "remedies": []
            },
            {
                "name": "RequestUnsupportedSchema",
                "code": "request.unsupported-schema",
                "severity": "error",
                "summary": "The request uses a schema version this build cannot read.",
                "template": "Request schema version {found} is not supported. This build reads version {supported}.",
                "remedies": [
                    "use-schema"
                ]
            },
            {
                "name": "UnknownOperation",
                "code": "request.unknown-operation",
                "severity": "error",
                "summary": "A step asks for an operation occam does not have.",
                "template": "{parameter} is {given}, which is not an operation occam has.",
                "remedies": []
            },
            {
                "name": "NotSupported",
                "code": "request.not-supported",
                "severity": "error",
                "summary": "A step asks for an operation occam does not do yet.",
                "template": "{parameter} is {operation}, which occam does not do yet.",
                "remedies": []
            }
        ])
    }

    fn input_codes() -> Value {
        json!([
            {
                "name": "InputMissing",
                "code": "input.missing",
                "severity": "error",
                "summary": "A request leaves out something it needs.",
                "template": "{parameter} is missing.",
                "remedies": []
            },
            {
                "name": "InputWrongType",
                "code": "input.wrong-type",
                "severity": "error",
                "summary": "A request gives something of the wrong kind where it gives it.",
                "template": "{parameter} is a {given}, where a {expected} is wanted.",
                "remedies": []
            },
            {
                "name": "InputUnknownName",
                "code": "input.unknown-name",
                "severity": "error",
                "summary": "A request gives a name that is not one of those allowed there.",
                "template": "{parameter} is {given}, which is not one of {expected}.",
                "remedies": []
            },
            {
                "name": "InputUnknownField",
                "code": "input.unknown-field",
                "severity": "error",
                "summary": "A request gives a field that is not taken there.",
                "template": "{parameter} is not a field the request takes there.",
                "remedies": []
            }
        ])
    }

    fn label_codes() -> Value {
        json!([
            {
                "name": "InputDuplicateLabel",
                "code": "input.duplicate-label",
                "severity": "error",
                "summary": "Two steps of a request have the same label.",
                "template": "{parameter} is {given}, which step {first} is already labelled.",
                "remedies": []
            },
            {
                "name": "InputLabelNotValid",
                "code": "input.label-not-valid",
                "severity": "error",
                "summary": "A label must be text that is not empty and is not an entity's id.",
                "template": "{parameter} is {given}: a label is text that is not empty and is not an entity's id.",
                "remedies": []
            },
            {
                "name": "InputUnknownLabel",
                "code": "input.unknown-label",
                "severity": "error",
                "summary": "An operand names a label no earlier step has.",
                "template": "{parameter} names {given}, which no earlier step is labelled.",
                "remedies": []
            }
        ])
    }

    fn construct_codes() -> Value {
        json!([
            {
                "name": "InputOneOf",
                "code": "input.one-of",
                "severity": "error",
                "summary": "A request gives none, or more than one, of the fields that are alternatives there.",
                "template": "{parameter} gives {given}, where exactly one of {expected} is wanted.",
                "remedies": []
            },
            {
                "name": "NoLength",
                "code": "construct.no-length",
                "severity": "error",
                "summary": "An edge would have no length: its ends, or its circle's radius, are within the band.",
                "template": "{parameter} gives an edge {length} long, no longer than the band of {band}.",
                "remedies": []
            },
            {
                "name": "PointsInLine",
                "code": "construct.points-in-line",
                "severity": "error",
                "summary": "An arc's three points lie on one line, so no circle passes through them.",
                "template": "{parameter} gives an arc whose middle point is {offset} from the line through its ends, no more than the band of {band}.",
                "remedies": []
            },
            {
                "name": "WireGap",
                "code": "construct.wire-gap",
                "severity": "error",
                "summary": "An edge of a wire does not begin where the one before it ends.",
                "template": "{parameter} begins {gap} from where the edge before it ends, at {end}, more than the band of {band}.",
                "remedies": ["meet-at"]
            },
            {
                "name": "WireOpen",
                "code": "construct.wire-open",
                "severity": "error",
                "summary": "A face needs a closed wire, and this one does not come back to where it begins.",
                "template": "{parameter} ends {gap} from where it begins, at {start}, more than the band of {band}.",
                "remedies": ["meet-at"]
            },
            {
                "name": "WireCrossesItself",
                "code": "construct.wire-crosses-itself",
                "severity": "error",
                "summary": "A wire touches or crosses itself.",
                "template": "{parameter} touches or crosses itself at {point}, where its edges {first} and {second}, counted from 0, come within the band of {band}.",
                "remedies": []
            },
        ])
    }

    fn face_codes() -> Value {
        json!([
            {
                "name": "WiresCross",
                "code": "construct.wires-cross",
                "severity": "error",
                "summary": "Two wires of a face touch or cross.",
                "template": "{parameter} touches or crosses the face's wire {other}, counted from 0 with the outer one first, at {point}, within the band of {band}.",
                "remedies": []
            },
            {
                "name": "HoleOutside",
                "code": "construct.hole-outside",
                "severity": "error",
                "summary": "A hole of a face is not inside its outer wire.",
                "template": "{parameter} is a hole that does not lie inside the face's outer wire: it begins at {point}.",
                "remedies": []
            },
            {
                "name": "HolesNested",
                "code": "construct.holes-nested",
                "severity": "error",
                "summary": "A hole of a face lies inside another of its holes.",
                "template": "{parameter} lies inside the face's wire {other}, another hole.",
                "remedies": []
            },
            {
                "name": "NotPlanar",
                "code": "construct.not-planar",
                "severity": "error",
                "summary": "A face's wires do not lie in one plane.",
                "template": "{parameter} reaches {point}, {distance} from the plane of the face's outer wire, more than the band of {band}.",
                "remedies": []
            },
            {
                "name": "NoInterior",
                "code": "construct.no-interior",
                "severity": "error",
                "summary": "A face's wire bounds no point clear of its edges.",
                "template": "{parameter} bounds no point further than {clearance} from the face's wires.",
                "remedies": []
            },
            {
                "name": "NotAWire",
                "code": "construct.not-a-wire",
                "severity": "error",
                "summary": "A face names something that is not a wire it can be made from.",
                "template": "{parameter} names {given}, which is not a wire a face can be made from.",
                "remedies": []
            },
        ])
    }

    fn io_codes() -> Value {
        json!([
            {
                "name": "NotReadable",
                "code": "io.not-readable",
                "severity": "error",
                "summary": "The bytes given to load are not a model in the native format.",
                "template": "The file is not a native model: it fails at line {line}, column {column}.",
                "remedies": []
            },
            {
                "name": "OtherVersion",
                "code": "io.other-version",
                "severity": "error",
                "summary": "A native model file is of a format version this build does not read.",
                "template": "The file is format version {version}; this build reads version {supported}.",
                "remedies": []
            },
            {
                "name": "NotSaved",
                "code": "io.not-saved",
                "severity": "error",
                "summary": "A model holds a curve or surface of a family the native format does not write yet, so it was not saved.",
                "template": "The geometry named is of a family the native format does not write yet, so nothing was saved.",
                "remedies": []
            }
        ])
    }

    fn primitive_codes() -> Value {
        json!([
            {
                "name": "InputNegative",
                "code": "input.negative",
                "severity": "error",
                "summary": "A request gives a negative value where none may be.",
                "template": "{parameter} is {value}, which is less than zero.",
                "remedies": []
            },
            {
                "name": "InputConeRadiiEqual",
                "code": "input.cone-radii-equal",
                "severity": "error",
                "summary": "A cone's two radii must differ.",
                "template": "{parameter} is {value}, as is the cone's other radius: a cone's radii differ, and equal ones make a cylinder.",
                "remedies": []
            },
            {
                "name": "InputNoDirection",
                "code": "input.no-direction",
                "severity": "error",
                "summary": "A request gives a direction that is zero or not finite.",
                "template": "{parameter} is zero or not finite, so it has no direction.",
                "remedies": []
            },
            {
                "name": "InputAxesParallel",
                "code": "input.axes-parallel",
                "severity": "error",
                "summary": "A frame's x axis lies along its z axis.",
                "template": "{parameter} lies along the frame's z axis, so it gives no x direction across it.",
                "remedies": []
            }
        ])
    }

    fn sweep_codes() -> Value {
        json!([
            {
                "name": "VectorInPlane",
                "code": "construct.vector-in-plane",
                "severity": "error",
                "summary": "An extrusion's vector lies in its face's plane, so it sweeps no volume.",
                "template": "{parameter} rises {height} from the face's plane, no more than the band of {band}.",
                "remedies": []
            },
            {
                "name": "NotAProfile",
                "code": "construct.not-a-profile",
                "severity": "error",
                "summary": "A sweep names something that is not a profile it can sweep.",
                "template": "{parameter} names {given}, which is not a profile a sweep can be made from.",
                "remedies": []
            },
            {
                "name": "AxisOffPlane",
                "code": "construct.axis-off-plane",
                "severity": "error",
                "summary": "A revolution's profile does not lie in one plane with its axis.",
                "template": "The profile reaches {point}, {distance} from the plane through {parameter} and the profile's farthest point, more than the band of {band}.",
                "remedies": []
            },
            {
                "name": "ProfileCrossesAxis",
                "code": "construct.profile-crosses-axis",
                "severity": "error",
                "summary": "A revolution's profile lies on both sides of its axis.",
                "template": "The profile reaches {point}, {distance} beyond {parameter} from its farthest point, more than the band of {band}.",
                "remedies": []
            }
        ])
    }

    fn remedies() -> Value {
        json!([
            {
                "name": "UseSchema",
                "kind": "use-schema",
                "summary": "Use the schema version given.",
                "parameters": ["version"],
            },
            {
                "name": "UseModelTolerance",
                "kind": "use-model-tolerance",
                "summary": "Ask for the model's tolerance, or for a coarser one.",
                "parameters": ["tolerance"],
            },
            {
                "name": "UseThreads",
                "kind": "use-threads",
                "summary": "Run on the number of threads given. One thread needs none started, so it always works.",
                "parameters": ["threads"],
            },
            {
                "name": "MeetAt",
                "kind": "meet-at",
                "summary": "Move the end the report names to the point given, so that the two ends meet.",
                "parameters": ["point"],
            },
        ])
    }

    #[test]
    fn the_catalogue_lists_every_code_and_remedy() {
        let value: Value = serde_json::from_str(&catalogue_json().unwrap()).unwrap();
        let codes: Vec<Value> = [
            report_and_units_codes(),
            measure_command_line_and_exec_codes(),
            geom_codes(),
            spline_and_mesh_codes(),
            edge_codes(),
            measure_codes(),
            topology_codes(),
            request_codes(),
            input_codes(),
            label_codes(),
            construct_codes(),
            face_codes(),
            io_codes(),
            primitive_codes(),
            sweep_codes(),
        ]
        .into_iter()
        .filter_map(|rows| rows.as_array().cloned())
        .flatten()
        .collect();
        assert_eq!(value, json!({"schema": 2, "codes": codes, "remedies": remedies()}));
    }

    #[test]
    fn a_remedy_names_the_parameters_it_carries() {
        assert_eq!(RemedyKind::UseSchema.parameters(), ["version"]);
    }

    #[test]
    fn severities_have_stable_text() {
        let texts: Vec<&str> = Severity::ALL.iter().map(|s| s.text()).collect();
        assert_eq!(texts, ["error", "warning", "note"]);
    }
}
