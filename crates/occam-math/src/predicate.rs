//! Source: shewchuk-1997
//! Robust decisions: where a polynomial of the inputs lies against a band `[-eps, eps]`. The interval filter runs four
//! items at a time in lanes (P6), and what it cannot certify goes to the exact stage on stored inputs, or is
//! `Undecided` on computed ones, "cannot decide at this resolution" (S2, O11).

use std::cmp::Ordering;

use crate::expansion::Expansion;
use crate::interval::Interval;
use crate::lanes::Lanes;
use crate::real::Real;

/// What a predicate is written over, once: [`Interval`] for the filter, [`Expansion`] for the exact stage, `f64` for
/// plain arithmetic.
pub trait Ring: Clone {
    /// The constant `x`, exactly.
    fn of(x: f64) -> Self;

    /// `self + rhs`.
    #[must_use]
    fn add(&self, rhs: &Self) -> Self;

    /// `self - rhs`.
    #[must_use]
    fn sub(&self, rhs: &Self) -> Self;

    /// `self * rhs`.
    #[must_use]
    fn mul(&self, rhs: &Self) -> Self;
}

impl<R: Real> Ring for Interval<R> {
    #[inline(always)]
    fn of(x: f64) -> Self {
        Self::exact(R::splat(x))
    }

    #[inline(always)]
    fn add(&self, rhs: &Self) -> Self {
        *self + *rhs
    }

    #[inline(always)]
    fn sub(&self, rhs: &Self) -> Self {
        *self - *rhs
    }

    #[inline(always)]
    fn mul(&self, rhs: &Self) -> Self {
        *self * *rhs
    }
}

impl Ring for Expansion {
    fn of(x: f64) -> Self {
        Self::of(x)
    }

    fn add(&self, rhs: &Self) -> Self {
        self.sum(rhs)
    }

    fn sub(&self, rhs: &Self) -> Self {
        self.difference(rhs)
    }

    fn mul(&self, rhs: &Self) -> Self {
        self.product(rhs)
    }
}

impl Ring for f64 {
    #[inline(always)]
    fn of(x: f64) -> Self {
        x
    }

    #[inline(always)]
    fn add(&self, rhs: &Self) -> Self {
        self + rhs
    }

    #[inline(always)]
    fn sub(&self, rhs: &Self) -> Self {
        self - rhs
    }

    #[inline(always)]
    fn mul(&self, rhs: &Self) -> Self {
        self * rhs
    }
}

/// A polynomial in `N` inputs, written once over [`Ring`]. S2's D1 to D5 are predicates in the geometry and
/// intersection crates.
pub trait Predicate<const N: usize>: Sync {
    /// The most inputs multiplied together in one term. It sets the range of inputs the exact stage is exact for.
    const DEGREE: u32;

    /// The polynomial's value at `inputs`.
    fn value<T: Ring>(&self, inputs: &[T; N]) -> T;
}

/// Where a value lies against the band `[-eps, eps]`.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Side {
    /// Below `-eps`.
    Below,
    /// From `-eps` to `eps`, both included.
    Within,
    /// Above `eps`.
    Above,
}

/// A decision, or "cannot decide at this resolution" (O11): the value's enclosure reaches across an edge of the band,
/// or the inputs are not ones the exact stage is exact for.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Verdict {
    /// The value certainly lies on this side.
    Decided(Side),
    /// No side can be certified.
    Undecided,
}

/// How a batch of decisions went: P6 says to record how many reached the exact stage, since each 1% costs about
/// 33 ns a decision on one thread. Every operation reports it as a `measure.decisions` note.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Tally {
    /// Decisions made, by the filter or the exact stage.
    pub decided: u64,
    /// Of those, how many needed the exact stage.
    pub exact: u64,
    /// Items left undecided.
    pub undecided: u64,
}

impl Tally {
    /// Both tallies together.
    #[must_use]
    pub const fn merged(self, other: Self) -> Self {
        Self {
            decided: self.decided.saturating_add(other.decided),
            exact: self.exact.saturating_add(other.exact),
            undecided: self.undecided.saturating_add(other.undecided),
        }
    }
}

/// The items the filter runs together.
const WIDTH: usize = crate::width::FILTER;

/// The side a value certainly lies on, given bounds below and above it, if it can be certified.
fn certain(lower: f64, upper: f64, eps: f64) -> Option<Side> {
    if upper < -eps {
        Some(Side::Below)
    } else if lower > eps {
        Some(Side::Above)
    } else if lower >= -eps && upper <= eps {
        Some(Side::Within)
    } else {
        None
    }
}

/// The interval filter on up to [`WIDTH`] items at once, each input given as its middle and radius.
fn filter<const N: usize, P: Predicate<N>, I>(
    predicate: &P,
    group: &[I],
    eps: f64,
    input: impl Fn(&I, usize) -> (f64, f64),
) -> [Option<Side>; WIDTH] {
    let columns: [Interval<Lanes<WIDTH>>; N] = std::array::from_fn(|k| {
        let (mut mid, mut rad) = ([0.0; WIDTH], [0.0; WIDTH]);
        for ((mid, rad), item) in mid.iter_mut().zip(&mut rad).zip(group) {
            (*mid, *rad) = input(item, k);
        }
        Interval::new(Lanes::new(mid), Lanes::new(rad))
    });
    let value = predicate.value(&columns);
    let mut sides = [None; WIDTH];
    for ((side, lower), upper) in sides.iter_mut().zip(value.lower().to_array()).zip(value.upper().to_array()) {
        *side = certain(lower, upper, eps);
    }
    sides
}

/// Whether every input is zero or of a size the exact stage is exact for at this degree: no product of `degree` of
/// them, or of their differences, overflows or loses bits below the smallest normal number.
fn exact_range<const N: usize>(item: &[f64; N], degree: u32) -> bool {
    let most = i32::try_from(960_u32.checked_div(degree).unwrap_or(960)).unwrap_or(960);
    let power = |exponent: i32| f64::from_bits(u64::try_from(exponent.saturating_add(1023)).unwrap_or(0) << 52);
    let (low, high) = (power(most.saturating_sub(53).saturating_neg()), power(most));
    item.iter().all(|x| *x == 0.0 || (low <= x.abs() && x.abs() <= high))
}

/// The exact stage: the value as an [`Expansion`], compared exactly with the band's edges.
fn exact<const N: usize, P: Predicate<N>>(predicate: &P, item: &[f64; N], eps: f64) -> Verdict {
    if !exact_range(item, P::DEGREE) {
        return Verdict::Undecided;
    }
    let value = predicate.value(&item.map(Expansion::of));
    let edge = Expansion::of(eps);
    match (value.difference(&edge).sign(), value.sum(&edge).sign()) {
        (Some(Ordering::Greater), Some(_)) => Verdict::Decided(Side::Above),
        (Some(_), Some(Ordering::Less)) => Verdict::Decided(Side::Below),
        (Some(_), Some(_)) => Verdict::Decided(Side::Within),
        _ => Verdict::Undecided,
    }
}

impl Tally {
    /// Counts one verdict.
    fn count(&mut self, verdict: Verdict) {
        match verdict {
            Verdict::Decided(_) => self.decided = self.decided.saturating_add(1),
            Verdict::Undecided => self.undecided = self.undecided.saturating_add(1),
        }
    }
}

/// Decisions on stored numbers, which are exact (S2 item 2): the interval filter four at a time in lanes, then the exact
/// stage for each item it cannot certify. An item is `Undecided` only when an input is not finite, or too large or too
/// small for the exact stage to be exact at the predicate's degree (`2^(960 / degree)` and `2^-(960 / degree - 53)`).
/// `eps` is taken as its absolute value. Appends one verdict per item to `out`, in order.
pub fn decide_stored<const N: usize, P: Predicate<N>>(
    predicate: &P,
    inputs: &[[f64; N]],
    eps: f64,
    out: &mut Vec<Verdict>,
) -> Tally {
    let eps = eps.abs();
    let mut tally = Tally::default();
    for group in inputs.chunks(WIDTH) {
        let quick = filter(predicate, group, eps, |item, k| (item.get(k).copied().unwrap_or(0.0), 0.0));
        for (item, side) in group.iter().zip(quick) {
            let verdict = side.map_or_else(
                || {
                    tally.exact = tally.exact.saturating_add(1);
                    exact(predicate, item, eps)
                },
                Verdict::Decided,
            );
            tally.count(verdict);
            out.push(verdict);
        }
    }
    tally
}

/// Decisions on computed values, which carry their certified radius (S2 item 3): the interval filter alone, so an
/// enclosure reaching across an edge of the band is `Undecided`, "cannot decide at this resolution". `eps` is taken as
/// its absolute value. Appends one verdict per item to `out`, in order.
pub fn decide_computed<const N: usize, P: Predicate<N>>(
    predicate: &P,
    inputs: &[[Interval; N]],
    eps: f64,
    out: &mut Vec<Verdict>,
) -> Tally {
    let eps = eps.abs();
    let mut tally = Tally::default();
    for group in inputs.chunks(WIDTH) {
        let quick = filter(predicate, group, eps, |item, k| item.get(k).map_or((0.0, 0.0), |x| (x.mid(), x.radius())));
        for side in quick.into_iter().take(group.len()) {
            let verdict = side.map_or(Verdict::Undecided, Verdict::Decided);
            tally.count(verdict);
            out.push(verdict);
        }
    }
    tally
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::lanes::Lanes;
    use crate::splitmix::SplitMix64;
    use crate::testing::{double, scaled};

    /// Which side of the line from `a` to `b` the point `c` is: `(b - a) x (c - a)`.
    struct Orientation;

    impl Predicate<6> for Orientation {
        const DEGREE: u32 = 2;

        fn value<T: Ring>(&self, [ax, ay, bx, by, cx, cy]: &[T; 6]) -> T {
            let first = bx.sub(ax).mul(&cy.sub(ay));
            first.sub(&by.sub(ay).mul(&cx.sub(ax)))
        }
    }

    /// The exact orientation, in integers, for inputs that are multiples of 2^-56 below 2^5.
    fn exact_side(item: &[f64; 6]) -> Side {
        let [ax, ay, bx, by, cx, cy] = item.map(|x| scaled(x, 56));
        let difference = |p: i128, q: i128| p.checked_sub(q).unwrap();
        let first = difference(bx, ax).checked_mul(difference(cy, ay)).unwrap();
        let value = difference(first, difference(by, ay).checked_mul(difference(cx, ax)).unwrap());
        match value.cmp(&0) {
            Ordering::Less => Side::Below,
            Ordering::Equal => Side::Within,
            Ordering::Greater => Side::Above,
        }
    }

    /// Kettner's classroom grid: a point near (0.5, 0.5) against the line through (12, 12) and (24, 24).
    fn grid() -> Vec<[f64; 6]> {
        let unit = f64::EPSILON / 2.0;
        let mut items = Vec::new();
        for i in 0..64_i32 {
            for j in 0..64_i32 {
                items.push([0.5 + f64::from(i) * unit, 0.5 + f64::from(j) * unit, 12.0, 12.0, 24.0, 24.0]);
            }
        }
        items
    }

    #[test]
    fn stored_decisions_agree_with_the_exact_sign_on_nearly_degenerate_inputs() {
        let items = grid();
        let mut out = Vec::new();
        let tally = decide_stored(&Orientation, &items, 0.0, &mut out);
        assert_eq!(out.len(), items.len());
        for (item, verdict) in items.iter().zip(&out) {
            assert_eq!(*verdict, Verdict::Decided(exact_side(item)), "{item:?}");
        }
        assert!(tally.exact > 0, "{tally:?}: the grid needs the exact stage");
        assert_eq!((tally.decided, tally.undecided), (4096, 0));
    }

    #[test]
    fn ordinary_inputs_never_reach_the_exact_stage() {
        let mut generator = SplitMix64::new(41);
        let items: Vec<[f64; 6]> = (0..1_001).map(|_| std::array::from_fn(|_| double(&mut generator, -3, 3))).collect();
        let mut out = Vec::new();
        let tally = decide_stored(&Orientation, &items, 0.0, &mut out);
        assert_eq!(tally, Tally { decided: 1_001, exact: 0, undecided: 0 });
        for (item, verdict) in items.iter().zip(&out) {
            assert_eq!(*verdict, Verdict::Decided(exact_side(item)), "{item:?}");
        }
    }

    #[test]
    fn the_band_includes_its_edges() {
        // The value is exactly 2 at (0, 0), (1, 0), (0, 2).
        let item = [[0.0, 0.0, 1.0, 0.0, 0.0, 2.0]];
        let side = |eps: f64| {
            let mut out = Vec::new();
            decide_stored(&Orientation, &item, eps, &mut out);
            out[0]
        };
        assert_eq!(side(1.0), Verdict::Decided(Side::Above));
        assert_eq!(side(2.0), Verdict::Decided(Side::Within));
        assert_eq!(side(3.0), Verdict::Decided(Side::Within));
        let below = [[0.0, 0.0, 1.0, 0.0, 0.0, -2.0]];
        let mut out = Vec::new();
        decide_stored(&Orientation, &below, 1.5, &mut out);
        assert_eq!(out, [Verdict::Decided(Side::Below)]);
    }

    #[test]
    fn stored_inputs_beyond_the_exact_range_are_undecided_rather_than_wrong() {
        let items = [
            [f64::NAN, 0.0, 1.0, 0.0, 0.0, 1.0],
            [1e300, 1e300, 1e300 + 1e284, 1e300, 1e300, 1e300 + 1e284],
            [0.0, 0.0, 1.0, 0.0, 0.0, 1.0],
        ];
        let mut out = Vec::new();
        let tally = decide_stored(&Orientation, &items, 0.0, &mut out);
        assert_eq!(out[0], Verdict::Undecided);
        assert_eq!(out[2], Verdict::Decided(Side::Above));
        assert!(tally.undecided >= 1 && tally.decided >= 1, "{tally:?}");
    }

    #[test]
    fn computed_decisions_are_undecided_when_the_enclosure_straddles_the_band() {
        // The value is about 2, give or take what the radius of the last input allows.
        let point = |y: f64, radius: f64| {
            let exact = Interval::exact;
            [[exact(0.0), exact(0.0), exact(1.0), exact(0.0), exact(0.0), Interval::new(y, radius)]]
        };
        let decide = |item: [[Interval; 6]; 1], eps: f64| {
            let mut out = Vec::new();
            let tally = decide_computed(&Orientation, &item, eps, &mut out);
            (out[0], tally)
        };
        assert_eq!(decide(point(2.0, 0.1), 1.0).0, Verdict::Decided(Side::Above));
        let (verdict, tally) = decide(point(2.0, 0.1), 2.0);
        assert_eq!(verdict, Verdict::Undecided);
        assert_eq!(tally, Tally { decided: 0, exact: 0, undecided: 1 });
        assert_eq!(decide(point(2.0, 0.1), 2.5).0, Verdict::Decided(Side::Within));
        assert_eq!(decide(point(2.0, 0.0), 2.0).0, Verdict::Undecided, "even an exact input's value is rounded");
    }

    #[test]
    fn computed_decisions_never_contradict_the_exact_side() {
        let mut generator = SplitMix64::new(42);
        let mut items = grid();
        items.extend((0..1_000).map(|_| std::array::from_fn(|_| double(&mut generator, -3, 3))));
        let computed: Vec<[Interval; 6]> = items.iter().map(|item| item.map(Interval::exact)).collect();
        let mut out = Vec::new();
        let tally = decide_computed(&Orientation, &computed, 0.0, &mut out);
        for (item, verdict) in items.iter().zip(&out) {
            if let Verdict::Decided(side) = verdict {
                assert_eq!(*side, exact_side(item), "{item:?}");
            }
        }
        assert!(tally.undecided > 0 && tally.decided > 0 && tally.exact == 0, "{tally:?}");
    }

    /// `2a + b - c`, which uses every operation of a ring.
    fn every_operation<T: Ring>([a, b, c]: &[T; 3]) -> T {
        T::of(2.0).mul(a).add(b).sub(c)
    }

    #[test]
    fn every_ring_does_every_operation() {
        let inputs = [1.5, 0.25, 4.0];
        assert_eq!(every_operation(&inputs), -0.75);
        assert_eq!(every_operation(&inputs.map(Interval::exact)).mid(), -0.75);
        assert_eq!(every_operation(&inputs.map(Expansion::of)), Expansion::of(-0.75));
        let lanes = every_operation(&inputs.map(|x| Interval::exact(Lanes::<2>::splat(x))));
        assert_eq!(lanes.mid().to_array(), [-0.75; 2]);
    }

    #[test]
    fn the_band_is_certain_only_when_the_enclosure_is_clear_of_its_edges() {
        assert_eq!(certain(-3.0, -2.5, 2.0), Some(Side::Below));
        assert_eq!(certain(-3.0, -2.0, 2.0), None, "touching -eps from below could be within");
        assert_eq!(certain(2.5, 3.0, 2.0), Some(Side::Above));
        assert_eq!(certain(2.0, 3.0, 2.0), None, "touching eps from above could be within");
        assert_eq!(certain(-2.0, 2.0, 2.0), Some(Side::Within));
        assert_eq!(certain(-2.1, 2.0, 2.0), None);
        assert_eq!(certain(-2.0, 2.1, 2.0), None);
        assert_eq!(certain(f64::NAN, 1.0, 2.0), None);
    }

    #[test]
    fn stored_inputs_that_would_underflow_are_undecided_rather_than_wrong() {
        // The value is 1e-400, which no double holds: the exact stage would lose it and call the points collinear.
        let items = [[0.0, 0.0, 1e-200, 0.0, 0.0, 1e-200]];
        let mut out = Vec::new();
        let tally = decide_stored(&Orientation, &items, 0.0, &mut out);
        assert_eq!(out, [Verdict::Undecided]);
        assert_eq!(tally, Tally { decided: 0, exact: 1, undecided: 1 });
    }

    #[test]
    fn the_exact_range_narrows_with_the_degree() {
        let item = [1e100, 1e-100, 0.0];
        assert!(exact_range(&item, 2));
        assert!(!exact_range(&item, 4), "1e100 is above 2^240");
        assert!(!exact_range(&[1e-150, 1.0, 1.0], 2), "1e-150 is below 2^-427");
        assert!(!exact_range(&[f64::INFINITY, 1.0, 1.0], 2));
        assert!(exact_range(&[-1e100, -1.0, -1e-100], 2), "the sign does not matter");
    }

    #[test]
    fn plain_arithmetic_runs_the_same_formula() {
        let value = Orientation.value(&[0.0, 0.0, 1.0, 0.0, 0.0, 2.0]);
        assert_eq!(value, 2.0);
        let interval = Orientation.value(&[0.0, 0.0, 1.0, 0.0, 0.0, 2.0].map(Interval::exact));
        assert_eq!(interval.mid(), 2.0);
        assert_eq!(Orientation.value(&[0.0, 0.0, 1.0, 0.0, 0.0, 2.0].map(Expansion::of)).estimate(), 2.0);
    }

    #[test]
    fn tallies_merge() {
        let one = Tally { decided: 3, exact: 1, undecided: 2 };
        assert_eq!(one.merged(one), Tally { decided: 6, exact: 2, undecided: 4 });
        assert_eq!(Tally { decided: u64::MAX, ..one }.merged(one).decided, u64::MAX);
    }
}
