//! `cargo xtask gates <gate>` and `cargo xtask gates all` (PLAN section 5). Each gate is a pure check over text, tested
//! in its module against a fixture holding one violation, plus a `run` that gathers that text from the repository.
//! Every gate fails closed: no input to check is a failure, never a pass.

mod complexity;
pub mod counts;
mod deps;
mod determinism;
pub mod docs;
mod errors;
mod layers;
mod mutants;
mod perf;
mod plan;
mod provenance;

use std::process::ExitCode;

use crate::repo::Repo;
use crate::verdict::{self, Finding, Verdict};

/// A gate: checks the repository, given its own command-line arguments.
type Gate = fn(&Repo, &[String]) -> Verdict;

/// The gates in force, in the order `gates all` runs them.
const GATES: [(&str, Gate); 11] = [
    ("layers", layers::run),
    ("complexity", complexity::run),
    ("provenance", provenance::run),
    ("docs", docs::run),
    ("plan", plan::run),
    ("deps", deps::run),
    ("errors", errors::run),
    ("determinism", determinism::run),
    ("counts", counts::run),
    ("mutants", mutants::run),
    ("perf", perf::run),
];

/// A gate selected to run, with its name and its own arguments.
type Selected<'a> = (&'static str, Gate, &'a [String]);

pub fn main(args: &[String]) -> ExitCode {
    let (json, args) = take_json(args);
    let Some(selected) = select(&args) else { return usage() };
    let repo = Repo::locate();
    let results: Vec<(&str, Verdict)> =
        selected.into_iter().map(|(name, gate, rest)| (name, gate(&repo, rest))).collect();
    if json {
        match verdict::to_json(&results) {
            Ok(text) => println!("{text}"),
            Err(error) => {
                eprintln!("gates: cannot write JSON: {error}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        print!("{}", verdict::to_text(&results));
    }
    if verdict::all_ok(&results) { ExitCode::SUCCESS } else { ExitCode::FAILURE }
}

/// Whether `--json` was given, and the arguments without it.
fn take_json(args: &[String]) -> (bool, Vec<String>) {
    let rest: Vec<String> = args.iter().filter(|arg| *arg != "--json").cloned().collect();
    (rest.len() != args.len(), rest)
}

/// The gates `args` name: `all`, every gate; `all --except <gate>[,<gate>]`, every gate but those; or one gate with its
/// arguments. `None` for anything else, an unknown gate in the list included.
fn select(args: &[String]) -> Option<Vec<Selected<'_>>> {
    let all = |except: &[&str]| {
        GATES.iter().filter(|(name, _)| !except.contains(name)).map(|&(name, gate)| (name, gate, &[][..])).collect()
    };
    match args.split_first()? {
        (name, []) if name == "all" => Some(all(&[])),
        (name, [flag, except]) if name == "all" && flag == "--except" => {
            let except: Vec<&str> = except.split(',').collect();
            except.iter().all(|e| GATES.iter().any(|(gate, _)| gate == e)).then(|| all(&except))
        }
        (name, rest) => GATES.iter().find(|(gate, _)| gate == name).map(|&(name, gate)| vec![(name, gate, rest)]),
    }
}

fn usage() -> ExitCode {
    let names: Vec<&str> = GATES.iter().map(|(name, _)| *name).collect();
    eprintln!("usage: cargo xtask gates <all [--except <gate>[,<gate>]]|{}> [--json]", names.join("|"));
    ExitCode::FAILURE
}

/// Refuses arguments a gate does not take.
fn no_args(args: &[String]) -> Result<(), Vec<Finding>> {
    if args.is_empty() {
        Ok(())
    } else {
        Err(vec![Finding::new(
            "gates.usage",
            "",
            format!("unexpected arguments: {}", args.join(" ")),
            "this gate takes none",
        )])
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(args: &[&str]) -> Vec<String> {
        args.iter().map(ToString::to_string).collect()
    }

    fn names(args: &[&str]) -> Option<Vec<&'static str>> {
        select(&owned(args)).map(|gates| gates.iter().map(|(name, ..)| *name).collect())
    }

    #[test]
    fn gates_are_selected_by_name() {
        let every: Vec<&str> = GATES.iter().map(|(name, _)| *name).collect();
        assert_eq!(names(&["all"]), Some(every.clone()));
        let but_mutants: Vec<&str> = every.iter().copied().filter(|name| *name != "mutants").collect();
        assert_eq!(names(&["all", "--except", "mutants"]), Some(but_mutants));
        assert_eq!(names(&["counts", "--update"]), Some(vec!["counts"]));
        let args = owned(&["counts", "--update"]);
        assert_eq!(select(&args).map(|gates| gates.first().map(|(.., rest)| rest.len())), Some(Some(1)));
    }

    #[test]
    fn several_gates_can_be_excepted() {
        let got = names(&["all", "--except", "mutants,counts"]).unwrap();
        assert!(!got.contains(&"mutants") && !got.contains(&"counts") && got.contains(&"layers"));
    }

    #[test]
    fn anything_else_is_refused() {
        let refused: [&[&str]; 5] =
            [&[], &["all", "--except", "nothing"], &["all", "mutants"], &["all", "--skip", "docs"], &["no"]];
        for args in refused {
            assert_eq!(names(args), None, "{args:?}");
        }
    }

    #[test]
    fn json_is_a_flag_anywhere() {
        assert_eq!(take_json(&owned(&["docs", "--json"])), (true, owned(&["docs"])));
        assert_eq!(take_json(&owned(&["--json", "all"])), (true, owned(&["all"])));
        assert_eq!(take_json(&owned(&["all"])), (false, owned(&["all"])));
    }

    #[test]
    fn a_gate_refuses_arguments_it_does_not_take() {
        assert_eq!(no_args(&[]), Ok(()));
        let found = no_args(&owned(&["x", "y"])).unwrap_err();
        assert_eq!(found[0].code, "gates.usage");
        assert!(found[0].message.contains("x y"), "{found:?}");
    }
}
