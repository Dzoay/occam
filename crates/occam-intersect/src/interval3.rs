//! Source: original
//! Vectors of three intervals: the arithmetic every certified decision here is made in, written once. Each function
//! encloses what it names for any vectors its operands hold, as occam-math's [`Interval`] does for numbers.

use occam_math::Interval;
use occam_units::Vec3;

/// `v`'s parts, each exactly.
pub(crate) fn exact3(v: Vec3) -> [Interval; 3] {
    [v.x, v.y, v.z].map(Interval::exact)
}

/// `a − b`.
pub(crate) fn minus([ax, ay, az]: [Interval; 3], [bx, by, bz]: [Interval; 3]) -> [Interval; 3] {
    [ax - bx, ay - by, az - bz]
}

/// `a · b`.
pub(crate) fn dot3([ax, ay, az]: [Interval; 3], [bx, by, bz]: [Interval; 3]) -> Interval {
    ax * bx + ay * by + az * bz
}

/// `(p − from) · towards`.
pub(crate) fn along(p: [Interval; 3], from: [Interval; 3], towards: [Interval; 3]) -> Interval {
    dot3(minus(p, from), towards)
}

/// `a × b`.
pub(crate) fn cross3([ax, ay, az]: [Interval; 3], [bx, by, bz]: [Interval; 3]) -> [Interval; 3] {
    [ay * bz - az * by, az * bx - ax * bz, ax * by - ay * bx]
}

/// The length of `v`.
pub(crate) fn norm3(v: [Interval; 3]) -> Interval {
    dot3(v, v).sqrt()
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Whether `x` holds `value` and is no wider than `slack`.
    fn holds(x: Interval, value: f64, slack: f64) -> bool {
        x.lower() <= value && value <= x.upper() && x.radius() <= slack
    }

    const A: Vec3 = Vec3::new(1.0, -2.0, 0.5);
    const B: Vec3 = Vec3::new(0.25, 4.0, -3.0);

    #[test]
    fn a_vector_is_held_exactly() {
        let parts = exact3(A);
        assert_eq!(parts.map(Interval::mid), [1.0, -2.0, 0.5]);
        assert_eq!(parts.map(Interval::radius), [0.0; 3]);
    }

    #[test]
    fn a_difference_a_product_and_a_length_hold_what_plain_arithmetic_gives() {
        let (first, second) = (exact3(A), exact3(B));
        let [x, y, z] = minus(first, second);
        assert!(holds(x, 0.75, 1e-15) && holds(y, -6.0, 1e-15) && holds(z, 3.5, 1e-15));
        assert!(holds(dot3(first, second), -9.25, 1e-14), "1/4 - 8 - 3/2");
        let [x, y, z] = cross3(first, second);
        assert!(holds(x, 4.0, 1e-14) && holds(y, 3.125, 1e-14) && holds(z, 4.5, 1e-14), "A x B");
        assert!(holds(norm3(exact3(Vec3::new(3.0, -4.0, 12.0))), 13.0, 1e-14));
    }

    #[test]
    fn how_far_along_is_the_product_with_the_difference() {
        let (p, from, towards) = (exact3(A), exact3(B), exact3(Vec3::new(0.0, 0.0, 2.0)));
        assert!(holds(along(p, from, towards), 7.0, 1e-14), "(0.5 + 3) * 2");
        let same = dot3(minus(p, from), towards);
        assert_eq!((along(p, from, towards).mid(), along(p, from, towards).radius()), (same.mid(), same.radius()));
    }

    #[test]
    fn a_wide_operand_widens_the_result() {
        let wide = [Interval::new(1.0, 0.5), Interval::exact(0.0), Interval::exact(0.0)];
        let x = dot3(wide, exact3(Vec3::new(2.0, 0.0, 0.0)));
        assert!(x.lower() <= 1.0 && x.upper() >= 3.0, "{x:?}");
        assert!(norm3(wide).lower() <= 0.5 && norm3(wide).upper() >= 1.5);
    }
}
