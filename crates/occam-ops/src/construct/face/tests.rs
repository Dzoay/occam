//! Source: original
//! The face's checks at their edges: the turn of a thin outline is measured finely enough to have its sign, a loop of no
//! area is taken as it is given, an open wire's gap runs from its start to its end, and a wire is taken only as the one
//! wire of its body.

use core::f64::consts::PI;

use occam_math::{Exec, sin_cos};
use occam_report::{Code, Input, Report};
use occam_topo::{Body, Face, Id, Snapshot, Wire, WireRecord};
use serde_json::{Value, json};

use crate::construct::tests::{build, built, number, refused, request};
use crate::{Labels, Request, execute};

/// The point `report` names.
fn point(report: &Report) -> [f64; 3] {
    match report.value("point").map(|v| v.value) {
        Some(occam_report::Value::Point(p)) => p,
        _ => [f64::NAN; 3],
    }
}

/// Whether `a` and `b` are the same point, within rounding.
fn near(a: [f64; 3], b: [f64; 3]) -> bool {
    a.iter().zip(b).all(|(x, y)| (x - y).abs() < 1e-9)
}

#[test]
fn a_thin_outline_turns_as_its_arc_does_cut_finely() {
    // A half circle of radius 1, back along sixteen edges a little inside it: cut into eight, as coarsely as an arc is
    // ever cut, the half circle bounds less than the edges do, and the outline would seem to turn the other way.
    let inside = |k: u32| {
        let (sin, cos) = sin_cos(PI - PI * f64::from(k) / 16.0);
        json!([0.995 * cos, 0.995 * sin, 0.0])
    };
    let mut corners: Vec<Value> = vec![json!([-1, 0, 0])];
    corners.extend((1..16).map(inside));
    corners.push(json!([1, 0, 0]));
    let mut edges = vec![json!({"arc": [[1, 0, 0], [0, 1, 0], [-1, 0, 0]]})];
    edges.extend(corners.windows(2).map(|pair| json!({"line": pair})));
    let (model, made) = built(json!([{"op": "face", "outer": {"edges": edges}}]));
    let face = model.resolve::<Face>(made[0].unwrap()).unwrap();
    let surface = model.view(face).unwrap().surface().unwrap();
    let at = |u: f64, v: f64| surface.point(occam_units::UvPoint::new(u, v)).to_vec3();
    let normal = (at(1.0, 0.0) - at(0.0, 0.0)).cross(at(0.0, 1.0) - at(0.0, 0.0));
    assert!(normal.z > 0.99, "the face faces +z, about which its outline runs anticlockwise: {normal:?}");
}

/// Two squares 2 across that meet at a corner, (12, 12), as one loop from `(14, 12)` that crosses itself there: round
/// the upper one anticlockwise, then the lower one clockwise, which bounds no area in all.
fn eight() -> Value {
    let corners = [[14, 12], [14, 13], [14, 14], [12, 14], [12, 12], [12, 10], [10, 10], [10, 12], [12, 12]];
    json!(corners.map(|[x, y]| [x, y, 0]))
}

#[test]
fn an_outer_wire_of_no_area_is_taken_as_its_plane_faces_and_its_holes_as_given() {
    // A hole anticlockwise in z = 0, against the outer wire's plane as its points give it, so not turned: its first edge
    // is the first to touch the outer wire, at (13.5, 14, 0), where it meets the upper square's top.
    let hole = json!({"points": [[13, 13, 0], [13.5, 14, 0], [12.5, 14, 0]], "closed": true});
    let report = refused(json!([{"op": "face", "outer": {"points": eight(), "closed": true}, "holes": [hole]}]));
    assert_eq!((report.code, number(&report, "other")), (Code::WiresCross, 0.0));
    assert!(near(point(&report), [13.5, 14.0, 0.0]), "{report:?}");
}

#[test]
fn a_hole_of_no_area_is_taken_as_given() {
    // Not turned, the hole's edges are named in the order given: the fourth and the eighth meet at (12, 12).
    let outer = json!({"points": [[0, 0, 0], [40, 0, 0], [40, 40, 0], [0, 40, 0]], "closed": true});
    let report = refused(json!([{"op": "face", "outer": outer, "holes": [{"points": eight(), "closed": true}]}]));
    assert_eq!(report.code, Code::WireCrossesItself);
    assert_eq!(report.input, Some(Input::at("steps[0].holes[0]").in_step(0, Some("face"))));
    assert_eq!((number(&report, "first"), number(&report, "second")), (3.0, 7.0));
    assert!(near(point(&report), [12.0, 12.0, 0.0]), "{report:?}");
}

#[test]
fn an_open_wire_s_gap_runs_from_its_start_to_its_end() {
    let report = refused(json!([{"op": "face", "outer": {"points": [[10, 0, 0], [20, 0, 0], [20, 10, 0]]}}]));
    assert_eq!(report.code, Code::WireOpen);
    assert!((number(&report, "gap") - 200.0_f64.sqrt()).abs() < 1e-12, "{report:?}");
}

/// Two closed wires, the second made the second wire of the first's body, and the two wires.
fn two_wired() -> (Snapshot, [Id<Wire>; 2]) {
    let square = |lo: i32, hi: i32| json!({"op": "wire", "points": [[lo, lo, 0], [hi, lo, 0], [hi, hi, 0], [lo, hi, 0]], "closed": true});
    let (model, made) = build(&request(json!([square(0, 10), square(20, 30)]))).unwrap();
    let bodies: Vec<Id<Body>> = made.iter().map(|m| model.resolve::<Body>(m.unwrap()).unwrap()).collect();
    let mut edit = model.edit();
    let wires: Vec<Id<Wire>> = bodies.iter().map(|&b| edit.record(b).unwrap().first_wire.unwrap()).collect();
    let (first, second) = (edit.record(wires[0]).unwrap(), edit.record(wires[1]).unwrap());
    edit.set(wires[0], WireRecord { next: Some(wires[1]), ..first }).unwrap();
    edit.set(wires[1], WireRecord { body: bodies[0], ..second }).unwrap();
    edit.remove(bodies[1]).unwrap();
    (edit.commit(&Exec::serial()).unwrap().0, [wires[0], wires[1]])
}

#[test]
fn neither_wire_of_a_body_with_two_bounds_a_face() {
    let (model, wires) = two_wired();
    for wire in wires {
        let text = request(json!([{"op": "face", "outer": wire.entity().text()}])).to_string();
        let request = Request::parse(&text).unwrap();
        let report = execute(&mut model.edit(), &request.steps[0], &Labels::default()).unwrap_err();
        let given = report.input.as_ref().and_then(|input| input.given.as_deref());
        assert_eq!((report.code, given), (Code::NotAWire, Some("a wire of a body with others")), "{report:?}");
    }
}
