//! Source: original
//! The request's tests: each kind of bad input gets its own code and says where it is in the request; every bad input is
//! reported at once; and a good request is converted from its units.

use occam_report::{Code, DisplayUnits, Fault, Input, NamedValue, RemedyKind, Report, followed_remedy};

use super::*;

/// The units every case but the units' own gives.
const UNITS: &str = r#""units": {"length": "mm", "angle": "deg"}"#;

/// A request of schema 1 with `units` as given, and `rest` after it.
fn request(units: &str, rest: &str) -> String {
    format!(r#"{{"schema": 1, {units}, {rest}}}"#)
}

/// What reading `text` reports, each as its code, fault and place.
fn refused(text: &str) -> Vec<(Code, Option<Fault>, Option<Input>)> {
    let reports = Request::parse(text).unwrap_err();
    reports.iter().map(|r| (r.code, r.fault, r.input.clone())).collect()
}

/// A bad input at `parameter`, of code `code`.
fn bad(code: Code, input: Input) -> (Code, Option<Fault>, Option<Input>) {
    (code, Some(Fault::InvalidInput), Some(input))
}

#[test]
fn a_request_reads_with_its_units_and_no_tolerance_unless_it_gives_one() {
    let read = Request::parse(&request(UNITS, r#""steps": []"#)).unwrap();
    let units = Units { length: LengthUnit::Millimetre, angle: AngleUnit::Degree };
    assert_eq!(read, Request { units, tolerance: None, steps: Vec::new() });
    let text = request(
        r#""units": {"length": "cm", "angle": "deg"}"#,
        r#""tolerance": {"length": 1e-5, "angle": 1e-3}, "steps": []"#,
    );
    let want = Tolerance::new(
        Length::new(1e-5, LengthUnit::Centimetre).unwrap(),
        Angle::new(1e-3, AngleUnit::Degree).unwrap(),
    );
    assert_eq!(Request::parse(&text).unwrap().tolerance, Some(want.unwrap()));
    assert_eq!(Request::parse(&text).unwrap().tolerance.unwrap().length(), Length::from_millimetres(1e-4));
}

#[test]
fn each_kind_of_bad_input_in_the_requests_own_fields_gets_its_code_and_its_place() {
    let units_with = |inner: &str| format!(r#""units": {{{inner}}}"#);
    let cases = [
        ("[1]".to_string(), vec![bad(Code::InputWrongType, Input::at("request").given("list").expected("object"))]),
        (format!(r#"{{{UNITS}, "steps": []}}"#), vec![bad(Code::InputMissing, Input::at("schema"))]),
        (
            format!(r#"{{"schema": "1", {UNITS}, "steps": []}}"#),
            vec![bad(Code::InputWrongType, Input::at("schema").given("text").expected("whole number"))],
        ),
        (r#"{"schema": 1, "steps": []}"#.to_string(), vec![bad(Code::InputMissing, Input::at("units"))]),
        (
            request(&units_with(r#""length": "furlong", "angle": "deg""#), r#""steps": []"#),
            vec![bad(Code::InputUnknownName, Input::at("units.length").given("furlong").expected("mm, cm, m, in"))],
        ),
        (
            request(&units_with(r#""length": "mm""#), r#""steps": []"#),
            vec![bad(Code::InputMissing, Input::at("units.angle"))],
        ),
        (
            request(&units_with(r#""length": "mm", "angle": "rad", "colour": 1"#), r#""steps": []"#),
            vec![bad(Code::InputUnknownField, Input::at("units.colour").given("colour"))],
        ),
        (
            request(UNITS, r#""tolerance": {"length": "1e-7", "angle": 1e-3}, "steps": []"#),
            vec![bad(Code::InputWrongType, Input::at("tolerance.length").given("text").expected("number"))],
        ),
        (
            request(UNITS, r#""tolerance": {"length": -1e-7, "angle": 1e-3}, "steps": []"#),
            vec![bad(Code::ToleranceOutOfRange, Input::at("tolerance"))],
        ),
        (
            request(UNITS, r#""tolerance": {"length": 1e308, "angle": 1e-3}, "steps": []"#)
                .replace(r#""mm""#, r#""m""#),
            vec![bad(Code::NotFinite, Input::at("tolerance.length"))],
        ),
        (
            request(UNITS, r#""steps": 3"#),
            vec![bad(Code::InputWrongType, Input::at("steps").given("number").expected("list"))],
        ),
        (
            request(UNITS, r#""step": []"#),
            vec![
                bad(Code::InputUnknownField, Input::at("step").given("step")),
                bad(Code::InputMissing, Input::at("steps")),
            ],
        ),
    ];
    for (text, want) in cases {
        assert_eq!(refused(&text), want, "{text}");
    }
}

#[test]
fn each_kind_of_bad_input_in_a_step_gets_its_code_its_place_and_its_step() {
    let in_step = |path: &str, op: Option<&str>| {
        let index = path.split(['[', ']']).nth(1).unwrap().parse().unwrap();
        Input::at(path).in_step(index, op)
    };
    let not_done =
        |path: &str, op: &str| (Code::NotSupported, Some(Fault::NotImplemented), Some(in_step(path, Some(op))));
    let cases = [
        ("[1]", vec![bad(Code::InputWrongType, in_step("steps[0]", None).given("number").expected("object"))]),
        (r#"[{"label": "a"}]"#, vec![bad(Code::InputMissing, in_step("steps[0].op", None))]),
        (
            r#"[{"op": 5}]"#,
            vec![bad(Code::InputWrongType, in_step("steps[0].op", None).given("number").expected("text"))],
        ),
        (r#"[{"op": "loft"}]"#, vec![bad(Code::UnknownOperation, in_step("steps[0].op", Some("loft")).given("loft"))]),
        (r#"[{"op": "fillet"}]"#, vec![not_done("steps[0].op", "fillet")]),
        (
            r#"[{"op": "fillet", "label": ""}]"#,
            vec![
                bad(Code::InputLabelNotValid, in_step("steps[0].label", Some("fillet")).given("")),
                not_done("steps[0].op", "fillet"),
            ],
        ),
        (
            r#"[{"op": "fillet", "label": "edge:1/1"}]"#,
            vec![
                bad(Code::InputLabelNotValid, in_step("steps[0].label", Some("fillet")).given("edge:1/1")),
                not_done("steps[0].op", "fillet"),
            ],
        ),
        (
            r#"[{"op": "fillet", "label": 7}]"#,
            vec![
                bad(Code::InputWrongType, in_step("steps[0].label", Some("fillet")).given("number").expected("text")),
                not_done("steps[0].op", "fillet"),
            ],
        ),
    ];
    for (steps, want) in cases {
        let text = request(UNITS, &format!(r#""steps": {steps}"#));
        assert_eq!(refused(&text), want, "{text}");
    }
}

#[test]
fn a_label_an_earlier_step_has_is_refused_naming_that_step() {
    let text = request(
        UNITS,
        r#""steps": [{"op": "box", "label": "a"}, {"op": "cylinder", "label": "b"}, {"op": "sphere", "label": "a"}]"#,
    );
    let reports = Request::parse(&text).unwrap_err();
    let duplicate: Vec<&Report> = reports.iter().filter(|r| r.code == Code::InputDuplicateLabel).collect();
    assert_eq!(duplicate.len(), 1);
    assert_eq!(duplicate[0].input, Some(Input::at("steps[2].label").in_step(2, Some("sphere")).given("a")));
    assert_eq!(duplicate[0].value("first"), Some(&NamedValue::count("first", 0)));
    assert_eq!(duplicate[0].message(DisplayUnits::default()), "steps[2].label is a, which step 0 is already labelled.");
}

#[test]
fn every_bad_input_is_reported_at_once_field_by_field() {
    let units = r#""units": {"length": "furlong", "angle": "deg", "extra": 1}"#;
    let text = format!(r#"{{"schema": 1, "colour": 2, {units}, "steps": [{{"op": "loft"}}, {{"op": "fillet"}}]}}"#);
    let codes: Vec<Code> = Request::parse(&text).unwrap_err().iter().map(|r| r.code).collect();
    let want = [
        Code::InputUnknownField,
        Code::InputUnknownField,
        Code::InputUnknownName,
        Code::UnknownOperation,
        Code::NotSupported,
    ];
    assert_eq!(codes, want);
}

#[test]
fn text_that_is_not_json_is_refused_saying_where() {
    let reports = Request::parse("{\n  \"schema\": 1,\n  oops").unwrap_err();
    assert_eq!(reports.len(), 1);
    assert_eq!((reports[0].code, reports[0].fault), (Code::RequestMalformed, Some(Fault::InvalidInput)));
    assert_eq!(reports[0].value("line"), Some(&NamedValue::count("line", 3)));
    assert_eq!(reports[0].value("column"), Some(&NamedValue::count("column", 3)));
}

#[test]
fn another_schema_version_is_refused_alone_with_the_version_to_use() {
    let text = |version: &str| format!(r#"{{"schema": {version}, {UNITS}, "steps": [{{"op": "loft"}}]}}"#);
    let reports = Request::parse(&text("9")).unwrap_err();
    assert_eq!(reports.len(), 1, "a request of another version is not read further");
    let report = &reports[0];
    assert_eq!((report.code, report.input.clone()), (Code::RequestUnsupportedSchema, Some(Input::at("schema"))));
    assert_eq!(
        report.message(DisplayUnits::default()),
        "Request schema version 9 is not supported. This build reads version 1."
    );
    let fixed = |version: &str| format!(r#"{{"schema": {version}, {UNITS}, "steps": []}}"#);
    let schema_9 = Request::parse(&fixed("9")).unwrap_err().remove(0);
    followed_remedy!(schema_9, Code::RequestUnsupportedSchema, RemedyKind::UseSchema, |remedy| {
        let version = remedy.value("version").and_then(NamedValue::number).unwrap_or(0.0);
        Request::parse(&fixed(&version.to_string())).map_err(|mut reports| reports.remove(0))
    });
}

#[test]
fn the_default_tolerance_is_o12s() {
    let tolerance = default_tolerance().unwrap();
    assert_eq!((tolerance.length(), tolerance.angle()), (Length::from_millimetres(1e-7), Angle::from_radians(1e-9)));
}
