//! Source: original
//! The command's parts that a running viewer cannot show: the page the build embeds, what the viewer's first line
//! says, a viewer that says nothing, one that says it will stop and does not, when a port is gone, and a process
//! started apart.

use std::io::{Read as _, Write as _};
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

#[cfg(unix)]
#[test]
fn a_viewer_that_says_nothing_has_not_started() {
    let mut child = Command::new("true").stdout(Stdio::piped()).spawn().unwrap();
    assert_eq!(first_line(&mut child, Duration::from_secs(5)), None, "it ended, saying nothing");
    let mut silent = Command::new("sleep").arg("5").stdout(Stdio::piped()).spawn().unwrap();
    assert_eq!(first_line(&mut silent, Duration::from_millis(100)), None, "nothing within the wait");
    let _ = silent.kill();
    let report = not_started(Path::new("/m/part.occam"));
    assert_eq!(
        report.message(DisplayUnits::default()),
        "The viewer for /m/part.occam did not start: its server stopped, or did not listen within 30 seconds."
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
