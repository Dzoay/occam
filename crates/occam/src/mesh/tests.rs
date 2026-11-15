//! Source: original
//! The session's meshes: one answer per face in the one JSON form and in words; after a run, only the faces it touched
//! meshed again and the rest kept bit for bit; a face refused answered with its report and an edge refused reported;
//! limits that are not positive refused by name.

use occam_math::Exec;
use occam_report::Code;
use serde_json::{Value, json};

use crate::{Encoding, Session};

/// A request in millimetres and degrees of `steps`, written as JSON.
fn request(steps: &Value) -> String {
    json!({"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": steps}).to_string()
}

/// A session whose model is a cube of 10 millimetres.
fn cube() -> Session {
    let mut session = Session::new(Encoding::Json);
    let (ran, _) = session.run(&request(&json!([{"op": "box", "size": [10, 10, 10]}])));
    assert!(ran.failed.is_none(), "{:?}", ran.failed);
    session
}

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}: {text}"))
}

#[test]
fn a_model_meshes_with_one_answer_per_face_in_its_one_json_form_and_in_words() {
    let meshed = cube().mesh(&Exec::serial(), 0.1, 0.35).unwrap();
    assert!(meshed.ok());
    let ids: Vec<String> = meshed.faces.iter().map(|(id, _)| id.text()).collect();
    assert_eq!(ids, (0..6).map(|k| format!("face:{k}/1")).collect::<Vec<_>>());
    let value = json_of(&meshed.json(None));
    let first = json!({"vertices": {"first": 0, "count": 4}, "triangles": {"first": 0, "count": 2}, "capped": false, "kept": false});
    assert_eq!(value["answers"][0], json!({"entity": "face:0/1", "mesh": first}));
    assert_eq!(value["answers"][5]["mesh"]["vertices"], json!({"first": 20, "count": 4}));
    assert_eq!(value["answers"][5]["mesh"]["triangles"], json!({"first": 10, "count": 2}));
    let head = [&value["schema"], &value["ok"], &value["vertices"], &value["triangles"]];
    assert_eq!(head, [&json!(2), &json!(true), &json!(24), &json!(12)]);
    assert_eq!(value["limits"], json!({"deflection": 0.1, "angle": 0.35}));
    assert_eq!((value["reports"].clone(), value.get("file")), (json!([]), None));
    assert_eq!(json_of(&meshed.json(Some("cube.occam.mesh")))["file"], "cube.occam.mesh");
    let said = "Meshed 6 of 6 faces to a deflection of 0.1 mm and an angle of 0.35 rad: 24 vertices and 12 triangles.";
    assert_eq!(meshed.words(), [(true, said.to_string())]);
}

#[test]
fn after_a_run_only_the_faces_it_touched_are_meshed_again() {
    let mut session = cube();
    let first = session.mesh(&Exec::serial(), 0.1, 0.35).unwrap();
    let bolt = json!([{"op": "cylinder", "radius": 4, "height": 10, "frame": {"origin": [20, 0, 0]}}]);
    let (ran, _) = session.run(&request(&bolt));
    assert!(ran.failed.is_none());
    let (refused, _) = session.run(&request(&json!([{"op": "sphere", "radius": -1}])));
    assert!(refused.failed.is_some(), "a run that stops short changes nothing");
    let second = session.mesh(&Exec::serial(), 0.1, 0.35).unwrap();
    let kept = |meshed: &crate::Meshed| -> Vec<bool> {
        meshed.faces.iter().map(|(_, placed)| placed.as_ref().unwrap().kept).collect()
    };
    assert_eq!(kept(&second), [[true; 6].as_slice(), &[false; 4]].concat(), "the cube's six faces kept");
    let six = 6 * 4;
    assert_eq!(second.buffers.positions[..3 * six], first.buffers.positions[..3 * six]);
    assert_eq!(second.buffers.indices[..3 * 12], first.buffers.indices[..3 * 12]);
    assert_eq!(kept(&session.mesh(&Exec::serial(), 0.1, 0.35).unwrap()), [true; 10], "nothing ran since");
    assert_eq!(kept(&session.mesh(&Exec::serial(), 0.05, 0.35).unwrap()), [false; 10], "other limits, all again");
}

#[test]
fn a_face_refused_answers_with_its_report_and_an_edge_refused_is_reported() {
    let mut session = Session::new(Encoding::Json);
    let disc = json!([
        {"op": "wire", "label": "rim", "edges": [{"circle": {"centre": [0, 0, 0], "radius": 1000, "normal": [0, 0, 1]}}]},
        {"op": "face", "outer": "rim"}
    ]);
    let (ran, _) = session.run(&request(&disc));
    assert!(ran.failed.is_none(), "{:?}", ran.failed);
    let meshed = session.mesh(&Exec::serial(), 1e-6, 0.35).unwrap();
    assert!(!meshed.ok());
    let codes: Vec<Code> = meshed.faces.iter().filter_map(|(_, p)| p.as_ref().err().map(|r| r.code)).collect();
    assert_eq!(codes, [Code::FaceNotMeshed], "a circle of 1000 needs too many samples at 1e-6");
    let reports: Vec<Code> = meshed.reports.iter().map(|r| r.code).collect();
    assert_eq!(reports, [Code::EdgeOverCap, Code::NotWatertight], "the circle, then the hole the disc leaves");
    let value = json_of(&meshed.json(None));
    assert_eq!((&value["ok"], &value["vertices"]), (&json!(false), &json!(0)));
    assert_eq!(value["answers"][0]["report"]["code"], "mesh.face-not-meshed");
    assert_eq!(value["answers"][0]["report"]["remedies"][0]["kind"], "use-deflection");
    assert_eq!(value["reports"][0]["code"], "mesh.edge-over-cap");
    let words = meshed.words();
    assert_eq!(words.len(), 4, "{words:?}");
    assert!(words[0].0 && words[0].1.starts_with("Meshed 0 of 1 faces "), "{words:?}");
    assert!(
        !words[1].0 && words[1].1.starts_with(&format!("{}: The face could not be meshed", meshed.faces[0].0.text()))
    );
    assert_eq!(words[2], (false, "The edge would need more than 16384 samples at a deflection of 1e-6 mm.".into()));
}

#[test]
fn limits_that_are_not_positive_are_refused_by_name() {
    let mut session = cube();
    for (deflection, angle, named) in [(0.0, 0.35, "deflection"), (0.1, f64::NAN, "angle")] {
        let refused = session.mesh(&Exec::serial(), deflection, angle).unwrap_err();
        let said: Vec<_> = refused.iter().map(|r| (r.code, r.input.as_ref().map(|i| i.parameter.clone()))).collect();
        assert_eq!(said, [(Code::LimitNotPositive, Some(named.to_string()))]);
    }
}

#[test]
fn a_session_with_no_model_meshes_an_empty_one() {
    let meshed = Session::new(Encoding::Binary).mesh(&Exec::serial(), 0.1, 0.35).unwrap();
    assert!(meshed.ok() && meshed.faces.is_empty() && meshed.buffers.vertices() == 0);
    assert_eq!(json_of(&meshed.json(None))["answers"], json!([]));
}
