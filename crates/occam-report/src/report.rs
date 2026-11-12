//! Source: original
//! The report itself (L1, L3, L5).

use std::ops::Deref;

use serde::{Deserialize, Serialize};

use crate::{Code, DisplayUnits, EntityRef, Fault, NamedValue, Reason, RemedyKind, Stage};

/// A fix occam suggests, with the parameters to apply it (L3).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Remedy {
    /// What kind of fix it is.
    pub kind: RemedyKind,
    /// The entities it applies to.
    pub entities: Vec<EntityRef>,
    /// The numbers to apply it with.
    pub values: Vec<NamedValue>,
}

impl Remedy {
    /// A remedy of this kind with no parameters yet.
    #[must_use]
    pub const fn new(kind: RemedyKind) -> Self {
        Self { kind, entities: Vec::new(), values: Vec::new() }
    }

    /// Adds an entity it applies to.
    #[must_use]
    pub fn with_entity(mut self, entity: EntityRef) -> Self {
        self.entities.push(entity);
        self
    }

    /// Adds a number to apply it with.
    #[must_use]
    pub fn with_value(mut self, value: NamedValue) -> Self {
        self.values.push(value);
        self
    }

    /// The parameter of this name.
    #[must_use]
    pub fn value(&self, name: &str) -> Option<&NamedValue> {
        find(&self.values, name)
    }
}

/// What a solver had achieved when it stopped (L5). The input model is unchanged on failure, so there is no last valid
/// one to name (S6: no agent used it).
#[derive(Clone, Debug, PartialEq, Serialize, Deserialize)]
pub struct Partial {
    /// Where it stopped.
    pub stage: Stage,
    /// The entities it failed on, such as a pair of faces.
    pub failing: Vec<EntityRef>,
    /// What it had measured, such as curves found and the residual gap.
    pub values: Vec<NamedValue>,
}

/// Where in a request a bad input is, as the request wrote it: an invalid input names the operation, the parameter, the
/// value and the rule (S6: blind on S1's report, an agent repaired a zero radius by inventing one). The rule is the code,
/// a number is among the values, and the names are here.
#[derive(Clone, Debug, Default, PartialEq, Eq, Serialize, Deserialize)]
pub struct Input {
    /// The step's index in the request, from 0, if the input is in a step.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub step: Option<u32>,
    /// The operation the step asks for, as written, if it names one.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    /// Where the input is, as a path into the request: `units.length`, `steps[2].radius`.
    pub parameter: String,
    /// What the request gave there, where it is text or of the wrong kind: a name, a label, a JSON type.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub given: Option<String>,
    /// What the rule wants there, where that is text: a JSON type, or the names allowed.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub expected: Option<String>,
}

impl Input {
    /// The input at `parameter`, a path into the request.
    #[must_use]
    pub fn at(parameter: &str) -> Self {
        Self { parameter: parameter.to_string(), ..Self::default() }
    }

    /// In step `index`, which asks for `operation` if it names one.
    #[must_use]
    pub fn in_step(mut self, index: u32, operation: Option<&str>) -> Self {
        self.step = Some(index);
        self.operation = operation.map(str::to_string);
        self
    }

    /// What the request gave there.
    #[must_use]
    pub fn given(mut self, given: &str) -> Self {
        self.given = Some(given.to_string());
        self
    }

    /// What the rule wants there.
    #[must_use]
    pub fn expected(mut self, expected: &str) -> Self {
        self.expected = Some(expected.to_string());
        self
    }

    /// The text of the field `name` for the wording, if it has one.
    fn field(&self, name: &str) -> Option<String> {
        match name {
            "step" => self.step.map(|step| step.to_string()),
            "operation" => self.operation.clone(),
            "parameter" => Some(self.parameter.clone()),
            "given" => self.given.clone(),
            "expected" => self.expected.clone(),
            _ => None,
        }
    }
}

/// What a [`Report`] says.
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Details {
    /// What happened.
    pub code: Code,
    /// What kind of failure it is, chosen where it was found. A note or a warning has none.
    #[serde(default)]
    pub fault: Option<Fault>,
    /// Why, from a closed list, where one applies.
    #[serde(default)]
    pub reason: Option<Reason>,
    /// The entities involved.
    pub entities: Vec<EntityRef>,
    /// The numbers involved, named and tagged by quantity.
    pub values: Vec<NamedValue>,
    /// What can be done about it.
    pub remedies: Vec<Remedy>,
    /// What had been achieved, if the operation failed partway.
    #[serde(default)]
    pub partial: Option<Partial>,
    /// Where in a request the input it is about is, if it is about one.
    #[serde(default)]
    pub input: Option<Input>,
}

/// Something occam has to say about an operation (L1). The details are on the heap, so that every `Result<T, Report>` in the
/// kernel stays small on the path that succeeds.
#[derive(Clone, Debug, PartialEq, Deserialize)]
#[serde(transparent)]
pub struct Report(Box<Details>);

impl Deref for Report {
    type Target = Details;

    fn deref(&self) -> &Details {
        &self.0
    }
}

impl Report {
    /// A report of this code with nothing else yet.
    #[must_use]
    pub fn new(code: Code, fault: Fault) -> Self {
        Self::with_fault(code, Some(fault))
    }

    /// A note or a warning on an operation that succeeded, which is no failure and so has no fault.
    #[must_use]
    pub fn note(code: Code) -> Self {
        Self::with_fault(code, None)
    }

    fn with_fault(code: Code, fault: Option<Fault>) -> Self {
        let (entities, values, remedies) = (Vec::new(), Vec::new(), Vec::new());
        Self(Box::new(Details { code, fault, reason: None, entities, values, remedies, partial: None, input: None }))
    }

    /// Says why, from the closed list.
    #[must_use]
    pub fn with_reason(mut self, reason: Reason) -> Self {
        self.0.reason = Some(reason);
        self
    }

    /// Adds the tolerance that decided the outcome, as `tolerance-in-force`, so that an agent reasons from the right one
    /// (S6: c04).
    #[must_use]
    pub fn with_tolerance_in_force(self, millimetres: f64) -> Self {
        self.with_value(NamedValue::length("tolerance-in-force", millimetres))
    }

    /// Adds an entity involved.
    #[must_use]
    pub fn with_entity(mut self, entity: EntityRef) -> Self {
        self.0.entities.push(entity);
        self
    }

    /// Adds a number involved.
    #[must_use]
    pub fn with_value(mut self, value: NamedValue) -> Self {
        self.0.values.push(value);
        self
    }

    /// Adds something that can be done about it.
    #[must_use]
    pub fn with_remedy(mut self, remedy: Remedy) -> Self {
        self.0.remedies.push(remedy);
        self
    }

    /// Records what had been achieved.
    #[must_use]
    pub fn with_partial(mut self, partial: Partial) -> Self {
        self.0.partial = Some(partial);
        self
    }

    /// Says where in a request the input it is about is.
    #[must_use]
    pub fn with_input(mut self, input: Input) -> Self {
        self.0.input = Some(input);
        self
    }

    /// The number of this name.
    #[must_use]
    pub fn value(&self, name: &str) -> Option<&NamedValue> {
        find(&self.values, name)
    }

    /// The first remedy of this kind.
    #[must_use]
    pub fn remedy(&self, kind: RemedyKind) -> Option<&Remedy> {
        self.remedies.iter().find(|remedy| remedy.kind == kind)
    }

    /// The wording from the catalogue, with numbers in the caller's units (L9) and the names of a request's input as
    /// written. A placeholder with no value or input field of its name stays as it is, so a mismatch shows.
    #[must_use]
    pub fn message(&self, units: DisplayUnits) -> String {
        let mut out = String::new();
        let mut rest = self.code.entry().template;
        while let Some((before, after)) = rest.split_once('{') {
            out.push_str(before);
            let Some((name, tail)) = after.split_once('}') else {
                out.push('{');
                rest = after;
                continue;
            };
            if let Some(value) = self.value(name) {
                out.push_str(&value.shown(units));
            } else if let Some(text) = self.input.as_ref().and_then(|input| input.field(name)) {
                out.push_str(&text);
            } else {
                out.push('{');
                out.push_str(name);
                out.push('}');
            }
            rest = tail;
        }
        out.push_str(rest);
        out
    }
}

fn find<'a>(values: &'a [NamedValue], name: &str) -> Option<&'a NamedValue> {
    values.iter().find(|value| value.name == name)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::{AngleUnit, EntityKind, LengthUnit};

    fn schema_report() -> Report {
        Report::new(Code::UnsupportedSchema, Fault::InvalidInput)
            .with_value(NamedValue::count("found", 9))
            .with_value(NamedValue::count("supported", 1))
    }

    #[test]
    fn the_wording_comes_from_the_catalogue_with_values_filled_in() {
        let want = "Schema version 9 is not supported. This build reads version 1.";
        assert_eq!(schema_report().message(DisplayUnits::default()), want);
    }

    #[test]
    fn a_placeholder_with_no_value_stays_visible() {
        let want = "Schema version {found} is not supported. This build reads version {supported}.";
        assert_eq!(Report::new(Code::UnsupportedSchema, Fault::InvalidInput).message(DisplayUnits::default()), want);
    }

    #[test]
    fn numbers_in_the_wording_use_the_callers_units() {
        let report = Report::new(Code::Malformed, Fault::InvalidInput).with_value(NamedValue::length("line", 20.0));
        let units = DisplayUnits { length: LengthUnit::Centimetre, angle: AngleUnit::Radian };
        assert_eq!(
            report.message(units),
            "The text is not a valid report: the problem is at line 2 cm, column {column}."
        );
    }

    #[test]
    fn accessors_find_by_name_and_kind() {
        let remedy = Remedy::new(RemedyKind::UseSchema).with_value(NamedValue::count("version", 1));
        let report = schema_report().with_remedy(remedy.clone());
        assert_eq!(report.value("found"), Some(&NamedValue::count("found", 9)));
        assert_eq!(report.value("absent"), None);
        assert_eq!(report.remedy(RemedyKind::UseSchema), Some(&remedy));
        assert_eq!(schema_report().remedy(RemedyKind::UseSchema), None);
        assert_eq!(remedy.value("version"), Some(&NamedValue::count("version", 1)));
        assert_eq!(remedy.value("absent"), None);
    }

    #[test]
    fn a_bad_input_is_named_by_where_it_is_in_the_request() {
        let input = Input::at("steps[2].units").in_step(2, Some("box")).given("furlong").expected("mm, cm, m or in");
        let report = Report::new(Code::InputUnknownName, Fault::InvalidInput).with_input(input.clone());
        assert_eq!(report.input, Some(input.clone()));
        let units = DisplayUnits::default();
        assert_eq!(report.message(units), "steps[2].units is furlong, which is not one of mm, cm, m or in.");
        let fields = ["step", "operation", "parameter", "given", "expected", "other"].map(|name| input.field(name));
        let want = [Some("2"), Some("box"), Some("steps[2].units"), Some("furlong"), Some("mm, cm, m or in"), None];
        assert_eq!(fields, want.map(|text| text.map(str::to_string)));
        let named = report.with_value(NamedValue::count("given", 3));
        assert_eq!(named.message(units), "steps[2].units is 3, which is not one of mm, cm, m or in.", "a value wins");
        let bare = Report::new(Code::InputUnknownName, Fault::InvalidInput).with_input(Input::at("units"));
        assert_eq!(bare.message(units), "units is {given}, which is not one of {expected}.");
        assert_eq!(Input::at("units").field("step"), None);
    }

    #[test]
    fn a_report_is_one_pointer_so_results_stay_small() {
        assert_eq!(std::mem::size_of::<Report>(), std::mem::size_of::<usize>());
    }

    #[test]
    fn builders_accumulate() {
        let edge = EntityRef::new(EntityKind::Edge, 7, 1);
        let partial = Partial { failing: vec![edge], values: vec![], stage: Stage::Pairs };
        let report = Report::new(Code::Malformed, Fault::InvalidInput).with_entity(edge).with_partial(partial.clone());
        assert_eq!(report.entities, [edge]);
        assert_eq!(report.partial, Some(partial));
        assert_eq!(Remedy::new(RemedyKind::UseSchema).with_entity(edge).entities, [edge]);
    }
}
