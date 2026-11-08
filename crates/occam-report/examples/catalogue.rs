//! Source: original
//! Prints the catalogue of codes and remedies as JSON. `gates errors` reads this.
#![allow(clippy::print_stdout, clippy::print_stderr, reason = "a command-line tool prints")]

use std::process::ExitCode;

use occam_report::{DisplayUnits, catalogue_json};

fn main() -> ExitCode {
    match catalogue_json() {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(report) => {
            eprintln!("{}", report.message(DisplayUnits::default()));
            ExitCode::FAILURE
        }
    }
}
