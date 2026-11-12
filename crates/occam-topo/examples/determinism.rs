//! Source: original
//! Prints the topology's determinism hashes, which `gates determinism` compares across builds and thread counts, or
//! with `--record` the record they are held to, as this build gives it (docs/DETERMINISM.md).
#![allow(clippy::print_stdout, clippy::print_stderr, reason = "a command-line tool prints")]

use std::env;
use std::num::NonZeroUsize;
use std::process::ExitCode;

use occam_math::Exec;
use occam_report::{DisplayUnits, Report};
use occam_topo::probe::{OPERATIONS, hashes};

const USAGE: &str = "usage: determinism --threads <count> | determinism --record";

/// The record's header. The record is what this build prints on one thread: see the probe's documentation.
const HEADER: &str = "\
# `gates determinism` and occam-topo's probe test: what the topology's probe must hash to, `<operation> <hash>`, in the
# order of `OPERATIONS` in crates/occam-topo/src/probe.rs. Unlike ci/determinism-hashes.txt, these do not come from a
# separate implementation, as there is no second topology: they are what the x86-64 build printed on one thread
# (`just determinism-topology`). Every other build and thread count is held to them. See docs/DETERMINISM.md before
# changing one.";

/// What the command line asks for.
enum Request {
    /// The record, on one thread.
    Record,
    /// The hashes, on this many threads.
    Run(NonZeroUsize),
}

fn request() -> Option<Request> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [flag] if flag == "--record" => Some(Request::Record),
        [flag, count] if flag == "--threads" => Some(Request::Run(count.parse().ok()?)),
        _ => None,
    }
}

/// Prints the record, as this build gives it on one thread.
fn record() {
    println!("{HEADER}");
    for (operation, hash) in OPERATIONS.iter().zip(hashes(&Exec::serial())) {
        println!("{operation} {hash:016x}");
    }
}

/// Prints the build line, then every hash on `threads` threads.
fn run(threads: NonZeroUsize) -> Result<(), Report> {
    let exec = Exec::threads(threads)?;
    let simd128 = cfg!(target_feature = "simd128");
    println!("build {} simd128={simd128} threads={}", env::consts::ARCH, exec.thread_count());
    for (operation, hash) in OPERATIONS.iter().zip(hashes(&exec)) {
        println!("{operation} scalar {hash:016x}");
    }
    Ok(())
}

fn main() -> ExitCode {
    let outcome = match request() {
        Some(Request::Record) => {
            record();
            Ok(())
        }
        Some(Request::Run(threads)) => run(threads),
        None => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    outcome.map_or_else(
        |report| {
            eprintln!("{}", report.message(DisplayUnits::default()));
            ExitCode::FAILURE
        },
        |()| ExitCode::SUCCESS,
    )
}
