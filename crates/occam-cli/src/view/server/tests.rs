//! Source: original
//! The viewer's decisions, each on its own: what a bind error is reported as, which event a page is owed, how its
//! pages are counted, since when it has had none, and that its watch ends once it is stopping.

use std::io::{Error, ErrorKind};

use occam::{Code, Fault};

use super::*;

/// A viewer of a model that is not there, with no page open, that would stop itself after `idle`.
fn app(idle: Duration) -> Arc<App> {
    let dir = std::env::temp_dir().join(format!("occam-server-{}", std::process::id()));
    Arc::new(App {
        model: dir.join("part.occam"),
        file: dir.join("part.occam.view"),
        viewer: Viewer { pid: 1, port: 1, token: "t".to_string() },
        idle,
        shown: watch::channel(Shown::default()).0,
        pages: AtomicU32::new(0),
    })
}

#[test]
fn a_port_asked_for_and_in_use_is_taken_and_any_other_bind_error_did_not_start_the_viewer() {
    let model = Path::new("/m/part.occam");
    let code = |kind: ErrorKind, port: Option<u16>| unbound(&Error::from(kind), port, model).code;
    assert_eq!(code(ErrorKind::AddrInUse, Some(8080)), Code::PortTaken);
    assert_eq!(code(ErrorKind::PermissionDenied, Some(80)), Code::NotStarted, "a port it may not use is not taken");
    assert_eq!(code(ErrorKind::AddrInUse, None), Code::NotStarted, "no port asked for");
    assert_eq!(code(ErrorKind::AddrNotAvailable, None), Code::NotStarted);
}

/// What the viewer shows: the model of `digest`, if one read whole, and the reports `refused`.
fn shown(digest: Option<u64>, refused: &[Report]) -> Shown {
    Shown {
        good: digest.map(|digest| (digest, Bytes::new())),
        refused: refused.to_vec(),
        stopping: false,
        marks: Default::default(),
    }
}

#[test]
fn a_page_is_sent_an_event_only_when_what_it_was_sent_changes() {
    let missing = [Report::new(Code::ModelMissing, Fault::InvalidInput)];
    let garbage = [Report::new(Code::NotReadable, Fault::InvalidInput)];
    let steps = [
        (shown(None, &missing), Some("refused"), "no model yet"),
        (shown(None, &missing), None, "still none"),
        (shown(Some(1), &[]), Some("model"), "the first model"),
        (shown(Some(1), &[]), None, "nothing new"),
        (shown(Some(1), &garbage), Some("refused"), "the file stops reading"),
        (shown(Some(1), &garbage), None, "and still does not"),
        (shown(Some(1), &[]), Some("model"), "it reads whole again, as the same model"),
        (shown(Some(2), &[]), Some("model"), "another model"),
        (shown(Some(2), &[]), None, "nothing new again"),
    ];
    let mut sent = Sent::default();
    for (now, want, step) in steps {
        assert_eq!(sent.since(&now), want, "{step}");
    }
}

#[test]
fn a_page_is_counted_while_its_stream_lasts() {
    let app = app(Duration::from_secs(3600));
    let (first, second) = (Page::open(&app), Page::open(&app));
    assert_eq!(app.pages.load(Ordering::Relaxed), 2);
    drop(first);
    assert_eq!(app.pages.load(Ordering::Relaxed), 1, "a page closed is counted out");
    drop(second);
    assert_eq!(app.pages.load(Ordering::Relaxed), 0);
}

#[test]
fn the_viewer_is_without_a_page_since_the_last_look_that_found_one() {
    let since = Instant::now();
    let now = since + Duration::from_secs(5);
    assert_eq!(quiet_since(since, 1, now), now, "a page open: quiet from now on, at the soonest");
    assert_eq!(quiet_since(since, 0, now), since, "none: quiet since as before");
}

#[test]
fn the_watch_ends_at_its_first_look_once_the_viewer_is_stopping() {
    let app = app(Duration::from_secs(3600));
    app.stop();
    assert!(app.stopping());
    let runtime = tokio::runtime::Builder::new_current_thread().enable_time().build().unwrap();
    let looking = watch(&app, Watcher::new(app.model.clone()));
    let ended = runtime.block_on(async { tokio::time::timeout(Duration::from_secs(2), looking).await });
    assert!(ended.is_ok(), "it ended, an hour of idle time from stopping by itself");
}

#[test]
fn a_page_is_sent_the_selection_then_the_highlight_each_once_until_it_changes() {
    let mut sent = Sent::default();
    let mut now = Shown { marks: ["s1".into(), "h1".into()], ..Shown::default() };
    assert_eq!(sent.marked(&now), Some(("selection", "s1".to_owned())));
    assert_eq!(sent.marked(&now), Some(("highlight", "h1".to_owned())));
    assert_eq!(sent.marked(&now), None, "each sent once");
    now.marks[1] = "h2".into();
    assert_eq!(sent.marked(&now), Some(("highlight", "h2".to_owned())), "only what changed");
    assert_eq!(sent.marked(&now), None);
}

#[test]
fn a_page_is_first_shown_the_selection_and_the_highlight_beside_the_model() {
    let dir = std::env::temp_dir().join(format!("occam-first-{}", std::process::id()));
    let _ = std::fs::create_dir_all(&dir);
    let model = dir.join("part.occam");
    let selection = Selection::new(7, vec![occam::EntityRef::new(occam::EntityKind::Face, 1, 1)]).json();
    std::fs::write(dir.join("part.occam.selection"), &selection).unwrap();
    let shown = first(&model, &mut Watcher::new(model.clone()));
    assert_eq!(shown.marks, [selection, Selection::none_json()], "the selection there, and no highlight");
    assert_eq!(shown.refused.iter().map(|r| r.code).collect::<Vec<_>>(), [Code::ModelMissing], "and no model yet");
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn marks_taken_in_say_whether_either_changed() {
    let mut shown = Shown::default();
    let marks = ["s".to_owned(), "h".to_owned()];
    assert!(shown.mark(marks.clone()), "new");
    assert_eq!(shown.marks, marks);
    assert!(!shown.mark(marks.clone()), "the same again");
    assert!(shown.mark(["s".to_owned(), "h2".to_owned()]), "the highlight alone");
}
