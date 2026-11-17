//! Source: original
//! Wires and faces built from requests, as the runner builds them: step by step through an edit that `commit` checks,
//! then the whole model checked again. A rectangle with a round hole is one face that passes the checker; a wire with a
//! gap, a wire that crosses itself, a hole outside its face and the rest are refused before any model is changed.

use occam_math::Exec;
use occam_report::{Code, EntityKind, EntityRef, Input, Remedy, RemedyKind, Report, followed_remedy};
use occam_topo::{Body, BodyKind, Changes, Edge, Edit, Face, Loop, Snapshot, Vertex, Wire};
use occam_units::Vec3;
use serde_json::{Value, json};

use crate::{Labels, Request, Step, default_tolerance, execute};

/// What a request made: the model, and what each step made.
pub(crate) type Built = (Snapshot, Vec<Option<EntityRef>>);

/// What steps run on a new model left: the model, the labels, and what each step made and changed.
pub(crate) struct Run {
    pub model: Snapshot,
    pub labels: Labels,
    pub made: Vec<Option<EntityRef>>,
    pub changes: Vec<Changes>,
}

/// Runs `steps` of `request` on a new model as the runner does, each through the runner's own step, with this build's
/// `execute`: the first refusal, of a step or of its commit, is the result. The one step loop of this crate's tests.
pub(crate) fn run(request: &Request, steps: &[Step]) -> Result<Run, Vec<Report>> {
    let model = Snapshot::empty(request.tolerance.unwrap_or(default_tolerance().unwrap()));
    let mut run = Run { model, labels: Labels::default(), made: Vec::new(), changes: Vec::new() };
    for step in steps {
        let work = |edit: &mut Edit| execute(edit, step, &run.labels);
        let (model, made, changes) = occam_fixtures::step(&run.model, step.index, step.operation.name(), work)
            .map_err(|(reports, _)| reports)?;
        if let (Some(label), Some(entity)) = (&step.label, made) {
            run.labels.insert(label, entity);
        }
        run.model = model;
        run.made.push(made);
        run.changes.push(changes);
    }
    Ok(run)
}

/// Runs the request `value` on a new model as the runner does: the first refusal, of the request or a step, is the
/// result.
pub(crate) fn build(value: &Value) -> Result<Built, Vec<Report>> {
    let request = Request::parse(&value.to_string())?;
    let run = run(&request, &request.steps)?;
    Ok((run.model, run.made))
}

/// A request of `steps` in millimetres and degrees.
#[expect(clippy::needless_pass_by_value, reason = "the tests write their steps in place with `json!`")]
pub(crate) fn request(steps: Value) -> Value {
    json!({"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": steps})
}

/// The one report of the request `steps`, which must be refused.
pub(crate) fn refused(steps: Value) -> Report {
    let reports = build(&request(steps)).map(|_| ()).unwrap_err();
    assert_eq!(reports.len(), 1, "{reports:#?}");
    reports.into_iter().next().unwrap()
}

/// The request `steps` built, every check passed.
pub(crate) fn built(steps: Value) -> Built {
    let (model, made) = build(&request(steps)).unwrap_or_else(|reports| panic!("{reports:#?}"));
    assert_eq!(model.check_all(&Exec::serial()), [], "the whole model passes the checker");
    (model, made)
}

/// The value named `name` of `report`, as millimetres or a count.
pub(crate) fn number(report: &Report, name: &str) -> f64 {
    report.value(name).and_then(occam_report::NamedValue::number).unwrap_or(f64::NAN)
}

/// The rectangle 40 by 20 with a round hole of radius 3 at its middle.
pub(crate) fn plate() -> Value {
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
fn edges_that_are_closed_join_the_last_to_the_first_as_points_do() {
    let corner = json!([{"line": [[0, 0, 0], [10, 0, 0]]}, {"line": [[10, 0, 0], [10, 10, 0]]}]);
    let square = json!([[5, 5, 0], [15, 5, 0], [15, 15, 0], [5, 15, 0], [5, 5, 0]]);
    let round = json!([{"arc": [[0, 0, 0], [5, 5, 0], [10, 0, 0]]}, {"line": [[10, 0, 0], [0, 0, 0]]}]);
    let edges_of = |wire: Value| {
        let (model, made) = built(json!([wire, {"op": "face", "label": "f", "outer": "w"}]));
        assert_eq!(made[1].map(|face| face.kind), Some(EntityKind::Face), "{wire}");
        model.ids::<Edge>().len()
    };
    let closed = |field: &str, given: &Value| json!({"op": "wire", "label": "w", field: given, "closed": true});
    assert_eq!(edges_of(closed("edges", &corner)), 3, "a straight edge from the last end to the first start");
    assert_eq!(edges_of(closed("points", &json!([[0, 0, 0], [10, 0, 0], [10, 10, 0]]))), 3, "as through points");
    let square: Vec<Value> = square.as_array().unwrap().windows(2).map(|pair| json!({"line": pair})).collect();
    assert_eq!(edges_of(closed("edges", &json!(square))), 4, "none where they already meet");
    assert_eq!(edges_of(closed("edges", &round)), 2, "nor after an arc that comes back");
    let open =
        refused(json!([{"op": "wire", "label": "w", "edges": corner, "closed": false}, {"op": "face", "outer": "w"}]));
    assert_eq!(open.code, Code::WireOpen);
    // Ends the band apart meet, so nothing joins them: a joining edge would be no longer than the band.
    let coarse = |gap: f64| {
        let mut edges = corner.as_array().unwrap().clone();
        edges.push(json!({"line": [[10, 10, 0], [0, gap, 0]]}));
        let mut text = request(json!([{"op": "wire", "edges": edges, "closed": true}]));
        text["tolerance"] = json!({"length": 0.5, "angle": 1e-6});
        build(&text).map(|(model, _)| model.ids::<Edge>().len())
    };
    assert_eq!((coarse(0.5), coarse(0.75)), (Ok(3), Ok(4)));
    let unread = refused(json!([{"op": "wire", "edges": corner, "closed": "yes"}]));
    assert_eq!(
        (unread.code, unread.input.as_ref().map(|input| input.parameter.as_str())),
        (Code::InputWrongType, Some("steps[0].closed"))
    );
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
fn a_wire_whose_arc_turns_back_along_the_line_before_it_is_refused_where_they_run_together() {
    // The arc, of radius 1e7, is tangent to the line where the line ends and turns back along it: 5e-8 from the line's
    // middle, inside the band, where neither crosses the other nor has an end.
    let edges = json!([{"line": [[0, 0, 0], [2, 0, 0]]}, {"arc": [[2, 0, 0], [0, 2e-7, 0], [-2, 8e-7, 0]]}]);
    let report = refused(json!([{"op": "wire", "edges": edges}]));
    let named = (report.code, number(&report, "first"), number(&report, "second"));
    assert_eq!(named, (Code::WireCrossesItself, 0.0, 1.0), "{report:?}");
    let Some(occam_report::Value::Point(p)) = report.value("point").map(|v| v.value) else { panic!("{report:?}") };
    assert!(p.iter().zip([1.0, 0.0, 0.0]).all(|(x, q)| (x - q).abs() < 1e-9), "the line's middle: {report:?}");
}

#[test]
fn a_wire_whose_points_are_all_in_line_is_refused_where_it_runs_back_along_itself() {
    // Its points span no plane, but in any plane through their line its second edge runs back along its first, to its
    // end two thirds of the way along the first; along x and along a diagonal.
    for along in [[1.0, 0.0, 0.0], [1.0, 2.0, -2.0]] {
        let report =
            refused(json!([{"op": "wire", "points": [[0, 0, 0], along.map(|a| 3.0 * a), along.map(|a| 2.0 * a)]}]));
        let code = (report.code, number(&report, "first"), number(&report, "second"));
        assert_eq!(code, (Code::WireCrossesItself, 0.0, 1.0), "{report:?}");
        let Some(occam_report::Value::Point(p)) = report.value("point").map(|v| v.value) else { panic!("{report:?}") };
        assert!(p.iter().zip(along).all(|(x, a)| (x - 2.0 * a).abs() < 1e-9), "{report:?}");
    }
    let (model, _) = built(json!([{"op": "wire", "points": [[0, 0, 0], [5, 0, 0], [10, 0, 0]]}]));
    assert_eq!(model.ids::<Edge>().len(), 2, "a wire that goes on along its line is made");
}

#[test]
fn one_line_closed_is_refused_at_the_wire_as_three_points_in_line_closed_are() {
    // Closed, a line is joined to its start by a second edge back along it, which it touches all along: at its middle,
    // as it is named. So are points that go there and back, and the wire that three points in line close.
    let line = json!({"op": "wire", "label": "w", "edges": [{"line": [[0, 0, 0], [10, 0, 0]]}], "closed": true});
    let report = refused(json!([line, {"op": "face", "outer": "w"}]));
    assert_eq!(report.input, Some(Input::at("steps[0]").in_step(0, Some("wire"))), "{report:?}");
    let named = (report.code, number(&report, "first"), number(&report, "second"));
    assert_eq!(named, (Code::WireCrossesItself, 0.0, 1.0));
    let Some(occam_report::Value::Point(p)) = report.value("point").map(|v| v.value) else { panic!("{report:?}") };
    assert!(p.iter().zip([5.0, 0.0, 0.0]).all(|(x, q)| (x - q).abs() < 1e-9), "{report:?}");
    let back = refused(json!([{"op": "wire", "points": [[0, 0, 0], [10, 0, 0], [0, 0, 0]]}]));
    assert_eq!((back.code, number(&back, "first"), number(&back, "second")), (Code::WireCrossesItself, 0.0, 1.0));
    let three = refused(json!([{"op": "wire", "points": [[0, 0, 0], [5, 0, 0], [10, 0, 0]], "closed": true}]));
    assert_eq!((three.code, number(&three, "first"), number(&three, "second")), (Code::WireCrossesItself, 0.0, 2.0));
}

#[test]
fn an_open_wire_is_judged_with_its_last_point_which_no_edge_begins_at() {
    // Each leaves the plane of every point before its last, so it lies in no plane and crosses nothing.
    let out_of_plane = [
        json!({"points": [[0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 0], [0, 0, 5]]}),
        json!({"points": [[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1], [0, 1, 0]]}),
        json!({"edges": [{"arc": [[1, 0, 0], [0, 1, 0], [-1, 0, 0]]}, {"line": [[-1, 0, 0], [1, 0, 1]]}]}),
    ];
    for mut wire in out_of_plane {
        wire["op"] = json!("wire");
        let (model, made) = built(json!([wire]));
        let body = model.resolve::<Body>(made[0].unwrap()).unwrap();
        assert_eq!(model.view(body).unwrap().kind(), BodyKind::Wire, "{wire}");
    }
    // Every point before its last is in line, its second edge running back along its first, and its last gives the
    // plane in which they do.
    let folded = refused(json!([{"op": "wire", "points": [[0, 0, 0], [3, 0, 0], [2, 0, 0], [2, 1, 0]]}]));
    assert_eq!((folded.code, number(&folded, "first"), number(&folded, "second")), (Code::WireCrossesItself, 0.0, 1.0));
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

/// The wire step of one arc through `points`.
fn arc(points: [[f64; 3]; 3]) -> Value {
    json!([{"op": "wire", "edges": [{"arc": points}]}])
}

/// The centre, radius and normal of the whole circle `remedy` writes.
fn circle_of(remedy: &Remedy) -> Option<([f64; 3], f64, [f64; 3])> {
    use occam_report::Value::{Point, Scalar};
    let value = |name: &str| remedy.value(name).map(|v| v.value);
    match (value("centre")?, value("radius")?, value("normal")?) {
        (Point(centre), Scalar(radius), Point(normal)) => Some((centre, radius, normal)),
        _ => None,
    }
}

/// The centre, radius and normal of the whole circle `report`'s remedy writes instead, if it offers one.
fn circle_instead(report: &Report) -> Option<([f64; 3], f64, [f64; 3])> {
    report.remedy(RemedyKind::UseCircle).and_then(circle_of)
}

#[test]
fn an_arc_whose_ends_meet_is_refused_and_writing_the_whole_circle_instead_follows_the_remedy() {
    let report = refused(arc([[0.0; 3], [1.0, 1.0, 0.0], [0.0; 3]]));
    assert_eq!((report.code, number(&report, "gap")), (Code::ArcEndsMeet, 0.0));
    assert_eq!(report.input, Some(Input::at("steps[0].edges[0]").in_step(0, Some("wire"))));
    assert_eq!(
        report.message(occam_report::DisplayUnits::default()),
        "steps[0].edges[0] gives an arc whose ends are 0 mm apart, no more than the band of 1e-7 mm: a whole circle is \
         written as a circle, by its centre, radius and normal."
    );
    let (centre, radius, normal) = circle_instead(&report).unwrap();
    assert_eq!((centre, normal), ([0.5, 0.5, 0.0], [0.0, 0.0, 1.0]));
    assert!((radius - 0.5_f64.sqrt()).abs() < 1e-15, "{radius}");
    followed_remedy!(report, Code::ArcEndsMeet, RemedyKind::UseCircle, |remedy| {
        let Some((centre, radius, normal)) = circle_of(remedy) else {
            return Err(Report::new(Code::InputMissing, occam_report::Fault::InvalidInput));
        };
        let circle = json!({"centre": centre, "radius": radius, "normal": normal});
        build(&request(json!([{"op": "wire", "edges": [{"circle": circle}]}])))
            .map(|_| ())
            .map_err(|reports| reports.into_iter().next().unwrap())
    });
}

#[test]
fn the_whole_circle_offered_for_an_arc_whose_ends_meet_passes_through_its_points_and_is_longer_than_the_band() {
    // Along z, x and y are as far from the diameter: x is taken.
    let upright = circle_instead(&refused(arc([[0.0; 3], [0.0, 0.0, 2.0], [0.0; 3]]))).unwrap();
    assert_eq!(upright, ([0.0, 0.0, 1.0], 1.0, [1.0, 0.0, 0.0]));
    let near = refused(arc([[1.0, 0.0, 0.0], [2.0, 2.0, 3.0], [1.0, 0.0, 5e-8]]));
    assert_eq!((near.code, number(&near, "gap")), (Code::ArcEndsMeet, 5e-8));
    let (centre, radius, normal) = circle_instead(&near).unwrap();
    let [cx, cy, cz] = centre;
    let to_start = Vec3::new(1.0 - cx, -cy, -cz);
    let normal = Vec3::new(normal[0], normal[1], normal[2]);
    assert!((to_start.norm() - radius).abs() < 1e-12 && to_start.dot(normal).abs() < 1e-12, "{near:?}");
    assert!((normal.norm() - 1.0).abs() < 1e-12 && (radius - 14.0_f64.sqrt() / 2.0).abs() < 1e-12, "{near:?}");
    // Three points that meet would make a circle of no length, so none is offered; nor one no longer than the band.
    let point = refused(arc([[0.0; 3]; 3]));
    assert_eq!((point.code, circle_instead(&point)), (Code::ArcEndsMeet, None));
    let coarse = |middle: f64| {
        let mut text = request(arc([[0.0; 3], [middle, 0.0, 0.0], [0.0; 3]]));
        text["tolerance"] = json!({"length": 0.5, "angle": 1e-6});
        build(&text).map(|_| ()).unwrap_err().into_iter().next().map(|report| circle_instead(&report).map(|c| c.1))
    };
    assert_eq!((coarse(1.0), coarse(1.5)), (Some(None), Some(Some(0.75))));
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
