//! Source: dupont-lazard-lazard-petitjean-2008
//! A nodal quartic, parametrized rationally from its node (Dupont, Lazard, Lazard and Petitjean 2008, part III). The
//! node `p₀` is the vertex of the cone `K` of the pencil at the double root, which is a cone of the pair where its apex
//! is on the other surface. Every line through `p₀` on `K` meets the curve once more, so a
//! rational parameter of `K`'s rulings is one of the curve. With `K`'s 3×3 part `Σ μᵢ eᵢ eᵢᵀ`, `μ₃` of the other sign,
//! the rulings are `g(φ) = e₁ cos φ/√|μ₁| + e₂ sin φ/√|μ₂| + e₃/√|μ₃|`, and on the other quadric `S` the ruling
//! `p₀ + w g` meets the curve where `w = −2 gᵀ(M p₀ + q) / gᵀ M g`, since `S(p₀)` is zero.
//!
//! That is a rational curve of degree two in `cos φ` and `sin φ`, a [`Fan`]: each eighth of the turn is a rational
//! Bézier curve of degree four, and the eight are one rational B-spline, exact. Where `K`'s part is definite its only
//! real point is `p₀`, an isolated contact. A cusp, where `K` meets the common tangent plane in one ruling, is the same
//! construction: the curve passes it once, at rest. Where a ruling of `K` runs along `S`'s asymptotic cone, as where two
//! cones share a ruling direction, `gᵀ M g` vanishes and the curve runs to infinity there: it is given within the
//! extent asked.

use occam_math::sin_cos;
use occam_units::Vec3;

use super::Pair;
use super::eigen::eigen;
use super::fan::Fan;
use super::quadric::Quadric;
use super::roots::Members;
use super::trig::Trig;
use crate::closed::unit;
use crate::decide::{Decisions, Undecided};

/// A nodal quartic, with its node `at`: its curve, from `start`, where it leaves the node, or none where its only
/// real point is its node, and whether it runs to infinity.
pub(super) struct Nodal {
    pub(super) fan: Option<Fan>,
    pub(super) at: Vec3,
    pub(super) start: f64,
    pub(super) reaching: bool,
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
///
/// # Errors
///
/// [`Undecided`] where the band cannot tell whether it reaches infinity.
pub(super) fn node(pair: &Pair, at: Vec3, phi: f64, decide: &mut Decisions) -> Result<Nodal, Undecided> {
    let p0 = at - pair.origin;
    let (cone, cut) = cone_and_cut(pair, phi);
    let Some(ways) = rulings(cone) else { return Ok(Nodal { fan: None, at, start: 0.0, reaching: false }) };
    let quadric = pair.quadric(cut);
    let (alpha, beta) = on_rulings(&quadric, ways, quadric.half_gradient(p0));
    // The curve starts at the node, on a ruling through it along which `β` is zero: one of its roots, or, at a cusp, its
    // double root, where it touches zero without crossing and is least.
    let least = |best: (f64, f64), theta: f64| {
        let size = beta.at(theta).abs();
        if size < best.1 { (theta, size) } else { best }
    };
    let start = beta.roots().into_iter().chain(beta.rate().roots()).fold((0.0, f64::INFINITY), least).0;
    let [g0, gc, gs] = ways;
    let way = |phi: f64| {
        let (sin, cos) = sin_cos(phi);
        unit(g0 + gc * cos + gs * sin)
    };
    let reaching = quadric.reached(&alpha, way, decide)?;
    Ok(Nodal { fan: Some(Fan::through(p0, ways, (&alpha, &beta))), at, start, reaching })
}
