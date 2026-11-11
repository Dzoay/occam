//! Source: original
//! Prints the determinism stream's hashes, which `gates determinism` compares across builds and instruction levels.
#![allow(clippy::print_stdout, clippy::print_stderr, reason = "a command-line tool prints")]

use std::env;
use std::num::NonZeroUsize;
use std::process::ExitCode;

use occam_math::stream::{self, OPERATIONS};
use occam_math::{Exec, Lanes, Level, Real};

const USAGE: &str = "usage: determinism --threads <count> [--level <name>] | determinism --levels";

/// Prints the stream's hashes when run as `R`, one line per operation.
fn show<R: Real>(executor: &str, level: Level, exec: &Exec) {
    for (operation, hash) in OPERATIONS.iter().zip(stream::run::<R>(level, exec)) {
        println!("{operation} {executor} {hash:016x}");
    }
}

/// What the command line asks for.
enum Request {
    /// The names of the levels this machine has, widest first.
    Levels,
    /// The stream, on this many threads, at the level of this name, or the widest when there is none.
    Run(NonZeroUsize, Option<String>),
}

fn request() -> Option<Request> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [flag] if flag == "--levels" => Some(Request::Levels),
        [flag, count] if flag == "--threads" => Some(Request::Run(count.parse().ok()?, None)),
        [flag, count, level_flag, name] if flag == "--threads" && level_flag == "--level" => {
            Some(Request::Run(count.parse().ok()?, Some(name.clone())))
        }
        _ => None,
    }
}

fn main() -> ExitCode {
    let (threads, name) = match request() {
        Some(Request::Levels) => {
            for level in Level::available() {
                println!("{}", level.name());
            }
            return ExitCode::SUCCESS;
        }
        Some(Request::Run(threads, name)) => (threads, name),
        None => {
            eprintln!("{USAGE}");
            return ExitCode::FAILURE;
        }
    };
    let level = match name {
        None => Level::detect(),
        Some(name) => {
            let Some(level) = Level::named(&name) else {
                eprintln!("this machine has no level `{name}`");
                return ExitCode::FAILURE;
            };
            level
        }
    };
    let exec = match Exec::threads(threads) {
        Ok(exec) => exec,
        Err(report) => {
            eprintln!("{}", report.message(occam_report::DisplayUnits::default()));
            return ExitCode::FAILURE;
        }
    };
    println!(
        "build {} simd128={} threads={} level={}",
        env::consts::ARCH,
        cfg!(target_feature = "simd128"),
        exec.thread_count(),
        level.name()
    );
    show::<f64>("scalar", level, &exec);
    show::<Lanes<2>>("lanes2", level, &exec);
    show::<Lanes<4>>("lanes4", level, &exec);
    show::<Lanes<8>>("lanes8", level, &exec);
    ExitCode::SUCCESS
}
