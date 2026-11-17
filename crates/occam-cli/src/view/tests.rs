//! Source: original
//! The command's parts that a running viewer cannot show: the page the build embeds, what the viewer's first line
//! says, a viewer that takes long to listen or ends without listening, one that says it will stop and does not, when a
//! port is gone, and a process started apart.

use std::io::Read as _;
use std::net::{Ipv4Addr, TcpListener};
use std::path::PathBuf;

use occam::DisplayUnits;

use super::*;

/// A directory of the test's own, empty.
fn scratch(name: &str) -> PathBuf {
    let dir = std::env::temp_dir().join(format!("occam-view-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

#[test]
fn the_page_built_in_is_every_file_but_the_maps_and_without_an_index_the_stub() {
    let page = scratch("embed");
    let stub = page.join("stub.html");
    for name in ["index.html", "viewer.js", "viewer.js.map", ".gitignore", "occam.wasm"] {
        std::fs::write(page.join(name), name).unwrap();
    }
    std::fs::create_dir(page.join("nested")).unwrap();
    let names: Vec<String> = embed::files(&page).into_iter().map(|(name, _)| name).collect();
    assert_eq!(
        names,
        ["index.html", "occam.wasm", "viewer.js"].map(String::from),
        "no map, no .gitignore, no directory"
    );
    let (source, built) = embed::source(&page, &stub);
    assert!(built);
    assert!(source.contains("pub const BUILT: bool = true;"), "{source}");
    assert!(
        source
            .contains(&format!("(\"viewer.js\", include_bytes!({:?}))", page.join("viewer.js").display().to_string()))
    );
    std::fs::remove_file(page.join("index.html")).unwrap();
    let (stubbed, built) = embed::source(&page, &stub);
    assert!(!built);
    let only = format!("(\"index.html\", include_bytes!({:?})),\n];", stub.display().to_string());
    assert!(stubbed.contains(&only) && !stubbed.contains("viewer.js"), "the stub alone: {stubbed}");
    assert!(!embed::source(&page.join("absent"), &stub).1, "no directory, the stub");
    let _ = std::fs::remove_dir_all(&page);
}

#[test]
fn the_viewers_first_line_is_where_it_listens_or_why_it_could_not() {
    let viewer = Viewer { pid: 9, port: 4000, token: "ab".to_string() };
    assert_eq!(said(&format!("{}\n", viewer.json())), Some(Ok(viewer)));
    let report = port_taken(4000);
    let line = report.to_json(DisplayUnits::default()).unwrap();
    assert_eq!(said(&line), Some(Err(report)));
    assert_eq!(said("a line from something else"), None);
}

/// The first line of a viewer that runs `script` in the shell, waited for a period of `every`, and how long it had been
/// waited for each time it was said to be still opening.
#[cfg(unix)]
fn waited_for(script: &str, every: Duration) -> (Option<String>, Vec<Duration>) {
    let mut child = Command::new("sh").args(["-c", script]).stdout(Stdio::piped()).spawn().unwrap();
    let mut still = Vec::new();
    let line = first_line(&mut child, every, |waited| still.push(waited));
    let _ = child.kill();
    let _ = child.wait();
    (line, still)
}

#[cfg(unix)]
#[test]
fn a_viewer_that_takes_longer_than_the_old_thirty_seconds_to_listen_is_waited_for_and_said_to_be_still_opening() {
    // Forty periods of 10 ms before it listens: at `occam view`'s 5 s a period, 200 s, past the 30 s it used to give.
    let (line, still) = waited_for("sleep 0.4; echo listening", Duration::from_millis(10));
    assert_eq!(line.as_deref(), Some("listening\n"));
    assert!(still.len() > 6, "said to be still opening each period, past six: {still:?}");
    assert!(still.windows(2).all(|pair| pair[0] < pair[1]), "for longer each time: {still:?}");
    assert_eq!(
        still_opening("part.occam", Duration::from_millis(15_400)),
        "The viewer is still opening part.occam: 15 s so far."
    );
}

#[cfg(unix)]
#[test]
fn a_viewer_that_ends_is_waited_for_no_longer_and_its_own_words_are_its_answer() {
    let report = port_taken(4000).to_json(DisplayUnits::default()).unwrap();
    let (line, _) = waited_for(&format!("sleep 0.1; echo '{report}'; exit 1"), Duration::from_millis(10));
    assert_eq!(line.as_deref().and_then(said), Some(Err(port_taken(4000))), "why it could not, in its words");
    let start = Instant::now();
    let (line, still) = waited_for("sleep 0.2; exit 3", Duration::from_secs(60));
    assert_eq!((line, still), (None, Vec::new()), "it ended, saying nothing");
    assert!(start.elapsed() < Duration::from_secs(30), "not waited for once it ended: {:?}", start.elapsed());
    assert_eq!(waited_for("true", Duration::from_millis(10)).0, None);
    let report = not_started(Path::new("/m/part.occam"));
    assert_eq!(
        report.message(DisplayUnits::default()),
        "The viewer for /m/part.occam did not start: its server stopped before it listened."
    );
}

#[test]
fn a_viewer_that_says_it_will_stop_and_still_answers_is_named() {
    let dir = scratch("stuck");
    let model = dir.join("part.occam");
    let listener = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = listener.local_addr().unwrap().port();
    let watching =
        Watching { model: model.display().to_string(), pid: 77, pages: 0, digest: None, reports: Vec::new() };
    let body = watching.json();
    std::thread::spawn(move || {
        for mut stream in listener.incoming().filter_map(Result::ok) {
            let mut request = [0_u8; 1024];
            let _ = stream.read(&mut request);
            let head = format!("HTTP/1.1 200 OK\r\ncontent-length: {}\r\nconnection: close\r\n\r\n", body.len());
            let _ = stream.write_all(format!("{head}{body}").as_bytes());
        }
    });
    state::write(&state::path(&model), &Viewer { pid: 77, port, token: "stuck".to_string() }).unwrap();
    let viewed = stop(&model, "part.occam", Duration::from_millis(200));
    assert!(!viewed.stopped);
    assert_eq!(viewed.reports.iter().map(|r| r.code).collect::<Vec<_>>(), [Code::NotStopped]);
    let words = viewed.reports[0].message(DisplayUnits::default());
    assert!(words.ends_with(", process 77, still answered 10 seconds after it was asked to stop."), "{words}");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn a_port_is_gone_once_its_listener_closes_within_the_wait_and_not_while_it_listens() {
    let closing = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    let port = closing.local_addr().unwrap().port();
    let closer = std::thread::spawn(move || {
        std::thread::sleep(Duration::from_millis(200));
        drop(closing);
    });
    assert!(gone(port, Duration::from_secs(5)), "closed a fifth of a second in, well within the wait");
    closer.join().unwrap();
    let held = TcpListener::bind((Ipv4Addr::LOCALHOST, 0)).unwrap();
    assert!(!gone(held.local_addr().unwrap().port(), Duration::from_millis(100)), "still listening when the wait ends");
}

#[cfg(unix)]
#[test]
fn a_detached_process_leads_a_process_group_of_its_own() {
    let mut command = Command::new("sleep");
    command.arg("5");
    detach(&mut command);
    let mut child = command.spawn().unwrap();
    let pid = child.id().to_string();
    let group = Command::new("ps").args(["-o", "pgid=", "-p", &pid]).output().unwrap();
    let _ = child.kill();
    let _ = child.wait();
    assert_eq!(String::from_utf8(group.stdout).unwrap().trim(), pid, "its group is its own process");
}
