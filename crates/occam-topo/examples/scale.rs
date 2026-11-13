//! Source: splitmix64
//! Times the topology at about a million faces, as S3-P3 timed its synthetic records: taking a snapshot, transactions
//! of edits behind it, each with its commit, and the whole-model check at one to eight threads. These are goals, not
//! gates (L46): `topology.scale` reports them. In release:
//!
//! `cargo run --release -p occam-topo --features workloads --example scale -- [squares across] [repetitions]`
//!
//! An edit splits one top square of the plate by its diagonal (six new entities and four changed records), and its
//! commit checks it, where S3-P3's edits were Euler operators with no checker. Times are medians of the repetitions.
#![allow(clippy::print_stdout, clippy::print_stderr, reason = "a command-line tool prints")]
#![allow(clippy::arithmetic_side_effects, reason = "a timing tool's own counting, far from any bound")]

use std::env;
use std::hint::black_box;
use std::num::NonZeroUsize;
use std::process::ExitCode;
use std::time::{Duration, Instant};

use occam_math::Exec;
use occam_topo::Face;
use occam_topo::workloads::Plate;

/// `SplitMix64` (Steele, Lea and Flood 2014): which squares the scattered edits split, the same on every run.
struct Squares(u64);

impl Squares {
    fn next(&mut self, below: usize) -> usize {
        self.0 = self.0.wrapping_add(0x9e37_79b9_7f4a_7c15);
        let mut z = self.0;
        z = (z ^ (z >> 30)).wrapping_mul(0xbf58_476d_1ce4_e5b9);
        z = (z ^ (z >> 27)).wrapping_mul(0x94d0_49bb_1331_11eb);
        usize::try_from((z ^ (z >> 31)) % u64::try_from(below).unwrap_or(1)).unwrap_or(0)
    }

    /// `count` different squares of a top `across` squares across, anywhere on it.
    fn scattered(&mut self, across: usize, count: usize) -> Vec<(usize, usize)> {
        let mut taken = std::collections::BTreeSet::new();
        while taken.len() < count {
            taken.insert(self.next(across * across));
        }
        let mut order: Vec<usize> = taken.into_iter().collect();
        for k in (1..order.len()).rev() {
            order.swap(k, self.next(k + 1));
        }
        order.into_iter().map(|n| (n % across, n / across)).collect()
    }

    /// `count` squares side by side along a row from a square chosen anywhere, wrapping to the next row.
    fn local(&mut self, across: usize, count: usize) -> Vec<(usize, usize)> {
        let start = self.next(across * across - count);
        (start..start + count).map(|n| (n % across, n / across)).collect()
    }
}

/// The median of `times`.
fn median(mut times: Vec<Duration>) -> Duration {
    times.sort_unstable();
    times.get(times.len() / 2).copied().unwrap_or_default()
}

fn micros(d: Duration) -> f64 {
    d.as_secs_f64() * 1e6
}

/// Taking a snapshot of the plate, `repeats` times.
fn snapshot(plate: &Plate, repeats: usize) -> Duration {
    median(
        (0..repeats)
            .map(|_| {
                let start = Instant::now();
                let taken = black_box(plate.snapshot().clone());
                let spent = start.elapsed();
                drop(taken);
                spent
            })
            .collect(),
    )
}

/// A transaction of `k` edits behind the plate's snapshot, each splitting the squares `pick` gives, and its commit:
/// the median time per edit.
fn edits(plate: &Plate, k: usize, repeats: usize, pick: &mut dyn FnMut() -> Vec<(usize, usize)>) -> Option<Duration> {
    let serial = Exec::serial();
    let mut times = Vec::new();
    for _ in 0..repeats {
        let squares = pick();
        let start = Instant::now();
        let edit = plate.split(plate.snapshot(), &squares);
        let committed = edit.commit(&serial).ok()?;
        let spent = start.elapsed();
        if committed.0.ids::<Face>().len() != plate.snapshot().ids::<Face>().len() + k {
            return None;
        }
        drop(committed);
        times.push(spent / u32::try_from(k).ok()?);
    }
    Some(median(times))
}

/// The whole-model check of the plate on `threads` threads: the median time, and whether it found nothing.
fn check_all(plate: &Plate, threads: usize, repeats: usize) -> Option<(Duration, bool)> {
    let exec = Exec::threads(NonZeroUsize::new(threads)?).ok()?;
    let mut clean = true;
    let times = (0..repeats)
        .map(|_| {
            let start = Instant::now();
            let found = black_box(plate.snapshot().check_all(&exec));
            let spent = start.elapsed();
            clean &= found.is_empty();
            spent
        })
        .collect();
    Some((median(times), clean))
}

fn run(across: usize, repeats: usize) -> Option<()> {
    let start = Instant::now();
    let plate = Plate::new(across, &Exec::serial())?;
    let built = start.elapsed();
    let faces = plate.snapshot().ids::<Face>().len();
    println!("plate {across} squares across: {faces} faces, built and committed in {:.1} s", built.as_secs_f64());
    println!("{repeats} repetitions each; medians\n");
    println!("take a snapshot: {:.1} us", micros(snapshot(&plate, repeats)));
    let mut squares = Squares(2026);
    let rows: [(&str, usize, bool); 5] = [
        ("k=1 scattered", 1, true),
        ("k=1 local", 1, false),
        ("k=100 scattered", 100, true),
        ("k=100 local", 100, false),
        ("k=10,000 scattered", 10_000, true),
    ];
    for (name, k, scattered) in rows {
        if k > across * across {
            println!("{name}: skipped, as the top has only {} squares", across * across);
            continue;
        }
        let mut pick = || if scattered { squares.scattered(across, k) } else { squares.local(across, k) };
        let each = edits(&plate, k, repeats, &mut pick)?;
        println!("{name}: {:.2} us an edit, its share of the snapshot and the commit included", micros(each));
    }
    println!();
    let mut one = None;
    for threads in [1, 2, 4, 8] {
        let (spent, clean) = check_all(&plate, threads, repeats)?;
        let base = *one.get_or_insert(spent);
        let speedup = base.as_secs_f64() / spent.as_secs_f64();
        println!(
            "check_all on {threads} threads: {:.1} ms, {speedup:.2} times one thread{}",
            spent.as_secs_f64() * 1e3,
            if clean { "" } else { ", NOT CLEAN" }
        );
    }
    Some(())
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    let number = |k: usize, default: usize| args.get(k).map_or(Some(default), |a| a.parse().ok());
    let (Some(across), Some(repeats)) = (number(0, 706), number(1, 7)) else {
        eprintln!("usage: scale [squares across, 706 by default] [repetitions, 7 by default]");
        return ExitCode::FAILURE;
    };
    if run(across, repeats).is_some() {
        ExitCode::SUCCESS
    } else {
        eprintln!("a workload failed: a commit refused it, or a thread pool could not start");
        ExitCode::FAILURE
    }
}
