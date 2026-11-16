//! Source: original
//! The viewer's server, which `occam view` starts as a process of its own (docs/architecture/live-view.md#the-server):
//! axum on one tokio thread, listening on 127.0.0.1 alone, every address it serves beginning with its token. It
//! serves the page built into the binary, the model as the binary twin, what it watches, a stream of server-sent
//! events and a stop. Several times a second it looks at the model file on a blocking thread, where a model is
//! opened, never on the async thread, and tells every open page when the file reads whole as another model, or does
//! not read. It stops when asked, when the state file names another viewer, or when no page has been open for its
//! idle time.

use std::convert::Infallible;
use std::net::{Ipv4Addr, TcpListener};
use std::path::{Path, PathBuf};
use std::sync::Arc;
use std::sync::atomic::{AtomicU32, Ordering};
use std::time::{Duration, Instant};

use axum::Router;
use axum::body::Bytes;
use axum::extract::State;
use axum::http::{HeaderName, StatusCode, Uri, header};
use axum::response::sse::{Event, KeepAlive, Sse};
use axum::response::{IntoResponse, Redirect, Response};
use axum::routing::{get, post};
use futures_util::stream::{self, Stream, StreamExt as _};
use occam::{DisplayUnits, Report, Viewer, Watching};
use tokio::sync::watch;

use super::watch::{Looked, Watcher};
use super::{page, port_taken, state, state_not_written};
use crate::commands::Said;

/// How often the model file is looked at.
const LOOK: Duration = Duration::from_millis(200);
/// How long the server waits for its connections to close once it is stopping.
const SHUTDOWN: Duration = Duration::from_secs(2);

/// What the pages are shown: the last model that read whole, as its digest and binary twin, and why the file does not
/// read now, if it does not.
#[derive(Clone, Default)]
struct Shown {
    good: Option<(u64, Bytes)>,
    refused: Vec<Report>,
    stopping: bool,
}

impl Shown {
    /// What a look found, taken in.
    fn take(&mut self, looked: Looked) {
        match looked {
            Looked::Read(model) => {
                self.good = Some((model.digest, Bytes::from(model.twin)));
                self.refused = Vec::new();
            }
            Looked::Refused(reports) => self.refused = reports,
        }
    }
}

/// The server's state, which every request shares.
struct App {
    model: PathBuf,
    file: PathBuf,
    viewer: Viewer,
    idle: Duration,
    shown: watch::Sender<Shown>,
    pages: AtomicU32,
}

impl App {
    fn stop(&self) {
        self.shown.send_modify(|shown| shown.stopping = true);
    }

    fn stopping(&self) -> bool {
        self.shown.borrow().stopping
    }

    /// What it says of the model it watches.
    fn watching(&self) -> Watching {
        let shown = self.shown.borrow();
        Watching {
            model: self.model.display().to_string(),
            pid: self.viewer.pid,
            pages: self.pages.load(Ordering::Relaxed),
            digest: shown.good.as_ref().map(|(digest, _)| *digest),
            reports: shown.refused.clone(),
        }
    }
}

/// Runs the viewer of `model`, listening on `port` of 127.0.0.1 or a free one, until it stops; it stops by itself
/// once no page has been open for `idle`. Once it listens, and has looked at the model once, it writes its state file
/// and says so on standard output, a line of the state file's JSON, which `occam view` waits for; if it cannot, it
/// says why instead, a report's JSON, and returns.
pub fn serve(model: &Path, port: Option<u16>, idle: Duration) -> Said {
    let listener = match TcpListener::bind((Ipv4Addr::LOCALHOST, port.unwrap_or(0))) {
        Ok(listener) => listener,
        Err(error) if error.kind() == std::io::ErrorKind::AddrInUse && port.is_some() => {
            return refused(&port_taken(port.unwrap_or_default()));
        }
        Err(_) => return refused(&super::not_started(model)),
    };
    let viewer = Viewer {
        pid: std::process::id(),
        port: listener.local_addr().map(|address| address.port()).unwrap_or_default(),
        token: state::token(),
    };
    let mut watcher = Watcher::new(model.to_path_buf());
    let mut shown = Shown::default();
    shown.take(watcher.first());
    let file = state::path(model);
    let runtime = tokio::runtime::Builder::new_current_thread().enable_io().enable_time().build();
    let (Ok(runtime), Ok(())) = (runtime, listener.set_nonblocking(true)) else {
        return refused(&super::not_started(model));
    };
    if state::write(&file, &viewer).is_err() {
        return refused(&state_not_written(&file));
    }
    let token = viewer.token.clone();
    let app = Arc::new(App {
        model: model.to_path_buf(),
        file: file.clone(),
        viewer,
        idle,
        shown: watch::channel(shown).0,
        pages: AtomicU32::new(0),
    });
    let mut out = std::io::stdout().lock();
    let said = std::io::Write::write_all(&mut out, format!("{}\n", app.viewer.json()).as_bytes());
    if said.and_then(|()| std::io::Write::flush(&mut out)).is_ok() {
        drop(out);
        runtime.block_on(run(listener, app, watcher));
    }
    state::remove_if_ours(&file, &token);
    Said { lines: Vec::new(), status: 0 }
}

/// What the server says when it cannot listen: `report`'s JSON, for `occam view` to read.
fn refused(report: &Report) -> Said {
    Said { lines: vec![(true, report.to_json(DisplayUnits::default()).unwrap_or_default())], status: 1 }
}

/// Serves on `listener` and watches the model with `watcher` until the server is stopped, then waits a little for its
/// connections to close.
async fn run(listener: TcpListener, app: Arc<App>, watcher: Watcher) {
    let Ok(listener) = tokio::net::TcpListener::from_std(listener) else { return };
    let mut stopping = app.shown.subscribe();
    let serve = axum::serve(listener, router(&app)).with_graceful_shutdown(async move {
        let _ = stopping.wait_for(|shown| shown.stopping).await;
    });
    let serving = tokio::spawn(async move { serve.await });
    watch(&app, watcher).await;
    let _ = tokio::time::timeout(SHUTDOWN, serving).await;
}

/// Looks at the model file every [`LOOK`] on a blocking thread, takes in what each look finds, and stops the server
/// when it is asked to, when the state file names another viewer, or when no page has been open for the idle time.
async fn watch(app: &Arc<App>, mut watcher: Watcher) {
    let mut tick = tokio::time::interval(LOOK);
    let mut idle_since = Instant::now();
    loop {
        tick.tick().await;
        if app.stopping() {
            return;
        }
        let (file, token) = (app.file.clone(), app.viewer.token.clone());
        let looked = tokio::task::spawn_blocking(move || {
            let looked = watcher.look();
            (watcher, looked, state::taken(&file, &token))
        });
        let Ok((back, looked, replaced)) = looked.await else { return app.stop() };
        watcher = back;
        if let Some(looked) = looked {
            app.shown.send_modify(|shown| shown.take(looked));
        }
        if app.pages.load(Ordering::Relaxed) > 0 {
            idle_since = Instant::now();
        }
        if replaced || idle_since.elapsed() >= app.idle {
            return app.stop();
        }
    }
}

/// Every route, each behind the token.
fn router(app: &Arc<App>) -> Router {
    let token = &app.viewer.token;
    let mut router = Router::new()
        .route(
            &format!("/{token}/"),
            get(|| async { reply(StatusCode::OK, "text/html; charset=utf-8", page::index()) }),
        )
        .route(&format!("/{token}/model"), get(model))
        .route(&format!("/{token}/events"), get(events))
        .route(&format!("/{token}/status"), get(status))
        .route(&format!("/{token}/stop"), post(stop));
    for (name, bytes) in page::files() {
        router = router.route(
            &format!("/{token}/{name}"),
            get(move || async move { reply(StatusCode::OK, page::content_type(name), bytes) }),
        );
    }
    router.fallback(elsewhere).with_state(Arc::clone(app))
}

/// A response of `status` with `body` as `kind`, never cached, sending no referrer.
fn reply(status: StatusCode, kind: &'static str, body: impl IntoResponse) -> Response {
    let headers = [
        (header::CONTENT_TYPE, kind),
        (header::CACHE_CONTROL, "no-store"),
        (header::REFERRER_POLICY, "no-referrer"),
        (header::X_CONTENT_TYPE_OPTIONS, "nosniff"),
    ];
    (status, headers, body).into_response()
}

/// The model as the binary twin, naming its digest, or what the viewer says of it where no model has read whole.
async fn model(State(app): State<Arc<App>>) -> Response {
    let good = app.shown.borrow().good.clone();
    match good {
        Some((digest, twin)) => {
            let mut response = reply(StatusCode::OK, "application/octet-stream", twin);
            if let Ok(value) = occam::digest_text(digest).parse() {
                response.headers_mut().insert(HeaderName::from_static("x-occam-digest"), value);
            }
            response
        }
        None => reply(StatusCode::NOT_FOUND, "application/json", app.watching().json()),
    }
}

/// What the viewer says of the model it watches.
async fn status(State(app): State<Arc<App>>) -> Response {
    reply(StatusCode::OK, "application/json", app.watching().json())
}

/// Stops the server.
async fn stop(State(app): State<Arc<App>>) -> Response {
    app.stop();
    reply(StatusCode::OK, "application/json", app.watching().json())
}

/// Anything else: the page's address without its last slash is sent to it, an address with the token that names
/// nothing is not found, and one without the token is refused.
async fn elsewhere(State(app): State<Arc<App>>, uri: Uri) -> Response {
    let base = format!("/{}", app.viewer.token);
    if uri.path() == base {
        let query = uri.query().map(|query| format!("?{query}")).unwrap_or_default();
        return Redirect::permanent(&format!("{base}/{query}")).into_response();
    }
    if uri.path().starts_with(&format!("{base}/")) {
        return reply(StatusCode::NOT_FOUND, "text/plain; charset=utf-8", "Nothing is here.");
    }
    reply(StatusCode::FORBIDDEN, "text/plain; charset=utf-8", "This viewer answers only addresses with its token.")
}

/// An open page, counted while its stream of events lasts.
struct Page(Arc<App>);

impl Page {
    fn open(app: &Arc<App>) -> Self {
        app.pages.fetch_add(1, Ordering::Relaxed);
        Self(Arc::clone(app))
    }
}

impl Drop for Page {
    fn drop(&mut self) {
        self.0.pages.fetch_sub(1, Ordering::Relaxed);
    }
}

/// What a page has been sent.
#[derive(Default)]
struct Sent {
    digest: Option<u64>,
    refused: Vec<Report>,
}

impl Sent {
    /// The events that tell a page what changed since: `model` when another model reads whole, or the file reads
    /// whole again, and `refused` when it does not read, each with what the viewer says of the model.
    fn since(&mut self, shown: &Shown, app: &App) -> Vec<Result<Event, Infallible>> {
        let digest = shown.good.as_ref().map(|(digest, _)| *digest);
        let read_again = !self.refused.is_empty() && shown.refused.is_empty();
        let name = if shown.refused != self.refused && !shown.refused.is_empty() {
            Some("refused")
        } else if digest.is_some() && (digest != self.digest || read_again) {
            Some("model")
        } else {
            None
        };
        (self.digest, self.refused) = (digest, shown.refused.clone());
        name.map(|name| Ok(Event::default().event(name).data(app.watching().json()))).into_iter().collect()
    }
}

/// The stream of events a page follows the model by: what the viewer shows when it opens, then each change, until
/// the server stops.
async fn events(State(app): State<Arc<App>>) -> Sse<impl Stream<Item = Result<Event, Infallible>>> {
    let start = (app.shown.subscribe(), Sent::default(), Page::open(&app));
    let stream = stream::unfold(start, |(mut shown, mut sent, page)| async move {
        loop {
            let now = shown.borrow_and_update().clone();
            if now.stopping {
                return None;
            }
            let events = sent.since(&now, &page.0);
            if !events.is_empty() {
                return Some((stream::iter(events), (shown, sent, page)));
            }
            shown.changed().await.ok()?;
        }
    });
    Sse::new(stream.flatten()).keep_alive(KeepAlive::default())
}
