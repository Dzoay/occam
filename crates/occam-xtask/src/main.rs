//! `cargo xtask`: the gates and the developer tasks. Never shipped.
#![allow(clippy::print_stdout, clippy::print_stderr, reason = "a command-line tool prints")]

mod gates;
mod glob;
mod manifest;
mod plan;
#[cfg(test)]
mod probes;
mod repo;
mod verdict;

use std::env;
use std::process::ExitCode;

use repo::Repo;

/// `cargo xtask next [--json]`: for each open milestone, its first step with no `Plan-Step:` footer on `origin/main`.
fn next(args: &[String]) -> ExitCode {
    let repo = Repo::locate();
    let gathered = repo.main_log().and_then(|log| Ok((log, repo.read(plan::PLAN)?)));
    let (log, text) = match gathered {
        Ok(found) => found,
        Err(finding) => {
            eprintln!("next: {}: {}", finding.file, finding.message);
            return ExitCode::FAILURE;
        }
    };
    let planned = plan::planned(&text, |path| repo.read(path).ok());
    let next = plan::next(&planned, &plan::footers(&log));
    if args.iter().any(|arg| arg == "--json") {
        match serde_json::to_string_pretty(&next) {
            Ok(json) => println!("{json}"),
            Err(error) => {
                eprintln!("next: cannot write JSON: {error}");
                return ExitCode::FAILURE;
            }
        }
    } else {
        print!("{}", plan::render(&next));
    }
    ExitCode::SUCCESS
}

/// `cargo xtask merge-counts <base> <ours> <theirs>`: git's merge driver for the test counts. It writes the merge to
/// `<ours>` and exits 0, or leaves `<ours>` alone and exits 1 where the merge does not decide.
fn merge_counts(args: &[String]) -> ExitCode {
    let [base, ours, theirs] = args else {
        eprintln!("usage: cargo xtask merge-counts <base> <ours> <theirs>");
        return ExitCode::FAILURE;
    };
    let read = |path: &String| Repo::read_outside(path).ok();
    let merged = (|| gates::counts::merge(&read(base)?, &read(ours)?, &read(theirs)?))();
    match merged.map(|text| std::fs::write(ours, text)) {
        Some(Ok(())) => ExitCode::SUCCESS,
        _ => ExitCode::FAILURE,
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.split_first() {
        Some((task, rest)) if task == "gates" => gates::main(rest),
        Some((task, rest)) if task == "next" => next(rest),
        Some((task, rest)) if task == "merge-counts" => merge_counts(rest),
        _ => {
            eprintln!("usage: cargo xtask <gates|next|merge-counts> ...");
            ExitCode::FAILURE
        }
    }
}
