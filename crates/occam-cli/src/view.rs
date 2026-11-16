//! Source: original
//! `occam view`: the model shown live in a page on this machine (docs/architecture/live-view.md), started by the agent
//! (docs/decisions/2026-11-14-the-agent-starts-the-viewer.md). The command starts the model's viewer, a server that is
//! this program run again with `--serve` as a process of its own, waits until it listens, says where its page is and
//! returns. The state file beside the model, `<model>.view`, names the viewer's process, port and token, so a second
//! `occam view` finds the one that runs and starts nothing; one whose viewer does not answer is stale, and is
//! replaced. `--status` says whether it runs and where, and `--stop` stops it.

mod ask;
#[cfg(test)]
mod embed;
mod open;
mod page;
mod server;
mod state;
mod watch;

use std::io::{BufRead as _, BufReader};
use std::path::Path;
use std::process::{Child, Command, Stdio};
use std::sync::mpsc;
use std::time::{Duration, Instant};

use occam::{Code, Fault, Input, NamedValue, Remedy, RemedyKind, Report, Viewed, Viewer, Watching};

use crate::args::ViewArgs;
use crate::commands::Said;

/// How long `occam view` waits, in seconds, for the viewer it started to say it listens.
const READY: u32 = 30;
/// How long `occam view --stop` waits, in seconds, for the viewer to stop once it has said it will.
const STOPPING: u32 = 10;

/// `occam view`, or, with `--serve`, the viewer itself.
pub fn view(args: &ViewArgs, json: bool) -> Said {
    let model = std::path::absolute(&args.model).unwrap_or_else(|_| args.model.clone());
    if args.serve {
        return server::serve(&model, args.port, Duration::from_secs(args.idle));
    }
    let given = args.model.display().to_string();
    let viewed = if args.stop {
        stop(&model, &given, Duration::from_secs(STOPPING.into()))
    } else if args.status {
        Viewed { running: running(&model), ..Viewed::of(&given) }
    } else {
        start(&model, &given, args)
    };
    let status = u8::from(!viewed.ok());
    Said { lines: if json { vec![(true, viewed.json())] } else { viewed.words() }, status }
}

/// The viewer the state file of `model` names, with what it says of the model, if it answers and watches `model`.
fn running(model: &Path) -> Option<(Viewer, Watching)> {
    let viewer = state::read(&state::path(model))?;
    let watching = watching(&viewer).filter(|watching| watching.model == model.display().to_string())?;
    Some((viewer, watching))
}

/// What `viewer` says of the model it watches, if it answers.
fn watching(viewer: &Viewer) -> Option<Watching> {
    let answer = ask::ask(viewer.port, "GET", &format!("/{}/status", viewer.token))?;
    (answer.status == 200).then(|| Watching::from_json(std::str::from_utf8(&answer.body).ok()?)).flatten()
}

/// `occam view`: the viewer of `model` that runs, or one started, and the page opened unless `--no-open` is given or
/// a page is open already.
fn start(model: &Path, given: &str, args: &ViewArgs) -> Viewed {
    let mut answer = Viewed::of(given);
    answer.running = running(model);
    if answer.running.is_none() {
        match spawn(model, args).map(|viewer| watching(&viewer).map(|watching| (viewer, watching))) {
            Ok(Some(started)) => (answer.running, answer.started) = (Some(started), true),
            Ok(None) => answer.reports.push(not_started(model)),
            Err(report) => answer.reports.push(report),
        }
    }
    if let Some((viewer, watching)) = &answer.running
        && !args.no_open
        && watching.pages == 0
    {
        answer.opened = open::page(&viewer.url());
    }
    answer
}

/// `occam view --stop`: the viewer of `model` asked to stop, and waited for up to `wait`. A state file whose viewer
/// does not answer is removed.
fn stop(model: &Path, given: &str, wait: Duration) -> Viewed {
    let mut answer = Viewed::of(given);
    let file = state::path(model);
    let Some((viewer, _)) = running(model) else {
        if let Some(stale) = state::read(&file) {
            state::remove_if_ours(&file, &stale.token);
        }
        return answer;
    };
    let asked = ask::ask(viewer.port, "POST", &format!("/{}/stop", viewer.token));
    if asked.is_some_and(|answer| answer.status == 200) && gone(viewer.port, wait) {
        state::remove_if_ours(&file, &viewer.token);
        answer.stopped = true;
    } else {
        answer.reports.push(not_stopped(model, viewer.pid));
    }
    answer
}

/// Whether nothing listens on `port` any longer, within `wait`.
fn gone(port: u16, wait: Duration) -> bool {
    let start = Instant::now();
    while ask::listens(port) {
        if start.elapsed() >= wait {
            return false;
        }
        std::thread::sleep(Duration::from_millis(50));
    }
    true
}

/// Starts the viewer of `model` as a process of its own and waits for it to say it listens: the viewer, as its state
/// file names it, or why it could not start.
fn spawn(model: &Path, args: &ViewArgs) -> Result<Viewer, Report> {
    let program = std::env::current_exe().map_err(|_| not_started(model))?;
    let mut command = Command::new(program);
    command.args(["view", "--serve", "--idle", &args.idle.to_string(), "--model"]).arg(model);
    if let Some(port) = args.port {
        command.args(["--port", &port.to_string()]);
    }
    command.stdin(Stdio::null()).stdout(Stdio::piped()).stderr(Stdio::null());
    detach(&mut command);
    let mut child = command.spawn().map_err(|_| not_started(model))?;
    let said = first_line(&mut child, Duration::from_secs(READY.into())).and_then(|line| said(&line));
    said.unwrap_or_else(|| {
        let _ = child.kill();
        Err(not_started(model))
    })
}

/// The first line `child` writes to standard output, if it writes one within `wait`.
fn first_line(child: &mut Child, wait: Duration) -> Option<String> {
    let out = child.stdout.take()?;
    let (send, receive) = mpsc::channel();
    std::thread::spawn(move || {
        let mut line = String::new();
        let read = BufReader::new(out).read_line(&mut line);
        let _ = send.send(read.ok().filter(|&count| count > 0).map(|_| line));
    });
    receive.recv_timeout(wait).ok().flatten()
}

/// What the viewer's first line says: that it listens, as its state file names it, or why it could not.
fn said(line: &str) -> Option<Result<Viewer, Report>> {
    Viewer::from_json(line).map(Ok).or_else(|| Report::from_json(line.trim()).ok().map(Err))
}

/// Starts `command` apart from this process's group, so that the viewer outlives it and its terminal.
#[cfg(unix)]
fn detach(command: &mut Command) {
    std::os::unix::process::CommandExt::process_group(command, 0);
}

/// Starts `command` with no console and in a process group of its own, so that the viewer outlives this process.
#[cfg(windows)]
fn detach(command: &mut Command) {
    const DETACHED_PROCESS: u32 = 0x0000_0008;
    const CREATE_NEW_PROCESS_GROUP: u32 = 0x0000_0200;
    std::os::windows::process::CommandExt::creation_flags(command, DETACHED_PROCESS | CREATE_NEW_PROCESS_GROUP);
}

#[cfg(not(any(unix, windows)))]
const fn detach(_: &mut Command) {}

/// The model as an input of the command line.
fn given_model(model: &Path) -> Input {
    Input::at("--model").given(&model.display().to_string())
}

/// `view.not-started` for the viewer of `model`.
fn not_started(model: &Path) -> Report {
    Report::new(Code::NotStarted, Fault::Internal)
        .with_input(given_model(model))
        .with_value(NamedValue::count("seconds", READY))
}

/// `view.port-taken` for `port`, with the remedy of asking for none.
fn port_taken(port: u16) -> Report {
    Report::new(Code::PortTaken, Fault::InvalidInput)
        .with_value(NamedValue::count("port", u32::from(port)))
        .with_remedy(Remedy::new(RemedyKind::UseAnyPort))
}

/// `view.state-not-written` for the state file `file`.
fn state_not_written(file: &Path) -> Report {
    Report::new(Code::StateNotWritten, Fault::InvalidInput).with_input(given_model(file))
}

/// `view.not-stopped` for the viewer of `model`, process `pid`.
fn not_stopped(model: &Path, pid: u32) -> Report {
    Report::new(Code::NotStopped, Fault::Internal)
        .with_input(given_model(model))
        .with_value(NamedValue::count("pid", pid))
        .with_value(NamedValue::count("seconds", STOPPING))
}

#[cfg(test)]
mod tests;
