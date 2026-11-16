//! Source: original
//! The tool as an agent meets it: a process, its standard output and its exit status. A wasm test run cannot start a process.
#![cfg(not(target_family = "wasm"))]
#![allow(clippy::unwrap_used, reason = "a failed step should fail the test")]
#![allow(clippy::arithmetic_side_effects, reason = "offsets into a small file in tests")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use occam::{Code, RemedyKind, Report};
use serde_json::{Value, json};

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
        r#"{"schema":2,"code":"measure.tolerance-below-model","severity":"error","fault":"invalid-input","#,
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
    let followed = asking_for(&remedy.value("tolerance").unwrap().number().unwrap().to_string());
    assert_eq!(followed.status.code(), Some(0));
    assert!(stdout(&followed).starts_with(r#"{"schema":2,"value":5.0,"#), "{}", stdout(&followed));
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

/// A box 10 by 20 by 30 millimetres, at the origin.
const BOX: &str = r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
    {"op": "box", "label": "box", "size": [10, 20, 30]}
]}"#;

/// A directory of the test's own, empty. Every test file of this crate makes its directories in the one temporary
/// directory cargo gives the crate, and the files' tests run at once, so no two tests anywhere here share a `name`.
fn scratch(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(name);
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

fn json_out(output: &Output) -> Value {
    let text = stdout(output);
    serde_json::from_str(&text).map_err(|e| format!("{e}: {text}")).unwrap()
}

/// The box, made by `occam run` into `dir/box.occam`, whose path it gives.
fn boxed(dir: &Path) -> PathBuf {
    let (model, request) = (dir.join("box.occam"), dir.join("box.json"));
    std::fs::write(&request, BOX).unwrap();
    let ran = occam(&["run", "--model", text(&model), text(&request), "--json"]);
    assert_eq!(ran.status.code(), Some(0), "{}{}", stdout(&ran), String::from_utf8_lossy(&ran.stderr));
    model
}

#[test]
fn a_run_makes_a_new_model_and_says_what_each_step_did() {
    let dir = scratch("run-new");
    let (model, request) = (dir.join("box.occam"), dir.join("box.json"));
    std::fs::write(&request, BOX).unwrap();
    let ran = occam(&["run", "--model", text(&model), text(&request), "--json"]);
    assert_eq!(ran.status.code(), Some(0));
    let value = json_out(&ran);
    assert_eq!((value["schema"].as_u64(), value["ok"].as_bool()), (Some(2), Some(true)));
    assert_eq!(value["steps"][0]["made"], "body:0/1");
    assert_eq!(
        value["steps"][0]["created"].as_array().map(Vec::len),
        Some(100),
        "8 vertices, 12 edges, 24 uses, 6 loops, faces and surfaces, 12 curves, 24 parameter curves, a shell and the body"
    );
    let saved = std::fs::read(&model).unwrap();
    assert!(saved.starts_with(b"{"), "a new model is the readable JSON");
}

#[test]
fn each_command_answers_for_a_known_model_in_one_json_form() {
    let model = boxed(&scratch("known"));
    let listed = occam(&["list", "--model", text(&model), "--kind", "body,face", "--json"]);
    assert_eq!(listed.status.code(), Some(0));
    let want = json!({
        "schema": 2,
        "ok": true,
        "answers": [
            {"kind": "body", "entities": ["body:0/1"]},
            {"kind": "face", "entities": ["face:0/1", "face:1/1", "face:2/1", "face:3/1", "face:4/1", "face:5/1"]}
        ],
        "reports": []
    });
    assert_eq!(json_out(&listed), want);

    let described = occam(&["describe", "--model", text(&model), "body:0/1", "vertex:7/1", "--json"]);
    assert_eq!(described.status.code(), Some(0));
    let value = json_out(&described);
    assert_eq!(value["answers"][0]["description"]["form"], "solid");
    assert_eq!(value["answers"][1]["description"]["form"], "vertex");
    let corner = value["answers"][1]["description"]["values"][0].clone();
    assert_eq!((corner["name"].as_str(), corner["value"].clone()), (Some("point"), json!([0.0, 20.0, 30.0])));

    let near = occam(&["adjacent", "--model", text(&model), "shell:0/1", "--json"]);
    assert_eq!(near.status.code(), Some(0));
    let neighbours = json_out(&near)["answers"][0]["adjacent"].as_array().cloned().unwrap();
    assert_eq!(neighbours[0], "body:0/1");
    assert_eq!(neighbours.iter().filter(|n| n.as_str().is_some_and(|n| n.starts_with("face:"))).count(), 6);

    let checked = occam(&["check", "--model", text(&model), "--json"]);
    assert_eq!(checked.status.code(), Some(0));
    assert_eq!(json_out(&checked), json!({"schema": 2, "ok": true, "answers": [], "reports": []}));
}

#[test]
fn an_unknown_id_is_refused_alone_and_the_others_answered() {
    let model = boxed(&scratch("stale"));
    for command in ["describe", "adjacent"] {
        let asked = occam(&[command, "--model", text(&model), "edge:0/1", "edge:40/1", "face:5/1", "--json"]);
        assert_eq!(asked.status.code(), Some(1), "{command}: an id was refused");
        let value = json_out(&asked);
        assert_eq!(value["ok"], false);
        let answers = value["answers"].as_array().cloned().unwrap();
        let said: Vec<(&str, bool)> =
            answers.iter().map(|a| (a["entity"].as_str().unwrap(), a.get("report").is_some())).collect();
        assert_eq!(said, [("edge:0/1", false), ("edge:40/1", true), ("face:5/1", false)], "{command}");
        assert_eq!(answers[1]["report"]["code"], "topo.stale-id");
    }
}

#[test]
fn in_words_answers_go_to_standard_output_and_refusals_to_standard_error() {
    let model = boxed(&scratch("words"));
    let asked = occam(&["describe", "--model", text(&model), "body:0/1", "body:1/1"]);
    assert_eq!(asked.status.code(), Some(1));
    assert!(stdout(&asked).starts_with("body:0/1 solid: "), "{}", stdout(&asked));
    assert!(String::from_utf8(asked.stderr).unwrap().starts_with("body:1/1: No live entity"));
    let listed = occam(&["list", "--model", text(&model), "--kind", "body"]);
    assert_eq!(stdout(&listed), "body: body:0/1\n");
}

#[test]
fn a_run_that_stops_short_leaves_the_file_as_it_was() {
    let dir = scratch("short");
    let model = boxed(&dir);
    let before = std::fs::read(&model).unwrap();
    let request = dir.join("bad.json");
    std::fs::write(
        &request,
        r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
        {"op": "box", "size": [1, 1, 1]}, {"op": "measure", "target": "body:9/1"}]}"#,
    )
    .unwrap();
    let ran = occam(&["run", "--model", text(&model), text(&request), "--json"]);
    assert_eq!(ran.status.code(), Some(1));
    let value = json_out(&ran);
    assert_eq!((value["ok"].as_bool(), value["failed"]["step"].as_u64()), (Some(false), Some(1)));
    assert_eq!(std::fs::read(&model).unwrap(), before, "the whole request or nothing");
}

#[test]
fn a_model_saved_as_the_binary_twin_stays_one_and_answers_alike() {
    let dir = scratch("binary");
    let (model, request) = (dir.join("box.occam"), dir.join("box.json"));
    std::fs::write(&request, BOX).unwrap();
    assert_eq!(occam(&["run", "--binary", "--model", text(&model), text(&request)]).status.code(), Some(0));
    assert!(std::fs::read(&model).unwrap().starts_with(b"\x89occam"));
    std::fs::write(
        &request,
        r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
        {"op": "sphere", "radius": 4}]}"#,
    )
    .unwrap();
    assert_eq!(occam(&["run", "--model", text(&model), text(&request)]).status.code(), Some(0));
    assert!(std::fs::read(&model).unwrap().starts_with(b"\x89occam"), "it keeps its encoding");
    let listed = occam(&["list", "--model", text(&model), "--kind", "body", "--json"]);
    assert_eq!(json_out(&listed)["answers"][0]["entities"], json!(["body:0/1", "body:1/1"]));
}

#[test]
fn a_file_that_cannot_be_read_is_named() {
    let dir = scratch("missing");
    let absent = dir.join("absent.occam");
    let listed = occam(&["list", "--model", text(&absent), "--kind", "edge", "--json"]);
    assert_eq!(listed.status.code(), Some(1));
    let report = &json_out(&listed)["reports"][0];
    assert_eq!(report["code"], "cli.file-not-read");
    assert_eq!(
        (report["input"]["parameter"].as_str(), report["input"]["given"].as_str()),
        (Some("--model"), Some(text(&absent)))
    );
    let ran = occam(&["run", "--model", text(&dir.join("new.occam")), text(&dir.join("absent.json")), "--json"]);
    assert_eq!(ran.status.code(), Some(1));
    assert_eq!(json_out(&ran)["failed"]["reports"][0]["code"], "cli.file-not-read");
    assert!(!dir.join("new.occam").exists(), "nothing was made");
}

#[test]
fn a_model_path_that_cannot_be_read_is_not_taken_for_a_new_model() {
    let dir = scratch("unreadable");
    let request = dir.join("box.json");
    std::fs::write(&request, BOX).unwrap();
    let ran = occam(&["run", "--model", text(&dir), text(&request), "--json"]);
    assert_eq!(ran.status.code(), Some(1));
    let report = &json_out(&ran)["failed"]["reports"][0];
    assert_eq!(
        (report["code"].as_str(), report["input"]["parameter"].as_str()),
        (Some("cli.file-not-read"), Some("--model"))
    );
}

#[test]
fn a_file_that_is_not_a_model_is_refused_by_the_reader() {
    let dir = scratch("not-a-model");
    let model = dir.join("box.occam");
    std::fs::write(&model, "not a model").unwrap();
    let checked = occam(&["check", "--model", text(&model), "--json"]);
    assert_eq!(checked.status.code(), Some(1));
    assert_eq!(json_out(&checked)["reports"][0]["code"], "io.not-readable");
}

#[test]
fn a_bad_command_line_names_the_argument_in_json() {
    let wrong = occam(&["list", "--model", "m.occam", "--kind", "section", "--json"]);
    assert_eq!(wrong.status.code(), Some(2));
    let report = Report::from_json(stdout(&wrong).trim()).unwrap();
    assert_eq!(report.code, Code::BadArgument);
    let input = report.input.clone().unwrap();
    assert_eq!((input.parameter.as_str(), input.given.as_deref()), ("--kind <KIND>", Some("section")));
    assert!(input.expected.unwrap().contains("edge-use"));
    let id = occam(&["describe", "--model", "m.occam", "edge-41", "--json"]);
    let input = Report::from_json(stdout(&id).trim()).unwrap().input.clone().unwrap();
    assert_eq!(input.given.as_deref(), Some("edge-41"));
    assert!(input.expected.unwrap().contains("edge:41/1"));
}

#[test]
fn help_lists_every_command() {
    let help = occam(&["--help"]);
    assert_eq!(help.status.code(), Some(0));
    for command in ["run", "list", "adjacent", "describe", "check", "mesh", "distance"] {
        assert!(stdout(&help).contains(&format!("  {command} ")), "{command} in {}", stdout(&help));
    }
}

#[test]
fn a_kind_given_to_list_without_its_flag_is_answered_with_the_flag_to_use() {
    // The blind run's c2 and c3 each wrote `list body`, and were told only that `body` was unexpected.
    let words = occam(&["list", "--model", "m.occam", "body"]);
    assert_eq!(words.status.code(), Some(2));
    let said = String::from_utf8(words.stderr).unwrap();
    assert!(said.contains("--kind body"), "the flag to use: {said}");
    let json = occam(&["list", "--model", "m.occam", "body,face", "--json"]);
    assert_eq!(json.status.code(), Some(2));
    let report = Report::from_json(stdout(&json).trim()).unwrap();
    let input = report.input.clone().unwrap();
    assert_eq!(
        (report.code, input.parameter.as_str(), input.given.as_deref()),
        (Code::BadArgument, "KIND", Some("body,face"))
    );
    assert_eq!(input.expected.as_deref(), Some("--kind body,face"));
}

/// The counts a buffers file `bytes` begins with, and its table of faces, six numbers a face, checking its length.
fn buffers(bytes: &[u8]) -> ([u64; 3], Vec<[u32; 6]>) {
    let count = |k: usize| u64::from_le_bytes(bytes.get(8 * k..8 * k + 8).unwrap().try_into().unwrap());
    let [vertices, triangles, faces] = [0, 1, 2].map(count).map(|n| usize::try_from(n).unwrap());
    let table = 24 + 8 * 8 * vertices + 4 * 3 * triangles;
    assert_eq!(bytes.len(), table + 4 * 6 * faces, "8 numbers of 8 bytes a vertex, 3 of 4 a triangle, 6 of 4 a face");
    let word = |at: usize| u32::from_le_bytes(bytes.get(at..at + 4).unwrap().try_into().unwrap());
    let rows = (0..faces).map(|k| std::array::from_fn(|i| word(table + 4 * (6 * k + i)))).collect();
    ([0, 1, 2].map(count), rows)
}

#[test]
fn mesh_writes_the_buffers_beside_the_model_and_gives_one_result_per_face() {
    let dir = scratch("mesh");
    let model = boxed(&dir);
    let meshed = occam(&["mesh", "--model", text(&model), "--deflection", "0.1", "--json"]);
    assert_eq!(meshed.status.code(), Some(0), "{}", stdout(&meshed));
    let value = json_out(&meshed);
    let file = dir.join("box.occam.mesh");
    assert_eq!((&value["ok"], &value["file"]), (&json!(true), &json!(text(&file))));
    let answers = value["answers"].as_array().unwrap();
    let ids: Vec<&str> = answers.iter().map(|a| a["entity"].as_str().unwrap()).collect();
    assert_eq!(ids, (0..6).map(|k| format!("face:{k}/1")).collect::<Vec<_>>(), "one result per face");
    let (counts, faces) = buffers(&std::fs::read(&file).unwrap());
    assert_eq!(counts, [24, 12, 6]);
    assert_eq!((&value["vertices"], &value["triangles"]), (&json!(24), &json!(12)));
    for (row, answer) in faces.iter().zip(answers) {
        assert_eq!(answer["entity"], format!("face:{}/{}", row[0], row[1]));
        let range = |name: &str| [&answer["mesh"][name]["first"], &answer["mesh"][name]["count"]].map(Value::as_u64);
        let want = |k: usize| [Some(u64::from(row[k])), Some(u64::from(row[k + 1]))];
        assert_eq!((range("vertices"), range("triangles")), (want(2), want(4)), "{answer}");
    }
    let elsewhere = dir.join("elsewhere.bin");
    let again = occam(&["mesh", "--model", text(&model), "--deflection", "0.1", "--out", text(&elsewhere)]);
    assert_eq!(again.status.code(), Some(0));
    assert_eq!(std::fs::read(&elsewhere).unwrap(), std::fs::read(&file).unwrap(), "the same buffers");
    let said = "Meshed 6 of 6 faces to a deflection of 0.1 mm and an angle of 0.35 rad: 24 vertices and 12 triangles.";
    assert_eq!(stdout(&again), format!("{said}\nThe buffers are in {}.\n", text(&elsewhere)));
}

#[test]
fn mesh_refuses_limits_that_are_not_positive_and_writes_nothing() {
    let dir = scratch("mesh-limits");
    let model = boxed(&dir);
    let refused = occam(&["mesh", "--model", text(&model), "--deflection", "0", "--json"]);
    assert_eq!(refused.status.code(), Some(1));
    let report = &json_out(&refused)["reports"][0];
    assert_eq!(
        (&report["code"], &report["input"]["parameter"]),
        (&json!("mesh.limit-not-positive"), &json!("deflection"))
    );
    assert!(!dir.join("box.occam.mesh").exists(), "nothing is written");
    let unwritable = occam(&["mesh", "--model", text(&model), "--deflection", "0.1", "--out", text(&dir), "--json"]);
    assert_eq!(unwritable.status.code(), Some(1));
    let report = &json_out(&unwritable)["reports"][0];
    assert_eq!((&report["code"], &report["input"]["parameter"]), (&json!("cli.file-not-written"), &json!("--out")));
}
