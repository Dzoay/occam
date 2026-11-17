//! Source: original
//! The measure's tests: each primitive's volume, area and centre of mass against its closed form, within the stated
//! tolerance, at a frame turned off the world's axes; the tight box of the round ones, which is not the box of their
//! boxes; what each kind of target has, and the refusals of what it does not; and a step's reading.
#![expect(clippy::arithmetic_side_effects, reason = "small counts in tests")]

use core::f64::consts::PI;

use occam_math::Exec;
use occam_report::{Code, EntityKind, Fault, Input, NamedValue, Report};
use occam_topo::{Body, BodyKind, Edge, EdgeUse, Face, Loop, Snapshot, Vertex};
use occam_units::Vec3;
use serde_json::{Value, json};

use super::mass::SUMS;
use super::quadrature::{Budget, Integral};
use super::*;
use crate::construct::tests::run;
use crate::operation::Operation;
use crate::request::{Request, default_tolerance};

/// The request with these steps, in millimetres.
fn request(steps: &Value) -> String {
    json!({"schema": 1, "units": {"length": "mm", "angle": "rad"}, "steps": steps}).to_string()
}

/// Runs `steps` on a new model, as the runner does, until the last, a measure, which measures it.
fn measured(steps: &Value) -> Result<(Snapshot, Measured), Vec<Report>> {
    let request = Request::parse(&request(steps))?;
    let (last, rest) = request.steps.split_last().unwrap();
    let run = run(&request, rest)?;
    let found = measure(&run.model, last, &run.labels).map_err(|r| vec![r])?;
    Ok((run.model, found))
}

/// What `step`, labelled `it`, measures with a measure of it asking for `what`, or for everything if `what` is null.
fn of(step: &Value, what: &Value) -> Measured {
    let mut step = step.clone();
    step["label"] = json!("it");
    let measure = if what.is_null() {
        json!({"op": "measure", "target": "it"})
    } else {
        json!({"op": "measure", "target": "it", "what": what})
    };
    measured(&json!([step, measure])).unwrap_or_else(|r| panic!("{step}: {r:#?}")).1
}

/// The value named `name`.
fn value<'a>(m: &'a Measured, name: &str) -> &'a NamedValue {
    m.values.iter().find(|v| v.name == name).unwrap_or_else(|| panic!("no {name} in {m:?}"))
}

/// The number named `name`.
fn number(m: &Measured, name: &str) -> f64 {
    value(m, name).number().unwrap()
}

/// The point named `name`.
fn point(m: &Measured, name: &str) -> Vec3 {
    match value(m, name).value {
        occam_report::Value::Point([x, y, z]) => Vec3::new(x, y, z),
        occam_report::Value::Scalar(_) => panic!("{name} is a number"),
    }
}

/// A frame turned off the world's axes, and shifted: its origin, and its unit `x` and `z`.
const ORIGIN: [f64; 3] = [3.0, -2.0, 7.0];
const X: [f64; 3] = [2.0 / 3.0, 1.0 / 3.0, -2.0 / 3.0];
const Z: [f64; 3] = [1.0 / 3.0, 2.0 / 3.0, 2.0 / 3.0];

fn v([x, y, z]: [f64; 3]) -> Vec3 {
    Vec3::new(x, y, z)
}

/// The point at `local` in the turned frame.
fn placed(local: Vec3) -> Vec3 {
    let (x, z) = (v(X), v(Z));
    v(ORIGIN) + x * local.x + z.cross(x) * local.y + z * local.z
}

/// `step` at the turned frame.
fn turned(step: &Value) -> Value {
    let mut step = step.clone();
    step["frame"] = json!({"origin": ORIGIN, "z": Z, "x": X});
    step
}

/// A primitive's volume, area and centre of mass in its own frame.
struct Closed {
    volume: f64,
    area: f64,
    centre: Vec3,
}

/// A prism along `y`, `depth` deep, whose section in `x` and `z` is the polygon `corners`, anticlockwise.
fn prism(corners: &[(f64, f64)], depth: f64) -> Closed {
    let (mut twice, mut cx, mut cz, mut round) = (0.0, 0.0, 0.0, 0.0);
    for (k, &(x0, z0)) in corners.iter().enumerate() {
        let (x1, z1) = corners[(k + 1) % corners.len()];
        let cross = x0 * z1 - x1 * z0;
        twice += cross;
        cx += (x0 + x1) * cross;
        cz += (z0 + z1) * cross;
        round += ((x1 - x0) * (x1 - x0) + (z1 - z0) * (z1 - z0)).sqrt();
    }
    let section = 0.5 * twice;
    let centre = Vec3::new(cx / (3.0 * twice), 0.5 * depth, cz / (3.0 * twice));
    Closed { volume: section * depth, area: 2.0 * section + round * depth, centre }
}

/// A solid of revolution about `z` of radius `r1` at 0 and `r2` at `h`: a cylinder or a cone.
fn frustum(r1: f64, r2: f64, h: f64) -> Closed {
    let squares = r1 * r1 + r1 * r2 + r2 * r2;
    let slant = ((r1 - r2) * (r1 - r2) + h * h).sqrt();
    let z = h * (r1 * r1 + 2.0 * r1 * r2 + 3.0 * r2 * r2) / (4.0 * squares);
    let area = PI * (r1 * r1 + r2 * r2) + PI * (r1 + r2) * slant;
    Closed { volume: PI * h * squares / 3.0, area, centre: Vec3::new(0.0, 0.0, z) }
}

/// Every primitive, with its closed form.
fn every() -> Vec<(Value, Closed)> {
    let centre = Vec3::new(0.0, 0.0, 0.0);
    vec![
        (
            json!({"op": "box", "size": [10, 20, 30]}),
            prism(&[(0.0, 0.0), (10.0, 0.0), (10.0, 30.0), (0.0, 30.0)], 20.0),
        ),
        (
            json!({"op": "wedge", "size": [10, 5, 4], "top": {"start": 2, "width": 3}}),
            prism(&[(0.0, 0.0), (10.0, 0.0), (5.0, 4.0), (2.0, 4.0)], 5.0),
        ),
        (
            json!({"op": "wedge", "size": [10, 5, 4], "top": {"start": 12, "width": 0}}),
            prism(&[(0.0, 0.0), (10.0, 0.0), (12.0, 4.0)], 5.0),
        ),
        (json!({"op": "cylinder", "radius": 5, "height": 10}), frustum(5.0, 5.0, 10.0)),
        (json!({"op": "cone", "radius1": 5, "radius2": 2, "height": 10}), frustum(5.0, 2.0, 10.0)),
        (json!({"op": "cone", "radius1": 1, "radius2": 6, "height": 3}), frustum(1.0, 6.0, 3.0)),
        (json!({"op": "cone", "radius1": 0, "radius2": 3, "height": 4}), frustum(0.0, 3.0, 4.0)),
        (json!({"op": "cone", "radius1": 3, "radius2": 0, "height": 4}), frustum(3.0, 0.0, 4.0)),
        (json!({"op": "sphere", "radius": 5}), Closed { volume: 4.0 * PI * 125.0 / 3.0, area: 100.0 * PI, centre }),
        (
            json!({"op": "torus", "major": 10, "minor": 2}),
            Closed { volume: 2.0 * PI * PI * 10.0 * 4.0, area: 4.0 * PI * PI * 10.0 * 2.0, centre },
        ),
    ]
}

/// Whether `got` is `want` to [`RELATIVE`] of `scale`.
fn close(got: f64, want: f64, scale: f64) -> bool {
    (got - want).abs() <= RELATIVE * scale
}

#[test]
fn each_primitive_has_the_volume_area_and_centre_of_mass_of_its_closed_form_at_any_frame() {
    for (step, closed) in every() {
        for (step, centre) in [(step.clone(), closed.centre), (turned(&step), placed(closed.centre))] {
            let m = of(&step, &Value::Null);
            let (low, high) = (point(&m, "bounds-low"), point(&m, "bounds-high"));
            let size = (high - low).norm();
            let (volume, area, at) = (number(&m, "volume"), number(&m, "area"), point(&m, "centre-of-mass"));
            assert!(close(volume, closed.volume, closed.volume), "{step}: volume {volume}, not {}", closed.volume);
            assert!(close(area, closed.area, closed.area), "{step}: area {area}, not {}", closed.area);
            assert!((at - centre).norm() <= RELATIVE * size, "{step}: centre {at:?}, not {centre:?}");
            let names: Vec<&str> = m.values.iter().map(|v| v.name.as_str()).collect();
            assert_eq!(names, ["volume", "area", "centre-of-mass", "bounds-low", "bounds-high"], "{step}");
        }
    }
}

/// The tight box of a disc of `radius` about `centre` facing the unit `axis`, as low and high corners.
fn disc(centre: Vec3, axis: Vec3, radius: f64) -> (Vec3, Vec3) {
    let reach = |a: f64| radius * (1.0 - a * a).sqrt();
    let half = Vec3::new(reach(axis.x), reach(axis.y), reach(axis.z));
    (centre - half, centre + half)
}

/// The box holding both boxes.
fn union((a, b): (Vec3, Vec3), (c, d): (Vec3, Vec3)) -> (Vec3, Vec3) {
    let low = |p: f64, q: f64| if q < p { q } else { p };
    let high = |p: f64, q: f64| if q > p { q } else { p };
    (Vec3::new(low(a.x, c.x), low(a.y, c.y), low(a.z, c.z)), Vec3::new(high(b.x, d.x), high(b.y, d.y), high(b.z, d.z)))
}

/// Whether the box of `m` is `(low, high)` to twice the band.
fn boxed_as(m: &Measured, (low, high): (Vec3, Vec3)) -> bool {
    let near = 2.0 * default_tolerance().unwrap().band().millimetres();
    let (got_low, got_high) = (point(m, "bounds-low"), point(m, "bounds-high"));
    [got_low - low, got_high - high].iter().all(|d| d.x.abs() <= near && d.y.abs() <= near && d.z.abs() <= near)
}

#[test]
fn the_box_of_a_turned_cylinder_is_tight_not_the_box_of_its_own_box() {
    // A cylinder of radius 5 and height 10 up the turned frame's z: its box is its two end discs', each reaching
    // 5 sqrt(1 - z_i^2) along axis i. Its own box, 10 by 10 by 10 along the frame, turned, has a box that is wider.
    let m = of(&turned(&json!({"op": "cylinder", "radius": 5, "height": 10})), &json!(["bounds"]));
    let (o, z) = (v(ORIGIN), v(Z));
    let tight = union(disc(o, z, 5.0), disc(o + z * 10.0, z, 5.0));
    assert!(boxed_as(&m, tight), "{m:?}, not {tight:?}");
    let corners: Vec<Vec3> = [-5.0, 5.0]
        .iter()
        .flat_map(|&x| [-5.0, 5.0].map(move |y| (x, y)))
        .flat_map(|(x, y)| [0.0, 10.0].map(|h| placed(Vec3::new(x, y, h))))
        .collect();
    let own = corners.iter().fold(tight, |b, &p| union(b, (p, p)));
    assert!(own.1.x - own.0.x > tight.1.x - tight.0.x + 1.0, "{own:?} against {tight:?}");
    assert_eq!(m.values.len(), 2, "the bounds alone, as asked");
}

#[test]
fn a_sphere_and_a_torus_reach_furthest_inside_their_faces() {
    // A sphere's box is its centre's, plus and minus its radius; a torus's, its circle's, reaching 10 sqrt(1 - z_i^2)
    // along axis i, plus and minus the tube's radius. Neither is where any edge reaches.
    let o = v(ORIGIN);
    let sphere = of(&turned(&json!({"op": "sphere", "radius": 5})), &json!(["bounds"]));
    let r = Vec3::new(5.0, 5.0, 5.0);
    assert!(boxed_as(&sphere, (o - r, o + r)), "{sphere:?}");
    let torus = of(&turned(&json!({"op": "torus", "major": 10, "minor": 2})), &json!(["bounds"]));
    let (low, high) = disc(o, v(Z), 10.0);
    let tube = Vec3::new(2.0, 2.0, 2.0);
    assert!(boxed_as(&torus, (low - tube, high + tube)), "{torus:?}");
    // Along its own axis, a torus at the world's frame reaches the tube's radius, at circles that its edges reach too.
    let flat = of(&json!({"op": "torus", "major": 10, "minor": 2}), &json!(["bounds"]));
    assert!(boxed_as(&flat, (Vec3::new(-12.0, -12.0, -2.0), Vec3::new(12.0, 12.0, 2.0))), "{flat:?}");
}

#[test]
fn what_is_asked_for_comes_in_one_order_and_once_each() {
    let step = json!({"op": "box", "size": [1, 2, 3]});
    let m = of(&step, &json!(["bounds", "volume", "bounds"]));
    let names: Vec<&str> = m.values.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["volume", "bounds-low", "bounds-high"]);
    assert!(boxed_as(&m, (Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 2.0, 3.0))), "{m:?}");
    let area = of(&step, &json!(["area"]));
    assert_eq!(area.values.len(), 1);
    assert!(close(number(&area, "area"), 22.0, 22.0));
    let centre = of(&step, &json!(["centre-of-mass"]));
    assert!((point(&centre, "centre-of-mass") - Vec3::new(0.5, 1.0, 1.5)).norm() < 1e-12, "{centre:?}");
}

/// A sheet: a rectangle 40 by 20 with a round hole of radius 3, made by the face step, labelled `plate`, and `measure`.
fn plate(measure: &Value) -> Result<(Snapshot, Measured), Vec<Report>> {
    measured(&json!([
        {"op": "wire", "label": "outline", "points": [[0, 0, 0], [40, 0, 0], [40, 20, 0], [0, 20, 0]], "closed": true},
        {"op": "wire", "label": "hole", "edges": [{"circle": {"centre": [20, 10, 0], "radius": 3, "normal": [0, 0, 1]}}]},
        {"op": "face", "label": "plate", "outer": "outline", "holes": ["hole"]},
        measure,
    ]))
}

#[test]
fn a_sheet_has_an_area_and_a_box_and_no_volume() {
    let (s, m) = plate(&json!({"op": "measure", "target": "plate"})).unwrap();
    let names: Vec<&str> = m.values.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["area", "bounds-low", "bounds-high"]);
    let want = 800.0 - 9.0 * PI;
    assert!(close(number(&m, "area"), want, want), "{m:?}");
    assert!(boxed_as(&m, (Vec3::new(0.0, 0.0, 0.0), Vec3::new(40.0, 20.0, 0.0))), "{m:?}");
    assert_eq!(m.target.kind, EntityKind::Face, "the face step's label names its face");
    // Its body, a sheet, by id: the same area.
    let body = s.ids::<Body>().into_iter().find(|&b| s.view(b).unwrap().kind() == BodyKind::Sheet).unwrap().entity();
    let (_, of_body) = plate(&json!({"op": "measure", "target": body.text(), "what": ["area"]})).unwrap();
    assert!(close(number(&of_body, "area"), want, want) && of_body.values.len() == 1, "{of_body:?}");
    // A volume, asked of it, is refused where it is asked, with what it does have.
    let refused = plate(&json!({"op": "measure", "target": body.text(), "what": ["area", "volume"]})).unwrap_err();
    assert_eq!(refused.len(), 1);
    let report = &refused[0];
    assert_eq!((report.code, report.fault), (Code::MeasureNotDefined, Some(Fault::InvalidInput)));
    let at = Input::at("steps[3].what[1]").in_step(3, Some("measure")).given("volume").expected("area, bounds");
    assert_eq!(report.input, Some(at));
    assert_eq!(report.entities, [body]);
    assert_eq!(
        report.message(occam_report::DisplayUnits::default()),
        "steps[3].what[1] is volume, which what the step measures does not have: it has area, bounds."
    );
}

#[test]
fn an_edge_a_vertex_and_a_wire_have_a_box_alone() {
    // A circle's wire body: its box, from its edge. Once a face has taken the wire, its body is gone.
    let circle = json!({"circle": {"centre": [20, 10, 0], "radius": 3, "normal": [0, 0, 1]}});
    let wire = json!([{"op": "wire", "label": "hole", "edges": [circle]}, {"op": "measure", "target": "hole"}]);
    let (_, hole) = measured(&wire).unwrap();
    let circle = (Vec3::new(17.0, 7.0, 0.0), Vec3::new(23.0, 13.0, 0.0));
    assert!(boxed_as(&hole, circle) && hole.values.len() == 2, "{hole:?}");
    let (s, _) = plate(&json!({"op": "measure", "target": "plate"})).unwrap();
    let gone = plate(&json!({"op": "measure", "target": "hole"})).unwrap_err();
    let at = Input::at("steps[3].target").in_step(3, Some("measure"));
    assert_eq!((gone[0].code, gone[0].input.clone()), (Code::StaleId, Some(at)));
    let edge = s.ids::<Edge>().into_iter().find(|&e| {
        let (a, b) = s.view(e).unwrap().ends();
        a == b
    });
    let (_, of_edge) = plate(&json!({"op": "measure", "target": edge.unwrap().entity().text()})).unwrap();
    assert!(boxed_as(&of_edge, circle), "{of_edge:?}");
    let vertex = s.ids::<Vertex>()[1];
    let at = s.view(vertex).unwrap().point().to_vec3();
    let (_, of_vertex) = plate(&json!({"op": "measure", "target": vertex.entity().text()})).unwrap();
    assert_eq!((point(&of_vertex, "bounds-low"), point(&of_vertex, "bounds-high")), (at, at));
    let refused = plate(&json!({"op": "measure", "target": vertex.entity().text(), "what": ["area"]})).unwrap_err();
    assert_eq!(refused[0].input.as_ref().and_then(|i| i.expected.clone()), Some("bounds".to_string()));
}

#[test]
fn a_target_that_is_not_a_body_or_a_part_with_a_box_is_refused_as_the_wrong_kind() {
    let (s, _) = plate(&json!({"op": "measure", "target": "plate"})).unwrap();
    for entity in [s.ids::<Loop>()[0].entity(), s.ids::<EdgeUse>()[0].entity()] {
        let refused = plate(&json!({"op": "measure", "target": entity.text()})).unwrap_err();
        let report = &refused[0];
        assert_eq!((report.code, report.entities.clone()), (Code::InputWrongType, vec![entity]));
        let at = Input::at("steps[3].target").in_step(3, Some("measure"));
        let given = entity.kind.text();
        assert_eq!(report.input, Some(at.given(given).expected("body, shell, face, wire, edge or vertex")));
    }
    // A label no step has, and an id of nothing, are refused as any operand is.
    let unknown = plate(&json!({"op": "measure", "target": "bracket"})).unwrap_err();
    assert_eq!(unknown[0].code, Code::InputUnknownLabel);
    let stale = plate(&json!({"op": "measure", "target": "body:40/1"})).unwrap_err();
    assert_eq!(stale[0].code, Code::StaleId);
}

#[test]
fn a_face_with_no_curve_in_its_parameters_has_no_area_measured_but_a_plane_still_has_a_box() {
    // A box whose first use loses its curve in its face's parameters: the checker passes it, but its area cannot be
    // integrated round that loop.
    let steps = json!([{"op": "box", "label": "it", "size": [1, 2, 3]}, {"op": "measure", "target": "it"}]);
    let (s, m) = measured(&steps).unwrap();
    let mut edit = s.edit();
    let used = s.ids::<EdgeUse>()[0];
    let mut record = edit.record(used).unwrap();
    record.pcurve = None;
    edit.set(used, record).unwrap();
    let (bare, _) = edit.commit(&Exec::serial()).unwrap();
    let step = |what: Value| {
        let steps = json!([{"op": "measure", "target": m.target.text(), "what": what}]);
        Request::parse(&request(&steps)).unwrap().steps.remove(0)
    };
    let report = measure(&bare, &step(json!(["area"])), &Labels::default()).unwrap_err();
    let edge = bare.view(used).unwrap().edge();
    let face = bare.ids::<Face>().into_iter().find(|&f| bare.view(f).unwrap().edges().contains(&edge)).unwrap();
    assert_eq!((report.code, report.entities.clone()), (Code::NoParameterCurve, vec![edge.entity(), face.entity()]));
    let found = measure(&bare, &step(json!(["bounds"])), &Labels::default()).unwrap();
    assert!(boxed_as(&found, (Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 2.0, 3.0))), "{found:?}");
}

#[test]
fn a_step_built_by_hand_without_its_target_is_refused_as_missing_it() {
    let snapshot = Snapshot::empty(default_tolerance().unwrap());
    let parameters = Map::new();
    let step = Step { index: 1, label: None, operation: Operation::Measure, parameters, args: Args::None };
    let report = measure(&snapshot, &step, &Labels::default()).unwrap_err();
    let at = Input::at("steps[1]").in_step(1, Some("measure"));
    assert_eq!((report.code, report.input.clone()), (Code::InputMissing, Some(at)));
}

/// What reading the measure step with `fields` reports.
fn read(fields: &Value) -> Vec<Report> {
    let mut step = json!({"op": "measure"});
    step.as_object_mut().unwrap().extend(fields.as_object().unwrap().clone());
    Request::parse(&request(&json!([step]))).err().unwrap_or_default()
}

#[test]
fn a_step_is_read_whole_and_every_bad_input_reported_where_it_is() {
    let at = |path: &str| Input::at(path).in_step(0, Some("measure"));
    let codes = |reports: &[Report]| reports.iter().map(|r| (r.code, r.input.clone())).collect::<Vec<_>>();
    assert_eq!(read(&json!({"target": "it"})), []);
    assert_eq!(read(&json!({"target": "it", "what": ["volume", "bounds"]})), []);
    assert_eq!(codes(&read(&json!({}))), [(Code::InputMissing, Some(at("steps[0].target")))]);
    let wrong = read(&json!({"target": 4, "what": "volume", "colour": "red"}));
    assert_eq!(
        codes(&wrong),
        [
            (Code::InputUnknownField, Some(at("steps[0].colour").given("colour").expected("target, what"))),
            (Code::InputWrongType, Some(at("steps[0].target").given("number").expected("text"))),
            (Code::InputWrongType, Some(at("steps[0].what").given("text").expected("list"))),
        ]
    );
    let names = read(&json!({"target": "it", "what": ["volume", "mass"]}));
    let allowed = "volume, area, centre-of-mass, bounds";
    assert_eq!(codes(&names), [(Code::InputUnknownName, Some(at("steps[0].what[1]").given("mass").expected(allowed)))]);
    assert_eq!(
        codes(&read(&json!({"target": "it", "what": []}))),
        [(Code::InputMissing, Some(at("steps[0].what[0]")))]
    );
}

#[test]
fn the_measurements_have_their_names_and_order() {
    assert_eq!(Measurement::ALL.map(Measurement::name), ["volume", "area", "centre-of-mass", "bounds"]);
    let mut sorted = Measurement::ALL;
    sorted.sort_unstable();
    assert_eq!(sorted, Measurement::ALL);
}

#[test]
fn a_measure_that_runs_out_of_budget_is_refused_with_how_far_it_got() {
    let steps = json!([{"op": "sphere", "label": "it", "radius": 5}, {"op": "measure", "target": "it"}]);
    let (s, m) = measured(&steps).unwrap();
    let faces = Target::of(&s, m.target).unwrap().unwrap().faces(&s);
    let patches: Vec<_> = faces.into_iter().map(|f| mass::patch(&s, f).unwrap()).collect();
    let origin = Vec3::new(0.0, 0.0, 0.0);
    let report = mass_of(&patches, (origin, 17.0), true, &Budget::new(3)).unwrap_err();
    assert_eq!((report.code, report.fault), (Code::NotConverged, Some(Fault::NumericallySensitive)));
    assert_eq!(report.value("asked").and_then(NamedValue::number), Some(RELATIVE));
    assert!(report.value("reached").and_then(NamedValue::number).is_some_and(|r| r >= 0.0), "{report:?}");
    // Not a solid: only the area is held to a tolerance.
    let sheet = mass_of(&patches, (origin, 17.0), false, &Budget::new(BUDGET)).unwrap();
    assert!(close(sheet[1], 100.0 * PI, 100.0 * PI), "{sheet:?}");
}

#[test]
fn a_shortfall_is_the_worst_part_of_a_tolerance_reached_among_those_asked() {
    let report = not_converged([2.0, 3.0, 1.0, 0.0, 0.0, 5.0], [4.0, 1.0, 2.0, 1.0, 1.0, f64::INFINITY]);
    assert_eq!(report.value("reached").and_then(NamedValue::number), Some(3.0 * RELATIVE));
    let report = not_converged([f64::NAN, 0.0, 0.0, 0.0, 0.0, 0.0], [1.0; SUMS]);
    assert_eq!(report.value("reached").and_then(NamedValue::number), Some(f64::INFINITY));
}

#[test]
fn the_first_pass_is_held_to_a_millionth_of_the_bodys_size_and_the_second_to_a_part_in_a_billion_of_what_it_found() {
    let free = f64::INFINITY;
    let fourth = FIRST * 27.0 * 3.0;
    assert_eq!(first(3.0, true), [FIRST * 27.0, FIRST * 9.0, fourth, fourth, fourth, free]);
    assert_eq!(first(3.0, false), [free, FIRST * 9.0, free, free, free, free], "only the area, of anything else");
    let moment = RELATIVE * 2.0 * 100.0;
    let rough = [-100.0, 50.0, 7.0, 7.0, 7.0, 3.0];
    assert_eq!(second(rough, 2.0, true), [RELATIVE * 100.0, RELATIVE * 50.0, moment, moment, moment, free]);
    assert_eq!(second(rough, 2.0, false), [free, RELATIVE * 50.0, free, free, free, free]);
    // A volume or an area found as next to nothing is held to the first pass's floor.
    let floor = second([1e-12, 1e-15, 0.0, 0.0, 0.0, 0.0], 2.0, true);
    let low = RELATIVE * 2.0 * (FIRST * 8.0);
    assert_eq!(floor, [RELATIVE * (FIRST * 8.0), RELATIVE * (FIRST * 4.0), low, low, low, free]);
}

#[test]
fn a_shell_has_an_area_and_a_box_and_a_wire_a_box() {
    let steps = json!([{"op": "box", "label": "it", "size": [1, 2, 3]}, {"op": "measure", "target": "it"}]);
    let (s, _) = measured(&steps).unwrap();
    let shell = s.ids::<occam_topo::Shell>()[0].entity();
    let step = |target: &str| {
        let steps = json!([{"op": "measure", "target": target}]);
        Request::parse(&request(&steps)).unwrap().steps.remove(0)
    };
    let of_shell = measure(&s, &step(&shell.text()), &Labels::default()).unwrap();
    let names: Vec<&str> = of_shell.values.iter().map(|v| v.name.as_str()).collect();
    assert_eq!(names, ["area", "bounds-low", "bounds-high"]);
    assert!(close(number(&of_shell, "area"), 22.0, 22.0), "{of_shell:?}");
    let wire = json!([{"op": "wire", "label": "it", "points": [[0, 0, 0], [3, 0, 0], [3, 4, 0]]}, {"op": "measure", "target": "it"}]);
    let (s, _) = measured(&wire).unwrap();
    let id = s.ids::<occam_topo::Wire>()[0].entity();
    let of_wire = measure(&s, &step(&id.text()), &Labels::default()).unwrap();
    assert!(boxed_as(&of_wire, (Vec3::new(0.0, 0.0, 0.0), Vec3::new(3.0, 4.0, 0.0))) && of_wire.values.len() == 2);
    assert_eq!(of_wire.target, id);
}

#[test]
fn an_integral_plus_nothing_is_itself() {
    let i = Integral { value: [1.0; SUMS], error: [0.5; SUMS] };
    assert_eq!(i.plus(Integral::ZERO, 1.0), i);
}

/// The plate, 40 by 20 with a hole of radius 3 at its middle, labelled `plate`, then `steps`, the last labelled `it`
/// and measured.
fn swept(steps: &[Value]) -> Measured {
    let mut all = vec![
        json!({"op": "wire", "label": "outline", "points": [[0, 0, 0], [40, 0, 0], [40, 20, 0], [0, 20, 0]], "closed": true}),
        json!({"op": "wire", "label": "hole", "edges": [{"circle": {"centre": [20, 10, 0], "radius": 3, "normal": [0, 0, 1]}}]}),
        json!({"op": "face", "label": "plate", "outer": "outline", "holes": ["hole"]}),
    ];
    all.extend_from_slice(steps);
    if let Some(last) = all.last_mut() {
        last["label"] = json!("it");
    }
    all.push(json!({"op": "measure", "target": "it"}));
    measured(&Value::Array(all)).unwrap_or_else(|r| panic!("{steps:?}: {r:#?}")).1
}

/// Whether `m` has `closed`'s volume, area and centre of mass, to the stated tolerance.
fn as_closed(m: &Measured, closed: &Closed) -> bool {
    let size = (point(m, "bounds-high") - point(m, "bounds-low")).norm();
    close(number(m, "volume"), closed.volume, closed.volume)
        && close(number(m, "area"), closed.area, closed.area)
        && (point(m, "centre-of-mass") - closed.centre).norm() <= RELATIVE * size
}

#[test]
fn the_extruded_and_revolved_solids_have_the_volume_area_and_centre_of_mass_of_their_closed_forms() {
    let section = 800.0 - 9.0 * PI;
    // Straight up 5: the section times the height, its outline's and its hole's sides.
    let straight = swept(&[json!({"op": "extrude", "face": "plate", "vector": [0, 0, 5]})]);
    let area = 2.0 * section + 5.0 * 120.0 + 5.0 * 6.0 * PI;
    let want = Closed { volume: 5.0 * section, area, centre: Vec3::new(20.0, 10.0, 2.5) };
    assert!(as_closed(&straight, &want), "{straight:?}");
    // Slanting along (0, 3, 5): the same volume, its centre half the vector on, and sides that are parallelograms; the
    // hole's, an oblique cylinder's, is the integral round the circle of 3 sqrt(25 + 9 sin^2 t), which the trapezoid
    // rule finds to the last bit, as it is periodic.
    let slanted = swept(&[json!({"op": "extrude", "face": "plate", "vector": [0, 3, 5]})]);
    let steps = 4096;
    let hole: f64 = (0..steps)
        .map(|k| {
            let (sin, _) = occam_math::sin_cos(2.0 * PI * f64::from(k) / f64::from(steps));
            3.0 * (25.0 + 9.0 * sin * sin).sqrt()
        })
        .sum::<f64>()
        * (2.0 * PI / f64::from(steps));
    let area = 2.0 * section + 2.0 * 40.0 * 34f64.sqrt() + 2.0 * 20.0 * 5.0 + hole;
    let want = Closed { volume: 5.0 * section, area, centre: Vec3::new(20.0, 11.5, 2.5) };
    assert!(as_closed(&slanted, &want), "{slanted:?}");
    assert!(boxed_as(&slanted, (Vec3::new(0.0, 0.0, 0.0), Vec3::new(40.0, 23.0, 5.0))), "{slanted:?}");
}

/// A face whose outline is `edges`, labelled `profile`, turned through `angle` radians about the z axis, and measured.
fn turned_about_z(edges: &Value, angle: f64) -> Measured {
    let steps = json!([
        {"op": "wire", "label": "outline", "edges": edges},
        {"op": "face", "label": "profile", "outer": "outline"},
        {"op": "revolve", "label": "it", "profile": "profile", "origin": [0, 0, 0], "axis": [0, 0, 1], "angle": angle},
        {"op": "measure", "target": "it"},
    ]);
    measured(&steps).unwrap_or_else(|r| panic!("{edges}: {r:#?}")).1
}

#[test]
fn the_revolved_solids_have_the_volume_area_and_centre_of_mass_of_their_closed_forms() {
    // A rectangle from 50 to 60 out and 5 up, turned a whole turn: a ring; and a quarter turn, from x towards y,
    // whose centre is out by the integral of r^2 over that of r, times the sine of a quarter turn over the quarter turn.
    let line = |a: [f64; 3], b: [f64; 3]| json!({"line": [a, b]});
    let rectangle = json!([
        line([50.0, 0.0, 0.0], [60.0, 0.0, 0.0]),
        line([60.0, 0.0, 0.0], [60.0, 0.0, 5.0]),
        line([60.0, 0.0, 5.0], [50.0, 0.0, 5.0]),
        line([50.0, 0.0, 5.0], [50.0, 0.0, 0.0]),
    ]);
    let ring = turned_about_z(&rectangle, 2.0 * PI);
    let want = Closed { volume: 5500.0 * PI, area: 3300.0 * PI, centre: Vec3::new(0.0, 0.0, 2.5) };
    assert!(as_closed(&ring, &want), "{ring:?}");
    assert!(boxed_as(&ring, (Vec3::new(-60.0, -60.0, 0.0), Vec3::new(60.0, 60.0, 5.0))), "{ring:?}");
    let quarter = turned_about_z(&rectangle, 0.5 * PI);
    let out = (60.0 * 60.0 * 60.0 - 50.0 * 50.0 * 50.0) / 3.0 / (550.0 * 0.5 * PI);
    let want = Closed { volume: 1375.0 * PI, area: 825.0 * PI + 100.0, centre: Vec3::new(out, out, 2.5) };
    assert!(as_closed(&quarter, &want), "{quarter:?}");
    // A half disc of radius 5 on the axis, turned whole: a sphere, its poles where its faces meet on the axis.
    let half = json!([{"arc": [[0, 0, -5], [5, 0, 0], [0, 0, 5]]}, line([0.0, 0.0, 5.0], [0.0, 0.0, -5.0])]);
    let sphere = turned_about_z(&half, 2.0 * PI);
    let want = Closed { volume: 500.0 * PI / 3.0, area: 100.0 * PI, centre: Vec3::new(0.0, 0.0, 0.0) };
    assert!(as_closed(&sphere, &want), "{sphere:?}");
    assert!(boxed_as(&sphere, (Vec3::new(-5.0, -5.0, -5.0), Vec3::new(5.0, 5.0, 5.0))), "{sphere:?}");
    // A disc of radius 2 about (10, 0, 0) facing y, turned whole: a torus.
    let disc = json!([{"circle": {"centre": [10, 0, 0], "radius": 2, "normal": [0, 1, 0]}}]);
    let torus = turned_about_z(&disc, 2.0 * PI);
    let want = Closed { volume: 80.0 * PI * PI, area: 80.0 * PI * PI, centre: Vec3::new(0.0, 0.0, 0.0) };
    assert!(as_closed(&torus, &want), "{torus:?}");
    assert!(boxed_as(&torus, (Vec3::new(-12.0, -12.0, -2.0), Vec3::new(12.0, 12.0, 2.0))), "{torus:?}");
}

#[test]
fn each_half_of_a_sphere_has_its_own_box_not_the_whole_spheres() {
    // A sphere of radius 5 is two faces split along y. Each reaches all of ±5 in x and z, but only its own side in y:
    // the turning point at the other side's pole is outside it, so its box stops at the seam.
    let (model, _) =
        measured(&json!([{"op": "sphere", "label": "ball", "radius": 5}, {"op": "measure", "target": "ball"}]))
            .unwrap();
    let mut sides = Vec::new();
    for face in model.ids::<Face>() {
        let text = json!({"schema": 1, "units": {"length": "mm", "angle": "rad"},
                          "steps": [{"op": "measure", "target": face.entity().text(), "what": ["bounds"]}]});
        let step = &Request::parse(&text.to_string()).unwrap().steps[0];
        let m = measure(&model, step, &Labels::default()).unwrap();
        let (low, high) = (point(&m, "bounds-low"), point(&m, "bounds-high"));
        let at_seam = if low.y.abs() < 1e-9 { 0.0 } else { -5.0 };
        assert!(
            boxed_as(&m, (Vec3::new(-5.0, at_seam, -5.0), Vec3::new(5.0, at_seam + 5.0, 5.0))),
            "{low:?}..{high:?}"
        );
        sides.push(at_seam);
    }
    sides.sort_by(f64::total_cmp);
    assert_eq!(sides, [-5.0, 0.0], "one face each side of the seam");
}
