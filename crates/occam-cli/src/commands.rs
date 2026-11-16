//! Source: original
//! Each command, from the command line as read to what is said: the files read and written, and the session they go
//! through. A command line clap refuses is `cli.bad-argument`, naming the argument, what was given and what was wanted.

use std::error::Error as _;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use clap::error::{ContextKind, ContextValue, Error, ErrorKind};
use clap::{CommandFactory, Parser};
use occam::{
    Answered, Code, DisplayUnits, DistanceQuery, Encoding, Exec, Failed, Fault, Input, Pick, Ran, Report, Selection,
    Session, distance_json, ran_json, ran_words,
};

use crate::args::{
    Cli, Command, DistanceArgs, HighlightArgs, IdsArgs, ListArgs, MeshArgs, ModelArgs, PickArgs, RunArgs, kind,
};
use crate::skill::skill;
#[cfg(not(target_family = "wasm"))]
use crate::view::view;

/// What to say, a line each with whether it goes to standard output, and the exit status: 0 when everything asked was
/// answered, 1 when anything was refused, and 2 when the command line is wrong.
#[derive(Debug, PartialEq, Eq)]
pub struct Said {
    pub lines: Vec<(bool, String)>,
    pub status: u8,
}

impl Said {
    /// `text` alone, to standard output if `out`.
    fn one(out: bool, text: String, status: u8) -> Self {
        Self { lines: vec![(out, text)], status }
    }

    /// One JSON form, which always goes to standard output.
    fn json(text: String, ok: bool) -> Self {
        Self::one(true, text, u8::from(!ok))
    }

    /// Lines in words, each going where it says.
    fn words(lines: Vec<(bool, String)>, ok: bool) -> Self {
        Self { lines, status: u8::from(!ok) }
    }
}

/// Whether the command line asks for JSON, read from it as given, for a command line clap refuses.
fn wants_json(arguments: &[OsString]) -> bool {
    arguments.iter().any(|argument| argument == "--json")
}

/// What `arguments`, the program's name first, ask for, done.
pub fn said(arguments: &[OsString]) -> Said {
    match Cli::try_parse_from(arguments) {
        Ok(cli) => command(cli.command, cli.json),
        Err(error) => refused(&error, wants_json(arguments)),
    }
}

/// The command `command`, done.
fn command(command: Command, json: bool) -> Said {
    match command {
        Command::Run(args) => run(&args, json),
        Command::List(args) => list(&args, json),
        Command::Adjacent(args) => ids(&args, json, Session::adjacent, Answered::Adjacent),
        Command::Describe(args) => ids(&args, json, Session::describe, Answered::Described),
        Command::Check(args) => check(&args, json),
        Command::Mesh(args) => mesh(&args, json),
        Command::Pick(args) => pick(&args, json),
        Command::Distance(args) => distance(&args, json),
        Command::Selection(args) => selection(&args, json),
        Command::Highlight(args) => highlight(&args, json),
        Command::Skill(args) => skill(&args, json),
        #[cfg(not(target_family = "wasm"))]
        Command::View(args) => view(&args, json),
    }
}

/// A command line clap refused, or the help or version it was asked for.
fn refused(error: &Error, json: bool) -> Said {
    let rendered = error.render().to_string().trim_end().to_string();
    match error.kind() {
        ErrorKind::DisplayHelp | ErrorKind::DisplayVersion => Said::one(true, rendered, 0),
        _ if json => {
            let report = bad_argument(error);
            Said::one(true, report.to_json(DisplayUnits::default()).unwrap_or(rendered), 2)
        }
        _ => Said::one(false, rendered, 2),
    }
}

/// The text of what `error` says of `kind`, if it says anything.
fn context(error: &Error, kind: ContextKind) -> Option<String> {
    match error.get(kind)? {
        ContextValue::String(text) => Some(text.clone()),
        ContextValue::Strings(texts) => Some(texts.join(", ")),
        _ => None,
    }
}

/// The commands, as a command line that names none or another is told.
fn commands() -> String {
    let command = Cli::command();
    let names: Vec<&str> = command.get_subcommands().map(clap::Command::get_name).collect();
    names.join(", ")
}

/// `cli.bad-argument` for what clap refused: the argument, what was given there and what was wanted.
fn bad_argument(error: &Error) -> Report {
    let input = match error.kind() {
        ErrorKind::InvalidSubcommand => {
            let given = context(error, ContextKind::InvalidSubcommand).unwrap_or_default();
            Input::at("COMMAND").given(&given).expected(&commands())
        }
        ErrorKind::MissingSubcommand | ErrorKind::DisplayHelpOnMissingArgumentOrSubcommand => {
            Input::at("COMMAND").expected(&commands())
        }
        _ => {
            let parameter = context(error, ContextKind::InvalidArg).unwrap_or_else(|| "the command line".to_string());
            let mut input = Input::at(&parameter);
            if let Some(given) = context(error, ContextKind::InvalidValue) {
                input = input.given(&given);
            }
            let expected = context(error, ContextKind::ValidValue).or_else(|| error.source().map(ToString::to_string));
            expected.map_or(input.clone(), |expected| input.expected(&expected))
        }
    };
    Report::new(Code::BadArgument, Fault::InvalidInput).with_input(input)
}

/// Threads for the whole-model check: as many as the machine has, which gives the same answers as one.
pub fn exec() -> Exec {
    std::thread::available_parallelism().ok().and_then(|count| Exec::threads(count).ok()).unwrap_or_else(Exec::serial)
}

/// `cli.file-not-read` for `path`, given as `parameter`.
pub fn not_read(parameter: &str, path: &Path) -> Report {
    Report::new(Code::FileNotRead, Fault::InvalidInput)
        .with_input(Input::at(parameter).given(&path.display().to_string()))
}

/// The model at `path`, opened, or why it would not open.
fn open(path: &Path, exec: &Exec) -> Result<Session, Vec<Report>> {
    let bytes = std::fs::read(path).map_err(|_| vec![not_read("--model", path)])?;
    Session::open(&bytes, exec)
}

/// `answered`, said as asked.
fn answer(answered: &Answered, json: bool) -> Said {
    if json { Said::json(answered.json(), answered.ok()) } else { Said::words(answered.words(), answered.ok()) }
}

/// `occam list`.
fn list(args: &ListArgs, json: bool) -> Said {
    if !args.stray.is_empty() {
        return stray_kinds(&args.stray.join(","), json);
    }
    let answered = match open(&args.model.model, &exec()) {
        Ok(session) => Answered::Listed(session.list(&args.kind.iter().filter_map(|k| kind(k)).collect::<Vec<_>>())),
        Err(reports) => Answered::Refused(reports),
    };
    answer(&answered, json)
}

/// Kinds given to `list` without `--kind`, refused as a wrong command line with the flag to use.
fn stray_kinds(given: &str, json: bool) -> Said {
    let wanted = format!("--kind {given}");
    if json {
        let input = Input::at("KIND").given(given).expected(&wanted);
        let report = Report::new(Code::BadArgument, Fault::InvalidInput).with_input(input);
        return Said::one(true, report.to_json(DisplayUnits::default()).unwrap_or_default(), 2);
    }
    Said::one(false, format!("error: list takes its kinds after --kind: write {wanted}"), 2)
}

/// `occam describe` and `occam adjacent`: each id asked of the model by `ask`, its answers made into `answered`.
fn ids<T>(
    args: &IdsArgs,
    json: bool,
    ask: impl Fn(&Session, &[occam::EntityRef]) -> Vec<T>,
    answered: impl Fn(Vec<T>) -> Answered,
) -> Said {
    let answered = match open(&args.model.model, &exec()) {
        Ok(session) => answered(ask(&session, &args.ids)),
        Err(reports) => Answered::Refused(reports),
    };
    answer(&answered, json)
}

/// `occam check`: the model opened, which measures every edge and checks the whole model, as `commit` does, so a model
/// that opens is sound.
fn check(args: &ModelArgs, json: bool) -> Said {
    let found = open(&args.model, &exec()).err().unwrap_or_default();
    answer(&Answered::Checked(found), json)
}

/// The session `occam run` works in: the model at `path` if there is one, or a new one, saved as the binary twin if
/// `binary`.
fn session(path: &Path, binary: bool) -> Result<Session, Vec<Report>> {
    let mut session = match std::fs::read(path) {
        Ok(bytes) => Session::open(&bytes, &exec())?,
        Err(error) if error.kind() == std::io::ErrorKind::NotFound => Session::new(Encoding::Json),
        Err(_) => return Err(vec![not_read("--model", path)]),
    };
    if binary {
        session.save_as(Encoding::Binary);
    }
    Ok(session)
}

/// Writes `bytes` to `path`, given as `parameter`, whole or not at all, through a file beside it.
pub fn write(parameter: &str, path: &Path, bytes: &[u8]) -> Result<(), Report> {
    whole(path, bytes).map_err(|_| {
        Report::new(Code::FileNotWritten, Fault::InvalidInput)
            .with_input(Input::at(parameter).given(&path.display().to_string()))
    })
}

/// Writes `bytes` to `path` whole or not at all: to a file beside it, renamed over it, so that a reader never sees half
/// of it. The file beside it is removed if that fails.
pub fn whole(path: &Path, bytes: &[u8]) -> std::io::Result<()> {
    let writing = beside(path, ".writing");
    std::fs::write(&writing, bytes).and_then(|()| std::fs::rename(&writing, path)).inspect_err(|_| {
        let _ = std::fs::remove_file(&writing);
    })
}

/// `occam run`: the request read, run on the model, and the model written back if the whole request went through.
fn run(args: &RunArgs, json: bool) -> Said {
    let read = std::fs::read_to_string(&args.request).map_err(|_| vec![not_read("REQUEST", &args.request)]);
    let ran = match read.and_then(|request| Ok((request, session(&args.model, args.binary)?))) {
        Ok((request, mut session)) => {
            let before = session.digest();
            let (mut ran, saved) = session.run(&request);
            match saved.map(|bytes| write("--model", &args.model, &bytes)) {
                Some(Err(report)) => ran.failed = Some(Failed { step: None, reports: vec![report] }),
                Some(Ok(())) => follow(&args.model, (before, session.digest()), &mut ran),
                None => {}
            }
            ran
        }
        Err(reports) => Ran::refused(None, reports),
    };
    let ok = ran.failed.is_none();
    if json { Said::json(ran_json(&ran), ok) } else { Said::words(ran_words(&ran), ok) }
}

/// The selection beside `model`, if there is one.
fn selected(model: &Path) -> Option<Selection> {
    Selection::from_json(&std::fs::read_to_string(beside(model, ".selection")).ok()?)
}

/// After a run saved `model`, changing its digest from `before` to `after`: the selection beside it carried over
/// through the run's changes, if it was made on the model before the run, so that a stale one never looks fresh, and
/// the highlight cleared, since it named what the run was to change. `ran` says what became of each.
fn follow(model: &Path, (before, after): (Option<u64>, Option<u64>), ran: &mut Ran) {
    if let (Some(selection), Some(after)) = (selected(model), after)
        && before == Some(selection.digest)
    {
        let (carried, said) = selection.carried(ran, after);
        if write("--model", &beside(model, ".selection"), carried.json().as_bytes()).is_ok() {
            ran.selection = Some(said);
        }
    }
    ran.highlight_cleared = std::fs::remove_file(beside(model, ".highlight")).is_ok();
}

/// `occam pick`: the model opened and meshed as the viewer's page meshes it, and what the ray hits first.
fn pick(args: &PickArgs, json: bool) -> Said {
    let [x, y, z] = args.from;
    let way = args.towards.map(|[a, b, c]| [a - x, b - y, c - z]);
    let asked =
        Pick { from: args.from, direction: args.direction.or(way).unwrap_or_default(), tolerance: args.tolerance };
    let exec = exec();
    let opened = open(&args.model.model, &exec);
    match opened.and_then(|mut session| session.pick(&exec, args.deflection, args.angle, asked)) {
        Ok(picked) if json => Said::json(picked.json(), picked.ok()),
        Ok(picked) => Said::words(picked.words(), picked.ok()),
        Err(reports) => answer(&Answered::Refused(reports), json),
    }
}

/// `occam selection`: what the person selected in the viewer's page, described against the model, or refused as
/// stale if it was made on another model.
fn selection(args: &ModelArgs, json: bool) -> Said {
    let session = match open(&args.model, &exec()) {
        Ok(session) => session,
        Err(reports) => return answer(&Answered::Refused(reports), json),
    };
    let Some(selection) = selected(&args.model) else {
        return nothing(json, "Nothing is selected in the viewer's page.");
    };
    if session.digest() != Some(selection.digest) {
        return answer(&Answered::Refused(vec![Selection::stale(&args.model.display().to_string())]), json);
    }
    answer(&Answered::Described(session.describe(&selection.ids)), json)
}

/// An answer of no ids, said as `line` in words.
fn nothing(json: bool, line: &str) -> Said {
    if json { Said::json(Answered::Described(Vec::new()).json(), true) } else { Said::one(true, line.to_string(), 0) }
}

/// `occam highlight`: the ids that name something live, written beside the model for the viewer's page, each
/// described; or, with `--clear`, the highlight removed.
fn highlight(args: &HighlightArgs, json: bool) -> Said {
    let file = beside(&args.model.model, ".highlight");
    if args.clear {
        let _ = std::fs::remove_file(&file);
        return nothing(json, "The highlight is cleared.");
    }
    let session = match open(&args.model.model, &exec()) {
        Ok(session) => session,
        Err(reports) => return answer(&Answered::Refused(reports), json),
    };
    let described = session.describe(&args.ids);
    let live = described.iter().filter(|(_, answer)| answer.is_ok()).map(|(id, _)| *id).collect();
    let shown = Selection::new(session.digest().unwrap_or_default(), live);
    if let Err(report) = write("--model", &file, shown.json().as_bytes()) {
        return answer(&Answered::Refused(vec![report]), json);
    }
    answer(&Answered::Described(described), json)
}

/// `occam mesh`: the model opened and meshed, its buffers written beside it or to `--out`, and how each face meshed.
fn mesh(args: &MeshArgs, json: bool) -> Said {
    let exec = exec();
    let meshed = open(&args.model.model, &exec).and_then(|mut opened| opened.mesh(&exec, args.deflection, args.angle));
    let meshed = match meshed {
        Ok(meshed) => meshed,
        Err(reports) => return answer(&Answered::Refused(reports), json),
    };
    let out = args.out.clone().unwrap_or_else(|| beside(&args.model.model, ".mesh"));
    if let Err(report) = write("--out", &out, &meshed.buffers.to_bytes()) {
        return answer(&Answered::Refused(vec![report]), json);
    }
    let file = out.display().to_string();
    if json {
        return Said::json(meshed.json(Some(&file)), meshed.ok());
    }
    let mut lines = meshed.words();
    lines.insert(1, (true, format!("The buffers are in {file}.")));
    Said::words(lines, meshed.ok())
}

/// The path of `model` with `suffix` added, for a file beside it.
pub fn beside(model: &Path, suffix: &str) -> PathBuf {
    let mut name = model.as_os_str().to_owned();
    name.push(suffix);
    PathBuf::from(name)
}

/// `occam distance`.
fn distance(args: &DistanceArgs, json: bool) -> Said {
    let [model_length_tolerance, model_angle_tolerance] = args.model_tolerance;
    let query = DistanceQuery {
        from: args.from,
        to: args.to,
        model_length_tolerance,
        model_angle_tolerance,
        tolerance: args.tolerance,
    };
    let answer = distance_json(&query);
    let text = if json { answer.json } else { answer.words };
    Said::one(json || !answer.refused, text, u8::from(answer.refused))
}
