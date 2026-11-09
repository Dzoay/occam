//! Source: original
//! Prints the determinism stream's hashes, which `gates determinism` compares across builds.
#![allow(clippy::print_stdout, clippy::print_stderr, reason = "a command-line tool prints")]

use std::env;
use std::process::ExitCode;

use occam_math::stream::{self, OPERATIONS};
use occam_math::{Lanes, Real};

/// Prints the stream's hashes when run as `R`, one line per operation.
fn show<R: Real>(executor: &str, threads: usize) {
    for (operation, hash) in OPERATIONS.iter().zip(stream::run::<R>(threads)) {
        println!("{operation} {executor} {hash:016x}");
    }
}

/// The `--threads <count>` given on the command line, and nothing else.
fn threads() -> Option<usize> {
    let args: Vec<String> = env::args().skip(1).collect();
    match args.as_slice() {
        [flag, count] if flag == "--threads" => count.parse().ok(),
        _ => None,
    }
}

fn main() -> ExitCode {
    let Some(threads) = threads() else {
        eprintln!("usage: determinism --threads <count>");
        return ExitCode::FAILURE;
    };
    println!("build {} simd128={} threads={threads}", env::consts::ARCH, cfg!(target_feature = "simd128"));
    show::<f64>("scalar", threads);
    show::<Lanes<2>>("lanes2", threads);
    show::<Lanes<4>>("lanes4", threads);
    show::<Lanes<8>>("lanes8", threads);
    ExitCode::SUCCESS
}
