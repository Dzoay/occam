//! Source: original
//! The view's JSON forms read back as they were written, and what the command says in words and in JSON.

use occam_report::{Code, Fault, Input};
use serde_json::{Value, json};

use super::*;

fn viewer() -> Viewer {
    Viewer { pid: 41, port: 40123, token: "9f0e".to_string() }
}

fn missing() -> Report {
    Report::new(Code::ModelMissing, Fault::InvalidInput).with_input(Input::at("--model").given("/m/part.occam"))
}

fn watching(reports: Vec<Report>) -> Watching {
    Watching { model: "/m/part.occam".to_string(), pid: 41, pages: 1, digest: Some(0xab), reports }
}

fn json_of(text: &str) -> Value {
    serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}: {text}"))
}

#[test]
fn a_state_file_reads_back_as_written_and_anything_else_is_stale() {
    let written = viewer().json();
    assert_eq!(json_of(&written), json!({"schema": 2, "pid": 41, "port": 40123, "token": "9f0e"}));
    assert_eq!(Viewer::from_json(&written), Some(viewer()));
    assert_eq!(viewer().url(), "http://127.0.0.1:40123/9f0e/");
    for stale in [
        "",
        "{}",
        "not json",
        r#"{"schema": 2, "pid": 41, "port": 70000, "token": "9f0e"}"#,
        r#"{"schema": 1, "pid": 41, "port": 40123, "token": "9f0e"}"#,
        r#"{"schema": 2, "pid": 41, "port": 1, "token": ""}"#,
    ] {
        assert_eq!(Viewer::from_json(stale), None, "{stale}");
    }
}

#[test]
fn what_a_viewer_says_of_its_model_reads_back_as_written() {
    let said = watching(vec![missing()]);
    let value = json_of(&said.json());
    assert_eq!(
        (&value["digest"], &value["pages"], &value["reports"][0]["code"]),
        (&json!("00000000000000ab"), &json!(1), &json!("view.model-missing"))
    );
    assert_eq!(Watching::from_json(&said.json()), Some(said));
    let none = Watching { digest: None, ..watching(Vec::new()) };
    assert_eq!(Watching::from_json(&none.json()), Some(none));
    assert_eq!(Watching::from_json(r#"{"schema": 2}"#), None);
    assert_eq!(digest_text(u64::MAX), "ffffffffffffffff");
}

#[test]
fn the_answer_gives_the_address_and_token_where_a_viewer_runs_and_its_reports_in_words_apart() {
    let started = Viewed { running: Some((viewer(), watching(Vec::new()))), started: true, ..Viewed::of("part.occam") };
    let value = json_of(&started.json());
    assert_eq!(
        value,
        json!({"schema": 2, "ok": true, "model": "part.occam", "running": true, "started": true, "stopped": false,
            "url": "http://127.0.0.1:40123/9f0e/", "token": "9f0e", "port": 40123, "pid": 41, "pages": 1,
            "digest": "00000000000000ab", "opened": false, "reports": []})
    );
    assert_eq!(started.words(), [(true, "The viewer for part.occam is at http://127.0.0.1:40123/9f0e/.".to_string())]);
    let refused = Viewed { running: Some((viewer(), watching(vec![missing()]))), ..Viewed::of("part.occam") };
    assert!(!refused.ok());
    let words = refused.words();
    assert_eq!(
        words[0],
        (
            true,
            "The viewer for part.occam runs at http://127.0.0.1:40123/9f0e/, process 41, with 1 page open.".to_string()
        )
    );
    assert!(!words[1].0, "to standard error");
    assert!(words[1].1.starts_with("There is no model at /m/part.occam: "), "{}", words[1].1);
    assert_eq!(json_of(&refused.json())["reports"][0]["code"], "view.model-missing");
    let stopped = Viewed { stopped: true, ..Viewed::of("part.occam") };
    assert_eq!(
        json_of(&stopped.json()),
        json!({"schema": 2, "ok": true, "model": "part.occam", "running": false,
        "started": false, "stopped": true, "reports": []})
    );
    assert_eq!(stopped.words(), [(true, "The viewer for part.occam has stopped.".to_string())]);
    assert_eq!(Viewed::of("part.occam").words(), [(true, "No viewer runs for part.occam.".to_string())]);
}
