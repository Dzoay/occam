//! Source: original
//! The lane kernels `gates perf` measures, each written once over [`Real`] and run at width 1 as `f64` and at its
//! chosen [`width`](crate::width) as [`Lanes`](crate::Lanes): natively by instruction count, and in the browser's
//! simd128 build by time. A kernel that is not clearly faster at its width runs at width 1 (L29).

use crate::interval::Interval;
use crate::predicate::{Predicate, Ring};
use crate::real::Real;
use crate::trig::{atan2, sin_cos};

/// The items each kernel runs over: a multiple of every width.
pub const ITEMS: usize = 4096;

/// `ITEMS * count` numbers from `-scale` to `scale`, the same on every target.
#[must_use]
pub fn inputs(count: usize, scale: f64, seed: u64) -> Vec<f64> {
    let mut state = seed;
    // 2^-21, so that the top 21 bits of the state give a fraction in [0, 1).
    let unit = 1.0 / 2_097_152.0;
    (0..ITEMS.saturating_mul(count))
        .map(|_| {
            state = state.wrapping_mul(0x5851_F42D_4C95_7F2D).wrapping_add(0x1405_7B7E_F767_814F);
            let fraction = f64::from(u32::try_from(state >> 43).unwrap_or(0)) * unit;
            (fraction * 2.0 - 1.0) * scale
        })
        .collect()
}

/// Calls `kernel` on `R::LANES` items at a time and gives every lane of what it returns. `items` holds the `N`
/// operands one after another, each a contiguous run, and so does the result for its `M` parts, as a batch method
/// lays out struct-of-arrays data.
fn each<R: Real, const N: usize, const M: usize>(items: &[f64], kernel: impl Fn([R; N]) -> [R; M]) -> Vec<f64> {
    let count = items.len().checked_div(N).unwrap_or(0);
    let mut out = vec![0.0; count.saturating_mul(M)];
    let mut inputs: [_; N] = std::array::from_fn(|operand| {
        let start = operand.saturating_mul(count);
        items.get(start..start.saturating_add(count)).unwrap_or_default().chunks_exact(R::LANES)
    });
    let mut blocks = out.chunks_exact_mut(count.max(1));
    let mut outputs: [_; M] = std::array::from_fn(|_| blocks.next().unwrap_or_default().chunks_exact_mut(R::LANES));
    for _ in 0..count.checked_div(R::LANES).unwrap_or(0) {
        let operands: [R; N] = std::array::from_fn(|operand| {
            let chunk = inputs.get_mut(operand).and_then(Iterator::next).unwrap_or_default();
            let mut lanes = chunk.iter();
            R::from_fn(|| lanes.next().copied().unwrap_or(0.0))
        });
        for (result, slots) in kernel(operands).into_iter().zip(&mut outputs) {
            let mut slots = slots.next().unwrap_or_default().iter_mut();
            result.for_each_lane(|value| {
                if let Some(slot) = slots.next() {
                    *slot = value;
                }
            });
        }
    }
    out
}

/// Comparison masks: the smallest of three by selection, as a box test does, from three operands.
#[must_use]
pub fn masks<R: Real>(items: &[f64]) -> Vec<f64> {
    each::<R, 3, 1>(items, |[a, b, c]| {
        let smaller = R::select(a.lt(b), a, b);
        [R::select(c.le(smaller), c, smaller)]
    })
}

/// The orientation of three points, `(b - a) x (c - a)`.
struct Orientation;

impl Predicate<6> for Orientation {
    const DEGREE: u32 = 2;

    #[inline(always)]
    fn value<T: Ring>(&self, [ax, ay, bx, by, cx, cy]: &[T; 6]) -> T {
        let first = bx.sub(ax).mul(&cy.sub(ay));
        first.sub(&by.sub(ay).mul(&cx.sub(ax)))
    }
}

/// The predicate filter: an orientation in interval arithmetic, from six operands, giving each enclosure's bounds.
#[must_use]
pub fn filter<R: Real>(items: &[f64]) -> Vec<f64> {
    each::<R, 6, 2>(items, |inputs| {
        let value = Orientation.value(&inputs.map(Interval::exact));
        [value.lower(), value.upper()]
    })
}

/// `sin_cos` of every item.
#[must_use]
pub fn sines<R: Real>(items: &[f64]) -> Vec<f64> {
    each::<R, 1, 2>(items, |[x]| {
        let (sin, cos) = sin_cos(x);
        [sin, cos]
    })
}

/// `atan2` from two operands, `y` first.
#[must_use]
pub fn angles<R: Real>(items: &[f64]) -> Vec<f64> {
    each::<R, 2, 1>(items, |[y, x]| [atan2(y, x)])
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::hash::canonical_bits;
    use crate::lanes::Lanes;

    fn bits(values: &[f64]) -> Vec<u64> {
        values.iter().map(|value| canonical_bits(*value)).collect()
    }

    #[test]
    fn the_inputs_fill_their_range_and_repeat() {
        let values = inputs(2, 4.0, 7);
        assert_eq!(values.len(), 2 * ITEMS);
        assert!(values.iter().all(|value| value.abs() <= 4.0));
        assert!(values.iter().any(|value| *value < -3.9) && values.iter().any(|value| *value > 3.9));
        assert_eq!(bits(&values), bits(&inputs(2, 4.0, 7)));
        assert_ne!(bits(&values), bits(&inputs(2, 4.0, 8)));
    }

    #[test]
    fn every_kernel_gives_the_same_bits_at_every_width() {
        let items = inputs(6, 3.0, 1);
        for (name, one, wide) in [
            ("masks", masks::<f64>(&items), masks::<Lanes<8>>(&items)),
            ("filter", filter::<f64>(&items), filter::<Lanes<4>>(&items)),
            ("sines", sines::<f64>(&items), sines::<Lanes<2>>(&items)),
            ("angles", angles::<f64>(&items), angles::<Lanes<4>>(&items)),
        ] {
            assert!(!one.is_empty(), "{name}");
            assert_eq!(bits(&one), bits(&wide), "{name}");
        }
    }

    #[test]
    fn the_kernels_compute_what_they_say() {
        assert_eq!(masks::<f64>(&[3.0, 5.0, 1.0, 4.0, 2.0, 6.0]), [1.0, 4.0]);
        let enclosure = filter::<f64>(&[0.0, 0.0, 1.0, 0.0, 0.0, 2.0]);
        assert!(enclosure[0] <= 2.0 && 2.0 <= enclosure[1]);
        assert_eq!(sines::<f64>(&[0.0, -0.0]), [0.0, -0.0, 1.0, 1.0]);
        assert_eq!(angles::<f64>(&[1.0, -2.0, 1.0, 2.0]), [atan2(1.0, 1.0), atan2(-2.0, 2.0)]);
    }
}
