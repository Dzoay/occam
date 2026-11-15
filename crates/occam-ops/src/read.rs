//! Source: original
//! Reading a request's JSON, every value with its place in the request, so that each bad input is reported where it is
//! and with its own code (S6). Nothing here knows an operation: it reads numbers, text, names from closed lists and
//! objects, and says what is wrong with them.

use occam_report::{Code, Fault, Input, NamedValue, Report};
use serde_json::{Map, Value};

/// A place in a request: its path, and the step it is in, if any, with that step's operation.
#[derive(Clone, Debug, PartialEq, Eq)]
pub(crate) struct At {
    path: String,
    step: Option<(u32, Option<String>)>,
}

impl At {
    /// The request itself.
    pub(crate) const fn root() -> Self {
        Self { path: String::new(), step: None }
    }

    /// Its field `name`.
    pub(crate) fn field(&self, name: &str) -> Self {
        let path = if self.path.is_empty() { name.to_string() } else { format!("{}.{name}", self.path) };
        Self { path, step: self.step.clone() }
    }

    /// Its item at `index`.
    pub(crate) fn item(&self, index: usize) -> Self {
        Self { path: format!("{}[{index}]", self.path), step: self.step.clone() }
    }

    /// The same place, known to be in step `index`, which asks for `operation` if it names one.
    pub(crate) fn in_step(mut self, index: u32, operation: Option<&str>) -> Self {
        self.step = Some((index, operation.map(str::to_string)));
        self
    }

    /// Its path, or `request` for the request itself.
    pub(crate) fn path(&self) -> &str {
        if self.path.is_empty() { "request" } else { &self.path }
    }

    /// The report's account of where it is.
    pub(crate) fn input(&self) -> Input {
        let input = Input::at(self.path());
        match &self.step {
            Some((index, operation)) => input.in_step(*index, operation.as_deref()),
            None => input,
        }
    }

    /// A report of `code` at this place, a bad input.
    pub(crate) fn report(&self, code: Code) -> Report {
        Report::new(code, Fault::InvalidInput).with_input(self.input())
    }
}

/// The name of a JSON value's kind, as a report gives it.
pub(crate) const fn kind(value: &Value) -> &'static str {
    match value {
        Value::Null => "null",
        Value::Bool(_) => "true or false",
        Value::Number(_) => "number",
        Value::String(_) => "text",
        Value::Array(_) => "list",
        Value::Object(_) => "object",
    }
}

/// `value` at `at` is not the kind `expected`.
pub(crate) fn wrong_type(at: &At, value: &Value, expected: &str) -> Report {
    let input = at.input().given(kind(value)).expected(expected);
    Report::new(Code::InputWrongType, Fault::InvalidInput).with_input(input)
}

/// Nothing is at `at`, where something is needed.
pub(crate) fn missing(at: &At) -> Report {
    at.report(Code::InputMissing)
}

/// The fields of the object at `at`, every one not in `known` reported as a field it does not take, with those it does.
pub(crate) fn object<'a>(
    at: &At,
    value: &'a Value,
    known: &[&str],
    out: &mut Vec<Report>,
) -> Option<&'a Map<String, Value>> {
    let Value::Object(fields) = value else {
        out.push(wrong_type(at, value, "object"));
        return None;
    };
    let unknown = fields.keys().filter(|name| !known.contains(&name.as_str()));
    let taken = known.join(", ");
    let report = |name: &String| {
        let input = at.field(name).input().given(name).expected(&taken);
        Report::new(Code::InputUnknownField, Fault::InvalidInput).with_input(input)
    };
    out.extend(unknown.map(report));
    Some(fields)
}

/// The field `name` of `fields`, which is needed.
pub(crate) fn needed<'a>(at: &At, fields: &'a Map<String, Value>, name: &str) -> Result<&'a Value, Report> {
    fields.get(name).ok_or_else(|| missing(&at.field(name)))
}

/// The number at `at`.
pub(crate) fn number(at: &At, value: &Value) -> Result<f64, Report> {
    value.as_f64().ok_or_else(|| wrong_type(at, value, "number"))
}

/// The whole number at `at`, from 0.
pub(crate) fn whole(at: &At, value: &Value) -> Result<u64, Report> {
    value.as_u64().ok_or_else(|| wrong_type(at, value, "whole number"))
}

/// The text at `at`.
pub(crate) fn text<'a>(at: &At, value: &'a Value) -> Result<&'a str, Report> {
    value.as_str().ok_or_else(|| wrong_type(at, value, "text"))
}

/// The name at `at`, one of `names`, as the value paired with it.
pub(crate) fn name<T: Copy>(at: &At, value: &Value, names: &[(&str, T)]) -> Result<T, Report> {
    let given = text(at, value)?;
    names.iter().find(|(name, _)| *name == given).map(|&(_, found)| found).ok_or_else(|| {
        let allowed: Vec<&str> = names.iter().map(|(name, _)| *name).collect();
        let input = at.input().given(given).expected(&allowed.join(", "));
        Report::new(Code::InputUnknownName, Fault::InvalidInput).with_input(input)
    })
}

/// `reports` without each `input.missing` that an `input.unknown-field` beside it explains: a field missing where an
/// unknown one is within two letters of its name, which is that field misspelt, so the one report says it all.
pub(crate) fn without_knock_ons(reports: Vec<Report>) -> Vec<Report> {
    let unknown: Vec<(&str, &str)> = reports
        .iter()
        .filter(|r| r.code == Code::InputUnknownField)
        .filter_map(|r| r.input.as_ref())
        .map(|input| split(&input.parameter))
        .collect();
    let explained = |report: &Report| {
        report.code == Code::InputMissing
            && report.input.as_ref().is_some_and(|input| {
                let (place, name) = split(&input.parameter);
                unknown.iter().any(|&(at, given)| at == place && within_two(given, name))
            })
    };
    let keep: Vec<bool> = reports.iter().map(|r| !explained(r)).collect();
    reports.into_iter().zip(keep).filter_map(|(report, kept)| kept.then_some(report)).collect()
}

/// The place a field is in and its name: `steps[0]` and `radius` from `steps[0].radius`.
fn split(parameter: &str) -> (&str, &str) {
    parameter.rsplit_once('.').unwrap_or(("", parameter))
}

/// Whether `a` becomes `b` by at most two letters added, dropped or changed (Levenshtein's distance), and is not it.
fn within_two(a: &str, b: &str) -> bool {
    let (a, b): (Vec<char>, Vec<char>) = (a.chars().collect(), b.chars().collect());
    if a == b || a.len().abs_diff(b.len()) > 2 {
        return false;
    }
    let mut row: Vec<usize> = (0..=b.len()).collect();
    for (i, x) in a.iter().enumerate() {
        let mut diagonal = i;
        let mut next = vec![i.saturating_add(1)];
        for (j, y) in b.iter().enumerate() {
            let changed = diagonal.saturating_add(usize::from(x != y));
            let dropped = row.get(j.saturating_add(1)).copied().unwrap_or(usize::MAX).saturating_add(1);
            let added = next.last().copied().unwrap_or(usize::MAX).saturating_add(1);
            diagonal = row.get(j.saturating_add(1)).copied().unwrap_or(usize::MAX);
            next.push(changed.min(dropped).min(added));
        }
        row = next;
    }
    row.last().is_some_and(|&distance| distance <= 2)
}

/// A count as a report's value.
pub(crate) fn count(name: &str, n: usize) -> NamedValue {
    NamedValue::count(name, u32::try_from(n).unwrap_or(u32::MAX))
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;

    #[test]
    fn a_place_is_a_path_into_the_request_and_knows_its_step() {
        let root = At::root();
        assert_eq!(root.path(), "request");
        let step = root.field("steps").item(2).in_step(2, Some("box"));
        let at = step.field("size").item(0);
        assert_eq!(at.path(), "steps[2].size[0]");
        assert_eq!(at.input(), Input::at("steps[2].size[0]").in_step(2, Some("box")));
        assert_eq!(root.field("units").field("length").input(), Input::at("units.length"));
    }

    #[test]
    fn each_kind_of_value_has_a_name() {
        let names = [json!(null), json!(true), json!(1), json!("a"), json!([]), json!({})].map(|v| kind(&v));
        assert_eq!(names, ["null", "true or false", "number", "text", "list", "object"]);
    }

    #[test]
    fn values_of_the_wrong_kind_are_reported_with_what_was_wanted() {
        let at = At::root().field("schema");
        let report = whole(&at, &json!(1.5)).unwrap_err();
        assert_eq!(report.code, Code::InputWrongType);
        assert_eq!(report.input, Some(Input::at("schema").given("number").expected("whole number")));
        assert_eq!(whole(&at, &json!(3)), Ok(3));
        assert_eq!(number(&at, &json!(-2.5)), Ok(-2.5));
        let report = number(&at, &json!("2")).unwrap_err();
        assert_eq!(report.input.as_ref().and_then(|input| input.given.as_deref()), Some("text"));
        assert_eq!(text(&at, &json!("a")), Ok("a"));
        let report = text(&at, &json!(1)).unwrap_err();
        assert_eq!(report.input.as_ref().and_then(|input| input.expected.as_deref()), Some("text"));
    }

    #[test]
    fn a_name_outside_its_list_says_which_names_are_allowed() {
        let at = At::root().field("units").field("length");
        let names = [("mm", 1), ("in", 2)];
        assert_eq!(name(&at, &json!("in"), &names), Ok(2));
        let report = name(&at, &json!("furlong"), &names).unwrap_err();
        assert_eq!(report.code, Code::InputUnknownName);
        assert_eq!(report.input, Some(Input::at("units.length").given("furlong").expected("mm, in")));
        assert_eq!(name(&at, &json!(3), &names).unwrap_err().code, Code::InputWrongType);
    }

    #[test]
    fn a_name_is_within_two_of_another_by_two_letters_added_dropped_or_changed_and_not_by_three() {
        let pairs = [
            ("radious", "radius", true),
            ("radus", "radius", true),
            ("step", "steps", true),
            ("hieght", "height", true),
            ("abc", "abcde", true),
            ("abc", "abcdef", false),
            ("colour", "radius", false),
            ("kitten", "sitting", false),
            ("same", "same", false),
            ("", "ab", true),
        ];
        for (a, b, near) in pairs {
            assert_eq!((within_two(a, b), within_two(b, a)), (near, near), "{a} and {b}");
        }
    }

    #[test]
    fn an_object_reports_each_field_it_does_not_take_and_a_missing_one_is_named() {
        let at = At::root().field("units");
        let mut out = Vec::new();
        let value = json!({"length": "mm", "lenght": "mm", "colour": 1});
        let fields = object(&at, &value, &["length", "angle"], &mut out).unwrap();
        let unknown: Vec<Option<Input>> = out.iter().map(|r| r.input.clone()).collect();
        let taken = |at: &str, given: &str| Some(Input::at(at).given(given).expected("length, angle"));
        assert_eq!(unknown, [taken("units.colour", "colour"), taken("units.lenght", "lenght")]);
        assert!(out.iter().all(|r| r.code == Code::InputUnknownField && r.fault == Some(Fault::InvalidInput)));
        assert_eq!(needed(&at, fields, "length"), Ok(&json!("mm")));
        assert_eq!(needed(&at, fields, "angle").unwrap_err().input, Some(Input::at("units.angle")));
        let mut out = Vec::new();
        assert_eq!(object(&at, &json!([1]), &[], &mut out), None);
        assert_eq!(out[0].input, Some(Input::at("units").given("list").expected("object")));
        assert_eq!(count("first", 4), NamedValue::count("first", 4));
    }
}
