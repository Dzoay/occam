//! Source: original
//! Helpers shared by the crate's tests: random doubles of a chosen size, and exact values as integers.

use crate::splitmix::SplitMix64;

/// `x * 2^shift` as an integer, which it must be exactly.
pub fn scaled(x: f64, shift: i32) -> i128 {
    let bits = x.to_bits();
    let exponent = i32::try_from((bits >> 52) & 0x7FF).unwrap();
    if exponent == 0 && bits << 1 == 0 {
        return 0;
    }
    assert!(exponent > 0 && exponent < 0x7FF, "{x} is normal");
    let mantissa = i128::from((bits & 0x000F_FFFF_FFFF_FFFF) | 0x0010_0000_0000_0000);
    let power = exponent.checked_sub(1075).unwrap().checked_add(shift).unwrap();
    let magnitude = if power >= 0 {
        let shifted = mantissa.checked_shl(u32::try_from(power).unwrap()).unwrap();
        assert_eq!(shifted.checked_shr(u32::try_from(power).unwrap()), Some(mantissa), "{x} * 2^{shift} fits");
        shifted
    } else {
        let down = u32::try_from(power.checked_neg().unwrap()).unwrap();
        assert!(mantissa.trailing_zeros() >= down, "{x} * 2^{shift} is an integer");
        mantissa.checked_shr(down).unwrap()
    };
    if x < 0.0 { magnitude.checked_neg().unwrap() } else { magnitude }
}

/// The exact sum of `values`, times `2^shift`, which must be an integer.
pub fn sum_scaled(values: &[f64], shift: i32) -> i128 {
    values.iter().fold(0, |sum: i128, value| sum.checked_add(scaled(*value, shift)).unwrap())
}

/// A double with a random 53-bit mantissa, a random sign, and an exponent from `low` to `high`.
pub fn double(generator: &mut SplitMix64, low: i32, high: i32) -> f64 {
    narrow(generator, low, high, 53)
}

/// As [`double`], with only the leading `bits` of the mantissa random and the rest zero, so that exact products of
/// several still fit an `i128`.
pub fn narrow(generator: &mut SplitMix64, low: i32, high: i32, bits: u32) -> f64 {
    let span = u64::try_from(high.checked_sub(low).unwrap().checked_add(1).unwrap()).unwrap();
    let exponent = low.checked_add(i32::try_from(generator.next_u64().checked_rem(span).unwrap()).unwrap()).unwrap();
    let biased = u64::try_from(exponent.checked_add(1023).unwrap()).unwrap();
    let kept = u64::MAX.checked_shl(53_u32.checked_sub(bits).unwrap()).unwrap() & 0x000F_FFFF_FFFF_FFFF;
    let mantissa = (generator.next_u64() >> 12) & kept;
    let magnitude = f64::from_bits((biased << 52) | mantissa);
    if generator.next_u64() & 1 == 0 { magnitude } else { -magnitude }
}
