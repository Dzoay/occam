//! Source: original
//! Picking, the selection and the highlight as an agent and the page reach them (docs/architecture/live-view.md):
//! `occam pick` answers the id a ray hits; the page posts its selection to `occam view`, which writes it beside the
//! model and refuses one for an older model; `occam selection` describes it or calls it stale; `occam run` carries it
//! over through its changes; and `occam highlight` writes the ids the page shows, which a run clears. Every viewer a
//! test starts is stopped when the test ends.
#![cfg(not(target_family = "wasm"))]
#![allow(clippy::unwrap_used, reason = "a failed step should fail the test")]
#![allow(
    clippy::indexing_slicing,
    clippy::string_slice,
    reason = "the helpers read the answers and streams a test is given, where a step that fails should fail the test"
)]

use std::io::{BufRead as _, BufReader, Read as _, Write as _};
use std::net::{Ipv4Addr, TcpStream};
use std::path::{Path, PathBuf};
use std::process::Command;
use std::time::{Duration, Instant};

use occam::{Code, RemedyKind, Report, followed_remedy};
use serde_json::{Value, json};

/// A box of 10 by 20 by 30 from the origin, and beside it a triangle made a face, a sheet of its own.
const PARTS: &str = r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
  {"op": "box", "size": [10, 20, 30]},
  {"op": "wire", "label": "w", "points": [[20, 0, 0], [24, 0, 0], [24, 3, 0]], "closed": true},
  {"op": "face", "outer": "w"}
]}"#;

/// What `occam` answers to `arguments`, with `--json`: its exit status and its JSON.
fn occam(arguments: &[&str]) -> (Option<i32>, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_occam")).args(arguments).arg("--json").output().unwrap();
    let text = String::from_utf8(output.stdout).unwrap();
    (output.status.code(), serde_json::from_str(&text).map_err(|e| format!("{e}: {text}")).unwrap())
}

/// A model file of its own for the test `name`, in an empty directory, built by `occam run` of `request`: the file
/// and what the run answered.
fn made(name: &str, request: &str) -> (PathBuf, Value) {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("select-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    let model = dir.join("part.occam");
    (model.clone(), ran(&model, request))
}

/// `occam run` of `request` on `model`, which must go through: what it answered.
fn ran(model: &Path, request: &str) -> Value {
    let file = model.with_extension("ops.json");
    std::fs::write(&file, request).unwrap();
    let (status, ran) = occam(&["run", "--model", text(model), text(&file)]);
    assert_eq!((status, &ran["ok"]), (Some(0), &json!(true)), "{ran}");
    ran
}

fn text(path: &Path) -> &str {
    path.to_str().unwrap()
}

/// A request of `steps`.
fn steps(steps: &Value) -> String {
    json!({"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": steps}).to_string()
}

/// The ids an answer's `answers` give, in order.
fn entities(answer: &Value) -> Vec<&str> {
    answer["answers"].as_array().unwrap().iter().map(|a| a["entity"].as_str().unwrap()).collect()
}

/// The model's viewer, started for the test and stopped when it is done with it.
struct Viewer {
    model: PathBuf,
    port: u16,
    token: String,
}

impl Viewer {
    fn start(model: &Path) -> Self {
        let (status, said) = occam(&["view", "--no-open", "--idle", "120", "--model", text(model)]);
        assert_eq!(status, Some(0), "{said}");
        let port = u16::try_from(said["port"].as_u64().unwrap()).unwrap();
        Self { model: model.to_path_buf(), port, token: said["token"].as_str().unwrap().to_string() }
    }

    /// The digest of the model the viewer shows, once it is not `old`.
    fn digest(&self, old: Option<&str>) -> String {
        let start = Instant::now();
        loop {
            let (_, said) = occam(&["view", "--status", "--model", text(&self.model)]);
            if let Some(digest) = said["digest"].as_str().filter(|&digest| Some(digest) != old) {
                return digest.to_string();
            }
            assert!(start.elapsed() < Duration::from_secs(10), "the viewer still shows {old:?}");
            std::thread::sleep(Duration::from_millis(50));
        }
    }

    /// What the viewer answers a post of `body` to `route`, as the page posts: the status and the body.
    fn post(&self, route: &str, body: &str) -> (u16, String) {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, self.port)).unwrap();
        let head = format!("POST /{}/{route} HTTP/1.1\r\nHost: localhost\r\nConnection: close\r\n", self.token);
        write!(stream, "{head}Content-Type: application/json\r\nContent-Length: {}\r\n\r\n{body}", body.len()).unwrap();
        let mut answer = String::new();
        stream.read_to_string(&mut answer).unwrap();
        let (head, body) = answer.split_once("\r\n\r\n").unwrap();
        (head.split(' ').nth(1).unwrap().parse().unwrap(), body.to_string())
    }

    /// The page's stream of events.
    fn events(&self) -> BufReader<TcpStream> {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, self.port)).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        write!(stream, "GET /{}/events HTTP/1.1\r\nHost: localhost\r\n\r\n", self.token).unwrap();
        BufReader::new(stream)
    }
}

impl Drop for Viewer {
    fn drop(&mut self) {
        let _ = occam(&["view", "--stop", "--model", text(&self.model)]);
    }
}

/// The data of the next event named `name` on `events`, skipping the others.
fn next(events: &mut BufReader<TcpStream>, name: &str) -> Value {
    let (mut line, mut named) = (String::new(), false);
    loop {
        line.clear();
        assert!(events.read_line(&mut line).unwrap() > 0, "the stream ended before a {name} event");
        if let Some(event) = line.trim_end().strip_prefix("event:") {
            named = event.trim() == name;
        } else if let Some(data) = line.trim_end().strip_prefix("data:").filter(|_| named) {
            return serde_json::from_str(data.trim()).unwrap();
        }
    }
}

/// The selection the page would post: `ids` on the model of `digest`.
fn selection(digest: &str, ids: &[&str]) -> String {
    json!({"schema": 2, "digest": digest, "ids": ids}).to_string()
}

#[test]
fn occam_pick_answers_the_id_a_ray_hits_and_one_that_misses_follows_its_remedy_to_a_hit() {
    let (model, _) = made("pick", PARTS);
    let pick = |extra: &[&str]| {
        let mut arguments = vec!["pick", "--model", text(&model), "--from", "5,10,50"];
        arguments.extend(extra);
        occam(&arguments)
    };
    let (status, top) = pick(&["--direction", "0,0,-1"]);
    assert_eq!(
        (status, &top["ok"], &top["point"], &top["distance"]),
        (Some(0), &json!(true), &json!([5.0, 10.0, 30.0]), &json!(20.0))
    );
    let (_, described) = occam(&["describe", "--model", text(&model), top["entity"].as_str().unwrap()]);
    assert_eq!(described["answers"][0]["description"]["form"], "plane", "{described}");
    let (_, towards) = pick(&["--towards", "5,10,0"]);
    assert_eq!(towards["entity"], top["entity"], "towards a point, the same ray");
    let (_, edge) = pick(&["--direction", "-0,-10.2,-20", "--tolerance", "0.5"]);
    assert!(edge["entity"].as_str().unwrap().starts_with("edge:"), "{edge}");
    let (status, missed) = pick(&["--direction", "0,0,1"]);
    assert_eq!((status, &missed["entity"]), (Some(1), &Value::Null), "{missed}");
    let report = Report::from_json(&missed["reports"][0].to_string()).unwrap();
    followed_remedy!(report, Code::RayMissed, RemedyKind::AimAt, |remedy| {
        assert!(remedy.value("point").is_some_and(|value| value.number().is_none()), "a point: {remedy:?}");
        let values = missed["reports"][0]["remedies"][0]["values"].as_array().unwrap();
        let point = values.iter().find(|value| value["name"] == "point").unwrap()["value"].as_array().unwrap();
        let point: Vec<String> = point.iter().map(|x| x.as_f64().unwrap().to_string()).collect();
        let (_, again) = pick(&["--towards", &point.join(",")]);
        again["entity"].as_str().map(|_| ()).ok_or_else(|| Report::from_json(&again["reports"][0].to_string()).unwrap())
    });
    let words = Command::new(env!("CARGO_BIN_EXE_occam"))
        .args(["pick", "--model", text(&model), "--from", "5,10,50", "--direction", "0,0,-1"])
        .output()
        .unwrap();
    let said = String::from_utf8(words.stdout).unwrap();
    assert_eq!(said, format!("The ray hits {} at (5, 10, 30) mm, 20 mm along it.\n", top["entity"].as_str().unwrap()));
    let (status, refused) = pick(&["--direction", "0,0,0"]);
    assert_eq!((status, &refused["reports"][0]["code"]), (Some(1), &json!("input.no-direction")));
}

#[test]
fn a_selection_the_page_posts_is_described_by_occam_selection_and_one_for_an_older_model_is_refused() {
    let (model, _) = made("posted", PARTS);
    let (status, none) = occam(&["selection", "--model", text(&model)]);
    assert_eq!(
        (status, none),
        (Some(0), json!({"schema": 2, "ok": true, "answers": [], "reports": []})),
        "nothing yet"
    );
    let viewer = Viewer::start(&model);
    let digest = viewer.digest(None);
    let mut events = viewer.events();
    assert_eq!(next(&mut events, "selection"), json!({"schema": 2, "digest": null, "ids": []}), "none at first");
    let (status, kept) = viewer.post("selection", &selection(&digest, &["face:1/1", "edge:3/1"]));
    assert_eq!(status, 200, "{kept}");
    assert_eq!(next(&mut events, "selection")["ids"], json!(["face:1/1", "edge:3/1"]), "every page is told");
    let file = std::fs::read_to_string(model.with_extension("occam.selection")).unwrap();
    assert_eq!(
        serde_json::from_str::<Value>(&file).unwrap(),
        json!({"schema": 2, "digest": digest, "ids": ["face:1/1", "edge:3/1"]})
    );
    let (status, described) = occam(&["selection", "--model", text(&model)]);
    assert_eq!((status, entities(&described)), (Some(0), vec!["face:1/1", "edge:3/1"]), "{described}");
    assert_eq!(described["answers"][0]["description"]["form"], "plane");
    let (status, old) = viewer.post("selection", &selection("0123456789abcdef", &["face:0/1"]));
    assert_eq!(status, 409, "{old}");
    let report = Report::from_json(&old).unwrap();
    assert_eq!((report.code, report.remedies[0].kind), (Code::SelectionStale, RemedyKind::SelectAgain));
    assert_eq!(entities(&occam(&["selection", "--model", text(&model)]).1), ["face:1/1", "edge:3/1"], "kept as it was");
    assert_eq!(viewer.post("selection", "not a selection").0, 400);
}

#[test]
fn a_selection_made_stale_is_refused_and_selecting_again_describes_it() {
    let (model, _) = made("stale", PARTS);
    let viewer = Viewer::start(&model);
    let digest = viewer.digest(None);
    assert_eq!(viewer.post("selection", &selection(&digest, &["face:2/1"])).0, 200);
    let other = model.with_extension("other.occam");
    ran(&other, &steps(&json!([{"op": "sphere", "radius": 3}])));
    std::fs::copy(&other, &model).unwrap();
    let (status, stale) = occam(&["selection", "--model", text(&model)]);
    assert_eq!((status, &stale["answers"]), (Some(1), &json!([])), "{stale}");
    let report = Report::from_json(&stale["reports"][0].to_string()).unwrap();
    followed_remedy!(report, Code::SelectionStale, RemedyKind::SelectAgain, |_| {
        let now = viewer.digest(Some(&digest));
        let (status, said) = viewer.post("selection", &selection(&now, &["face:0/1"]));
        assert_eq!(status, 200, "{said}");
        let (status, described) = occam(&["selection", "--model", text(&model)]);
        if let Some(refusal) = described["reports"].get(0) {
            return Err(Report::from_json(&refusal.to_string()).unwrap());
        }
        assert_eq!((status, entities(&described)), (Some(0), vec!["face:0/1"]));
        Ok(())
    });
}

#[test]
fn a_run_that_copies_the_selected_body_leaves_the_selection_on_the_original_and_one_that_sweeps_it_away_drops_it() {
    let (model, built) = made("carried", PARTS);
    let face =
        built["steps"][2]["created"].as_array().unwrap().iter().find(|e| e.as_str().unwrap().starts_with("face:"));
    let face = face.unwrap().as_str().unwrap().to_string();
    let viewer = Viewer::start(&model);
    let digest = viewer.digest(None);
    assert_eq!(viewer.post("selection", &selection(&digest, &["face:0/1", &face])).0, 200);
    let copied = ran(&model, &steps(&json!([{"op": "copy", "target": "body:0/1", "frame": {"origin": [0, 50, 0]}}])));
    let kept = json!([{"entity": "face:0/1", "now": ["face:0/1"]}, {"entity": face, "now": [face]}]);
    assert_eq!(copied["selection"], kept, "the copy is new, and the original stays selected: {copied}");
    let (_, described) = occam(&["selection", "--model", text(&model)]);
    assert_eq!(entities(&described), ["face:0/1", face.as_str()], "on the model the run made: {described}");
    let file = model.with_extension("ops.json");
    std::fs::write(&file, steps(&json!([{"op": "extrude", "face": face, "vector": [0, 0, 5]}]))).unwrap();
    let words = Command::new(env!("CARGO_BIN_EXE_occam")).args(["run", "--model", text(&model), text(&file)]).output();
    let said = String::from_utf8(words.unwrap().stdout).unwrap();
    assert!(
        said.ends_with(&format!(
            "face:0/1 stays selected.\n{face} was selected and is gone, so the selection drops it.\n"
        )),
        "{said}"
    );
    let (status, left) = occam(&["selection", "--model", text(&model)]);
    assert_eq!((status, entities(&left)), (Some(0), vec!["face:0/1"]), "the face swept away dropped out");
}

#[test]
fn occam_highlight_writes_the_ids_the_page_is_sent_and_a_run_clears_them() {
    let (model, _) = made("highlight", PARTS);
    let viewer = Viewer::start(&model);
    let digest = viewer.digest(None);
    let mut events = viewer.events();
    assert_eq!(next(&mut events, "highlight")["ids"], json!([]));
    let (status, shown) = occam(&["highlight", "--model", text(&model), "face:4/1", "face:99/1", "edge:0/1"]);
    assert_eq!(status, Some(1), "one id names nothing: {shown}");
    assert_eq!(shown["answers"][1]["report"]["code"], "topo.stale-id");
    assert_eq!(next(&mut events, "highlight"), json!({"schema": 2, "digest": digest, "ids": ["face:4/1", "edge:0/1"]}));
    let moved =
        ran(&model, &steps(&json!([{"op": "transform", "target": "body:0/1", "frame": {"origin": [1, 0, 0]}}])));
    assert_eq!(moved["highlight-cleared"], json!(true), "{moved}");
    assert!(!model.with_extension("occam.highlight").exists());
    assert_eq!(next(&mut events, "highlight")["ids"], json!([]), "the page is told");
    let (status, _) = occam(&["highlight", "--model", text(&model), "face:4/1"]);
    assert_eq!(status, Some(0));
    let (status, cleared) = occam(&["highlight", "--model", text(&model), "--clear"]);
    assert_eq!((status, &cleared["ok"]), (Some(0), &json!(true)));
    assert!(!model.with_extension("occam.highlight").exists(), "cleared");
    let measured = ran(&model, &steps(&json!([{"op": "measure", "target": "body:0/1"}])));
    assert!(measured.get("highlight-cleared").is_none(), "a run that changes nothing clears nothing: {measured}");
}
