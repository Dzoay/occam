//! Source: original
//! Times loading a model of about 10⁵ faces in the native format, parsing and checking apart, as the `construction`
//! exit asks and the files risk watches: a binary encoding would speed up only the parsing. A goal, not a gate (L46).
//! In release:
//!
//! `cargo run --release -p occam-io --example load -- [squares across] [repetitions]`
//!
//! The model is occam-topo's plate, `2k² + 4k` faces for `k` squares across (223 gives 100,350). Parsing is timed for
//! the file as saved, which is compact, and for the same JSON pretty-printed, as it was first written. Checking is every
//! edge measured and the whole model checked, on one thread and on every thread the machine has. Times are medians.
#![allow(clippy::print_stdout, clippy::print_stderr, reason = "a command-line tool prints")]
#![allow(clippy::arithmetic_side_effects, reason = "a timing tool's own counting, far from any bound")]

use std::env;
use std::num::NonZeroUsize;
use std::process::ExitCode;
use std::thread;
use std::time::{Duration, Instant};

use occam_math::Exec;
use occam_topo::Face;
use occam_topo::workloads::Plate;

/// The median of `times`.
fn median(mut times: Vec<Duration>) -> Duration {
    times.sort_unstable();
    times.get(times.len() / 2).copied().unwrap_or_default()
}

/// Parses `bytes` `repeats` times: the median.
fn parsing(bytes: &[u8], repeats: usize) -> Option<Duration> {
    let mut times = Vec::new();
    for _ in 0..repeats {
        let start = Instant::now();
        occam_io::parse(bytes).ok()?;
        times.push(start.elapsed());
    }
    Some(median(times))
}

/// Measures and checks what `bytes` hold `repeats` times on `exec`: the median.
fn checking(bytes: &[u8], exec: &Exec, repeats: usize) -> Option<Duration> {
    let mut times = Vec::new();
    for _ in 0..repeats {
        let restore = occam_io::parse(bytes).ok()?;
        let start = Instant::now();
        restore.finish(exec).ok()?;
        times.push(start.elapsed());
    }
    Some(median(times))
}

/// Megabytes, to a tenth.
fn megabytes(bytes: &[u8]) -> f64 {
    f64::from(u32::try_from(bytes.len() / 1000).unwrap_or(u32::MAX)) / 1000.0
}

fn main() -> ExitCode {
    let arguments: Vec<String> = env::args().skip(1).collect();
    let number = |index: usize, default: usize| arguments.get(index).and_then(|a| a.parse().ok()).unwrap_or(default);
    let (across, repeats) = (number(0, 223), number(1, 5));
    let serial = Exec::serial();
    let start = Instant::now();
    let Some(plate) = Plate::new(across, &serial) else {
        eprintln!("the plate did not commit");
        return ExitCode::FAILURE;
    };
    let faces = plate.snapshot().ids::<Face>().len();
    println!("plate {across} squares across: {faces} faces, built in {:.1} s", start.elapsed().as_secs_f64());
    let start = Instant::now();
    let Ok(bytes) = occam_io::save(plate.snapshot()) else {
        eprintln!("the plate did not save");
        return ExitCode::FAILURE;
    };
    println!("saved in {:.2} s", start.elapsed().as_secs_f64());
    let pretty = serde_json::from_slice::<serde_json::Value>(&bytes).and_then(|v| serde_json::to_vec_pretty(&v));
    let Ok(pretty) = pretty else {
        eprintln!("the file did not read as JSON");
        return ExitCode::FAILURE;
    };
    let threads = thread::available_parallelism().unwrap_or(NonZeroUsize::MIN);
    let Ok(pool) = Exec::threads(threads) else {
        eprintln!("the threads did not start");
        return ExitCode::FAILURE;
    };
    println!("{repeats} repetitions each; medians");
    for (name, file) in [("compact", &bytes), ("pretty", &pretty)] {
        let Some(time) = parsing(file, repeats) else {
            eprintln!("the {name} file did not parse");
            return ExitCode::FAILURE;
        };
        println!("{name}: {:.1} MB, parse {:.3} s", megabytes(file), time.as_secs_f64());
    }
    for (name, exec) in [("1 thread", &serial), ("all threads", &pool)] {
        let Some(time) = checking(&bytes, exec, repeats) else {
            eprintln!("the plate did not load");
            return ExitCode::FAILURE;
        };
        println!("{name} ({}): measure and check {:.3} s", exec.thread_count(), time.as_secs_f64());
    }
    ExitCode::SUCCESS
}
