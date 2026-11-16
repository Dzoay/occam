//! Source: original
//! The occam command-line tool: a thin facade over the `occam` crate, for agents and people (docs/architecture/
//! command-line.md). `--json` gives the one JSON form of the result or of the reports refusing it on standard output (L8);
//! in words, what was answered goes to standard output and what was refused to standard error. The exit status is 0 when
//! everything asked was answered, 1 when anything was refused, and 2 when the command line is wrong.

mod args;
mod commands;
#[cfg(not(target_family = "wasm"))]
mod view;

use std::io::Write as _;
use std::process::ExitCode;

fn main() -> ExitCode {
    let arguments: Vec<_> = std::env::args_os().collect();
    let said = commands::said(&arguments);
    let (mut out, mut err) = (std::io::stdout().lock(), std::io::stderr().lock());
    for (to_out, line) in &said.lines {
        // A reader that stops early, as `head` does, is not a failure of the command.
        let _ = if *to_out { writeln!(out, "{line}") } else { writeln!(err, "{line}") };
    }
    ExitCode::from(said.status)
}

#[cfg(test)]
mod tests {
    use std::ffi::OsString;

    use clap::Parser;
    use occam::{Code, EntityKind, EntityRef, Report};

    use super::args::{Cli, Command, kind};
    use super::commands::{Said, said};

    fn asked(arguments: &[&str]) -> Said {
        let mut all = vec![OsString::from("occam")];
        all.extend(arguments.iter().map(OsString::from));
        said(&all)
    }

    const GOOD: &[&str] =
        &["distance", "--from", "0,0,1", "--to", "3,4,1", "--model-tolerance", "0.001,0.00001", "--tolerance", "0.01"];

    #[test]
    fn without_the_flag_the_answer_is_in_words() {
        assert_eq!(asked(GOOD), Said { lines: vec![(true, "5 mm".to_string())], status: 0 });
    }

    #[test]
    fn flags_can_come_in_any_order_and_json_anywhere() {
        let arguments = [
            "--json",
            "distance",
            "--tolerance",
            "0.01",
            "--model-tolerance",
            "0.001,0.00001",
            "--to",
            "3,4,1",
            "--from",
            "0,0,1",
        ];
        let answer = asked(&arguments);
        assert_eq!(answer.status, 0);
        assert!(answer.lines[0].1.starts_with(r#"{"schema":2,"value":5.0,"#), "{answer:?}");
    }

    /// What a command line that is wrong says in JSON: the report's parameter, given and expected.
    fn wrong(arguments: &[&str]) -> (String, Option<String>, Option<String>) {
        let mut arguments = arguments.to_vec();
        arguments.push("--json");
        let answer = asked(&arguments);
        assert_eq!(answer.status, 2, "{answer:?}");
        let report = Report::from_json(&answer.lines[0].1).unwrap();
        assert_eq!(report.code, Code::BadArgument);
        let input = report.input.clone().unwrap();
        (input.parameter, input.given, input.expected)
    }

    #[test]
    fn a_bad_argument_is_named_with_what_was_given_and_wanted() {
        let listed = if cfg!(target_family = "wasm") { "" } else { ", view" };
        let commands =
            Some(format!("run, list, adjacent, describe, check, mesh, pick, distance, selection, highlight{listed}"));
        assert_eq!(
            wrong(&["cut"]),
            ("COMMAND".to_string(), Some("cut".to_string()), commands.clone()),
            "unknown command"
        );
        assert_eq!(wrong(&[]), ("COMMAND".to_string(), None, commands), "no command");
        assert_eq!(wrong(&["distance", "--sideways"]).0, "--sideways", "unknown flag");
        let (parameter, given, expected) = wrong(&["distance", "--from", "1,2"]);
        assert_eq!((parameter.as_str(), given.as_deref()), ("--from <X,Y,Z>", Some("1,2")), "too few coordinates");
        assert_eq!(expected.as_deref(), Some("3 numbers separated by commas"));
        let (_, given, expected) = wrong(&["distance", "--tolerance", "x"]);
        assert_eq!((given.as_deref(), expected.as_deref()), (Some("x"), Some("a number")), "not a number");
        assert!(wrong(&["distance", "--from", "0,0,0"]).0.contains("--to"), "a missing flag is named");
    }

    #[test]
    fn a_bad_argument_is_said_the_way_the_caller_asked() {
        let answer = asked(&["distance", "--sideways"]);
        assert_eq!(answer.status, 2);
        let (out, words) = &answer.lines[0];
        assert!(!out, "to standard error");
        assert!(words.starts_with("error: unexpected argument '--sideways'"), "{words}");
        assert!(words.contains("Usage: occam distance"), "{words}");
    }

    /// The command line `arguments`, read.
    fn read(arguments: &[&str]) -> Cli {
        Cli::try_parse_from(["occam"].iter().chain(arguments)).unwrap()
    }

    #[test]
    fn kinds_may_be_given_again_or_separated_by_commas() {
        let listed = read(&["list", "--model", "m.occam", "--kind", "edge,face", "--kind", "edge-use"]).command;
        let Command::List(args) = listed else { panic!("a list: {listed:?}") };
        assert_eq!(args.kind, ["edge", "face", "edge-use"]);
        assert_eq!(args.model.model.to_str(), Some("m.occam"));
        let kinds: Vec<Option<EntityKind>> = args.kind.iter().map(|k| kind(k)).collect();
        assert_eq!(kinds, [Some(EntityKind::Edge), Some(EntityKind::Face), Some(EntityKind::EdgeUse)]);
    }

    #[test]
    fn ids_are_read_in_their_one_text_form_in_the_order_given() {
        let described = read(&["describe", "--model", "m.occam", "face:5/1", "edge:41/2", "--json"]).command;
        let Command::Describe(args) = described else { panic!("a describe: {described:?}") };
        let want = [EntityRef::new(EntityKind::Face, 5, 1), EntityRef::new(EntityKind::Edge, 41, 2)];
        assert_eq!(args.ids, want);
        let run = read(&["run", "ops.json", "--model", "m.occam", "--binary"]);
        assert!(
            matches!(run.command, Command::Run(ref args) if args.binary && args.request.to_str() == Some("ops.json"))
        );
        assert!(!run.json);
    }

    #[test]
    fn help_is_an_answer() {
        let answer = asked(&["describe", "--help"]);
        assert_eq!(answer.status, 0);
        assert!(answer.lines[0].0);
        assert!(answer.lines[0].1.contains("kind:slot/generation"), "{answer:?}");
    }
}
