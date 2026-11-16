//! Source: original
//! The exit test of `construction` (docs/plan/construction.md): a solid built from JSON alone through the `occam`
//! command, the bracket test's plate with its cylinder beside it, then listed, described, measured and checked. It drives
//! the built binary as an agent would, with JSON files and the JSON it answers, and takes every id from those answers.
//! The occam skill an agent is given (docs/plan/view.md) is held to the commands: every example in it runs as it says.
#![cfg(not(target_family = "wasm"))]
#![allow(clippy::unwrap_used, reason = "a failed step should fail the test")]
#![allow(
    clippy::indexing_slicing,
    reason = "the command lines and answers a test reads, where a step that fails should fail the test"
)]

use std::io::{Read as _, Write as _};
use std::net::TcpStream;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use serde_json::{Value, json};

/// The plate of the bracket test (docs/architecture/bracket-test.md): a profile 60 mm by 10 mm in the xz plane, made a
/// face and extruded 40 mm along y, and the cylinder its bolt hole is to be cut with, as tall as the plate and standing
/// in it. Until `booleans` the cylinder stays a body of its own beside the plate.
const PLATE: &str = r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
  {"op": "wire", "label": "profile", "points": [[0, 0, 0], [60, 0, 0], [60, 0, 10], [0, 0, 10]], "closed": true},
  {"op": "face", "label": "section", "outer": "profile"},
  {"op": "extrude", "label": "plate", "face": "section", "vector": [0, 40, 0]},
  {"op": "cylinder", "label": "bolt", "radius": 4, "height": 10, "frame": {"origin": [30, 20, 0]}}
]}"#;

/// What `occam` answers to `arguments`: its exit status, and its standard output read as JSON.
fn occam(arguments: &[&str]) -> (Option<i32>, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_occam")).args(arguments).arg("--json").output().unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    let value = serde_json::from_str(&text).map_err(|e| format!("{e}: {text}")).unwrap();
    (output.status.code(), value)
}

/// What `occam` says in words to `arguments`, on standard output.
fn words(arguments: &[&str]) -> String {
    let output = Command::new(env!("CARGO_BIN_EXE_occam")).args(arguments).output().unwrap();
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    String::from_utf8(output.stdout).unwrap()
}

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

/// The texts of a JSON list.
fn texts(value: &Value) -> Vec<String> {
    value.as_array().unwrap().iter().map(|v| v.as_str().unwrap().to_string()).collect()
}

/// What `value` holds at `pointer`, such as `/steps/2/made`.
fn at<'a>(value: &'a Value, pointer: &str) -> &'a Value {
    value.pointer(pointer).ok_or_else(|| format!("no {pointer} in {value}")).unwrap()
}

/// The value named `name` among a description's or a measure's `values`.
fn named<'a>(values: &'a Value, name: &str) -> &'a Value {
    let found = values.as_array().unwrap().iter().find(|v| v.get("name").and_then(Value::as_str) == Some(name));
    at(found.ok_or_else(|| format!("no {name} in {values}")).unwrap(), "/value")
}

/// `got` is `want` to a part in 10⁹ of `scale`.
fn near(got: &Value, want: f64, scale: f64) -> bool {
    got.as_f64().is_some_and(|got| (got - want).abs() <= 1e-9 * scale)
}

/// The plate and the cylinder, built by one `occam run` into `model`: their ids, and the run's answer.
fn built(dir: &Path, model: &Path) -> (String, String, Value) {
    let request = dir.join("plate.json");
    std::fs::write(&request, PLATE).unwrap();
    let (status, ran) = occam(&["run", "--model", text(model), text(&request)]);
    assert_eq!((status, at(&ran, "/ok")), (Some(0), &json!(true)), "{ran}");
    let operations: Vec<&str> =
        at(&ran, "/steps").as_array().unwrap().iter().map(|s| at(s, "/operation").as_str().unwrap()).collect();
    assert_eq!(operations, ["wire", "face", "extrude", "cylinder"]);
    let (plate, bolt) = (at(&ran, "/steps/2/made").as_str().unwrap(), at(&ran, "/steps/3/made").as_str().unwrap());
    assert!(plate.starts_with("body:") && bolt.starts_with("body:"), "{plate}, {bolt}");
    assert_eq!(at(&ran, "/tolerance/from"), "default");
    (plate.to_string(), bolt.to_string(), ran)
}

/// The model's entities by kind, as `occam list` gives them.
fn listed(model: &Path) -> Value {
    let (status, listed) = occam(&["list", "--model", text(model), "--kind", "body,shell,face,edge,vertex"]);
    assert_eq!((status, at(&listed, "/ok")), (Some(0), &json!(true)), "{listed}");
    listed
}

/// The edge `occam describe` finds among `edges` running from `start` to `finish`, a line, and its description.
fn line_from(model: &Path, edges: &[String], (start, finish): ([f64; 3], [f64; 3])) -> (String, Value) {
    let mut arguments = vec!["describe", "--model", text(model)];
    arguments.extend(edges.iter().map(String::as_str));
    let (status, described) = occam(&arguments);
    assert_eq!(status, Some(0), "{described}");
    let answers = at(&described, "/answers").as_array().unwrap();
    assert_eq!(answers.len(), edges.len(), "one answer per id");
    let is = |answer: &&Value| {
        let values = at(answer, "/description/values");
        at(answer, "/description/form") == "line"
            && named(values, "start") == &json!(start)
            && named(values, "finish") == &json!(finish)
    };
    let found: Vec<&Value> = answers.iter().filter(is).collect();
    assert_eq!(found.len(), 1, "one line from {start:?} to {finish:?}");
    let line = found.first().unwrap();
    (at(line, "/entity").as_str().unwrap().to_string(), at(line, "/description").clone())
}

/// What `occam run` measures of `plate` and `bolt` in `model`, which it leaves as it was.
fn measured(dir: &Path, model: &Path, (plate, bolt): (&str, &str)) -> (Value, Value) {
    let request = dir.join("measure.json");
    let steps = json!([{"op": "measure", "target": plate}, {"op": "measure", "target": bolt, "what": ["volume"]}]);
    let measure = json!({"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": steps});
    std::fs::write(&request, measure.to_string()).unwrap();
    let before = std::fs::read(model).unwrap();
    let (status, ran) = occam(&["run", "--model", text(model), text(&request)]);
    assert_eq!((status, at(&ran, "/ok")), (Some(0), &json!(true)), "{ran}");
    assert_eq!(std::fs::read(model).unwrap(), before, "a measure changes nothing");
    (at(&ran, "/steps/0/measured").clone(), at(&ran, "/steps/1/measured").clone())
}

#[test]
fn a_solid_is_built_from_json_alone_then_listed_described_measured_and_checked() {
    let dir = scratch("bracket");
    let model = dir.join("bracket.occam");
    let (plate, bolt, ran) = built(&dir, &model);

    let listed = listed(&model);
    let count = |k: usize| listed["answers"][k]["entities"].as_array().unwrap().len();
    assert_eq!(texts(&listed["answers"][0]["entities"]), [plate.clone(), bolt.clone()]);
    let counts = [1, 2, 3, 4].map(count);
    assert_eq!(counts, [2, 10, 18, 12], "shells, faces, edges, vertices: the plate's 6, 12, 8, the cylinder's 4, 6, 4");

    let edges = texts(&listed["answers"][3]["entities"]);
    let (edge, described) = line_from(&model, &edges, ([0.0, 0.0, 10.0], [0.0, 40.0, 10.0]));
    let made = ran["steps"][2]["described"].as_array().unwrap().iter().find(|d| d["entity"] == edge.as_str());
    let made = &made.unwrap()["origin"];
    assert_eq!((&made["operation"], &made["request-step"]), (&json!("extrude"), &json!(2)), "made by the extrude");
    assert_eq!(described["origin"]["step"], made["step"], "described later as the step that made it");
    let later = (&described["origin"]["operation"], &described["origin"]["request-step"]);
    assert_eq!(later, (&json!("extrude"), &json!(2)), "and numbered and named as run gave it, after the run");

    let (_, near_edge) = occam(&["adjacent", "--model", text(&model), &edge]);
    let faces: Vec<String> =
        texts(&near_edge["answers"][0]["adjacent"]).into_iter().filter(|n| n.starts_with("face:")).collect();
    let (_, shells) = occam(&["adjacent", "--model", text(&model), &plate]);
    let shell = texts(&shells["answers"][0]["adjacent"]).into_iter().find(|n| n.starts_with("shell:")).unwrap();
    let (_, plate_faces) = occam(&["adjacent", "--model", text(&model), &shell]);
    let plate_faces = texts(&plate_faces["answers"][0]["adjacent"]);
    assert_eq!(faces.len(), 2, "an edge of a solid joins two faces: {faces:?}");
    assert!(faces.iter().all(|f| plate_faces.contains(f)), "both the plate's: {faces:?} in {plate_faces:?}");

    let (of_plate, of_bolt) = measured(&dir, &model, (&plate, &bolt));
    assert_eq!((&of_plate["target"], &of_bolt["target"]), (&json!(plate), &json!(bolt)));
    assert!(near(named(&of_plate["values"], "volume"), 24_000.0, 24_000.0), "{of_plate}");
    assert!(near(named(&of_plate["values"], "area"), 6_800.0, 6_800.0), "{of_plate}");
    let centre = named(&of_plate["values"], "centre-of-mass").as_array().unwrap().clone();
    assert!(centre.iter().zip([30.0, 20.0, 5.0]).all(|(got, want)| near(got, want, 60.0)), "{centre:?}");
    let cylinder = std::f64::consts::PI * 16.0 * 10.0;
    assert!(near(named(&of_bolt["values"], "volume"), cylinder, cylinder), "{of_bolt}");

    let (status, checked) = occam(&["check", "--model", text(&model)]);
    assert_eq!((status, checked), (Some(0), json!({"schema": 2, "ok": true, "answers": [], "reports": []})));
}

#[test]
fn an_agent_picks_the_cylinders_side_and_describe_says_it_is_a_cylinder_of_radius_4() {
    let dir = scratch("pick-the-side");
    let model = dir.join("bracket.occam");
    let (plate, bolt, _) = built(&dir, &model);
    let (_, of_bolt) = measured(&dir, &model, (&plate, &bolt));
    let request = dir.join("centre.json");
    let centre = json!({"op": "measure", "target": bolt, "what": ["centre-of-mass"]});
    std::fs::write(
        &request,
        json!({"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [centre]}).to_string(),
    )
    .unwrap();
    let (_, ran) = occam(&["run", "--model", text(&model), text(&request)]);
    let axis: Vec<f64> =
        at(&ran, "/steps/0/measured/values/0/value").as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    assert!(near(named(&of_bolt["values"], "volume"), std::f64::consts::PI * 160.0, 503.0), "{of_bolt}");
    // The cylinder stands inside the plate, so the ray starts inside the plate too, 15 mm off the cylinder's axis,
    // on the side away from the seam its side is closed along at +x, which a ray along x would hit.
    let from = format!("{},{},{}", axis[0], axis[1] + 15.0, axis[2]);
    let towards = format!("{},{},{}", axis[0], axis[1], axis[2]);
    let (status, picked) = occam(&["pick", "--model", text(&model), "--from", &from, "--towards", &towards]);
    assert_eq!((status, at(&picked, "/ok")), (Some(0), &json!(true)), "{picked}");
    let side = at(&picked, "/entity").as_str().unwrap();
    let distance = at(&picked, "/distance").as_f64().unwrap();
    assert!((distance - 11.0).abs() <= 0.1, "4 mm short of the axis, within the mesh's deflection: {picked}");
    let (status, described) = occam(&["describe", "--model", text(&model), side]);
    assert_eq!(status, Some(0), "{described}");
    let description = at(&described, "/answers/0/description");
    assert_eq!(at(description, "/form"), "cylinder", "{described}");
    assert!(near(named(at(description, "/values"), "radius"), 4.0, 4.0), "{described}");
    assert!(texts(at(description, "/adjacent")).iter().any(|near| near.starts_with("shell:")), "{described}");
}

#[test]
fn an_agent_learns_to_write_a_request_from_the_command_alone() {
    let help = words(&["--help"]);
    assert!(help.contains("`occam run --help` shows how to write the request"), "{help}");
    let run = words(&["run", "--help"]);
    assert!(run.contains(&occam::guide()), "run's help is the guide: {run}");
    for operation in occam::Operation::ALL.into_iter().filter(|operation| operation.supported()) {
        assert!(run.contains(&format!("\n  {:<10} ", operation.name())), "{operation:?} in {run}");
    }
    let guide = occam::guide();
    let examples: Vec<String> = guide
        .split("\n  {\"schema\"")
        .skip(1)
        .map(|rest| format!("{{\"schema\"{}\n  ]}}", rest.split("\n  ]}").next().unwrap()))
        .collect();
    assert_eq!(examples.len(), 2, "a box measured, then wires written in place and a revolve: {guide}");
    let ran: Vec<Value> =
        examples.iter().enumerate().map(|(k, example)| ran_alone(&format!("example{k}"), example)).collect();
    assert!(near(named(&ran[0]["steps"][1]["measured"]["values"], "volume"), 8_000.0, 8_000.0), "{}", ran[0]);
    let made: Vec<&str> = [1, 3].map(|k| ran[1]["steps"][k]["made"].as_str().unwrap_or_default()).to_vec();
    assert!(made.iter().all(|m| m.starts_with("body:")), "the extrude and the revolve each make a body: {}", ran[1]);
}

/// `request` run alone on a new model in its own scratch directory `name`, which must go through.
fn ran_alone(name: &str, request: &str) -> Value {
    let dir = scratch(name);
    let (model, path) = (dir.join("example.occam"), dir.join("example.json"));
    std::fs::write(&path, request).unwrap();
    let (status, ran) = occam(&["run", "--model", text(&model), text(&path)]);
    assert_eq!((status, at(&ran, "/ok")), (Some(0), &json!(true)), "the guide's own example runs: {request} {ran}");
    ran
}

#[test]
fn every_number_said_in_words_is_short_enough_to_read() {
    let dir = scratch("numbers-in-words");
    let model = dir.join("bracket.occam");
    let (plate, bolt, _) = built(&dir, &model);
    let listed = listed(&model);
    let mut arguments = vec!["describe", "--model", text(&model), &plate, &bolt];
    let all: Vec<String> = (2..5).flat_map(|k| texts(&listed["answers"][k]["entities"])).collect();
    arguments.extend(all.iter().map(String::as_str));
    let said = words(&arguments);
    let numbers = said.split(|c: char| !(c.is_ascii_digit() || matches!(c, '.' | 'e' | '-')));
    let longest = numbers.max_by_key(|n| n.len()).unwrap_or_default();
    assert!(longest.len() <= 24, "{longest} in {said}");
}

/// A flat disc of radius 1000 mm: at a deflection of 1e-6 mm its circle would need more samples than an edge may have.
const DISC: &str = r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
  {"op": "wire", "label": "rim", "edges": [{"circle": {"centre": [0, 0, 0], "radius": 1000, "normal": [0, 0, 1]}}]},
  {"op": "face", "label": "disc", "outer": "rim"}
]}"#;

#[test]
fn an_agent_follows_the_remedies_of_a_face_and_an_edge_that_will_not_mesh() {
    let dir = scratch("mesh-remedy");
    let (model, request) = (dir.join("disc.occam"), dir.join("disc.json"));
    std::fs::write(&request, DISC).unwrap();
    let (status, ran) = occam(&["run", "--model", text(&model), text(&request)]);
    assert_eq!(status, Some(0), "{ran}");
    let (status, refused) = occam(&["mesh", "--model", text(&model), "--deflection", "1e-6"]);
    assert_eq!((status, at(&refused, "/ok")), (Some(1), &json!(false)), "{refused}");
    let (face, edge) = (at(&refused, "/answers/0/report"), at(&refused, "/reports/0"));
    assert_eq!((&face["code"], &edge["code"]), (&json!("mesh.face-not-meshed"), &json!("mesh.edge-over-cap")));
    for report in [face, edge] {
        let remedies = at(report, "/remedies").as_array().unwrap();
        let remedy = remedies.iter().find(|r| r["kind"] == "use-deflection").unwrap();
        let deflection = named(&remedy["values"], "deflection").as_f64().unwrap().to_string();
        let (status, meshed) = occam(&["mesh", "--model", text(&model), "--deflection", &deflection]);
        assert_eq!((status, at(&meshed, "/ok")), (Some(0), &json!(true)), "at {deflection}: {meshed}");
        assert!(at(&meshed, "/answers/0/mesh/triangles/count").as_u64() > Some(0), "{meshed}");
    }
}

/// The skill an agent is given, as `occam skill` prints it.
const SKILL: &str = include_str!("../../../skills/occam/SKILL.md");

/// An example in the skill: a request it has the agent write, as the file it names and its text, or a command line.
#[derive(Debug)]
enum Example {
    Request(String, String),
    Command(Vec<String>),
}

/// Every example in `skill`, in order: each `json` block, as the file of `.json` the text before it names last, and
/// each line of each `sh` block.
fn examples(skill: &str) -> Vec<Example> {
    let (mut found, mut named) = (Vec::new(), None);
    let mut lines = skill.lines();
    while let Some(line) = lines.next() {
        let Some(kind) = line.strip_prefix("```") else {
            let file = |part: &&str| Path::new(part).extension().is_some_and(|extension| extension == "json");
            named = line.split('`').rfind(file).map(String::from).or(named);
            continue;
        };
        let block: Vec<&str> = lines.by_ref().take_while(|line| *line != "```").collect();
        assert!(["json", "sh"].contains(&kind), "the skill has a block of {kind:?}");
        if kind == "json" {
            found.push(Example::Request(named.take().unwrap(), block.join("\n")));
        } else {
            found.extend(block.iter().map(|line| Example::Command(line.split(' ').map(String::from).collect())));
        }
    }
    found
}

/// What the viewer of `model` says, once it has read a model whole.
fn viewing(model: &Path) -> Value {
    let start = Instant::now();
    loop {
        let (_, said) = occam(&["view", "--status", "--model", text(model)]);
        if said["digest"].is_string() {
            return said;
        }
        assert!(start.elapsed() < Duration::from_secs(20), "the viewer reads no model: {said}");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The face on top of `model`, a plane whose box is flat at the greatest height, selected as a click in the viewer's
/// page selects it: posted to the viewer with the digest of the model it shows. Its id, and its description.
fn person_selects_the_top(model: &Path) -> (String, Value) {
    let (_, listed) = occam(&["list", "--model", text(model), "--kind", "face"]);
    let faces = texts(at(&listed, "/answers/0/entities"));
    let mut arguments = vec!["describe", "--model", text(model)];
    arguments.extend(faces.iter().map(String::as_str));
    let (_, described) = occam(&arguments);
    let height = |answer: &Value, end: &str| named(at(answer, "/description/values"), end)[2].as_f64().unwrap();
    let flat = |answer: &&Value| {
        answer["description"]["form"] == "plane" && (height(answer, "high") - height(answer, "low")).abs() <= 1e-9
    };
    let answers = at(&described, "/answers").as_array().unwrap();
    let top = answers.iter().filter(flat).max_by(|a, b| height(a, "high").total_cmp(&height(b, "high"))).unwrap();
    let id = at(top, "/entity").as_str().unwrap().to_string();
    let said = viewing(model);
    let body = json!({"schema": 2, "digest": said["digest"], "ids": [id]}).to_string();
    let mut stream = TcpStream::connect(("127.0.0.1", u16::try_from(said["port"].as_u64().unwrap()).unwrap())).unwrap();
    let token = said["token"].as_str().unwrap();
    let head = format!("POST /{token}/selection HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n");
    write!(stream, "{head}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
    let mut answer = String::new();
    stream.read_to_string(&mut answer).unwrap();
    assert!(answer.starts_with("HTTP/1.1 200"), "the viewer takes the selection: {answer}");
    (id, at(top, "/description").clone())
}

/// Stops the viewer of the model when dropped, so that a test that fails leaves none running.
struct Stopped(PathBuf);

impl Drop for Stopped {
    fn drop(&mut self) {
        let _ = Command::new(env!("CARGO_BIN_EXE_occam")).args(["view", "--stop", "--model"]).arg(&self.0).output();
    }
}

/// The middle of a face's box, from its description.
fn middle(description: &Value) -> Vec<f64> {
    let [low, high] = ["low", "high"].map(|end| named(at(description, "/values"), end).as_array().unwrap().clone());
    low.iter().zip(&high).map(|(low, high)| f64::midpoint(low.as_f64().unwrap(), high.as_f64().unwrap())).collect()
}

/// Each command line of `examples`, by what it asks: its command and the flags it gives but `--model`.
fn asked(examples: &[Example]) -> Vec<String> {
    let commands = examples.iter().filter_map(|example| match example {
        Example::Command(line) => Some(line),
        Example::Request(..) => None,
    });
    let flags = |line: &[String]| line.iter().filter(|a| a.starts_with("--") && *a != "--model").cloned().collect();
    commands.map(|line| [vec![line[1].clone()], flags(line)].concat().join(" ")).collect()
}

#[test]
fn every_example_in_the_skill_runs_as_the_skill_says() {
    let dir = scratch("skill-examples");
    let model = dir.join("part.occam");
    let _stopped = Stopped(model.clone());
    let examples = examples(SKILL);
    let order = ["view", "run", "selection", "highlight", "run", "run", "view --stop"];
    assert_eq!(
        asked(&examples),
        order,
        "start the viewer, build, read the selection, highlight, change, measure, stop"
    );
    let mut selected = None;
    for example in &examples {
        let line = match example {
            Example::Request(name, request) => {
                std::fs::write(dir.join(name), request).unwrap();
                continue;
            }
            Example::Command(line) => line,
        };
        assert_eq!(line[..3], ["occam", line[1].as_str(), "--model"], "{line:?}");
        assert_eq!(line[3], "part.occam", "one model throughout: {line:?}");
        if line[1] == "selection" {
            selected = Some(person_selects_the_top(&model));
        }
        if line[1] == "highlight" {
            assert_eq!(line[4..], [selected.clone().unwrap().0], "the skill highlights the face selected");
        }
        let before = model.exists();
        let mut command = Command::new(env!("CARGO_BIN_EXE_occam"));
        let output = command.args(&line[1..]).current_dir(&dir).env("BROWSER", "true").output().unwrap();
        let (out, err) = (String::from_utf8_lossy(&output.stdout), String::from_utf8_lossy(&output.stderr));
        if line[1] == "view" && !before {
            assert_eq!(output.status.code(), Some(1), "{out}{err}");
            assert!(out.contains("is at http://127.0.0.1:") && err.starts_with("There is no model at"), "{out}{err}");
        } else {
            assert_eq!(output.status.code(), Some(0), "{line:?}: {out}{err}");
        }
        if line[1] == "selection" {
            assert!(out.starts_with(&format!("{} plane:", selected.clone().unwrap().0)), "{out}");
        }
    }
    let (_, description) = selected.unwrap();
    let pin: Value = serde_json::from_str(&std::fs::read_to_string(dir.join("pin.json")).unwrap()).unwrap();
    let origin = at(&pin, "/steps/0/frame/origin").as_array().unwrap();
    let centre = middle(&description);
    assert!(origin.iter().zip(&centre).all(|(got, want)| near(got, *want, 40.0)), "the centre: {centre:?}, {pin}");
    let (_, status) = occam(&["view", "--status", "--model", text(&model)]);
    assert_eq!(status["running"], false, "the skill's last example stops the viewer: {status}");
    let refused = occam::guide().contains("Refused by this build, by name: boolean, fillet, chamfer.");
    assert!(!SKILL.contains("no boolean, fillet or chamfer") || refused, "the skill says what this build refuses");
}
