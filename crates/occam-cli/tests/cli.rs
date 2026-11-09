//! Source: original
//! The tool as an agent meets it: a process, its standard output and its exit status. A wasm test run cannot start a process.
#![cfg(not(target_family = "wasm"))]
#![allow(clippy::unwrap_used, reason = "a failed step should fail the test")]

use std::process::{Command, Output};

use occam::{Code, RemedyKind, Report};

fn occam(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_occam")).args(arguments).output().unwrap()
}

fn asking_for(tolerance: &str) -> Output {
    occam(&[
        "distance",
        "--from",
        "0,0,0",
        "--to",
        "3,4,0",
        "--model-tolerance",
        "0.001,0.00001",
        "--tolerance",
        tolerance,
        "--json",
    ])
}

fn stdout(output: &Output) -> String {
    String::from_utf8(output.stdout.clone()).unwrap()
}

#[test]
fn a_refusal_is_json_and_following_its_remedy_succeeds() {
    let refused = asking_for("0.0001");
    assert_eq!(refused.status.code(), Some(1));
    let want = concat!(
        r#"{"schema":1,"code":"measure.tolerance-below-model","severity":"error","#,
        r#""message":"The tolerance asked for, 0.0001 mm, is finer than the model's tolerance, 0.001 mm.","entities":[],"#,
        r#""values":[{"name":"requested","quantity":"length","value":0.0001},"#,
        r#"{"name":"model","quantity":"length","value":0.001}],"#,
        r#""remedies":[{"kind":"use-model-tolerance","entities":[],"#,
        r#""values":[{"name":"tolerance","quantity":"length","value":0.001}]}]}"#,
        "\n",
    );
    assert_eq!(stdout(&refused), want);

    let report = Report::from_json(stdout(&refused).trim()).unwrap();
    assert_eq!(report.code, Code::ToleranceBelowModel);
    let remedy = report.remedy(RemedyKind::UseModelTolerance).unwrap();
    let followed = asking_for(&remedy.value("tolerance").unwrap().value.to_string());
    assert_eq!(followed.status.code(), Some(0));
    assert!(stdout(&followed).starts_with(r#"{"schema":1,"value":5.0,"#), "{}", stdout(&followed));
}

#[test]
fn in_words_a_refusal_goes_to_standard_error() {
    let refused = occam(&[
        "distance",
        "--from",
        "0,0,0",
        "--to",
        "3,4,0",
        "--model-tolerance",
        "0.001,0.00001",
        "--tolerance",
        "0.0001",
    ]);
    assert_eq!(refused.status.code(), Some(1));
    assert_eq!(stdout(&refused), "");
    let words = "The tolerance asked for, 0.0001 mm, is finer than the model's tolerance, 0.001 mm.\n";
    assert_eq!(String::from_utf8(refused.stderr).unwrap(), words);
}

#[test]
fn in_words_a_success_goes_to_standard_output() {
    let done = occam(&[
        "distance",
        "--from",
        "0,0,0",
        "--to",
        "3,4,0",
        "--model-tolerance",
        "0.001,0.00001",
        "--tolerance",
        "0.01",
    ]);
    assert_eq!(done.status.code(), Some(0));
    assert_eq!(stdout(&done), "5 mm\n");
    assert_eq!(String::from_utf8(done.stderr).unwrap(), "");
}

#[test]
fn a_bad_command_line_exits_with_two() {
    assert_eq!(occam(&["sideways"]).status.code(), Some(2));
}
