//! Source: shewchuk-1997
//! Which side of a line a point of the plane lies on (Shewchuk 1997): the one predicate every crate above decides it
//! by, through [`decide_stored`](crate::decide_stored) or [`decide_computed`](crate::decide_computed).

use crate::predicate::{Predicate, Ring};

/// Twice the signed area of the triangle `a`, `b`, `c`, from `[ax, ay, bx, by, cx, cy]`: `(b - a) x (c - a)`, above
/// zero when `c` lies left of `a` to `b`, so that `a`, `b`, `c` run anticlockwise.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub struct Orientation;

impl Predicate<6> for Orientation {
    const DEGREE: u32 = 2;

    #[inline(always)]
    fn value<T: Ring>(&self, [ax, ay, bx, by, cx, cy]: &[T; 6]) -> T {
        let first = bx.sub(ax).mul(&cy.sub(ay));
        first.sub(&by.sub(ay).mul(&cx.sub(ax)))
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::interval::Interval;
    use crate::predicate::{Side, Verdict, decide_stored};

    /// The side `c` is of `a` to `b`, decided on the numbers stored.
    fn side(a: [f64; 2], b: [f64; 2], c: [f64; 2]) -> Verdict {
        let mut out = Vec::new();
        decide_stored(&Orientation, &[[a[0], a[1], b[0], b[1], c[0], c[1]]], 0.0, &mut out);
        out[0]
    }

    #[test]
    fn a_point_is_above_on_the_left_below_on_the_right_and_within_on_the_line() {
        let (a, b) = ([0.0, 0.0], [2.0, 1.0]);
        assert_eq!(side(a, b, [0.0, 1.0]), Verdict::Decided(Side::Above), "left");
        assert_eq!(side(a, b, [1.0, 0.0]), Verdict::Decided(Side::Below), "right");
        assert_eq!(side(a, b, [4.0, 2.0]), Verdict::Decided(Side::Within), "on the line, beyond its end");
        assert_eq!(side(b, a, [0.0, 1.0]), Verdict::Decided(Side::Below), "the line turned round");
    }

    #[test]
    fn a_point_one_rounding_off_the_line_is_told_from_one_on_it() {
        let (a, b) = ([0.0, 0.0], [1e10, 1e10]);
        assert_eq!(side(a, b, [0.5, 0.5]), Verdict::Decided(Side::Within));
        assert_eq!(side(a, b, [0.5, 0.5_f64.next_up()]), Verdict::Decided(Side::Above));
        assert_eq!(side(a, b, [0.5, 0.5_f64.next_down()]), Verdict::Decided(Side::Below));
    }

    #[test]
    fn the_value_is_twice_the_signed_area() {
        let exact = [0.0, 0.0, 4.0, 0.0, 0.0, 3.0].map(Interval::<f64>::exact);
        let value = Orientation.value(&exact);
        assert_eq!((value.mid(), value.lower() <= 12.0 && 12.0 <= value.upper()), (12.0, true));
        let shifted = [1.0, 2.0, 5.0, 2.0, 1.0, 5.0].map(Interval::<f64>::exact);
        assert_eq!(Orientation.value(&shifted).mid(), 12.0, "from a, not from the origin");
        assert_eq!(Orientation.value(&[0.0, 0.0, 2.0, 0.0, 0.0, 3.0]), 6.0);
        assert_eq!(Orientation.value(&[1.0, 1.0, 3.0, 1.0, 1.0, -2.0]), -6.0, "clockwise");
    }
}
