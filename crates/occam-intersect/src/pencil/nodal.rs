//! Source: dupont-lazard-lazard-petitjean-2008
//! A nodal quartic, parametrized rationally from its node (Dupont, Lazard, Lazard and Petitjean 2008, part III). The
//! node `p₀` is the vertex of the cone `K` of the pencil at the double root, which is a cone of the pair where its apex
//! is on the other surface. Every line through `p₀` on `K` meets the curve once more, so a
//! rational parameter of `K`'s rulings is one of the curve. With `K`'s 3×3 part `Σ μᵢ eᵢ eᵢᵀ`, `μ₃` of the other sign,
//! the rulings are `g(φ) = e₁ cos φ/√|μ₁| + e₂ sin φ/√|μ₂| + e₃/√|μ₃|`, and on the other quadric `S` the ruling
//! `p₀ + w g` meets the curve where `w = −2 gᵀ(M p₀ + q) / gᵀ M g`, since `S(p₀)` is zero.
//!
//! That is a rational curve of degree two in `cos φ` and `sin φ`, and so of degree four in the parameter of a rational
//! quadratic arc: each eighth of the turn is a rational Bézier curve of degree four, and the eight are one rational
//! B-spline, exact. Where `K`'s part is definite its only real point is `p₀`, an isolated contact. A cusp, where `K`
//! meets the common tangent plane in one ruling, is the same construction: the curve passes it once, at rest.

use std::f64::consts::{FRAC_PI_8, TAU};

use occam_geom::{BsplineCurve, SplineCurve};
use occam_math::sin_cos;
use occam_units::{Point, Vec3};

use super::Pair;
use super::eigen::eigen;
use super::quadric::Quadric;
use super::roots::Members;
use super::trig::Trig;

/// A nodal quartic's curve, or none where its only real point is its node, and the node.
pub(super) struct Nodal {
    pub(super) curve: Option<SplineCurve>,
    pub(super) at: Vec3,
}

/// The arcs a turn is cut into.
const ARCS: u32 = 8;

/// `f g` for two quadratics in Bernstein form, in Bernstein form of degree four.
fn product([f0, f1, f2]: [f64; 3], [g0, g1, g2]: [f64; 3]) -> [f64; 5] {
    let ends = (f64::midpoint(f0 * g1, f1 * g0), f64::midpoint(f1 * g2, f2 * g1));
    [f0 * g0, ends.0, (f0 * g2 + 4.0 * f1 * g1 + f2 * g0) / 6.0, ends.1, f2 * g2]
}

/// `T W²` on the arc whose circle is `(C, S, W)` in Bernstein form, for `T` of degree two in `cos φ` and `sin φ`.
fn homogeneous(trig: &Trig, (c, s, w): ([f64; 3], [f64; 3], [f64; 3])) -> [f64; 5] {
    let k: [f64; 5] = std::array::from_fn(|i| trig.0.get(i).copied().unwrap_or(0.0));
    let (ww, cw, sw, cc, ss, cs) =
        (product(w, w), product(c, w), product(s, w), product(c, c), product(s, s), product(c, s));
    std::array::from_fn(|i| {
        let at = |b: &[f64; 5]| b.get(i).copied().unwrap_or(0.0);
        k[0] * at(&ww) + k[1] * at(&cw) + k[2] * at(&sw) + k[3] * (at(&cc) - at(&ss)) + 2.0 * k[4] * at(&cs)
    })
}

/// `gᵀ M g` and `gᵀ h` for `g = g₀ + g_c cos φ + g_s sin φ`.
fn on_rulings(quadric: &Quadric, [g0, gc, gs]: [Vec3; 3], h: Vec3) -> (Trig, Trig) {
    let m = |u: Vec3, v: Vec3| quadric.form(u, v);
    let alpha = Trig(vec![
        m(g0, g0) + f64::midpoint(m(gc, gc), m(gs, gs)),
        2.0 * m(g0, gc),
        2.0 * m(g0, gs),
        0.5 * (m(gc, gc) - m(gs, gs)),
        m(gc, gs),
    ]);
    (alpha, Trig(vec![g0.dot(h), gc.dot(h), gs.dot(h)]))
}

/// The 3×3 part of the pencil's member at `phi`, the cone whose vertex is the node, and the surface its rulings are cut
/// by: the second where the member is the first of the pair, and else the first. Where the curve is bounded no ruling
/// of the cone runs along a surface's asymptotic cone, so either surface meets each ruling again at a finite point.
fn cone_and_cut(pair: &Pair, phi: f64) -> ([Vec3; 3], usize) {
    (Members::of(pair).part(phi), usize::from(sin_cos(phi).0.abs() <= 1e-9))
}

/// The rulings `g₀, g_c, g_s` of the cone with 3×3 part `m`: `None` where it is no real cone, its part definite.
/// None of its eigenvalues is zero, since its vertex is a point.
fn rulings(m: [Vec3; 3]) -> Option<[Vec3; 3]> {
    let (mu, vectors) = eigen(m.map(|r| [r.x, r.y, r.z]));
    let e = vectors.map(|[x, y, z]| Vec3::new(x, y, z));
    let positive = mu.iter().filter(|x| **x > 0.0).count();
    if positive == 0 || positive == 3 {
        return None;
    }
    let odd = mu.iter().position(|x| (*x > 0.0) == (positive == 1)).unwrap_or(0);
    let scaled = |i: usize| e.get(i).copied().unwrap_or(e[0]) * (1.0 / mu.get(i).copied().unwrap_or(1.0).abs().sqrt());
    let rest: Vec<usize> = (0..3).filter(|i| *i != odd).collect();
    Some([scaled(odd), scaled(rest.first().copied().unwrap_or(0)), scaled(rest.last().copied().unwrap_or(0))])
}

/// The nodal quartic of `pair` with its node `at`, in the world, the vertex of the pencil's member at `phi`.
pub(super) fn node(pair: &Pair, at: Vec3, phi: f64) -> Result<Nodal, &'static str> {
    let p0 = at - pair.origin;
    let (cone, cut) = cone_and_cut(pair, phi);
    let Some(ways) = rulings(cone) else { return Ok(Nodal { curve: None, at }) };
    let quadric = pair.quadric(cut);
    let (alpha, beta) = on_rulings(&quadric, ways, quadric.half_gradient(p0));
    if !alpha.roots().is_empty() {
        return Err("a nodal quartic that reaches infinity");
    }
    // The curve starts at the node, on a ruling through it along which `β` is zero: one of its roots, or, at a cusp, its
    // double root, where it touches zero without crossing and is least.
    let least = |best: (f64, f64), theta: f64| {
        let size = beta.at(theta).abs();
        if size < best.1 { (theta, size) } else { best }
    };
    let start = beta.roots().into_iter().chain(beta.rate().roots()).fold((0.0, f64::INFINITY), least).0;
    let coordinate = |pick: fn(Vec3) -> f64| {
        let ruling = Trig(ways.iter().map(|v| pick(*v)).collect());
        alpha.scaled(pick(p0)).plus(&beta.times(&ruling).scaled(-2.0))
    };
    let numerator = [coordinate(|v| v.x), coordinate(|v| v.y), coordinate(|v| v.z)];
    let (mut poles, mut weights) = (Vec::new(), Vec::new());
    for i in 0..ARCS {
        let from = start + TAU * f64::from(i) / f64::from(ARCS);
        let ((s0, c0), (sm, cm), (s1, c1)) =
            (sin_cos(from), sin_cos(from + FRAC_PI_8), sin_cos(from + 2.0 * FRAC_PI_8));
        let circle = ([c0, cm, c1], [s0, sm, s1], [1.0, sin_cos(FRAC_PI_8).1, 1.0]);
        let w = homogeneous(&alpha, circle);
        let [x, y, z] = numerator.clone().map(|n| homogeneous(&n, circle));
        for k in usize::from(i != 0)..5 {
            let at = |b: &[f64; 5]| b.get(k).copied().unwrap_or(f64::NAN);
            let weight = at(&w);
            let local = Vec3::new(at(&x), at(&y), at(&z)) * (1.0 / weight);
            poles.push(Point::from_vec3(pair.world(local)));
            weights.push(weight);
        }
    }
    let flip = weights.first().is_some_and(|w| w.is_sign_negative());
    let weights: Vec<f64> = weights.into_iter().map(|w| if flip { -w } else { w }).collect();
    if weights.iter().any(|w| *w <= 0.0) {
        return Err("a nodal quartic whose weights change sign");
    }
    let mut knots = vec![0.0; 5];
    for i in 1..ARCS {
        knots.extend([f64::from(i); 4]);
    }
    knots.extend([f64::from(ARCS); 5]);
    let form = BsplineCurve { degree: 4, knots, poles, weights: Some(weights) };
    SplineCurve::new(&form).map(|c| Nodal { curve: Some(c), at }).map_err(|_| "a nodal quartic with no B-spline")
}
