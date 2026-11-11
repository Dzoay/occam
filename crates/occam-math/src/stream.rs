//! Source: original
//! The determinism stream: a fixed sequence of operations whose outputs must hash the same on every build
//! (`docs/DETERMINISM.md`). `gates determinism` runs it through `examples/determinism.rs`.

use std::thread;

use crate::hash::Digest;
use crate::real::Real;
use crate::splitmix::SplitMix64;

/// How many operations the stream hashes.
pub const OPERATION_COUNT: usize = 10;

/// The operations, in the order [`Hashes`] gives them.
pub const OPERATIONS: [&str; OPERATION_COUNT] = ["add", "sub", "mul", "div", "neg", "abs", "sqrt", "mixed", "lt", "le"];

/// One hash per operation, in the order of [`OPERATIONS`].
pub type Hashes = [u64; OPERATION_COUNT];

/// The blocks the stream is divided into, so that threads can share it.
const BLOCKS: u64 = 64;

/// The inputs per block. Divisible by every lane width from 1 to 8.
const STEPS: usize = 840;

/// The sign and mantissa bits of an `f64`.
const SIGN_AND_MANTISSA: u64 = 0x800F_FFFF_FFFF_FFFF;

/// An awkward number: the cases IEEE arithmetic treats specially.
fn special(bits: u64) -> f64 {
    match bits & 15 {
        0 => 0.0,
        1 => -0.0,
        2 => 1.0,
        3 => -1.0,
        4 => f64::INFINITY,
        5 => f64::NEG_INFINITY,
        6 => f64::NAN,
        7 => f64::MAX,
        8 => f64::MIN,
        9 => f64::MIN_POSITIVE,
        10 => f64::from_bits(1),
        11 => -f64::from_bits(1),
        12 => 0.1,
        13 => 1.0 / 3.0,
        14 => 9_007_199_254_740_992.0, // 2^53
        _ => 1e-300,
    }
}

/// A number of the size a length has in millimetres: any sign and mantissa, with an exponent from 2^-15 to 2^16.
fn length(bits: u64) -> f64 {
    let exponent = 0x3F0_u64.wrapping_add((bits >> 52) & 0x1F);
    f64::from_bits((bits & SIGN_AND_MANTISSA).wrapping_add(exponent << 52))
}

/// The next input: an awkward number, any bit pattern at all, a subnormal, or a length.
fn draw(generator: &mut SplitMix64) -> f64 {
    let kind = generator.next_u64() >> 62;
    let bits = generator.next_u64();
    match kind {
        0 => special(bits),
        1 => f64::from_bits(bits),
        2 => f64::from_bits(bits & SIGN_AND_MANTISSA),
        _ => length(bits),
    }
}

/// Every operation on one set of inputs.
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
fn outputs<R: Real>(a: R, b: R, c: R) -> [R; OPERATION_COUNT] {
    [
        a + b,
        a - b,
        a * b,
        a / b,
        -a,
        a.abs(),
        a.sqrt(),
        ((a + b) * c - a / b).abs().sqrt(),
        R::select(a.lt(b), a, b),
        R::select(a.le(b), b, c),
    ]
}

/// The hashes of one block: its inputs depend on `index` alone, so a block gives the same hashes on any thread.
fn block<R: Real>(index: u64) -> Hashes {
    let [mut first, mut second, mut third] =
        [0, 1, 2].map(|operand| SplitMix64::new((index << 2).wrapping_add(operand)));
    let mut digests = [Digest::EMPTY; OPERATION_COUNT];
    for _ in (0..STEPS).step_by(R::LANES) {
        let (a, b, c) =
            (R::from_fn(|| draw(&mut first)), R::from_fn(|| draw(&mut second)), R::from_fn(|| draw(&mut third)));
        for (digest, output) in digests.iter_mut().zip(outputs(a, b, c)) {
            output.for_each_lane(|x| digest.write_value(x));
        }
    }
    digests.map(Digest::finish)
}

/// The hashes of a run of blocks.
fn blocks<R: Real>(indices: &[u64]) -> Vec<Hashes> {
    indices.iter().map(|index| block::<R>(*index)).collect()
}

/// The stream's hash for each operation, given the blocks' hashes in block order.
fn combine(blocks: impl IntoIterator<Item = Hashes>) -> Hashes {
    let mut digests = [Digest::EMPTY; OPERATION_COUNT];
    for hashes in blocks {
        for (digest, hash) in digests.iter_mut().zip(hashes) {
            digest.write_u64(hash);
        }
    }
    digests.map(Digest::finish)
}

/// Runs the stream as `R`, the blocks shared among `threads` threads, and gives the hash of each operation. The calling
/// thread takes the first share and the others are spawned, so one thread spawns nothing. Results are merged in block
/// order, which is why the thread count cannot matter.
#[must_use]
pub fn run<R: Real>(threads: usize) -> Hashes {
    let indices: Vec<u64> = (0..BLOCKS).collect();
    let mut shares = indices.chunks(indices.len().div_ceil(threads.max(1)));
    let own = shares.next().unwrap_or_default();
    combine(thread::scope(|scope| {
        let workers: Vec<_> = shares.map(|share| scope.spawn(move || blocks::<R>(share))).collect();
        let mut merged = blocks::<R>(own);
        // A worker that panicked adds no blocks, so the hashes disagree and the gate fails.
        for worker in workers {
            merged.extend(worker.join().unwrap_or_default());
        }
        merged
    }))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::Lanes;

    /// What the stream must give. The file is the one home of these numbers, and `gates determinism` reads it too.
    const RECORDED: &str = include_str!("../../../ci/determinism-hashes.txt");

    fn want() -> Hashes {
        let mut lines = RECORDED.lines().filter(|line| !line.starts_with('#'));
        OPERATIONS.map(|name| {
            let (operation, hash) = lines.next().unwrap().split_once(' ').unwrap();
            assert_eq!(operation, name, "the record is in the order of the operations");
            u64::from_str_radix(hash, 16).unwrap()
        })
    }

    #[test]
    fn a_stream_of_operations_gives_the_same_hash_on_every_target_scalar_and_lanes() {
        let want = want();
        assert_eq!(run::<f64>(1), want, "scalar");
        assert_eq!(run::<Lanes<2>>(1), want, "two lanes");
        assert_eq!(run::<Lanes<4>>(1), want, "four lanes");
        assert_eq!(run::<Lanes<8>>(1), want, "eight lanes");
    }

    #[test]
    fn every_lane_width_from_one_to_eight_agrees() {
        let want = want();
        assert_eq!(run::<Lanes<1>>(1), want);
        assert_eq!(run::<Lanes<3>>(1), want);
        assert_eq!(run::<Lanes<5>>(1), want);
        assert_eq!(run::<Lanes<6>>(1), want);
        assert_eq!(run::<Lanes<7>>(1), want);
    }

    // Threads are not available on wasm32-wasip2, where the stream is single-threaded by contract.
    #[cfg(not(target_family = "wasm"))]
    #[test]
    fn the_hash_does_not_depend_on_the_thread_count() {
        let want = want();
        for threads in [0, 2, 3, 7, 64, 200] {
            assert_eq!(run::<f64>(threads), want, "{threads} threads");
            assert_eq!(run::<Lanes<4>>(threads), want, "{threads} threads, four lanes");
        }
    }

    #[test]
    fn the_blocks_differ_from_one_another() {
        assert_ne!(block::<f64>(0), block::<f64>(1));
    }

    #[test]
    fn the_operands_differ_from_one_another() {
        let mut first = SplitMix64::new(0);
        let mut second = SplitMix64::new(1);
        assert_ne!(draw(&mut first).to_bits(), draw(&mut second).to_bits());
    }

    #[test]
    fn combining_depends_on_the_order_of_the_blocks() {
        let (x, y) = ([1; OPERATION_COUNT], [2; OPERATION_COUNT]);
        assert_ne!(combine([x, y]), combine([y, x]));
    }

    #[test]
    fn a_length_spans_the_exponents_it_claims_and_keeps_its_sign_and_mantissa() {
        assert_eq!(length(0).to_bits(), 0x3F00_0000_0000_0000, "2^-15");
        assert_eq!(length(0x01F0_0000_0000_0000).to_bits(), 0x40F0_0000_0000_0000, "2^16");
        assert_eq!(length(0x8000_0000_0000_0001).to_bits(), 0xBF00_0000_0000_0001, "sign and mantissa");
    }

    #[test]
    fn the_inputs_reach_every_awkward_case() {
        let values: Vec<f64> = (0..16).map(special).collect();
        let bits: std::collections::BTreeSet<u64> = values.iter().map(|x| x.to_bits()).collect();
        assert_eq!(bits.len(), 16, "the sixteen special values are distinct");
        let mut generator = SplitMix64::new(0);
        let drawn: Vec<f64> = (0..4096).map(|_| draw(&mut generator)).collect();
        assert!(drawn.iter().any(|x| x.is_nan()));
        assert!(drawn.iter().any(|x| x.is_infinite()));
        assert!(drawn.iter().any(|x| x.is_subnormal()));
        assert!(drawn.iter().any(|x| x.to_bits() == (-0.0_f64).to_bits()));
        assert!(drawn.iter().any(|x| x.is_normal() && x.abs() < 1e5 && x.abs() > 1e-5));
    }
}
