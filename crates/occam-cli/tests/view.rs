//! Source: original
//! `occam view` as an agent starts it (docs/architecture/live-view.md): the command returns once its viewer listens,
//! one viewer runs for a model, `--status` and `--stop` answer for it, a stale state file is replaced, every address
//! wants the token, the model goes out as the binary twin and each change that reads whole as an event, and a file
//! that is not a model, or is not there, is named while the viewer keeps watching. Every viewer a test starts is
//! stopped when the test ends, and stops itself two minutes after if that fails. A wasm test run cannot start one.
#![cfg(not(target_family = "wasm"))]
#![allow(clippy::unwrap_used, reason = "a failed step should fail the test")]
#![allow(
    clippy::indexing_slicing,
    clippy::string_slice,
    clippy::arithmetic_side_effects,
    reason = "the helpers read the answers and streams a test is given, where a step that fails should fail the test"
)]

use std::io::{Read as _, Write as _};
use std::net::{Ipv4Addr, TcpListener, TcpStream};
use std::path::{Path, PathBuf};
use std::process::{Command, Output};
use std::time::{Duration, Instant};

use occam::{Code, RemedyKind, Report, followed_remedy};
use serde_json::Value;

/// How long a stray viewer of these tests lives with no page open.
const IDLE: &str = "120";

/// A box, and a cylinder beside it, as two requests.
const BOX: &str =
    r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [{"op": "box", "size": [10, 20, 30]}]}"#;
const CYLINDER: &str = r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
  {"op": "cylinder", "radius": 4, "height": 10, "frame": {"origin": [30, 0, 0]}}]}"#;

/// What `occam` does with `arguments`.
fn run_occam(arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_occam")).args(arguments).output().unwrap()
}

/// Its exit status and its JSON, `--json` added.
fn answer(arguments: &[&str]) -> (Option<i32>, Value) {
    let output = Command::new(env!("CARGO_BIN_EXE_occam")).args(arguments).arg("--json").output().unwrap();
    let said = String::from_utf8(output.stdout).unwrap();
    (output.status.code(), serde_json::from_str(&said).map_err(|e| format!("{e}: {said}")).unwrap())
}

/// An empty directory for the test `name`, in the one cargo gives this crate's tests.
fn fresh(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("view-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

fn utf8(path: &Path) -> &str {
    path.to_str().unwrap()
}

/// `request` run on `model` by `occam run`.
fn build(model: &Path, request: &str) {
    let file = model.with_extension("request.json");
    std::fs::write(&file, request).unwrap();
    let ran = run_occam(&["run", "--model", utf8(model), utf8(&file)]);
    assert_eq!(ran.status.code(), Some(0), "{}", String::from_utf8_lossy(&ran.stderr));
}

/// The viewer of a model, started by `occam view`, and stopped when the test is done with it, however it ends.
struct Viewing {
    model: PathBuf,
    status: Option<i32>,
    started: Value,
}

impl Viewing {
    /// `occam view` of `model`, with `extra` arguments, opening no page.
    fn of(model: &Path, extra: &[&str]) -> Self {
        let mut arguments = vec!["view", "--model", utf8(model), "--no-open", "--idle", IDLE];
        arguments.extend(extra);
        let (status, started) = answer(&arguments);
        Self { model: model.to_path_buf(), status, started }
    }

    fn port(&self) -> u16 {
        u16::try_from(self.started["port"].as_u64().unwrap()).unwrap()
    }

    fn token(&self) -> &str {
        self.started["token"].as_str().unwrap()
    }

    /// What `occam view --status` says of it.
    fn status(&self) -> (Option<i32>, Value) {
        answer(&["view", "--model", utf8(&self.model), "--status"])
    }
}

impl Drop for Viewing {
    fn drop(&mut self) {
        let _ = run_occam(&["view", "--model", utf8(&self.model), "--stop"]);
    }
}

/// What the server on `port` answers `method` at `path`: its status, its head and its body.
fn http(port: u16, method: &str, path: &str) -> (u16, String, Vec<u8>) {
    let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, port)).unwrap();
    write!(stream, "{method} {path} HTTP/1.1\r\nHost: localhost\r\nContent-Length: 0\r\nConnection: close\r\n\r\n")
        .unwrap();
    let mut all = Vec::new();
    stream.read_to_end(&mut all).unwrap();
    let split = all.windows(4).position(|w| w == b"\r\n\r\n").unwrap();
    let head = String::from_utf8(all[..split].to_vec()).unwrap();
    let code = head.split(' ').nth(1).unwrap().parse().unwrap();
    (code, head, all[split + 4..].to_vec())
}

/// Waits up to ten seconds for `done`, failing with `what` if it does not come.
fn eventually(what: &str, mut done: impl FnMut() -> bool) {
    let start = Instant::now();
    while !done() {
        assert!(start.elapsed() < Duration::from_secs(10), "{what} within ten seconds");
        std::thread::sleep(Duration::from_millis(50));
    }
}

/// The codes of the reports `value` lists.
fn codes(value: &Value) -> Vec<String> {
    value["reports"].as_array().unwrap().iter().map(|r| r["code"].as_str().unwrap().to_string()).collect()
}

#[test]
fn occam_view_returns_once_its_viewer_listens_and_a_second_prints_the_same_address_starting_nothing() {
    let model = fresh("again").join("part.occam");
    build(&model, BOX);
    let first = Viewing::of(&model, &[]);
    assert_eq!(first.status, Some(0), "{}", first.started);
    assert_eq!((&first.started["running"], &first.started["started"]), (&Value::Bool(true), &Value::Bool(true)));
    let (code, _, _) = http(first.port(), "GET", &format!("/{}/status", first.token()));
    assert_eq!(code, 200, "it listens as soon as the command returns");
    let url = first.started["url"].as_str().unwrap();
    assert_eq!(url, format!("http://127.0.0.1:{}/{}/", first.port(), first.token()));
    let second = Viewing::of(&model, &[]);
    assert_eq!(second.started["started"], Value::Bool(false), "{}", second.started);
    for field in ["url", "token", "pid", "port"] {
        assert_eq!(second.started[field], first.started[field], "{field}");
    }
    let words = run_occam(&["view", "--model", utf8(&model), "--no-open"]);
    assert!(String::from_utf8(words.stdout).unwrap().contains(url), "in words too");
    let state = std::fs::read_to_string(model.with_extension("occam.view")).unwrap();
    assert!(state.contains(&format!("\"token\":\"{}\"", first.token())), "the state file names it: {state}");
}

#[test]
fn stop_stops_the_viewer_and_status_then_says_none_runs() {
    let model = fresh("stop").join("part.occam");
    build(&model, BOX);
    let viewing = Viewing::of(&model, &[]);
    let (status, stopped) = answer(&["view", "--model", utf8(&model), "--stop"]);
    assert_eq!((status, &stopped["stopped"], &stopped["running"]), (Some(0), &Value::Bool(true), &Value::Bool(false)));
    assert!(TcpStream::connect((Ipv4Addr::LOCALHOST, viewing.port())).is_err(), "nothing listens");
    assert!(!model.with_extension("occam.view").exists(), "its state file is gone");
    let (status, none) = viewing.status();
    assert_eq!((status, &none["running"]), (Some(0), &Value::Bool(false)));
    let words = run_occam(&["view", "--model", utf8(&model), "--status"]);
    assert_eq!(String::from_utf8(words.stdout).unwrap(), format!("No viewer runs for {}.\n", utf8(&model)));
}

#[cfg(unix)]
#[test]
fn a_state_file_left_by_a_killed_viewer_is_replaced() {
    let model = fresh("killed").join("part.occam");
    build(&model, BOX);
    let killed = Viewing::of(&model, &[]);
    let pid = killed.started["pid"].to_string();
    assert!(Command::new("kill").args(["-KILL", &pid]).status().unwrap().success());
    eventually("the killed viewer gone", || TcpStream::connect((Ipv4Addr::LOCALHOST, killed.port())).is_err());
    assert!(model.with_extension("occam.view").exists(), "it left its state file");
    assert_eq!(killed.status().1["running"], Value::Bool(false), "the state file is stale");
    let again = Viewing::of(&model, &[]);
    assert_eq!((again.status, &again.started["started"]), (Some(0), &Value::Bool(true)), "{}", again.started);
    assert_ne!(again.token(), killed.token());
    let state = std::fs::read_to_string(model.with_extension("occam.view")).unwrap();
    assert!(state.contains(again.token()), "the state file names the new one: {state}");
}

#[test]
fn every_address_wants_the_token() {
    let model = fresh("token").join("part.occam");
    build(&model, BOX);
    let viewing = Viewing::of(&model, &[]);
    let token = viewing.token();
    for path in ["/", "/model", "/events", "/status", "/viewer.js", "/occam.wasm", "/0123/model", "/0123/"] {
        assert_eq!(http(viewing.port(), "GET", path).0, 403, "{path}");
    }
    assert_eq!(http(viewing.port(), "POST", "/stop").0, 403);
    assert_eq!(viewing.status().1["running"], Value::Bool(true), "a stop without the token stops nothing");
    for path in ["/", "/model", "/status"] {
        assert_eq!(http(viewing.port(), "GET", &format!("/{token}{path}")).0, 200, "{path}");
    }
    assert_eq!(http(viewing.port(), "GET", &format!("/{token}/nothing")).0, 404, "the token and nothing there");
    let (code, head, _) = http(viewing.port(), "GET", &format!("/{token}?inspect"));
    assert_eq!(code, 308, "the address without its last slash is sent to it");
    assert!(head.contains(&format!("location: /{token}/?inspect")), "{head}");
}

#[test]
fn a_build_without_the_recipe_serves_the_stub_page_that_names_it() {
    let model = fresh("stub").join("part.occam");
    build(&model, BOX);
    let viewing = Viewing::of(&model, &[]);
    let (code, head, body) = http(viewing.port(), "GET", &format!("/{}/", viewing.token()));
    let page = String::from_utf8(body).unwrap();
    assert_eq!(code, 200);
    assert!(head.contains("content-type: text/html"), "{head}");
    if env!("OCCAM_PAGE") == "stub" {
        assert!(page.contains("built without its page") && page.contains("just web-build"), "{page}");
    } else {
        assert!(page.contains("<html lang=\"en\" data-live>") && page.contains("viewer.js"), "{page}");
        let (code, head, _) = http(viewing.port(), "GET", &format!("/{}/occam.wasm", viewing.token()));
        assert_eq!(code, 200);
        assert!(head.contains("content-type: application/wasm"), "{head}");
    }
}

/// A page's stream of server-sent events, read one event at a time.
struct Events {
    stream: TcpStream,
    text: String,
}

impl Events {
    /// The stream of events of `viewing`, opened.
    fn of(viewing: &Viewing) -> Self {
        let mut stream = TcpStream::connect((Ipv4Addr::LOCALHOST, viewing.port())).unwrap();
        stream.set_read_timeout(Some(Duration::from_secs(10))).unwrap();
        write!(stream, "GET /{}/events HTTP/1.1\r\nHost: localhost\r\n\r\n", viewing.token()).unwrap();
        Self { stream, text: String::new() }
    }

    /// The next event: its name and its data's JSON.
    fn next(&mut self) -> (String, Value) {
        let mut buffer = [0_u8; 4096];
        loop {
            if let Some(at) = self.text.find("\n\n").filter(|&at| self.text[..at].contains("data:")) {
                let block: String = self.text.drain(..at + 2).collect();
                let field =
                    |name: &str| block.lines().find_map(|line| line.strip_prefix(name)).unwrap().trim().to_string();
                return (field("event:"), serde_json::from_str(&field("data:")).unwrap());
            }
            if let Some(at) = self.text.find("\n\n") {
                self.text.drain(..at + 2);
                continue;
            }
            let read = self.stream.read(&mut buffer).unwrap();
            assert!(read > 0, "the stream ended: {}", self.text);
            self.text.push_str(std::str::from_utf8(&buffer[..read]).unwrap());
        }
    }
}

#[test]
fn the_model_goes_out_as_the_twin_and_each_change_that_reads_whole_as_an_event_naming_its_digest() {
    let model = fresh("events").join("part.occam");
    build(&model, BOX);
    let viewing = Viewing::of(&model, &[]);
    let digest = viewing.started["digest"].as_str().unwrap().to_string();
    let (code, head, twin) = http(viewing.port(), "GET", &format!("/{}/model", viewing.token()));
    assert_eq!(code, 200);
    assert!(twin.starts_with(b"\x89occam"), "the binary twin, though the file is JSON");
    assert!(head.contains(&format!("x-occam-digest: {digest}")), "{head}");
    let mut events = Events::of(&viewing);
    let (name, opened) = events.next();
    assert_eq!((name.as_str(), &opened["digest"]), ("model", &Value::from(digest.clone())));
    assert_eq!(viewing.status().1["pages"], 1, "a page follows it");
    build(&model, CYLINDER);
    let (name, changed) = events.next();
    let next = changed["digest"].as_str().unwrap().to_string();
    assert_eq!(name, "model");
    assert_ne!(next, digest, "another model");
    assert_eq!(viewing.status().1["digest"], next.as_str());
    std::fs::write(&model, b"not a model").unwrap();
    let (name, refused) = events.next();
    assert_eq!((name.as_str(), codes(&refused)), ("refused", vec!["io.not-readable".to_string()]));
    assert_eq!(refused["digest"], next.as_str(), "the last model that read whole is still the one drawn");
}

#[test]
fn a_model_that_is_not_there_is_named_and_the_viewer_draws_it_once_a_run_makes_it() {
    let model = fresh("missing").join("part.occam");
    let viewing = Viewing::of(&model, &[]);
    assert_eq!(viewing.status, Some(1), "something refused");
    assert_eq!((&viewing.started["running"], &viewing.started["digest"]), (&Value::Bool(true), &Value::Null));
    assert_eq!(codes(&viewing.started), ["view.model-missing"]);
    let report = Report::from_json(&viewing.started["reports"][0].to_string()).unwrap();
    followed_remedy!(report, Code::ModelMissing, RemedyKind::MakeModel, |_| {
        build(&model, BOX);
        let mut said = Value::Null;
        eventually("the model drawn", || {
            said = viewing.status().1;
            said["digest"].is_string()
        });
        Report::from_json(&said["reports"][0].to_string()).map_or(Ok(()), Err)
    });
    let words = run_occam(&["view", "--model", utf8(&model), "--status"]);
    assert_eq!((words.status.code(), words.stderr.len()), (Some(0), 0), "nothing refused once it is there");
}

#[test]
fn a_file_that_stops_being_a_model_is_named_and_the_viewer_keeps_watching() {
    let model = fresh("garbage").join("part.occam");
    build(&model, BOX);
    let viewing = Viewing::of(&model, &[]);
    let digest = viewing.started["digest"].clone();
    std::fs::write(&model, b"not a model").unwrap();
    eventually("the garbage named", || codes(&viewing.status().1) == ["io.not-readable"]);
    let words = run_occam(&["view", "--model", utf8(&model), "--status"]);
    assert_eq!(words.status.code(), Some(1));
    let refused = String::from_utf8(words.stderr).unwrap();
    assert!(refused.starts_with("The file is not a native model: it fails at line 1, column "), "{refused}");
    assert_eq!(viewing.status().1["digest"], digest, "the last model that read whole");
    std::fs::remove_file(&model).unwrap();
    eventually("the file's going named", || codes(&viewing.status().1) == ["view.model-missing"]);
    build(&model, BOX);
    eventually("the model back", || codes(&viewing.status().1).is_empty());
    assert_eq!(viewing.status().1["digest"], digest, "the same model again");
}

#[test]
fn a_port_taken_is_named_and_leaving_out_the_port_listens_on_a_free_one() {
    let model = fresh("port").join("part.occam");
    build(&model, BOX);
    let holder = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = holder.local_addr().unwrap().port().to_string();
    let taken = Viewing::of(&model, &["--port", &port]);
    assert_eq!((taken.status, &taken.started["running"]), (Some(1), &Value::Bool(false)), "{}", taken.started);
    assert_eq!(codes(&taken.started), ["view.port-taken"]);
    let report = Report::from_json(&taken.started["reports"][0].to_string()).unwrap();
    followed_remedy!(report, Code::PortTaken, RemedyKind::UseAnyPort, |_| {
        let free = Viewing::of(&model, &[]);
        let first = free.started["reports"].get(0).map(|r| Report::from_json(&r.to_string()));
        first.map_or(Ok(free.port()), |report| Err(report.unwrap()))
    });
}

#[cfg(unix)]
#[test]
fn a_state_file_that_cannot_be_written_starts_no_viewer() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = fresh("unwritable");
    let model = dir.join("part.occam");
    build(&model, BOX);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o555)).unwrap();
    let refused = Viewing::of(&model, &[]);
    std::fs::set_permissions(&dir, std::fs::Permissions::from_mode(0o755)).unwrap();
    assert_eq!((refused.status, &refused.started["running"]), (Some(1), &Value::Bool(false)), "{}", refused.started);
    assert_eq!(codes(&refused.started), ["view.state-not-written"]);
}

#[test]
fn the_viewer_stops_itself_once_no_page_has_been_open_for_its_idle_time() {
    let model = fresh("idle").join("part.occam");
    build(&model, BOX);
    let (status, started) = answer(&["view", "--model", utf8(&model), "--no-open", "--idle", "1"]);
    assert_eq!((status, &started["running"]), (Some(0), &Value::Bool(true)), "{started}");
    assert!(model.with_extension("occam.view").exists(), "it runs");
    eventually("the idle viewer gone", || !model.with_extension("occam.view").exists());
    let (_, none) = answer(&["view", "--model", utf8(&model), "--status"]);
    assert_eq!(none["running"], Value::Bool(false));
}

#[cfg(unix)]
#[test]
fn the_page_opens_in_the_browser_named_unless_asked_not_to() {
    use std::os::unix::fs::PermissionsExt as _;
    let dir = fresh("browser");
    let model = dir.join("part.occam");
    build(&model, BOX);
    let (browser, opened) = (dir.join("browser"), dir.join("opened"));
    std::fs::write(&browser, format!("#!/bin/sh\necho \"$1\" >> '{}'\n", utf8(&opened))).unwrap();
    std::fs::set_permissions(&browser, std::fs::Permissions::from_mode(0o755)).unwrap();
    let with = |extra: &[&str]| {
        let mut command = Command::new(env!("CARGO_BIN_EXE_occam"));
        command.args(["view", "--model", utf8(&model), "--idle", IDLE, "--json"]).args(extra).env("BROWSER", &browser);
        serde_json::from_slice::<Value>(&command.output().unwrap().stdout).unwrap()
    };
    let stopper = Viewing { model: model.clone(), status: None, started: Value::Null };
    let started = with(&["--no-open"]);
    assert_eq!(started["opened"], Value::Bool(false));
    let again = with(&[]);
    assert_eq!(again["opened"], Value::Bool(true), "{again}");
    eventually("the browser named given the page's address", || {
        std::fs::read_to_string(&opened).is_ok_and(|text| text == format!("{}\n", started["url"].as_str().unwrap()))
    });
    drop(stopper);
}
