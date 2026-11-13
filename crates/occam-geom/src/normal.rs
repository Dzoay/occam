//! Source: do-carmo-1976
//! The unit normal and its derivatives to any order, which an offset is made of (do Carmo, chapter 3, the Gauss map and
//! parallel surfaces). The normal `N = M / r` of an unnormalised `M` and its length `r`: a surface's `M` is `Su x Sv`
//! and a curve's `C' x V`, products, so Leibniz's rule gives their derivatives from the base's; `r` has `r r = M . M`,
//! which gives each of its derivatives from those below it; and `N` is a quotient by `r`, divided as a rational spline
//! is by its weight. Exact to any order, generic over [`Real`], so the rungs run in lanes.

use occam_math::Real;
use occam_report::{Code, Fault, NamedValue, Quantity, Report};
use occam_units::{Tangent, Vec3, World};

use crate::basis::binomial;
use crate::spline::{curve_quotient, index, surface_quotient};

/// The order above `order`, whose derivatives an offset's of `order` need.
///
/// # Errors
///
/// `geom.no-derivative` for the highest order a count holds, whose order above it cannot name.
pub(crate) fn above(order: u8) -> Result<u8, Report> {
    order.checked_add(1).ok_or_else(|| {
        Report::new(Code::NoDerivative, Fault::InvalidInput)
            .with_value(NamedValue::new("order", Quantity::Count, f64::from(order)))
            .with_value(NamedValue::new("highest", Quantity::Count, f64::from(order.saturating_sub(1))))
    })
}

/// Moves the base's `point` and the rates `out` holds from `start` on into `into`, the point first, and leaves `out` as
/// it was before them, so that an offset's `derivatives` reads its base's through the caller's own buffer.
pub(crate) fn take(point: Vec3, (out, start): (&mut Vec<Tangent<World>>, usize), into: &mut [Vec3]) {
    let rates = out.iter().skip(start).map(|t| t.rate());
    for (slot, value) in into.iter_mut().zip(std::iter::once(point).chain(rates)) {
        *slot = value;
    }
    out.truncate(start);
}

/// `(n choose i) (m choose j)` in every lane.
#[inline(always)]
fn choose<R: Real>((n, i): (usize, usize), (m, j): (usize, usize)) -> R {
    R::splat(binomial(n, i) * binomial(m, j))
}

/// The surface's `M = Su x Sv` partials to `order` into `normals`, at [`index`], from the base's partials to
/// `order + 1` in `base`, the point first: `M(a, b)` sums `(a choose i) (b choose j) S(i + 1, j) x S(a - i, b - j + 1)`.
#[inline(always)]
fn surface_cross<R: Real>(order: usize, base: &[Vec3<R>], normals: &mut [Vec3<R>]) {
    let nan = R::splat(f64::NAN);
    let at = |in_u: usize, in_v: usize| base.get(index(in_u, in_v)).copied().unwrap_or(Vec3::new(nan, nan, nan));
    for degree in 0..=order {
        for in_v in 0..=degree {
            let in_u = degree.saturating_sub(in_v);
            let zero = R::splat(0.0);
            let mut sum = Vec3::new(zero, zero, zero);
            for low_u in 0..=in_u {
                for low_v in 0..=in_v {
                    let across = at(low_u.saturating_add(1), low_v);
                    let along = at(in_u.saturating_sub(low_u), in_v.saturating_sub(low_v).saturating_add(1));
                    sum = sum + across.cross(along) * choose::<R>((in_u, low_u), (in_v, low_v));
                }
            }
            if let Some(slot) = normals.get_mut(index(in_u, in_v)) {
                *slot = sum;
            }
        }
    }
}

/// The length `r = |M|`'s partials to `order` into `lengths`, at [`index`], from `M`'s in `normals`: `r(0, 0)` is the
/// root of `M . M`, and each other `r(a, b)` is `(M . M)(a, b)` less every product `r(i, j) r(a - i, b - j)` but the
/// two with `r(0, 0)`, over `2 r(0, 0)`, by degree, so that what it reads comes first.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
fn surface_length<R: Real>(order: usize, normals: &[Vec3<R>], lengths: &mut [R]) {
    let nan = R::splat(f64::NAN);
    let vector = |in_u: usize, in_v: usize| normals.get(index(in_u, in_v)).copied().unwrap_or(Vec3::new(nan, nan, nan));
    for degree in 0..=order {
        for in_v in 0..=degree {
            let in_u = degree.saturating_sub(in_v);
            let (mut square, mut known) = (R::splat(0.0), R::splat(0.0));
            for low_u in 0..=in_u {
                for low_v in 0..=in_v {
                    let (high_u, high_v) = (in_u.saturating_sub(low_u), in_v.saturating_sub(low_v));
                    let weight = choose::<R>((in_u, low_u), (in_v, low_v));
                    square = square + vector(low_u, low_v).dot(vector(high_u, high_v)) * weight;
                    if (low_u, low_v) != (0, 0) && (high_u, high_v) != (0, 0) {
                        let length = |u: usize, v: usize| lengths.get(index(u, v)).copied().unwrap_or(nan);
                        known = known + length(low_u, low_v) * length(high_u, high_v) * weight;
                    }
                }
            }
            let first = lengths.first().copied().unwrap_or(nan);
            let value = if degree == 0 { square.sqrt() } else { (square - known) / (R::splat(2.0) * first) };
            if let Some(slot) = lengths.get_mut(index(in_u, in_v)) {
                *slot = value;
            }
        }
    }
}

/// A surface's unit normal `N = (Su x Sv) / |Su x Sv|` and its partials to `order` into `m`, at [`index`], from the
/// base's partials to `order + 1` in `base`, the point first. `lengths` holds as many numbers as `m`. Where `Su x Sv`
/// vanishes, at a degenerate line or a singular point, every one is NaN.
#[inline(always)]
pub(crate) fn surface<R: Real>(order: usize, base: &[Vec3<R>], m: &mut [Vec3<R>], lengths: &mut [R]) {
    surface_cross(order, base, m);
    surface_length(order, m, lengths);
    surface_quotient(order, m, lengths);
}

/// A curve's normal `N = (C' x V) / |C' x V|` for the reference direction `V`, and its derivatives to `order` into
/// `m`, from the base's point and derivatives to `order + 1` in `base`. `lengths` holds as many numbers as `m`. Where
/// `C'` runs along `V`, every one is NaN.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
pub(crate) fn curve<R: Real>(order: usize, base: &[Vec3<R>], reference: Vec3, m: &mut [Vec3<R>], lengths: &mut [R]) {
    let v = Vec3::new(R::splat(reference.x), R::splat(reference.y), R::splat(reference.z));
    for (slot, rate) in m.iter_mut().zip(base.iter().skip(1)).take(order.saturating_add(1)) {
        *slot = rate.cross(v);
    }
    let nan = R::splat(f64::NAN);
    for k in 0..=order {
        let at = |i: usize| m.get(i).copied().unwrap_or(Vec3::new(nan, nan, nan));
        let length = |i: usize| lengths.get(i).copied().unwrap_or(nan);
        let (mut square, mut known) = (R::splat(0.0), R::splat(0.0));
        for i in 0..=k {
            let c = R::splat(binomial(k, i));
            square = square + at(i).dot(at(k.saturating_sub(i))) * c;
            if i > 0 && i < k {
                known = known + length(i) * length(k.saturating_sub(i)) * c;
            }
        }
        let value = if k == 0 { square.sqrt() } else { (square - known) / (R::splat(2.0) * length(0)) };
        if let Some(slot) = lengths.get_mut(k) {
            *slot = value;
        }
    }
    curve_quotient(order, m, lengths);
}

#[cfg(test)]
mod tests {
    use occam_math::Lanes;

    use super::*;
    use crate::spline::triangle;

    fn v(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3::new(x, y, z)
    }

    /// The partials to order 3 of `(u, v, u^2 + u v^2)`, which has a normal everywhere: `Su = (1, 0, 2u + v^2)`,
    /// `Sv = (0, 1, 2 u v)`, `Suu = (0, 0, 2)`, `Suv = (0, 0, 2v)`, `Svv = (0, 0, 2u)`, `Suvv = (0, 0, 2)` and no others.
    fn graph(u: f64, w: f64) -> Vec<Vec3> {
        let z = |c: f64| v(0.0, 0.0, c);
        vec![
            v(u, w, u * u + u * w * w),
            v(1.0, 0.0, 2.0 * u + w * w),
            v(0.0, 1.0, 2.0 * u * w),
            z(2.0),
            z(2.0 * w),
            z(2.0 * u),
            z(0.0),
            z(0.0),
            z(2.0),
            z(0.0),
        ]
    }

    /// The unit normal of [`graph`] at `(u, w)`.
    fn unit_at(u: f64, w: f64) -> Vec3 {
        let (su, sv) = (v(1.0, 0.0, 2.0 * u + w * w), v(0.0, 1.0, 2.0 * u * w));
        let n = su.cross(sv);
        n * (1.0 / n.norm())
    }

    #[test]
    fn the_normal_and_its_partials_agree_with_differences_of_the_closed_form() {
        let (at_u, at_v, step) = (0.3, -0.7, 1e-5);
        let (mut normals, mut lengths) = (vec![v(9.0, 9.0, 9.0); triangle(2)], vec![9.0; triangle(2)]);
        surface(2, &graph(at_u, at_v), &mut normals, &mut lengths);
        assert!((normals[0] - unit_at(at_u, at_v)).norm() < 1e-15);
        let half = 0.5 / step;
        let du = (unit_at(at_u + step, at_v) - unit_at(at_u - step, at_v)) * half;
        let dv = (unit_at(at_u, at_v + step) - unit_at(at_u, at_v - step)) * half;
        assert!((normals[1] - du).norm() < 1e-8 && (normals[2] - dv).norm() < 1e-8, "{normals:?}");
        let first = |at_u: f64, at_v: f64| {
            let (mut out, mut work) = (vec![v(0.0, 0.0, 0.0); 3], vec![0.0; 3]);
            surface(1, &graph(at_u, at_v), &mut out, &mut work);
            out
        };
        let duv = (first(at_u, at_v + step)[1] - first(at_u, at_v - step)[1]) * half;
        let dvv = (first(at_u, at_v + step)[2] - first(at_u, at_v - step)[2]) * half;
        assert!((normals[4] - duv).norm() < 1e-7 && (normals[5] - dvv).norm() < 1e-7, "{normals:?}");
        assert!((lengths[0] - unit_at(at_u, at_v).z.recip()).abs() < 1e-14, "|M| is 1 over the normal's z here");
    }

    #[test]
    fn where_the_derivatives_are_parallel_every_partial_is_nan() {
        let base = [
            v(0.0, 0.0, 0.0),
            v(1.0, 0.0, 0.0),
            v(2.0, 0.0, 0.0),
            v(0.0, 0.0, 0.0),
            v(0.0, 0.0, 0.0),
            v(0.0, 0.0, 0.0),
        ];
        let (mut m, mut lengths) = ([v(0.0, 0.0, 0.0); 3], [0.0; 3]);
        surface(1, &base, &mut m, &mut lengths);
        assert!(m.iter().all(|n| n.x.is_nan()), "{m:?}");
    }

    #[test]
    fn a_curves_normal_is_square_to_it_and_the_reference_and_turns_with_it() {
        // The circle (cos t, sin t, 0) with V = Z: C' x V is (cos t, sin t, 0), the outward normal, already of length 1.
        let (s, c) = occam_math::sin_cos(0.4);
        let base = [v(c, s, 0.0), v(-s, c, 0.0), v(-c, -s, 0.0), v(s, -c, 0.0)];
        let (mut m, mut lengths) = ([v(0.0, 0.0, 0.0); 3], [0.0; 3]);
        curve(2, &base, v(0.0, 0.0, 1.0), &mut m, &mut lengths);
        for (got, want) in m.iter().zip([v(c, s, 0.0), v(-s, c, 0.0), v(-c, -s, 0.0)]) {
            assert!((*got - want).norm() < 1e-15, "{got:?} against {want:?}");
        }
        assert!((lengths[0] - 1.0).abs() < 1e-15 && lengths[1].abs() < 1e-15 && lengths[2].abs() < 1e-15);
        let along = [v(0.0, 0.0, 0.0), v(0.0, 0.0, 2.0), v(0.0, 0.0, 0.0)];
        let (mut m, mut lengths) = ([v(0.0, 0.0, 0.0); 2], [0.0; 2]);
        curve(1, &along, v(0.0, 0.0, 1.0), &mut m, &mut lengths);
        assert!(m[0].x.is_nan(), "a curve along its reference has no normal");
    }

    #[test]
    fn lanes_give_the_bits_of_single_numbers() {
        let at = [(0.3, -0.7), (1.1, 0.2)];
        let lanes = |pick: fn(&Vec3) -> f64, k: usize| Lanes::<2>::new(at.map(|(u, w)| pick(&graph(u, w)[k])));
        let base: Vec<Vec3<Lanes<2>>> =
            (0..10).map(|k| Vec3::new(lanes(|p| p.x, k), lanes(|p| p.y, k), lanes(|p| p.z, k))).collect();
        let zero = Lanes::splat(0.0);
        let (mut m, mut lengths) = (vec![Vec3::new(zero, zero, zero); 6], vec![zero; 6]);
        surface(2, &base, &mut m, &mut lengths);
        for (lane, (u, w)) in at.into_iter().enumerate() {
            let (mut one, mut l) = (vec![v(0.0, 0.0, 0.0); 6], vec![0.0; 6]);
            surface(2, &graph(u, w), &mut one, &mut l);
            for (got, want) in m.iter().zip(&one) {
                assert_eq!(
                    [got.x, got.y, got.z].map(|x| x.to_array()[lane].to_bits()),
                    [want.x, want.y, want.z].map(f64::to_bits)
                );
            }
        }
    }
}
