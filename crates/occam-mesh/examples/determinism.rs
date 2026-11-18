//! Source: original
//! Prints the mesher's determinism hashes, which `gates determinism` compares across builds and thread counts, or with
//! `--record` the record they are held to, as this build gives it (docs/DETERMINISM.md).
#![allow(clippy::print_stdout, clippy::print_stderr, reason = "a command-line tool prints")]

use std::env;
use std::iter::once;
use std::num::NonZeroUsize;
use std::process::ExitCode;

use occam_fixtures::mesh::probe::{OPERATIONS, hashes};
use occam_math::Exec;
use occam_report::DisplayUnits;

const USAGE: &str = "usage: determinism --threads <count> | determinism --record";

/// The record's header. The record is what this build prints on one thread: see the probe's documentation.
const HEADER: &str = "\
# `gates determinism` and occam-mesh's probe test: what the mesher's probe must hash to, `<operation> <hash>`, in the
# order of `OPERATIONS` in crates/occam-fixtures/src/mesh/probe.rs. Unlike ci/determinism-hashes.txt, these do not come from a
# separate implementation, as there is no second mesher: they are what the x86-64 build printed on one thread
# (`just determinism-mesh`). Every other build and thread count is held to them. See docs/DETERMINISM.md before
# changing one.
";

/// The record, as this build gives it on one thread.
fn recorded() -> String {
    let rows = OPERATIONS.iter().zip(hashes(&Exec::serial())).map(|(name, hash)| format!("{name} {hash:016x}\n"));
    once(HEADER.to_string()).chain(rows).collect()
}

/// The build line, then every hash on `threads` threads, as the gate reads them; or why the threads did not start.
fn hashed(threads: NonZeroUsize) -> Result<String, String> {
    let exec = Exec::threads(threads).map_err(|report| report.message(DisplayUnits::default()))?;
    let (arch, simd128, count) = (env::consts::ARCH, cfg!(target_feature = "simd128"), exec.thread_count());
    let rows = OPERATIONS.iter().zip(hashes(&exec)).map(|(name, hash)| format!("{name} scalar {hash:016x}\n"));
    Ok(once(format!("build {arch} simd128={simd128} threads={count}\n")).chain(rows).collect())
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let printed = match args.as_slice() {
        [flag] if flag == "--record" => Ok(recorded()),
        [flag, count] if flag == "--threads" => count.parse().map_err(|_| USAGE.to_string()).and_then(hashed),
        _ => Err(USAGE.to_string()),
    };
    match printed {
        Ok(text) => {
            print!("{text}");
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}
