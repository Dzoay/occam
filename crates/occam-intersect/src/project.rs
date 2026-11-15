//! Source: von-neumann-1950
//! Alternating projections (von Neumann): a point projected onto one surface, then onto the other, and back, comes to
//! where they meet. Newton's method loses rank where the surfaces touch, as their normals are parallel there, and the
//! projections do not. Near a touch each round moves the point only as far as the surfaces' curvature carries it, so
//! each round here is followed by a search along the way it moved: doubling the move while the gap shrinks, then the
//! vertex of the parabola through the last three gaps, which is where a touch is when the gap is a square of the
//! distance from it, as it is between two smooth surfaces. The search is ours, not von Neumann's.

use occam_units::{UvPoint, Vec3};

use crate::correct::{Here, Other, Pair, foot};

/// The most rounds of projections.
const ROUNDS: u32 = 64;

/// The most doublings of a move.
const DOUBLINGS: u32 = 60;

/// A point of `p` and the gap there between the two surfaces.
#[derive(Clone, Copy, Debug)]
struct Met {
    here: Here,
    gap: f64,
}

/// The point of `p` nearest `x`, from the parameters `seed`, and the gap between the surfaces there.
fn met(pair: &Pair<'_>, x: Vec3, seed: &Here) -> Met {
    let p = foot(pair.p, x, seed.p);
    let at = pair.p.point(p).to_vec3();
    match pair.other {
        Other::Implicit(form) => Met { here: Here { at, p, q: seed.q }, gap: form.at(at).0.abs() },
        Other::Parametric => {
            let q = foot(pair.q, at, seed.q);
            Met { here: Here { at, p, q }, gap: (at - pair.q.point(q).to_vec3()).norm() }
        }
    }
}

/// The point `x` projected onto `q`.
fn onto_q(pair: &Pair<'_>, x: Vec3, seed: UvPoint) -> Vec3 {
    match pair.other {
        Other::Implicit(form) => form.onto(x),
        Other::Parametric => pair.q.point(foot(pair.q, x, seed)).to_vec3(),
    }
}

/// The lesser gap of two.
fn nearer(one: Met, other: Met) -> Met {
    if other.gap < one.gap { other } else { one }
}

/// The vertex of the parabola through three points `(t, g)`, if it has one between the first and the last.
fn vertex([(t0, g0), (t1, g1), (t2, g2)]: [(f64, f64); 3]) -> Option<f64> {
    let (a, b) = ((t1 - t0) * (g1 - g2), (t1 - t2) * (g1 - g0));
    let t = t1 - 0.5 * ((t1 - t0) * a - (t1 - t2) * b) / (a - b);
    (t > t0 && t < t2).then_some(t)
}

/// From `base`, the least gap along the way `step` goes, found by doubling a move while the gap shrinks, then at the
/// vertex of the parabola through the last three gaps. The first move is `step`, or the band where `step` is
/// shorter: near a touch a round moves the point by the cube of its distance from it, too little for the gap to show.
fn search(pair: &Pair<'_>, base: Met, step: Vec3) -> Met {
    let length = step.norm();
    if !length.is_finite() || length <= 0.0 {
        return base;
    }
    let (toward, first) = (step * (1.0 / length), if length > pair.band() { length } else { pair.band() });
    let at = |t: f64, seed: &Here| met(pair, base.here.at + toward * t, seed);
    let mut best = at(first, &base.here);
    if best.gap >= base.gap {
        return base;
    }
    let mut samples = [(0.0, base.gap), (0.0, base.gap), (first, best.gap)];
    for _ in 0..DOUBLINGS {
        let t = samples[2].0 * 2.0;
        let next = at(t, &best.here);
        samples = [samples[1], samples[2], (t, next.gap)];
        if next.gap >= best.gap {
            break;
        }
        best = next;
    }
    vertex(samples).map_or(best, |t| nearer(best, at(t, &best.here)))
}

/// Where the two surfaces meet near `from`, within a quarter of the band of both, by alternating projections, and
/// the rounds it took; `None` where they did not come within it, or no round brings them nearer. The rounds go on
/// until a round moves the point by no more than the band, so that at a touch they come to the touch itself, not
/// just within the band of both surfaces, which a touch is over a disc `sqrt(2 r band)` across. `q`'s parameters are
/// those of its foot.
pub(crate) fn project(pair: &Pair<'_>, from: &Here) -> (Option<Here>, u32) {
    let mut best = met(pair, from.at, from);
    for round in 1..=ROUNDS {
        let next = met(pair, onto_q(pair, best.here.at, best.here.q), &best.here);
        let better = nearer(nearer(best, next), search(pair, best, next.here.at - best.here.at));
        let moved = (better.here.at - best.here.at).norm();
        best = better;
        if moved <= pair.band() && best.gap <= pair.band() * 0.25 {
            let q = pair.q_at(best.here.at, best.here.q);
            return (q.map(|q| Here { q, ..best.here }), round);
        }
        if moved == 0.0 {
            return (None, round);
        }
    }
    (None, ROUNDS)
}
