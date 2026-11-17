//! Source: original
//! The session's tests: each command's answer and its JSON form for a known model, a box; an id that names nothing is
//! refused alone and the others answered; a run that goes through is kept and saved in the session's encoding, and one
//! that stops short, or whose model cannot be saved, leaves the model as it was.

use occam_report::{Code, DisplayUnits, Fault, NamedValue};
use serde_json::{Value, json};

use super::*;

/// A box 10 by 20 by 30 millimetres, at the origin.
const BOX: &str = r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
    {"op": "box", "label": "box", "size": [10, 20, 30]}
]}"#;

/// The box, made in a new session.
fn boxed(encoding: Encoding) -> Session {
    let mut session = Session::new(encoding);
    let (ran, saved) = session.run(BOX);
    assert!(ran.failed.is_none(), "{:?}", ran.failed);
    assert!(saved.is_some(), "a run that changes the model is saved");
    session
}

/// A request in millimetres and degrees of `steps`, written as JSON.
fn in_millimetres(steps: &str) -> String {
    format!(r#"{{"schema": 1, "units": {{"length": "mm", "angle": "deg"}}, "steps": {steps}}}"#)
}

fn entity(text: &str) -> EntityRef {
    EntityRef::parse(text).unwrap_or_else(|| panic!("{text}"))
}

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}: {text}"))
}

#[test]
fn a_box_lists_each_kind_in_slot_order() {
    let session = boxed(Encoding::Json);
    let listed = session.list(&[EntityKind::Body, EntityKind::Shell, EntityKind::Vertex]);
    let counts: Vec<(EntityKind, usize)> = listed.iter().map(|l| (l.kind, l.entities.len())).collect();
    assert_eq!(counts, [(EntityKind::Body, 1), (EntityKind::Shell, 1), (EntityKind::Vertex, 8)]);
    let vertices: Vec<String> = listed[2].entities.iter().map(|e| e.text()).collect();
    assert_eq!(vertices, (0..8).map(|slot| format!("vertex:{slot}/1")).collect::<Vec<_>>());
    let answered = Answered::Listed(session.list(&[EntityKind::Body, EntityKind::Face]));
    assert!(answered.ok());
    let want = json!({
        "schema": 2,
        "ok": true,
        "answers": [
            {"kind": "body", "entities": ["body:0/1"]},
            {"kind": "face", "entities": ["face:0/1", "face:1/1", "face:2/1", "face:3/1", "face:4/1", "face:5/1"]}
        ],
        "reports": []
    });
    assert_eq!(json_of(&answered.json()), want);
}

#[test]
fn describing_answers_each_id_and_refuses_a_stale_one_alone() {
    let session = boxed(Encoding::Json);
    let ids = [entity("body:0/1"), entity("face:99/1"), entity("vertex:0/1")];
    let answers = session.describe(&ids);
    assert_eq!(answers.len(), 3, "one answer per id");
    let body = answers[0].1.as_ref().unwrap();
    assert_eq!((body.entity, body.form.as_str()), (ids[0], "solid"));
    let stale = answers[1].1.as_ref().unwrap_err();
    assert_eq!(
        (stale.code, stale.fault, stale.entities.as_slice()),
        (Code::StaleId, Some(Fault::InvalidInput), &ids[1..2])
    );
    assert_eq!(answers[2].1.as_ref().map(|d| d.form.as_str()), Ok("vertex"));

    let answered = Answered::Described(answers);
    assert!(!answered.ok(), "an id was refused");
    let value = json_of(&answered.json());
    assert_eq!((value["schema"].as_u64(), value["ok"].as_bool()), (Some(2), Some(false)));
    let list = value["answers"].as_array().unwrap();
    assert_eq!(list.len(), 3);
    assert_eq!(
        (list[0]["entity"].as_str(), list[0]["description"]["form"].as_str()),
        (Some("body:0/1"), Some("solid"))
    );
    assert_eq!(list[1]["entity"], "face:99/1");
    assert_eq!(list[1]["report"]["code"], "topo.stale-id");
    assert!(list[1].get("description").is_none(), "{}", list[1]);
    assert_eq!(list[2]["description"]["entity"], "vertex:0/1");
    assert_eq!(value["reports"], json!([]));
}

#[test]
fn adjacency_is_the_descriptions_neighbours_and_a_stale_id_is_refused_alone() {
    let session = boxed(Encoding::Json);
    let ids = [entity("face:0/1"), entity("edge:0/7"), entity("shell:0/1")];
    let answers = session.adjacent(&ids);
    let described = session.describe(&ids[..1]);
    let face = described[0].1.as_ref().unwrap();
    assert_eq!(answers[0].1.as_ref().unwrap(), &face.adjacent);
    assert!(face.adjacent.len() > 4, "a box's face meets its shell, surface, loop and four edges: {:?}", face.adjacent);
    assert_eq!(answers[1].1.as_ref().map_err(|r| r.code), Err(Code::StaleId));
    let shell = answers[2].1.as_ref().unwrap();
    assert!(shell.contains(&entity("body:0/1")), "{shell:?}");
    assert_eq!(shell.iter().filter(|e| e.kind == EntityKind::Face).count(), 6);

    let value = json_of(&Answered::Adjacent(answers).json());
    let list = value["answers"].as_array().unwrap();
    let neighbours: Vec<&str> = list[0]["adjacent"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    assert_eq!(neighbours, face.adjacent.iter().map(|e| e.text()).collect::<Vec<_>>());
    assert_eq!(
        (list[1]["entity"].as_str(), list[1]["report"]["code"].as_str()),
        (Some("edge:0/7"), Some("topo.stale-id"))
    );
}

#[test]
fn a_check_says_what_it_found_or_that_the_model_is_sound() {
    let sound = Answered::Checked(Vec::new());
    assert!(sound.ok());
    assert_eq!(json_of(&sound.json()), json!({"schema": 2, "ok": true, "answers": [], "reports": []}));
    assert_eq!(sound.words(), [(true, "The model is sound.".to_string())]);
    let found = Report::new(Code::InvariantFailed, Fault::Internal).with_entity(entity("face:3/1"));
    let message = found.message(DisplayUnits::default());
    let unsound = Answered::Checked(vec![found.clone()]);
    assert!(!unsound.ok());
    assert_eq!(json_of(&unsound.json())["reports"][0]["code"], "check.invariant-failed");
    assert_eq!(unsound.words(), [(false, message.clone())]);
    assert_eq!(Answered::Refused(vec![found]).words(), [(false, message)]);
}

#[test]
fn a_description_in_words_gives_its_values_origin_and_neighbours_where_it_has_them() {
    let session = boxed(Encoding::Json);
    let ids = [entity("vertex:0/1"), entity("body:0/1"), entity("surface:0/1")];
    let lines = Answered::Described(session.describe(&ids)).words();
    let vertex = "vertex:0/1 vertex: point (0, 0, 0) mm; adjacent edge:0/1 edge:3/1 edge:9/1";
    assert_eq!(lines[0], (true, vertex.to_string()));
    assert!(lines[1].1.starts_with("body:0/1 solid: shells 1, faces 6, wires 0, low ("), "{}", lines[1].1);
    assert!(
        lines[1].1.ends_with(" mm; made by step 0, box; adjacent shell:0/1"),
        "as `run` numbers it: {}",
        lines[1].1
    );
    assert!(lines[2].1.starts_with("surface:0/1 plane: origin (5, 10, 0) mm, normal ("), "{}", lines[2].1);
    assert!(!lines[2].1.contains("adjacent"), "geometry names no neighbour: {}", lines[2].1);
}

#[test]
fn a_refused_command_says_why_and_answers_nothing() {
    let report = Report::new(Code::NotReadable, Fault::InvalidInput)
        .with_value(NamedValue::count("line", 1))
        .with_value(NamedValue::count("column", 1));
    let answered = Answered::Refused(vec![report]);
    assert!(!answered.ok());
    let value = json_of(&answered.json());
    assert_eq!((value["ok"].as_bool(), value["answers"].as_array().map(Vec::len)), (Some(false), Some(0)));
    assert_eq!(value["reports"][0]["code"], "io.not-readable");
}

#[test]
fn a_saved_model_opens_again_with_the_same_ids_in_either_encoding() {
    for encoding in [Encoding::Json, Encoding::Binary] {
        let mut session = Session::new(encoding);
        let (ran, saved) = session.run(BOX);
        let bytes = saved.unwrap();
        assert_eq!(bytes.starts_with(occam_io::MAGIC), encoding == Encoding::Binary, "{encoding:?}");
        let opened = Session::open(&bytes, &Exec::serial()).unwrap();
        assert_eq!(opened.encoding(), encoding);
        let (made, again) = (ran.model.unwrap(), opened.model().cloned().unwrap());
        assert_eq!(made.digest(), again.digest(), "{encoding:?}");
        assert_eq!(opened.list(&[EntityKind::Edge]), session.list(&[EntityKind::Edge]));
    }
}

#[test]
fn bytes_that_are_not_a_model_are_refused_on_opening() {
    let refused = Session::open(b"{\"format\": \"something else\"}", &Exec::serial()).err().unwrap();
    assert_eq!(refused.iter().map(|r| r.code).collect::<Vec<_>>(), [Code::NotReadable]);
}

#[test]
fn a_run_that_stops_short_leaves_the_model_as_it_was_and_saves_nothing() {
    let mut session = boxed(Encoding::Json);
    let before = session.model().unwrap().digest();
    let bad = r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
        {"op": "box", "size": [1, 1, 1]},
        {"op": "measure", "target": "body:9/1"}
    ]}"#;
    let (ran, saved) = session.run(bad);
    assert_eq!(
        (ran.done.len(), ran.failed.as_ref().and_then(|f| f.step)),
        (1, Some(1)),
        "the box was made, then the run stopped"
    );
    assert_eq!(saved, None);
    assert_eq!(session.model().unwrap().digest(), before, "the whole request or nothing");
}

#[test]
fn a_run_that_changes_nothing_saves_nothing() {
    let mut session = boxed(Encoding::Json);
    let (ran, saved) = session.run(&in_millimetres(r#"[{"op": "measure", "target": "body:0/1"}]"#));
    assert!(ran.failed.is_none(), "{:?}", ran.failed);
    assert_eq!(saved, None);
}

#[test]
fn a_model_the_format_cannot_write_yet_is_not_kept_and_the_run_says_why() {
    let mut session = boxed(Encoding::Binary);
    let before = session.model().unwrap().digest();
    // A circle extruded aslant makes a surface of extrusion, which the format does not write yet.
    let slanted = r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
        {"op": "wire", "label": "hole", "edges": [{"circle": {"centre": [50, 0, 0], "radius": 3, "normal": [0, 0, 1]}}]},
        {"op": "face", "label": "disc", "outer": "hole"},
        {"op": "extrude", "face": "disc", "vector": [0, 3, 5]}
    ]}"#;
    let (ran, saved) = session.run(slanted);
    assert_eq!(saved, None);
    assert_eq!(ran.done.len(), 3, "every step ran");
    let failed = ran.failed.as_ref().unwrap();
    assert_eq!(failed.step, None, "no step failed: the result could not be saved");
    assert_eq!(failed.reports.iter().map(|r| r.code).collect::<Vec<_>>(), [Code::NotSaved]);
    assert!(!failed.reports[0].described.is_empty(), "the geometry named is described");
    assert_eq!(session.model().unwrap().digest(), before);
}

#[test]
fn a_run_in_words_says_what_each_step_did_and_why_it_stopped() {
    let mut session = Session::new(Encoding::Json);
    let (ran, _) = session.run(BOX);
    let lines = ran_words(&ran);
    assert_eq!(lines.len(), 1, "{lines:?}");
    assert!(lines[0].0);
    assert!(lines[0].1.starts_with("step 0 box \"box\": made body:0/1; created "), "{}", lines[0].1);
    let (refused, _) = session.run(&in_millimetres(r#"[{"op": "sphere", "radius": -1}]"#));
    let lines = ran_words(&refused);
    assert!(lines.iter().any(|(ok, line)| !ok && line.contains("-1")), "{lines:?}");
}

#[test]
fn answers_in_words_go_one_line_each_the_refused_ones_apart() {
    let session = boxed(Encoding::Json);
    let lines = Answered::Described(session.describe(&[entity("body:0/1"), entity("body:3/1")])).words();
    assert_eq!(lines.len(), 2);
    assert!(lines[0].0 && lines[0].1.starts_with("body:0/1 solid: "), "{:?}", lines[0]);
    assert!(!lines[1].0 && lines[1].1.starts_with("body:3/1: No live entity"), "{:?}", lines[1]);
    let listed = Answered::Listed(session.list(&[EntityKind::Body])).words();
    assert_eq!(listed, [(true, "body: body:0/1".to_string())]);
    assert_eq!(Answered::Checked(Vec::new()).words(), [(true, "The model is sound.".to_string())]);
}

#[test]
fn a_model_opened_from_its_twin_has_the_digest_it_was_saved_with_whichever_encoding_it_came_in() {
    let session = boxed(Encoding::Json);
    let digest = session.digest().unwrap();
    assert_eq!(digest, session.model().unwrap().digest());
    let twin = session.encoded(Encoding::Binary).unwrap().unwrap();
    assert!(twin.starts_with(occam_io::MAGIC), "the binary twin, though the session saves JSON");
    assert_eq!(Session::open(&twin, &Exec::serial()).unwrap().digest(), Some(digest));
    let json = session.encoded(Encoding::Json).unwrap().unwrap();
    assert_eq!(Session::open(&json, &Exec::serial()).unwrap().digest(), Some(digest));
    assert_eq!(
        (Session::new(Encoding::Binary).digest(), Session::new(Encoding::Json).encoded(Encoding::Binary)),
        (None, None)
    );
}

#[test]
fn a_model_given_after_another_is_meshed_again_only_where_it_differs() {
    let boxed = boxed(Encoding::Binary);
    let twin = boxed.encoded(Encoding::Binary).unwrap().unwrap();
    let mut grown = Session::open(&twin, &Exec::serial()).unwrap();
    let (ran, _) = grown
        .run(&in_millimetres(r#"[{"op": "cylinder", "radius": 2, "height": 5, "frame": {"origin": [20, 0, 0]}}]"#));
    assert!(ran.failed.is_none(), "{:?}", ran.failed);
    let again = grown.again(&boxed, &Exec::serial(), 0.1, 0.35).unwrap();
    assert_eq!(again.faces, [6, 7, 8, 9], "the cylinder's four faces, after the box's six");
    let edges = boxed.list(&[EntityKind::Edge]).remove(0).entities;
    let kept: Vec<EntityRef> = (0..6).map(|slot| EntityRef::new(EntityKind::Face, slot, 1)).chain(edges).collect();
    assert_eq!(again.kept, kept, "the box's faces and the edges they alone use");
    let texts: Vec<String> = kept.iter().map(|id| id.text()).collect();
    assert_eq!(json_of(&again.json()), json!({"schema": 2, "faces": [6, 7, 8, 9], "kept": texts}));
    let nothing = grown.again(&grown, &Exec::serial(), 0.1, 0.35).unwrap();
    assert_eq!(nothing.faces, Vec::<usize>::new(), "a model given again as it was");
    let all = grown.again(&Session::new(Encoding::Binary), &Exec::serial(), 0.1, 0.35).unwrap();
    assert_eq!((all.faces, all.kept), ((0..10).collect(), Vec::new()), "after no model, every face");
    let refused = grown.again(&boxed, &Exec::serial(), 0.0, 0.35).unwrap_err();
    assert_eq!(refused.iter().map(|r| r.code).collect::<Vec<_>>(), [Code::LimitNotPositive]);
}

#[test]
fn a_pick_on_a_model_picked_before_computes_no_digest_and_one_after_a_run_picks_the_model_it_made() {
    let mut session = boxed(Encoding::Json);
    let exec = Exec::serial();
    let down = Pick { from: [5.0, 10.0, 50.0], direction: [0.0, 0.0, -1.0], tolerance: 0.0 };
    let counted = digests();
    let first = session.pick(&exec, 0.1, 0.35, down).unwrap();
    assert_eq!(digests() - counted, 1, "the picker made, with the model's digest");
    let again = session.pick(&exec, 0.1, 0.35, down).unwrap();
    let edge = session.list(&[EntityKind::Edge]).remove(0).entities[0];
    assert!(session.samples(&exec, 0.1, 0.35, edge).unwrap().is_some());
    assert_eq!(digests() - counted, 1, "no digest for a pick or samples on the model picked before");
    assert_eq!((&again, first.digest), (&first, session.digest()), "the same answer, naming the model");
    let (ran, _) =
        session.run(&in_millimetres(r#"[{"op": "box", "size": [10, 20, 10], "frame": {"origin": [0, 0, 30]}}]"#));
    assert!(ran.failed.is_none(), "{:?}", ran.failed);
    let counted = digests();
    let after = session.pick(&exec, 0.1, 0.35, down).unwrap();
    assert_eq!(digests() - counted, 1, "a new picker for the model the run made");
    assert_eq!(after.digest, session.digest());
    assert_ne!(after.digest, first.digest);
    let (was, now) = (first.hit.unwrap(), after.hit.unwrap());
    assert_eq!((was.point[2], now.point[2]), (30.0, 40.0), "the new box's top, above the first's");
}
