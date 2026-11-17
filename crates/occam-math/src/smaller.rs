//! Source: original
//! The smaller and the larger of two numbers, each the first where they tie. `f64::min` and `f64::max` may return
//! either zero, so they are banned (`docs/DETERMINISM.md`, rule 2), and every crate above takes these instead.

/// The smaller of two numbers: `a` where they are equal, so the sign of a zero is fixed, and `a` where either is not a
/// number.
#[inline]
#[must_use]
pub fn smaller(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// The larger of two numbers: `a` where they are equal, so the sign of a zero is fixed, and `a` where either is not a
/// number.
#[inline]
#[must_use]
pub fn larger(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn the_smaller_and_the_larger_are_picked_whichever_comes_first() {
        assert_eq!((smaller(1.0, 2.0), smaller(2.0, 1.0)), (1.0, 1.0));
        assert_eq!((larger(1.0, 2.0), larger(2.0, 1.0)), (2.0, 2.0));
        assert_eq!((smaller(-3.0, f64::NEG_INFINITY), larger(-3.0, f64::INFINITY)), (f64::NEG_INFINITY, f64::INFINITY));
    }

    #[test]
    fn equal_numbers_give_the_first_so_a_zero_keeps_its_sign() {
        let bits = |x: f64| x.to_bits();
        assert_eq!((bits(smaller(0.0, -0.0)), bits(smaller(-0.0, 0.0))), (bits(0.0), bits(-0.0)), "the first zero");
        assert_eq!((bits(larger(0.0, -0.0)), bits(larger(-0.0, 0.0))), (bits(0.0), bits(-0.0)), "the first zero");
    }

    #[test]
    fn a_number_is_kept_against_what_is_not_one_and_what_is_not_one_first_stays() {
        assert_eq!((smaller(1.0, f64::NAN), larger(1.0, f64::NAN)), (1.0, 1.0));
        assert!(smaller(f64::NAN, 1.0).is_nan() && larger(f64::NAN, 1.0).is_nan());
    }

    #[test]
    fn a_fold_from_an_infinity_gives_the_least_and_the_greatest() {
        let values = [3.0, -1.0, 2.0];
        assert_eq!(values.into_iter().fold(f64::INFINITY, smaller), -1.0);
        assert_eq!(values.into_iter().fold(f64::NEG_INFINITY, larger), 3.0);
    }
}
