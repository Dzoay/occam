//! Source: original
//! The spline workloads are what they say, and give the same bits at either width. Beside the workloads, not in their
//! file, which occam-geom's unit tests compile too.

use occam_geom::{Jets, OffsetCurve, OffsetSurface, ParametricCurve, Scratch, SplineCurve, SplineSurface};
use occam_units::{Direction, Length, Point, UvPoint, Vec3};

use crate::spline_workloads::{curve_seconds, grid, lane_sums, params, seconds, spline, spline_curve, splines, total};

/// The offsets the benchmark measures: the curve moved within its plane and the surface along its normal, by 0.3.
fn offsets() -> (OffsetCurve<SplineCurve>, OffsetSurface<SplineSurface>) {
    let distance = Length::from_millimetres(0.3);
    let up = Direction::normalised(Vec3::new(0.0, 0.0, 1.0));
    let curve = OffsetCurve::new(SplineCurve::new(&spline_curve()).unwrap(), distance, up).unwrap();
    (curve, OffsetSurface::new(SplineSurface::new(&spline()).unwrap(), distance).unwrap())
}

#[test]
fn the_measured_surface_and_its_sum_are_what_they_say() {
    let form = spline();
    let pole = |i: usize, j: usize| form.poles[j * 7 + i].to_vec3();
    // (1.7 x + 0.1 y^2, 1.3 y - 0.2 x, ((7 x + 3 y) mod 5) 0.4 - 0.8) at (2, 3) and at (4, 6).
    for (got, want) in [(pole(2, 3), Vec3::new(4.3, 3.5, 0.4)), (pole(4, 6), Vec3::new(10.4, 7.0, -0.4))] {
        assert!((got - want).norm() < 1e-12, "{got:?} against {want:?}");
    }
    assert_eq!((form.degree, form.count, form.knots_u.len(), form.weights), ((3, 3), (7, 7), 11, None));
    let mut out = Jets::default();
    out.points.push(Point::from_vec3(Vec3::new(1.0, 2.0, 3.0)));
    out.du.push(occam_units::Tangent::new(Vec3::new(4.0, 5.0, 6.0)));
    out.dvv.push(occam_units::Tangent::new(Vec3::new(7.0, 8.0, 10.0)));
    assert_eq!(total(&out), 6.0 + 15.0 + 25.0);
}

#[test]
fn the_offsets_give_the_same_bits_at_either_width_and_their_parameters_are_even() {
    let (curve, surface) = offsets();
    let ts = params(9);
    assert_eq!((ts.len(), ts[0], ts[4], ts[8]), (9, 0.0, 0.5, 1.0));
    let (one, wide) = (curve_seconds::<f64, _>(&curve, &ts), curve_seconds::<occam_math::Lanes<4>, _>(&curve, &ts));
    assert!(one.is_finite() && one != 0.0, "{one}");
    assert_eq!(one.to_bits(), wide.to_bits());
    let first = curve_seconds::<f64, _>(&curve, &ts[..1]);
    let (p, d1, d2) = ParametricCurve::eval_second(&curve, 0.0);
    let want = [p.to_vec3(), d1.rate(), d2.rate()].iter().map(|v| v.x + v.y + v.z).sum::<f64>();
    assert_eq!(first.to_bits(), want.to_bits());
    // The measured curve lies in z = 0, so each lane's sum is checked where z is not zero too.
    let v = Vec3::new(
        occam_math::Lanes::new([1.0, 8.0]),
        occam_math::Lanes::new([2.0, 16.0]),
        occam_math::Lanes::new([4.0, 32.0]),
    );
    assert_eq!(lane_sums(v).take(2).collect::<Vec<_>>(), [7.0, 56.0]);
    let uv = grid(5);
    let (mut one, mut wide) = (Jets::default(), Jets::default());
    seconds::<1, _>(&surface, &uv, &mut one);
    seconds::<4, _>(&surface, &uv, &mut wide);
    assert_eq!((one.points.len(), one.duu.len()), (25, 25));
    assert!(total(&one).is_finite() && one.first_undefined().is_none());
    assert_eq!(total(&one).to_bits(), total(&wide).to_bits());
}

#[test]
fn the_measured_curve_bends_in_its_third_derivative() {
    let form = spline_curve();
    assert_eq!((form.degree, form.poles.len(), form.knots.len()), (3, 7, 11));
    // (1.5 x, ((3 x) mod 5) 0.6 - 1.2, 0) at x = 2 and x = 4.
    assert_eq!(
        (form.poles[2].to_vec3(), form.poles[4].to_vec3()),
        (Vec3::new(3.0, -0.6, 0.0), Vec3::new(6.0, 0.0, 0.0))
    );
    let mut rates = Vec::new();
    let base = SplineCurve::new(&form).unwrap();
    ParametricCurve::derivatives(&base, 0.6, 3, &mut rates, &mut Scratch::default()).unwrap();
    assert!(rates[2].rate().norm() > 1.0, "{:?}", rates[2]);
}

#[test]
fn the_kernel_gives_the_same_bits_at_either_width() {
    let surface = SplineSurface::new(&spline()).unwrap();
    let uv = grid(16);
    let (mut one, mut wide) = (Jets::default(), Jets::default());
    splines::<1, _>(&surface, &uv, &mut one);
    splines::<4, _>(&surface, &uv, &mut wide);
    assert_eq!((one.points.len(), one.du.len(), one.duu.len()), (256, 256, 0));
    assert_eq!(total(&one).to_bits(), total(&wide).to_bits());
    assert!(total(&one).is_finite() && one.first_undefined().is_none());
    assert_eq!(
        (uv[0], uv[17], uv[255]),
        (UvPoint::new(0.0, 0.0), UvPoint::new(1.0 / 15.0, 1.0 / 15.0), UvPoint::new(1.0, 1.0))
    );
}
