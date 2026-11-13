//! Source: do-carmo-1976
//! A surface whose parameters are skewed and stretched, so that every coefficient of its fundamental forms is some size,
//! and its curvatures found a second way: a tangent written in the derivatives' basis by Cramer's rule on cross products
//! rather than through the first form's inverse, and the principal curvatures from normal curvatures in an orthonormal
//! frame of the tangent plane, the second form's mixed term by polarisation (do Carmo, chapter 3).

use occam_units::{Point, UvPoint, Vec3};

use crate::{Scratch, SplineSurface, Surface};

/// A bicubic patch whose parameter lines cross at no right angle and run at no even speed, curved both ways.
pub(crate) fn skewed() -> SplineSurface {
    let heights = [[0.0, 0.4, 0.1, -0.3], [0.5, 1.2, 0.9, 0.2], [0.2, 1.0, 1.5, 0.8], [-0.4, 0.3, 0.7, 1.1]];
    let poles = (0..4_u32)
        .flat_map(|j| (0..4_u32).map(move |i| (f64::from(i), f64::from(j))))
        .zip(heights.iter().flatten())
        .map(|((i, j), z)| Point::from_vec3(Vec3::new(i + 0.6 * j, j + 0.3 * i * i, *z)))
        .collect();
    SplineSurface::bezier((4, 4), poles, None).unwrap()
}

/// The first and second derivatives of `s` at `uv`: `[su, sv, suu, suv, svv]`.
fn jets(s: &dyn Surface, uv: UvPoint) -> [Vec3; 5] {
    let mut out = Vec::new();
    s.derivatives(uv, 2, &mut out, &mut Scratch::default()).unwrap();
    std::array::from_fn(|k| out[k].rate())
}

/// The unit normal of `s` at `uv`, `su x sv` normalised.
pub(crate) fn unit_normal(s: &dyn Surface, uv: UvPoint) -> Vec3 {
    let [su, sv, ..] = jets(s, uv);
    let across = su.cross(sv);
    across * (1.0 / across.norm())
}

/// The normal curvature of `s` at `uv` along the tangent `w`, towards [`unit_normal`]: `II(w, w) / I(w, w)`.
pub(crate) fn normal_curvature(s: &dyn Surface, uv: UvPoint, w: Vec3) -> f64 {
    let [su, sv, suu, suv, svv] = jets(s, uv);
    let across = su.cross(sv);
    let area = across.dot(across);
    let (du, dv) = (w.cross(sv).dot(across) / area, su.cross(w).dot(across) / area);
    let tangent = su * du + sv * dv;
    let bent = suu * (du * du) + suv * (2.0 * du * dv) + svv * (dv * dv);
    bent.dot(unit_normal(s, uv)) / tangent.dot(tangent)
}

/// The principal curvatures of `s` at `uv`, the larger first.
pub(crate) fn principal(s: &dyn Surface, uv: UvPoint) -> (f64, f64) {
    let [su, ..] = jets(s, uv);
    let first = su * (1.0 / su.norm());
    let second = unit_normal(s, uv).cross(first);
    let (a, c) = (normal_curvature(s, uv, first), normal_curvature(s, uv, second));
    let b = (2.0 * normal_curvature(s, uv, first + second) - a - c) / 2.0;
    let (mean, half) = (0.5 * a + 0.5 * c, (0.25 * (a - c) * (a - c) + b * b).sqrt());
    (mean + half, mean - half)
}

/// The points of [`skewed`] the tests measure at, inside its patch and spread over it.
pub(crate) fn places() -> [UvPoint; 4] {
    [UvPoint::new(0.2, 0.3), UvPoint::new(0.7, 0.1), UvPoint::new(0.5, 0.8), UvPoint::new(0.9, 0.6)]
}
