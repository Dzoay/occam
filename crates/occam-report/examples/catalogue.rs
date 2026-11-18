//! Source: original
//! Prints the catalogue of codes and remedies as JSON. `gates errors` reads this. With `--record` it prints the same
//! JSON one value a line, which is what `ci/catalogue.json` holds (`just catalogue-record`), so that a change to any
//! code or remedy is a line of that file's diff.
#![allow(clippy::print_stdout, clippy::print_stderr, reason = "a command-line tool prints")]

use std::process::ExitCode;

use occam_report::{DisplayUnits, catalogue_json};

/// `json` one value a line, or the reason it cannot be read.
fn record(json: &str) -> Result<String, String> {
    let value: serde_json::Value = serde_json::from_str(json).map_err(|error| error.to_string())?;
    serde_json::to_string_pretty(&value).map_err(|error| error.to_string())
}

fn main() -> ExitCode {
    let listed = catalogue_json().map_err(|report| report.message(DisplayUnits::default()));
    let printed =
        if std::env::args().any(|arg| arg == "--record") { listed.and_then(|json| record(&json)) } else { listed };
    match printed {
        Ok(json) => {
            println!("{json}");
            ExitCode::SUCCESS
        }
        Err(message) => {
            eprintln!("{message}");
            ExitCode::FAILURE
        }
    }
}
