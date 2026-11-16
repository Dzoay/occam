//! Source: original

use occam_report::EntityKind;
use occam_topo::Lineage;
use serde_json::{Value, json};

use super::*;
use crate::{Session, ran_json, ran_words, session::Encoding};

fn id(text: &str) -> EntityRef {
    EntityRef::parse(text).unwrap()
}

fn request(steps: &str) -> String {
    format!(r#"{{"schema": 1, "units": {{"length": "mm", "angle": "deg"}}, "steps": [{steps}]}}"#)
}

#[test]
fn a_selection_reads_back_as_written_each_id_once_and_anything_else_is_none() {
    let selection = Selection { digest: 0xab, ids: vec![id("face:3/1"), id("edge:7/2")] };
    let written = selection.json();
    let value: Value = serde_json::from_str(&written).unwrap();
    assert_eq!(value, json!({"schema": 2, "digest": "00000000000000ab", "ids": ["face:3/1", "edge:7/2"]}));
    assert_eq!(Selection::from_json(&written), Some(selection.clone()));
    let twice = r#"{"schema": 2, "digest": "00000000000000ab", "ids": ["face:3/1", "edge:7/2", "face:3/1"]}"#;
    assert_eq!(Selection::from_json(twice), Some(selection), "an id given twice is kept where it came first");
    let none: Value = serde_json::from_str(&Selection::none_json()).unwrap();
    assert_eq!(none, json!({"schema": 2, "digest": null, "ids": []}));
    for not in [
        "",
        "{}",
        &Selection::none_json(),
        r#"{"schema": 1, "digest": "ab", "ids": []}"#,
        r#"{"schema": 2, "digest": "not hex", "ids": []}"#,
        r#"{"schema": 2, "digest": "ab", "ids": ["face 3"]}"#,
        r#"{"schema": 2, "digest": "ab"}"#,
    ] {
        assert_eq!(Selection::from_json(not), None, "{not}");
    }
}

/// `steps` run on `session`, which must go through and change the model, and the selection `selected` carried over.
fn carried(session: &mut Session, selected: &[&str], steps: &str) -> (Ran, Selection, Vec<Carried>) {
    let before = Selection { digest: session.digest().unwrap(), ids: selected.iter().map(|text| id(text)).collect() };
    let (ran, saved) = session.run(&request(steps));
    assert!(ran.failed.is_none() && saved.is_some(), "{ran:?}");
    let (after, said) = before.carried(&ran, session.digest().unwrap());
    (ran, after, said)
}

#[test]
fn a_copy_of_the_selected_body_leaves_the_selection_on_the_original() {
    let mut session = Session::new(Encoding::Json);
    let _ = session.run(&request(r#"{"op": "box", "size": [1, 2, 3]}"#));
    let copy = r#"{"op": "copy", "target": "body:0/1", "frame": {"origin": [5, 0, 0]}}"#;
    let (ran, after, said) = carried(&mut session, &["face:2/1", "body:0/1"], copy);
    assert!(ran.done[0].changes.lineage.iter().any(|l| l.old == id("face:2/1") && l.how == Derivation::Copied));
    assert_eq!(after, Selection { digest: session.digest().unwrap(), ids: vec![id("face:2/1"), id("body:0/1")] });
    assert_eq!(
        said,
        [
            Carried { was: id("face:2/1"), now: vec![id("face:2/1")] },
            Carried { was: id("body:0/1"), now: vec![id("body:0/1")] }
        ]
    );
}

#[test]
fn a_run_that_sweeps_the_selected_face_away_drops_it_and_says_so() {
    let mut session = Session::new(Encoding::Json);
    let profile = r#"{"op": "wire", "label": "w", "points": [[0, 0, 0], [4, 0, 0], [4, 3, 0]], "closed": true},
        {"op": "face", "outer": "w"}"#;
    let (made, _) = session.run(&request(profile));
    let face = made.done[1].changes.created.iter().copied().find(|e| e.kind == EntityKind::Face).unwrap();
    let extrude = format!(r#"{{"op": "extrude", "face": "{}", "vector": [0, 0, 2]}}"#, face.text());
    let (mut ran, after, said) = carried(&mut session, &[&face.text()], &extrude);
    assert_eq!((after.ids, said.clone()), (vec![], vec![Carried { was: face, now: vec![] }]), "swept away");
    ran.selection = Some(said);
    ran.highlight_cleared = true;
    let value: Value = serde_json::from_str(&ran_json(&ran)).unwrap();
    assert_eq!(value["selection"], json!([{"entity": face.text(), "now": []}]));
    assert_eq!(value["highlight-cleared"], json!(true));
    let words = ran_words(&ran);
    let gone = format!("{} was selected and is gone, so the selection drops it.", face.text());
    assert_eq!(words[words.len() - 2..], [(true, gone), (true, "The highlight is cleared.".to_string())]);
}

#[test]
fn an_entity_split_trimmed_or_merged_is_replaced_by_what_it_became_and_one_changed_keeps_its_id() {
    let (whole, kept, other) = (id("face:1/1"), id("face:2/1"), id("face:3/1"));
    let lineage = |new: &str, how| Lineage { new: id(new), old: whole, how };
    let changes = Changes {
        created: vec![id("face:7/1"), id("face:8/1"), id("face:9/1")],
        changed: vec![kept],
        deleted: vec![whole, other],
        lineage: vec![
            lineage("face:7/1", Derivation::Split),
            lineage("face:8/1", Derivation::Trimmed),
            lineage("face:9/1", Derivation::Swept),
        ],
    };
    assert_eq!(after(whole, &changes), [id("face:7/1"), id("face:8/1")], "what a sweep makes is its own");
    assert_eq!(after(kept, &changes), [kept]);
    assert_eq!(after(other, &changes), [], "deleted with nothing in its place");
    let merged = Changes {
        deleted: vec![whole, kept],
        lineage: [whole, kept].map(|old| Lineage { new: other, old, how: Derivation::Merged }).into(),
        ..Changes::default()
    };
    let ran = crate::run(None, &request(r#"{"op": "box", "size": [1, 1, 1]}"#));
    let ran = Ran { done: vec![crate::Done { changes: merged, ..ran.done[0].clone() }], ..ran };
    let both = Selection { digest: 1, ids: vec![whole, kept] };
    assert_eq!(both.carried(&ran, 2).0, Selection { digest: 2, ids: vec![other] }, "two merged into one, named once");
}

#[test]
fn a_stale_selection_is_refused_with_the_remedy_of_selecting_again() {
    let report = Selection::stale("/m/part.occam");
    let said = report.message(occam_report::DisplayUnits::default());
    assert_eq!(
        said,
        "The selection was made on another model than /m/part.occam holds now, so it names nothing: select again."
    );
    assert_eq!((report.code, report.remedies[0].kind), (Code::SelectionStale, RemedyKind::SelectAgain));
}
