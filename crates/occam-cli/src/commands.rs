//! Source: original
//! Each command, from the command line as read to what is said: the files read and written, and the session they go
//! through. A command line clap refuses is `cli.bad-argument`, naming the argument, what was given and what was wanted.

use std::error::Error as _;
use std::ffi::OsString;
use std::path::{Path, PathBuf};

use clap::error::{ContextKind, ContextValue, Error, ErrorKind};
use clap::{CommandFactory, Parser};
use occam::{
    Answered, Code, DisplayUnits, DistanceQuery, Encoding, Exec, Failed, Fault, Input, Ran, Report, Session,
    distance_json, ran_json, ran_words,
};

use crate::args::{Cli, Command, DistanceArgs, IdsArgs, ListArgs, MeshArgs, ModelArgs, RunArgs, kind};

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
        Command::Distance(args) => distance(&args, json),
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
fn exec() -> Exec {
    std::thread::available_parallelism().ok().and_then(|count| Exec::threads(count).ok()).unwrap_or_else(Exec::serial)
}

/// `cli.file-not-read` for `path`, given as `parameter`.
fn not_read(parameter: &str, path: &Path) -> Report {
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
fn write(parameter: &str, path: &Path, bytes: &[u8]) -> Result<(), Report> {
    let writing = beside(path, ".writing");
    let written = std::fs::write(&writing, bytes).and_then(|()| std::fs::rename(&writing, path));
    written.map_err(|_| {
        let _ = std::fs::remove_file(&writing);
        Report::new(Code::FileNotWritten, Fault::InvalidInput)
            .with_input(Input::at(parameter).given(&path.display().to_string()))
    })
}

/// `occam run`: the request read, run on the model, and the model written back if the whole request went through.
fn run(args: &RunArgs, json: bool) -> Said {
    let read = std::fs::read_to_string(&args.request).map_err(|_| vec![not_read("REQUEST", &args.request)]);
    let ran = match read.and_then(|request| Ok((request, session(&args.model, args.binary)?))) {
        Ok((request, mut session)) => {
            let (mut ran, saved) = session.run(&request);
            if let Some(Err(report)) = saved.map(|bytes| write("--model", &args.model, &bytes)) {
                ran.failed = Some(Failed { step: None, reports: vec![report] });
            }
            ran
        }
        Err(reports) => Ran::refused(None, reports),
    };
    let ok = ran.failed.is_none();
    if json { Said::json(ran_json(&ran), ok) } else { Said::words(ran_words(&ran), ok) }
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
fn beside(model: &Path, suffix: &str) -> PathBuf {
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
