//! Source: original
//! What a family does where a formula cannot run in lanes, as a spline's cannot where its lanes fall in different spans:
//! the lanes of a value, and a value made from numbers worked out one lane at a time, so that each lane has the bits of
//! a single call.

use occam_math::Real;
use occam_units::Vec3;

/// The most lanes a value may have here.
pub(crate) const MAX_LANES: usize = 16;

/// The lanes of `x`, lane 0 first, and zero beyond them.
pub(crate) fn lanes<R: Real>(x: R) -> [f64; MAX_LANES] {
    let mut out = [0.0; MAX_LANES];
    let mut slots = out.iter_mut();
    x.for_each_lane(|value| {
        if let Some(slot) = slots.next() {
            *slot = value;
        }
    });
    out
}

/// The vector whose lane `k` is `each(k)`.
pub(crate) fn from_lanes<R: Real>(each: impl Fn(usize) -> Vec3) -> Vec3<R> {
    let part = |pick: fn(Vec3) -> f64| {
        let mut k = 0_usize;
        R::from_fn(|| {
            let x = pick(each(k));
            k = k.saturating_add(1);
            x
        })
    };
    Vec3::new(part(|v| v.x), part(|v| v.y), part(|v| v.z))
}

#[cfg(test)]
mod tests {
    use occam_math::Lanes;

    use super::*;

    #[test]
    fn lanes_come_out_in_order_and_go_back_in_order() {
        let x = Lanes::new([1.0, 2.0, 3.0]);
        assert_eq!(lanes(x)[..4], [1.0, 2.0, 3.0, 0.0]);
        let each = [Vec3::new(1.0, 2.0, 3.0), Vec3::new(4.0, 5.0, 6.0), Vec3::new(7.0, 8.0, 9.0)];
        let v: Vec3<Lanes<3>> = from_lanes(|k| each[k]);
        assert_eq!(
            (v.x.to_array(), v.y.to_array(), v.z.to_array()),
            ([1.0, 4.0, 7.0], [2.0, 5.0, 8.0], [3.0, 6.0, 9.0])
        );
        assert_eq!(lanes(7.5)[0], 7.5);
    }
}
