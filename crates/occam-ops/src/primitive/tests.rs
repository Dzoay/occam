//! Source: original
//! The primitives' tests: each is a closed solid that passes the checker, with the counts and genus its layout gives
//! and every face's normal pointing out; at any frame it is the same solid moved there; and each bad parameter is
//! refused where it is, before any geometry.
#![expect(clippy::arithmetic_side_effects, reason = "small counts in tests")]

use occam_math::Exec;
use occam_report::{Code, Input, NamedValue, Report};
use occam_topo::{Body, Edge, Face, Loop, Shell, Snapshot, Vertex};
use occam_units::Vec3;
use serde_json::{Value, json};

use super::*;
use crate::operand::Labels;
use crate::request::{Request, default_tolerance};

/// The request with these steps, in millimetres.
fn request(steps: &Value) -> String {
    json!({"schema": 1, "units": {"length": "mm", "angle": "rad"}, "steps": steps}).to_string()
}

/// Reads `steps` and runs each on a new model, committing each: the model, or what refused it.
fn made(steps: &Value) -> Result<Snapshot, Vec<Report>> {
    let request = Request::parse(&request(steps))?;
    let mut snapshot = Snapshot::empty(default_tolerance().map_err(|r| vec![r])?);
    for step in &request.steps {
        let mut edit = snapshot.edit();
        crate::execute(&mut edit, step, &Labels::default()).map_err(|r| vec![r])?;
        snapshot = edit.commit(&Exec::serial())?.0;
    }
    Ok(snapshot)
}

/// A model's faces, edges, vertices and loops, and its genus.
type Counts = (usize, usize, usize, usize, i64);

/// The counts of `snapshot`.
fn counts(snapshot: &Snapshot) -> Counts {
    let faces = snapshot.ids::<Face>().len();
    let (edges, vertices, loops) =
        (snapshot.ids::<Edge>().len(), snapshot.ids::<Vertex>().len(), snapshot.ids::<Loop>().len());
    let whole = |n: usize| i64::try_from(n).unwrap();
    let euler = whole(vertices) + 2 * whole(faces) - whole(edges) - whole(loops);
    (faces, edges, vertices, loops, whole(snapshot.ids::<Shell>().len()) - euler / 2)
}

/// Every primitive, with its counts: faces, edges, vertices, loops and genus.
fn every() -> Vec<(Value, Counts)> {
    vec![
        (json!({"op": "box", "size": [10, 20, 30]}), (6, 12, 8, 6, 0)),
        (json!({"op": "wedge", "size": [10, 5, 4], "top": {"start": 2, "width": 3}}), (6, 12, 8, 6, 0)),
        (json!({"op": "wedge", "size": [10, 5, 4], "top": {"start": 12, "width": 0}}), (5, 9, 6, 5, 0)),
        (json!({"op": "cylinder", "radius": 5, "height": 10}), (4, 6, 4, 4, 0)),
        (json!({"op": "cone", "radius1": 5, "radius2": 2, "height": 10}), (4, 6, 4, 4, 0)),
        (json!({"op": "cone", "radius1": 1, "radius2": 6, "height": 3}), (4, 6, 4, 4, 0)),
        (json!({"op": "cone", "radius1": 0, "radius2": 3, "height": 4}), (3, 4, 3, 3, 0)),
        (json!({"op": "cone", "radius1": 3, "radius2": 0, "height": 4}), (3, 4, 3, 3, 0)),
        (json!({"op": "sphere", "radius": 5}), (2, 2, 2, 2, 0)),
        (json!({"op": "torus", "major": 10, "minor": 2}), (4, 8, 4, 4, 1)),
    ]
}

#[test]
fn each_primitive_is_a_closed_solid_with_the_counts_and_genus_of_its_layout() {
    for (step, expected) in every() {
        let s = made(&json!([step])).unwrap_or_else(|reports| panic!("{step}: {reports:#?}"));
        assert_eq!(counts(&s), expected, "{step}");
        assert_eq!((s.ids::<Body>().len(), s.ids::<Shell>().len()), (1, 1), "{step}");
        assert_eq!(s.check_all(&Exec::serial()), [], "{step}");
    }
}

/// Each face's point at its outer loop's witness, and the surface's normal there, turned over if the face is.
fn witnesses(s: &Snapshot) -> Vec<(Vec3, Vec3)> {
    s.ids::<Face>()
        .into_iter()
        .map(|f| {
            let face = s.view(f).unwrap();
            let witness = s.view(face.outer()).unwrap().witness();
            let surface = face.surface().unwrap();
            let jet = surface.first(witness);
            let normal = jet.du.rate().cross(jet.dv.rate());
            (surface.point(witness).to_vec3(), if face.reversed() { normal * -1.0 } else { normal })
        })
        .collect()
}

/// The middle of a model's vertices: inside each of the convex primitives.
fn centroid(s: &Snapshot) -> Vec3 {
    let points: Vec<Vec3> = s.ids::<Vertex>().into_iter().map(|v| s.view(v).unwrap().point().to_vec3()).collect();
    let n = f64::from(u32::try_from(points.len()).unwrap());
    points.into_iter().fold(Vec3::new(0.0, 0.0, 0.0), |a, p| a + p * (1.0 / n))
}

/// The centre of the tube nearest `p` of a torus of major radius `major` about the world's `z` axis.
fn tube_centre(p: Vec3, major: f64) -> Vec3 {
    let flat = Vec3::new(p.x, p.y, 0.0);
    flat * (major / flat.norm())
}

#[test]
fn every_face_of_every_primitive_faces_out() {
    for (step, (faces, ..)) in every() {
        let s = made(&json!([step])).unwrap();
        let torus = step["op"] == "torus";
        let inside = centroid(&s);
        let found = witnesses(&s);
        assert_eq!(found.len(), faces, "{step}");
        for (p, normal) in found {
            let from = if torus { p - tube_centre(p, 10.0) } else { p - inside };
            assert!(from.dot(normal) > 0.0, "{step}: at {p:?} the normal {normal:?} points in");
        }
    }
}

/// The same step at the frame `frame`.
fn at_frame(step: &Value, frame: &Value) -> Value {
    let mut step = step.clone();
    step["frame"] = frame.clone();
    step
}

/// Every vertex's point, and every face's point at its witness, of a model.
fn places(s: &Snapshot) -> Vec<Vec3> {
    let vertices = s.ids::<Vertex>().into_iter().map(|v| s.view(v).unwrap().point().to_vec3());
    vertices.chain(witnesses(s).into_iter().map(|(p, _)| p)).collect()
}

#[test]
fn a_primitive_at_a_frame_is_the_same_solid_moved_there() {
    let (origin, z, x) = (Vec3::new(1.0, -2.0, 3.0), Vec3::new(1.0, 1.0, 1.0), Vec3::new(1.0, -1.0, 0.0));
    let frame = json!({"origin": [origin.x, origin.y, origin.z], "z": [z.x, z.y, z.z], "x": [x.x, x.y, x.z]});
    let (z, x) = (z * (1.0 / z.norm()), x * (1.0 / x.norm()));
    let y = z.cross(x);
    let moved = |p: Vec3| origin + x * p.x + y * p.y + z * p.z;
    for (step, expected) in every() {
        let standard = made(&json!([step])).unwrap();
        let placed = made(&json!([at_frame(&step, &frame)])).unwrap_or_else(|r| panic!("{step}: {r:#?}"));
        assert_eq!(counts(&placed), expected, "{step}");
        assert_eq!(placed.check_all(&Exec::serial()), [], "{step}");
        for (p, q) in places(&standard).into_iter().zip(places(&placed)) {
            assert!((moved(p) - q).norm() < 1e-12, "{step}: {p:?} goes to {q:?}, not {:?}", moved(p));
        }
    }
}

/// The reports refusing `steps`, as each one's code, place and values.
fn refused(steps: &Value) -> Vec<(Code, Option<Input>, Vec<NamedValue>)> {
    let reports = made(steps).map(|_| ()).unwrap_err();
    reports.into_iter().map(|r| (r.code, r.input.clone(), r.values.clone())).collect()
}

/// The place `path` in step 0, which asks for `op`.
fn place(path: &str, op: &str) -> Input {
    Input::at(path).in_step(0, Some(op))
}

/// A refusal with code `code` at `path` of step 0, asking for `op`, with one length named `name`.
fn one(code: Code, (path, op): (&str, &str), (name, value): (&str, f64)) -> (Code, Option<Input>, Vec<NamedValue>) {
    (code, Some(place(path, op)), vec![NamedValue::length(name, value)])
}

#[test]
fn a_size_that_is_not_positive_is_refused_with_its_parameter_each_one() {
    let found = refused(&json!([{"op": "box", "size": [10, 0, -2]}]));
    let size = |k: usize, v: f64| one(Code::SizeNotPositive, (&format!("steps[0].size[{k}]"), "box"), ("size", v));
    assert_eq!(found, [size(1, 0.0), size(2, -2.0)]);
    let found = refused(&json!([{"op": "cylinder", "radius": -1, "height": 0}]));
    let named = |name: &str, v: f64| one(Code::SizeNotPositive, (&format!("steps[0].{name}"), "cylinder"), ("size", v));
    assert_eq!(found, [named("radius", -1.0), named("height", 0.0)]);
    let found = refused(&json!([{"op": "sphere", "radius": 0}]));
    assert_eq!(found, [one(Code::SizeNotPositive, ("steps[0].radius", "sphere"), ("size", 0.0))]);
}

#[test]
fn a_cone_torus_or_wedge_that_breaks_its_rule_is_refused_naming_the_parameter() {
    let cone = |r1: f64, r2: f64| refused(&json!([{"op": "cone", "radius1": r1, "radius2": r2, "height": 1}]));
    let equal = |v: f64| one(Code::InputConeRadiiEqual, ("steps[0].radius2", "cone"), ("value", v));
    assert_eq!(cone(0.0, 0.0), [equal(0.0)], "both zero");
    assert_eq!(cone(2.0, 2.0), [equal(2.0)]);
    assert_eq!(cone(-1.0, 2.0), [one(Code::InputNegative, ("steps[0].radius1", "cone"), ("value", -1.0))]);
    let torus = refused(&json!([{"op": "torus", "major": 3, "minor": 3}]));
    let range = vec![NamedValue::length("value", 3.0), NamedValue::length("low", 0.0), NamedValue::length("high", 3.0)];
    assert_eq!(torus, [(Code::OutOfRange, Some(place("steps[0].minor", "torus")), range)]);
    let wedge = refused(&json!([{"op": "wedge", "size": [1, 1, 1], "top": {"start": 0, "width": -0.5}}]));
    assert_eq!(wedge, [one(Code::InputNegative, ("steps[0].top.width", "wedge"), ("value", -0.5))]);
}

#[test]
fn a_frame_with_no_direction_or_an_x_along_its_z_is_refused_where_it_is() {
    let frame = |f: Value| refused(&json!([{"op": "sphere", "radius": 1, "frame": f}]));
    assert_eq!(
        frame(json!({"z": [0, 0, 0]})),
        [(Code::InputNoDirection, Some(place("steps[0].frame.z", "sphere")), vec![])]
    );
    let along = frame(json!({"z": [0, 0, 2], "x": [0, 0, -5]}));
    assert_eq!(along, [(Code::InputAxesParallel, Some(place("steps[0].frame.x", "sphere")), vec![])]);
    let found = frame(json!({"origin": [1, 2], "y": [0, 1, 0]}));
    let codes: Vec<(Code, Option<Input>)> = found.into_iter().map(|(c, i, _)| (c, i)).collect();
    let unknown = Input::at("steps[0].frame.y").in_step(0, Some("sphere")).given("y");
    assert_eq!(
        codes,
        [(Code::InputUnknownField, Some(unknown)), (Code::WrongCount, Some(place("steps[0].frame.origin", "sphere")))]
    );
}

#[test]
fn a_frame_without_an_x_takes_one_across_its_z() {
    let along_x = made(&json!([{"op": "box", "size": [1, 2, 3], "frame": {"z": [1, 0, 0]}}])).unwrap();
    assert_eq!(along_x.check_all(&Exec::serial()), []);
    let tilted = made(&json!([{"op": "box", "size": [1, 2, 3], "frame": {"z": [0, 1, 0], "x": [1, 5, 0]}}])).unwrap();
    let corner =
        |s: &Snapshot| s.ids::<Vertex>().into_iter().map(|v| s.view(v).unwrap().point().to_vec3()).nth(1).unwrap();
    assert!((corner(&tilted) - Vec3::new(1.0, 0.0, 0.0)).norm() < 1e-15, "only x's part across z counts");
    assert!((corner(&along_x) - Vec3::new(0.0, 1.0, 0.0)).norm() < 1e-15, "the world's y, as x along z gives none");
}

#[test]
fn every_bad_parameter_of_a_request_is_reported_at_once() {
    let found =
        refused(&json!([{"op": "cylinder", "radus": 1, "height": 2}, {"op": "torus", "major": "big", "minor": 1}]));
    let codes: Vec<(Code, Option<Input>)> = found.into_iter().map(|(c, i, _)| (c, i)).collect();
    let unknown = Input::at("steps[0].radus").in_step(0, Some("cylinder")).given("radus");
    let wrong = Input::at("steps[1].major").in_step(1, Some("torus")).given("text").expected("number");
    let expected = [
        (Code::InputUnknownField, Some(unknown)),
        (Code::InputMissing, Some(place("steps[0].radius", "cylinder"))),
        (Code::InputWrongType, Some(wrong)),
    ];
    assert_eq!(codes, expected);
}

#[test]
fn a_request_in_inches_is_read_into_millimetres_once_and_built_there() {
    let text = json!({"schema": 1, "units": {"length": "in", "angle": "deg"}, "steps": [
        {"op": "box", "size": [1, 2, 3], "frame": {"origin": [1, 0, 0]}}
    ]});
    let request = Request::parse(&text.to_string()).unwrap();
    let want = PrimitiveArgs {
        shape: Shape::Box([25.4, 2.0 * 25.4, 3.0 * 25.4]),
        pose: Pose { origin: Vec3::new(25.4, 0.0, 0.0), ..Pose::STANDARD },
    };
    assert_eq!(request.steps[0].args, crate::Args::Primitive(want), "each converted once, as the units say");
    let mut edit = Snapshot::empty(default_tolerance().unwrap()).edit();
    let made = crate::execute(&mut edit, &request.steps[0], &Labels::default()).unwrap().unwrap();
    let (s, _) = edit.commit(&Exec::serial()).unwrap();
    assert_eq!(s.resolve::<Body>(made).map(|_| ()), Ok(()));
    let xs = s.ids::<Vertex>().into_iter().map(|v| s.view(v).unwrap().point().to_vec3().x);
    assert_eq!(xs.fold(0.0_f64, |a, x| if x > a { x } else { a }), 50.8, "from 25.4 mm, 25.4 mm long");
}
