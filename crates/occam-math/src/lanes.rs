//! Source: original
//! [`Lanes`]: several numbers side by side, as a plain array. Each operation is the `f64` operation on every lane, in a
//! loop the compiler vectorises where it is inlined, so there is no `unsafe` and only one definition of each result.

use std::ops::{Add, Div, Mul, Neg, Sub};

use crate::mask::LaneMask;
use crate::real::Real;

/// `W` numbers side by side. Every operation works lane by lane.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Lanes<const W: usize>([f64; W]);

impl<const W: usize> Lanes<W> {
    /// The lanes, lane 0 first.
    #[must_use]
    pub const fn new(lanes: [f64; W]) -> Self {
        Self(lanes)
    }

    /// The lanes as an array, lane 0 first.
    #[must_use]
    pub const fn to_array(self) -> [f64; W] {
        self.0
    }

    /// `op` on every lane.
    #[inline(always)]
    fn map(self, op: impl Fn(f64) -> f64) -> Self {
        Self(self.0.map(op))
    }

    /// `test` on every pair of lanes.
    #[inline(always)]
    fn compare(self, rhs: Self, test: impl Fn(f64, f64) -> bool) -> LaneMask<W> {
        let mut mask = [false; W];
        for ((held, a), b) in mask.iter_mut().zip(self.0).zip(rhs.0) {
            *held = test(a, b);
        }
        LaneMask::new(mask)
    }

    /// `op` on every pair of lanes.
    #[inline(always)]
    fn zip(mut self, rhs: Self, op: impl Fn(f64, f64) -> f64) -> Self {
        for (lane, other) in self.0.iter_mut().zip(rhs.0) {
            *lane = op(*lane, other);
        }
        self
    }
}

impl<const W: usize> Add for Lanes<W> {
    type Output = Self;

    #[inline(always)]
    fn add(self, rhs: Self) -> Self {
        self.zip(rhs, |a, b| a + b)
    }
}

impl<const W: usize> Sub for Lanes<W> {
    type Output = Self;

    #[inline(always)]
    fn sub(self, rhs: Self) -> Self {
        self.zip(rhs, |a, b| a - b)
    }
}

impl<const W: usize> Mul for Lanes<W> {
    type Output = Self;

    #[inline(always)]
    fn mul(self, rhs: Self) -> Self {
        self.zip(rhs, |a, b| a * b)
    }
}

impl<const W: usize> Div for Lanes<W> {
    type Output = Self;

    #[inline(always)]
    fn div(self, rhs: Self) -> Self {
        self.zip(rhs, |a, b| a / b)
    }
}

impl<const W: usize> Neg for Lanes<W> {
    type Output = Self;

    #[inline(always)]
    fn neg(self) -> Self {
        self.map(|a| -a)
    }
}

impl<const W: usize> Real for Lanes<W> {
    const LANES: usize = W;

    type Mask = LaneMask<W>;

    #[inline(always)]
    fn splat(x: f64) -> Self {
        Self([x; W])
    }

    /// A plain loop, because `array::from_fn` can fall out of line and stop a kernel vectorising (P4).
    #[inline(always)]
    fn from_fn(mut next: impl FnMut() -> f64) -> Self {
        let mut lanes = [0.0; W];
        for lane in &mut lanes {
            *lane = next();
        }
        Self(lanes)
    }

    #[inline(always)]
    fn for_each_lane(self, mut visit: impl FnMut(f64)) {
        for lane in self.0 {
            visit(lane);
        }
    }

    #[inline(always)]
    fn abs(self) -> Self {
        self.map(f64::abs)
    }

    #[inline(always)]
    fn sqrt(self) -> Self {
        self.map(f64::sqrt)
    }

    #[inline(always)]
    fn copysign(self, sign: Self) -> Self {
        self.zip(sign, f64::copysign)
    }

    #[inline(always)]
    fn next_up(self) -> Self {
        self.map(f64::next_up)
    }

    #[inline(always)]
    fn next_down(self) -> Self {
        self.map(f64::next_down)
    }

    #[inline(always)]
    fn lt(self, rhs: Self) -> LaneMask<W> {
        self.compare(rhs, |a, b| a < b)
    }

    #[inline(always)]
    fn le(self, rhs: Self) -> LaneMask<W> {
        self.compare(rhs, |a, b| a <= b)
    }

    #[inline(always)]
    fn select(mask: LaneMask<W>, yes: Self, mut no: Self) -> Self {
        for ((lane, held), picked) in no.0.iter_mut().zip(mask.to_array()).zip(yes.0) {
            if held {
                *lane = picked;
            }
        }
        no
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const A: [f64; 4] = [1.5, -2.25, 0.1, 1e300];
    const B: [f64; 4] = [0.5, 4.0, 0.2, 1e300];

    fn bits(lanes: [f64; 4]) -> [u64; 4] {
        lanes.map(f64::to_bits)
    }

    #[test]
    fn binary_operations_work_lane_by_lane() {
        let (a, b) = (Lanes::new(A), Lanes::new(B));
        let lanes = |op: fn(f64, f64) -> f64| [op(1.5, 0.5), op(-2.25, 4.0), op(0.1, 0.2), op(1e300, 1e300)];
        assert_eq!(bits((a + b).to_array()), bits(lanes(|x, y| x + y)));
        assert_eq!(bits((a - b).to_array()), bits(lanes(|x, y| x - y)));
        assert_eq!(bits((a * b).to_array()), bits(lanes(|x, y| x * y)));
        assert_eq!(bits((a / b).to_array()), bits(lanes(|x, y| x / y)));
    }

    #[test]
    fn unary_operations_work_lane_by_lane() {
        let a = Lanes::new(A);
        assert_eq!(bits((-a).to_array()), bits([-1.5, 2.25, -0.1, -1e300]));
        assert_eq!(bits(a.abs().to_array()), bits([1.5, 2.25, 0.1, 1e300]));
        let roots = Lanes::new([4.0, 2.0, 0.0, -0.0]).sqrt();
        assert_eq!(bits(roots.to_array()), bits([2.0, f64::from_bits(0x3FF6_A09E_667F_3BCD), 0.0, -0.0]));
        let signs = Lanes::new([-0.0, 0.0, -1.0, 2.0]).copysign(Lanes::new([1.0, -0.0, 3.0, -4.0]));
        assert_eq!(bits(signs.to_array()), bits([0.0, -0.0, 1.0, -2.0]));
        let steps = [1.0, -0.0, f64::MAX, f64::NEG_INFINITY];
        assert_eq!(bits(Lanes::new(steps).next_up().to_array()), bits(steps.map(f64::next_up)));
        assert_eq!(bits(Lanes::new(steps).next_down().to_array()), bits(steps.map(f64::next_down)));
    }

    #[test]
    fn comparisons_work_lane_by_lane_and_are_false_beside_nan() {
        let a = Lanes::new([1.0, f64::NAN, -0.0, 2.0]);
        let b = Lanes::new([2.0, 1.0, 0.0, 2.0]);
        assert_eq!(a.lt(b).to_array(), [true, false, false, false]);
        assert_eq!(a.le(b).to_array(), [true, false, true, true]);
        assert_eq!(b.lt(a).to_array(), [false, false, false, false]);
        assert_eq!(b.le(a).to_array(), [false, false, true, true]);
    }

    #[test]
    fn select_works_lane_by_lane_with_bits_unchanged() {
        let mask = LaneMask::new([true, false, true, false]);
        let picked = Lanes::select(mask, Lanes::new([-0.0, 1.0, 2.0, 3.0]), Lanes::new([0.0, -1.0, f64::NAN, -0.0]));
        assert_eq!(bits(picked.to_array()), bits([-0.0, -1.0, 2.0, -0.0]));
    }

    #[test]
    fn splat_fills_every_lane() {
        assert_eq!(Lanes::<3>::splat(2.5).to_array(), [2.5; 3]);
    }

    #[test]
    fn lanes_are_built_and_visited_from_lane_zero() {
        let mut next = 0.0;
        let lanes = Lanes::<3>::from_fn(|| {
            next += 1.0;
            next
        });
        assert_eq!(lanes.to_array(), [1.0, 2.0, 3.0]);
        let mut seen = Vec::new();
        lanes.for_each_lane(|x| seen.push(x));
        assert_eq!(seen, [1.0, 2.0, 3.0]);
    }

    #[test]
    fn a_lane_count_is_its_width() {
        assert_eq!(Lanes::<4>::LANES, 4);
    }
}
