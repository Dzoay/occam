//! Source: shewchuk-1997
//! [`Expansion`]: an exact value held as a sum of doubles, for the exact stage of a decision. Exact towers are not
//! built by default: their work grows about five times a step (S2), and P6 measured about 3.3 µs a decision.

use std::cmp::Ordering;

use crate::exact::{fast_two_sum, two_product, two_sum};

/// An exact value: the sum of its components, which are ordered by increasing magnitude, never zero, and
/// non-overlapping, so the largest alone gives the sign (Shewchuk 1997).
///
/// Arithmetic is exact while every component stays finite and no product's error falls below about 2^-969, that is,
/// for inputs of moderate size multiplied a few at a time. [`Expansion::sign`] says when a component is not finite.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Expansion(Vec<f64>);

/// `e + b`, exactly, with zeros removed (Shewchuk's Grow-Expansion).
fn grow(e: &[f64], b: f64) -> Vec<f64> {
    let mut grown = Vec::with_capacity(e.len().saturating_add(1));
    let mut running = b;
    for component in e {
        let (sum, error) = two_sum(running, *component);
        running = sum;
        if error != 0.0 {
            grown.push(error);
        }
    }
    if running != 0.0 {
        grown.push(running);
    }
    grown
}

impl Expansion {
    /// `x`, exactly. Either zero has no components.
    #[must_use]
    pub fn of(x: f64) -> Self {
        Self(grow(&[], x))
    }

    /// The components, smallest first.
    #[must_use]
    pub fn components(&self) -> &[f64] {
        &self.0
    }

    /// `self + other`, exactly: `other`'s components grown in one at a time.
    #[must_use]
    pub fn sum(&self, other: &Self) -> Self {
        Self(other.0.iter().fold(self.0.clone(), |sum, component| grow(&sum, *component)))
    }

    /// `self - other`, exactly.
    #[must_use]
    pub fn difference(&self, other: &Self) -> Self {
        self.sum(&other.negated())
    }

    /// `self * b`, exactly, with zeros removed (Shewchuk's Scale-Expansion).
    #[must_use]
    pub fn scale(&self, b: f64) -> Self {
        let mut components = self.0.iter();
        let Some(first) = components.next() else {
            return Self::default();
        };
        let mut scaled = Vec::with_capacity(self.0.len().saturating_mul(2));
        let mut keep = |error: f64| {
            if error != 0.0 {
                scaled.push(error);
            }
        };
        let (mut running, error) = two_product(*first, b);
        keep(error);
        for component in components {
            let (product, product_error) = two_product(*component, b);
            let (sum, error) = two_sum(running, product_error);
            keep(error);
            let (next, error) = fast_two_sum(product, sum);
            keep(error);
            running = next;
        }
        keep(running);
        Self(scaled)
    }

    /// `self * other`, exactly: the sum of `self` scaled by each of `other`'s components.
    #[must_use]
    pub fn product(&self, other: &Self) -> Self {
        other.0.iter().fold(Self::default(), |product, component| product.sum(&self.scale(*component)))
    }

    /// `-self`, exactly.
    #[must_use]
    pub fn negated(&self) -> Self {
        Self(self.0.iter().map(|component| -component).collect())
    }

    /// The exact sign, from the largest component: `None` if any component is not finite, since the arithmetic was
    /// then not exact.
    #[must_use]
    pub fn sign(&self) -> Option<Ordering> {
        if !self.0.iter().all(|component| component.is_finite()) {
            return None;
        }
        // The largest component is finite and never zero, so it compares with zero.
        self.0.last().map_or(Some(Ordering::Equal), |largest| largest.partial_cmp(&0.0))
    }

    /// An approximation of the value: the components summed from the smallest.
    #[must_use]
    pub fn estimate(&self) -> f64 {
        self.0.iter().fold(0.0, |sum, component| sum + component)
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::splitmix::SplitMix64;
    use crate::testing::{double, narrow, scaled, sum_scaled};

    /// The exact value of an expansion, times `2^shift`.
    fn value(e: &Expansion, shift: i32) -> i128 {
        sum_scaled(e.components(), shift)
    }

    /// The places of the highest and lowest set bits of a normal double, as powers of two.
    fn bit_span(x: f64) -> (i32, i32) {
        let bits = x.to_bits();
        let exponent = i32::try_from((bits >> 52) & 0x7FF).unwrap();
        assert!(exponent > 0 && exponent < 0x7FF, "{x} is normal");
        let mantissa = (bits & 0x000F_FFFF_FFFF_FFFF) | 0x0010_0000_0000_0000;
        let trailing = i32::try_from(mantissa.trailing_zeros()).unwrap();
        (exponent.checked_sub(1023).unwrap(), exponent.checked_sub(1075).unwrap().checked_add(trailing).unwrap())
    }

    /// The components are in increasing magnitude, never zero, and the bits of each lie wholly above the last's.
    fn assert_well_formed(e: &Expansion) {
        let mut below: Option<i32> = None;
        for component in e.components() {
            assert_ne!(*component, 0.0, "{e:?} has a zero");
            let (highest, lowest) = bit_span(*component);
            if let Some(previous) = below {
                assert!(previous < lowest, "{e:?} overlaps");
            }
            below = Some(highest);
        }
    }

    fn of_all(values: &[f64]) -> Expansion {
        values.iter().fold(Expansion::default(), |sum, value| sum.sum(&Expansion::of(*value)))
    }

    #[test]
    fn zero_has_no_components_and_its_sign_is_equal() {
        assert_eq!(Expansion::of(0.0).components(), [] as [f64; 0]);
        assert_eq!(Expansion::of(-0.0).components(), [] as [f64; 0]);
        assert_eq!(Expansion::default().sign(), Some(Ordering::Equal));
        assert_eq!(Expansion::of(2.5).scale(0.0), Expansion::default());
        assert_eq!(Expansion::of(1.5).difference(&Expansion::of(1.5)), Expansion::default());
    }

    #[test]
    fn sums_and_differences_are_exact_and_well_formed() {
        let mut generator = SplitMix64::new(21);
        for _ in 0..2_000 {
            let terms: Vec<f64> = (0..4).map(|_| double(&mut generator, -30, 30)).collect();
            let sum = of_all(&terms);
            assert_well_formed(&sum);
            assert_eq!(value(&sum, 90), sum_scaled(&terms, 90), "{terms:?}");
            let other = of_all(&[double(&mut generator, -30, 30), double(&mut generator, -30, 30)]);
            let difference = sum.difference(&other);
            assert_well_formed(&difference);
            assert_eq!(value(&difference, 90), value(&sum, 90).checked_sub(value(&other, 90)).unwrap());
        }
    }

    #[test]
    fn scaling_and_products_are_exact_and_well_formed() {
        let mut generator = SplitMix64::new(22);
        for _ in 0..2_000 {
            let mut short = |low, high| narrow(&mut generator, low, high, 20);
            let a = of_all(&[short(-3, 3), short(-30, -25)]);
            let b = of_all(&[short(-3, 3), short(-30, -25)]);
            let factor = short(-3, 3);
            let scaled_up = a.scale(factor);
            assert_well_formed(&scaled_up);
            let want = value(&a, 50).checked_mul(scaled(factor, 50)).unwrap();
            assert_eq!(value(&scaled_up, 100), want, "{a:?} * {factor}");
            let product = a.product(&b);
            assert_well_formed(&product);
            assert_eq!(value(&product, 100), value(&a, 50).checked_mul(value(&b, 50)).unwrap(), "{a:?} * {b:?}");
        }
    }

    #[test]
    fn negation_is_exact() {
        let a = of_all(&[1.0, f64::EPSILON / 4.0, -3.0 * f64::EPSILON * f64::EPSILON]);
        assert_eq!(value(&a.negated(), 105), value(&a, 105).checked_neg().unwrap());
        assert_eq!(a.negated().sign(), Some(Ordering::Less));
    }

    #[test]
    fn the_sign_is_exact_where_plain_doubles_round_it_away() {
        let (a, b) = (1.0 + f64::EPSILON, 1.0 - f64::EPSILON);
        assert_eq!(a * b - 1.0, 0.0, "plain arithmetic loses it");
        let exact = Expansion::of(a).product(&Expansion::of(b)).difference(&Expansion::of(1.0));
        assert_eq!(exact.sign(), Some(Ordering::Less));
        assert_eq!(exact.estimate(), -f64::EPSILON * f64::EPSILON);
    }

    /// `(q - p) * (s - r)`, exactly, in units of 2^-106.
    fn exact_product(p: f64, q: f64, r: f64, s: f64) -> i128 {
        let first = scaled(q, 53).checked_sub(scaled(p, 53)).unwrap();
        first.checked_mul(scaled(s, 53).checked_sub(scaled(r, 53)).unwrap()).unwrap()
    }

    #[test]
    fn the_orientation_of_nearly_collinear_points_is_exact() {
        // Kettner and others' classroom example: a point near (0.5, 0.5) against the line through (12, 12) and
        // (24, 24), where plain doubles get many signs wrong.
        let unit = f64::EPSILON / 2.0;
        let (bx, by, cx, cy) = (12.0, 12.0, 24.0, 24.0);
        let mut wrong = 0;
        for i in 0..64_i32 {
            for j in 0..64_i32 {
                let (ax, ay) = (0.5 + f64::from(i) * unit, 0.5 + f64::from(j) * unit);
                let plain = (bx - ax) * (cy - ay) - (by - ay) * (cx - ax);
                let of = Expansion::of;
                let first = of(bx).difference(&of(ax)).product(&of(cy).difference(&of(ay)));
                let exact = first.difference(&of(by).difference(&of(ay)).product(&of(cx).difference(&of(ax))));
                let want = exact_product(ax, bx, ay, cy).checked_sub(exact_product(ay, by, ax, cx)).unwrap();
                assert_eq!(exact.sign(), Some(want.cmp(&0)), "({i}, {j})");
                if plain.partial_cmp(&0.0) != Some(want.cmp(&0)) {
                    wrong += 1;
                }
            }
        }
        assert!(wrong > 0, "the case is hard enough that plain doubles err");
    }

    #[test]
    fn a_component_that_is_not_finite_has_no_sign() {
        assert_eq!(Expansion::of(f64::NAN).sign(), None);
        assert_eq!(Expansion::of(f64::INFINITY).sum(&Expansion::of(1.0)).sign(), None);
        assert_eq!(Expansion::of(f64::MAX).scale(4.0).sign(), None);
    }
}
