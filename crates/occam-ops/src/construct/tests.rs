//! Source: original
//! Wires and faces built from requests, as the runner builds them: step by step through an edit that `commit` checks,
//! then the whole model checked again. A rectangle with a round hole is one face that passes the checker; a wire with a
//! gap, a wire that crosses itself, a hole outside its face and the rest are refused before any model is changed.

use occam_math::Exec;
use occam_report::{Code, EntityKind, EntityRef, Input, RemedyKind, Report, followed_remedy};
use occam_topo::{Body, BodyKind, Edge, Face, Loop, Snapshot, Vertex, Wire};
use serde_json::{Value, json};

use crate::{Labels, Request, default_tolerance, execute};

/// What a request made: the model, and what each step made.
type Built = (Snapshot, Vec<Option<EntityRef>>);

/// Runs the request `value` on a new model as the runner does: the first refusal, of the request or a step, is the
/// result.
fn build(value: &Value) -> Result<Built, Vec<Report>> {
    let request = Request::parse(&value.to_string())?;
    let mut model = Snapshot::empty(request.tolerance.unwrap_or(default_tolerance().unwrap()));
    let (mut labels, mut made) = (Labels::default(), Vec::new());
    for step in &request.steps {
        let mut edit = model.edit();
        let one = execute(&mut edit, step, &labels).map_err(|report| vec![report])?;
        model = edit.commit(&Exec::serial())?.0;
        if let (Some(label), Some(entity)) = (&step.label, one) {
            labels.insert(label, entity);
        }
        made.push(one);
    }
    Ok((model, made))
}

/// A request of `steps` in millimetres and degrees.
#[expect(clippy::needless_pass_by_value, reason = "the tests write their steps in place with `json!`")]
fn request(steps: Value) -> Value {
    json!({"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": steps})
}

/// The one report of the request `steps`, which must be refused.
fn refused(steps: Value) -> Report {
    let reports = build(&request(steps)).map(|_| ()).unwrap_err();
    assert_eq!(reports.len(), 1, "{reports:#?}");
    reports.into_iter().next().unwrap()
}

/// The request `steps` built, every check passed.
fn built(steps: Value) -> Built {
    let (model, made) = build(&request(steps)).unwrap_or_else(|reports| panic!("{reports:#?}"));
    assert_eq!(model.check_all(&Exec::serial()), [], "the whole model passes the checker");
    (model, made)
}

/// The value named `name` of `report`, as millimetres or a count.
fn number(report: &Report, name: &str) -> f64 {
    report.value(name).and_then(occam_report::NamedValue::number).unwrap_or(f64::NAN)
}

/// The rectangle 40 by 20 with a round hole of radius 3 at its middle.
fn plate() -> Value {
    json!([
        {"op": "wire", "label": "outline", "points": [[0, 0, 0], [40, 0, 0], [40, 20, 0], [0, 20, 0]], "closed": true},
        {"op": "wire", "label": "hole", "edges": [{"circle": {"centre": [20, 10, 0], "radius": 3, "normal": [0, 0, 1]}}]},
        {"op": "face", "label": "plate", "outer": "outline", "holes": ["hole"]}
    ])
}

#[test]
fn a_rectangle_with_a_round_hole_becomes_one_face_that_passes_the_checker() {
    let (model, made) = built(plate());
    let face = made[2].unwrap();
    assert_eq!(face.kind, EntityKind::Face);
    assert_eq!(
        (model.ids::<Face>().len(), model.ids::<Loop>().len(), model.ids::<Edge>().len(), model.ids::<Vertex>().len()),
        (1, 2, 5, 5)
    );
    assert_eq!(model.ids::<Wire>(), [], "the face took its wires");
    let bodies = model.ids::<Body>();
    assert_eq!(bodies.len(), 1);
    assert_eq!(model.view(bodies[0]).unwrap().kind(), BodyKind::Sheet);
    assert_eq!(model.ids::<Edge>().len(), 5, "the wires' edges bound the face, with their ids");
}

#[test]
fn a_face_from_wires_written_in_place_is_the_same_face() {
    let steps = json!([{"op": "face", "outer": {"points": [[0, 0, 0], [40, 0, 0], [40, 20, 0], [0, 20, 0]], "closed": true},
        "holes": [{"edges": [{"circle": {"centre": [20, 10, 0], "radius": 3, "normal": [0, 0, -1]}}]}]}]);
    let (model, _) = built(steps);
    assert_eq!((model.ids::<Face>().len(), model.ids::<Loop>().len(), model.ids::<Edge>().len()), (1, 2, 5));
}

#[test]
fn a_wire_with_a_gap_is_refused_naming_the_two_ends_and_the_gap_and_meeting_them_follows_the_remedy() {
    let wire = |second: Value| json!([{"op": "wire", "edges": [{"line": [[0, 0, 0], [10, 0, 0]]}, {"line": [second, [10, 10, 0]]}]}]);
    let report = refused(wire(json!([10, 0.5, 0])));
    assert_eq!(report.code, Code::WireGap);
    assert_eq!(report.input, Some(Input::at("steps[0].edges[1]").in_step(0, Some("wire"))));
    assert!((number(&report, "gap") - 0.5).abs() < 1e-12);
    assert_eq!(report.value("end").map(|v| v.value), Some(occam_report::Value::Point([10.0, 0.0, 0.0])));
    followed_remedy!(report, Code::WireGap, RemedyKind::MeetAt, |remedy| {
        let Some(occam_report::Value::Point(p)) = remedy.value("point").map(|v| v.value) else {
            return Err(Report::new(Code::InputMissing, occam_report::Fault::InvalidInput));
        };
        build(&request(wire(json!(p)))).map(|_| ()).map_err(|reports| reports.into_iter().next().unwrap())
    });
}

#[test]
fn a_face_from_an_open_wire_is_refused_and_ending_it_where_it_began_follows_the_remedy() {
    let points = |last: Option<Value>| {
        let mut points = vec![json!([0, 0, 0]), json!([10, 0, 0]), json!([10, 10, 0])];
        points.extend(last);
        json!([{"op": "wire", "label": "w", "points": points}, {"op": "face", "outer": "w"}])
    };
    let report = refused(points(None));
    assert_eq!(report.code, Code::WireOpen);
    assert_eq!(report.input, Some(Input::at("steps[1].outer").in_step(1, Some("face"))));
    assert!((number(&report, "gap") - 200.0_f64.sqrt()).abs() < 1e-9);
    followed_remedy!(report, Code::WireOpen, RemedyKind::MeetAt, |remedy| {
        let Some(occam_report::Value::Point(p)) = remedy.value("point").map(|v| v.value) else {
            return Err(Report::new(Code::InputMissing, occam_report::Fault::InvalidInput));
        };
        build(&request(points(Some(json!(p))))).map(|_| ()).map_err(|reports| reports.into_iter().next().unwrap())
    });
}

#[test]
fn a_hole_outside_its_face_is_refused() {
    let steps = json!([
        {"op": "wire", "label": "outline", "points": [[0, 0, 0], [40, 0, 0], [40, 20, 0], [0, 20, 0]], "closed": true},
        {"op": "wire", "label": "hole", "edges": [{"circle": {"centre": [60, 10, 0], "radius": 3, "normal": [0, 0, 1]}}]},
        {"op": "face", "outer": "outline", "holes": ["hole"]}
    ]);
    let report = refused(steps);
    assert_eq!(report.code, Code::HoleOutside);
    assert_eq!(report.input, Some(Input::at("steps[2].holes[0]").in_step(2, Some("face"))));
}

#[test]
fn a_wire_that_crosses_itself_is_refused_where_it_crosses() {
    let bow = json!([[0, 0, 0], [10, 10, 0], [10, 0, 0], [0, 10, 0]]);
    let report = refused(json!([{"op": "wire", "points": bow, "closed": true}]));
    assert_eq!(report.code, Code::WireCrossesItself);
    let Some(occam_report::Value::Point([x, y, z])) = report.value("point").map(|v| v.value) else {
        panic!("{report:?}")
    };
    assert!((x - 5.0).abs() < 1e-9 && (y - 5.0).abs() < 1e-9 && z.abs() < 1e-9, "{report:?}");
    assert_eq!((number(&report, "first"), number(&report, "second")), (0.0, 2.0));
    let inline = refused(json!([{"op": "face", "outer": {"points": bow, "closed": true}}]));
    assert_eq!(
        (inline.code, inline.input.clone()),
        (Code::WireCrossesItself, Some(Input::at("steps[0].outer").in_step(0, Some("face"))))
    );
}

#[test]
fn a_hole_crossing_the_outer_wire_or_inside_another_hole_is_refused() {
    let circle =
        |x: f64, r: f64| json!({"edges": [{"circle": {"centre": [x, 10, 0], "radius": r, "normal": [0, 0, 1]}}]});
    let face = |holes: Value| json!([{"op": "face", "outer": {"points": [[0, 0, 0], [40, 0, 0], [40, 20, 0], [0, 20, 0]], "closed": true}, "holes": holes}]);
    let crossing = refused(face(json!([circle(38.0, 5.0)])));
    assert_eq!((crossing.code, number(&crossing, "other")), (Code::WiresCross, 0.0));
    assert_eq!(crossing.input, Some(Input::at("steps[0].holes[0]").in_step(0, Some("face"))));
    let nested = refused(face(json!([circle(20.0, 6.0), circle(20.0, 2.0)])));
    assert_eq!((nested.code, number(&nested, "other")), (Code::HolesNested, 1.0));
    assert_eq!(nested.input, Some(Input::at("steps[0].holes[1]").in_step(0, Some("face"))));
}

#[test]
fn wires_out_of_one_plane_are_refused_with_the_point_furthest_from_it() {
    let tilted = json!([[0, 0, 0], [40, 0, 0], [40, 20, 0], [0, 20, 0.5]]);
    let report = refused(json!([{"op": "face", "outer": {"points": tilted, "closed": true}}]));
    assert_eq!(report.code, Code::NotPlanar);
    assert!(number(&report, "distance") > 0.1, "{report:?}");
}

#[test]
fn edges_with_no_length_and_arcs_through_points_in_line_are_refused() {
    let short = refused(json!([{"op": "wire", "points": [[0, 0, 0], [0, 0, 0.000_000_01], [5, 0, 0]]}]));
    assert_eq!(
        (short.code, short.input.clone()),
        (Code::NoLength, Some(Input::at("steps[0].points[1]").in_step(0, Some("wire"))))
    );
    let flat = refused(json!([{"op": "wire", "edges": [{"arc": [[0, 0, 0], [5, 0, 0], [10, 0, 0]]}]}]));
    assert_eq!((flat.code, number(&flat, "offset")), (Code::PointsInLine, 0.0));
    let tiny = refused(
        json!([{"op": "wire", "edges": [{"circle": {"centre": [0, 0, 0], "radius": 1e-9, "normal": [0, 0, 1]}}]}]),
    );
    assert_eq!(tiny.code, Code::NoLength);
}

#[test]
fn a_concave_outline_with_arcs_bounds_a_face_with_a_witness_inside_it() {
    let slot = json!([
        {"line": [[0, 0, 0], [30, 0, 0]]},
        {"arc": [[30, 0, 0], [35, 5, 0], [30, 10, 0]]},
        {"line": [[30, 10, 0], [10, 10, 0]]},
        {"line": [[10, 10, 0], [10, 30, 0]]},
        {"line": [[10, 30, 0], [0, 30, 0]]},
        {"line": [[0, 30, 0], [0, 0, 0]]}
    ]);
    let (model, made) = built(json!([{"op": "wire", "label": "l", "edges": slot}, {"op": "face", "outer": "l"}]));
    let face = model.resolve::<Face>(made[1].unwrap()).unwrap();
    let witness = model.view(model.view(face).unwrap().loops()[0]).unwrap().witness();
    assert!(witness.u().is_finite() && witness.v().is_finite());
}

#[test]
fn a_wire_in_inches_is_converted_once_to_millimetres() {
    let steps = json!([{"op": "wire", "label": "w", "points": [[0, 0, 0], [1, 0, 0]]}]);
    let (model, made) =
        build(&json!({"schema": 1, "units": {"length": "in", "angle": "deg"}, "steps": steps})).unwrap();
    let body = model.resolve::<Body>(made[0].unwrap()).unwrap();
    assert_eq!(model.view(body).unwrap().kind(), BodyKind::Wire);
    let edge = model.ids::<Edge>()[0];
    let (lo, hi) = model.view(edge).unwrap().range();
    assert!((hi - lo - 25.4).abs() < 1e-12, "{lo} {hi}");
}

#[test]
fn a_wire_or_face_step_with_bad_fields_is_refused_with_each_one_in_place() {
    let reports = Request::parse(
        &request(json!([
            {"op": "wire", "points": [[0, 0]], "edges": []},
            {"op": "wire", "points": [[0, 0, 0], [1, "a", 0]], "closed": "yes"},
            {"op": "face", "holes": "h"},
            {"op": "wire", "edges": [{"line": [[0, 0, 0]], "arc": []}, {"circle": {"centre": [0, 0, 0], "radius": -1, "normal": [0, 0, 0]}}]}
        ]))
        .to_string(),
    )
    .unwrap_err();
    let found: Vec<(Code, String)> =
        reports.iter().map(|r| (r.code, r.input.as_ref().map_or(String::new(), |i| i.parameter.clone()))).collect();
    let want = [
        (Code::InputOneOf, "steps[0]"),
        (Code::InputWrongType, "steps[1].closed"),
        (Code::InputWrongType, "steps[1].points[1][1]"),
        (Code::InputMissing, "steps[2].outer"),
        (Code::InputWrongType, "steps[2].holes"),
        (Code::InputOneOf, "steps[3].edges[0]"),
        (Code::SizeNotPositive, "steps[3].edges[1].circle.radius"),
        (Code::DegenerateDirection, "steps[3].edges[1].circle.normal"),
    ];
    assert_eq!(found, want.map(|(code, at)| (code, at.to_string())));
}

#[test]
fn a_face_named_by_something_that_is_not_a_wire_is_refused() {
    let steps = json!([
        {"op": "wire", "label": "w", "points": [[0, 0, 0], [10, 0, 0], [10, 10, 0]], "closed": true},
        {"op": "face", "label": "f", "outer": "w"},
        {"op": "face", "outer": "f"}
    ]);
    let report = refused(steps);
    assert_eq!(report.code, Code::NotAWire);
    assert_eq!(report.input.as_ref().and_then(|i| i.given.clone()), Some("face:0/1".to_string()));
    assert_eq!(report.entities, [EntityRef::new(EntityKind::Face, 0, 1)], "named, so that it is described");
    let unknown = refused(json!([{"op": "face", "outer": "nowhere"}]));
    assert_eq!(unknown.code, Code::InputUnknownLabel);
}
