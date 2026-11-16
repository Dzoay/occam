//! Source: original
//! A cross-check of the classification by the pencil's determinant: where the surfaces touch, found on the rulings.
//! On the ruling at `θ` the other quadric is least or most at `v = −β/α`, where it is `−Δ/α`, so it touches the ruled
//! surface only where `Δ/α` has a critical point, a root of `Δ'α − Δα'`. Each such point on the other surface within
//! the band, with the two normals within the angle band of each other, is a touch, and a pencil with none has no
//! multiple root, with one a node, and with two a split.

use occam_math::Side;
use occam_units::Vec3;

use super::super::Pair;
use super::super::quadric::{Along, Ruling, sine_between};
use crate::decide::{Decisions, Undecided};

/// A touch found on the rulings.
pub(super) struct Touch {
    pub(super) at: Vec3,
}

/// The touches on the rulings of `ruling`, surface `r` of the pair, with surface `o`: `None` where the other quadric
/// is the same along every ruling and touches, so that the two are tangent along a whole curve.
pub(super) fn touches(
    pair: &Pair,
    (r, o, ruling): (usize, usize, &Ruling),
    along: &Along,
    decide: &mut Decisions,
) -> Result<Option<Vec<Touch>>, Undecided> {
    let critical = along.critical();
    let scale = along.delta.size() * along.alpha.size();
    let flat = critical.size() <= 1e-13 * scale;
    let candidates = if flat { vec![0.0] } else { critical.roots(1e-13 * scale) };
    let mut found: Vec<Touch> = Vec::new();
    for theta in candidates {
        let v = along.middle(theta);
        let at = pair.world(ruling.point(theta, v));
        if decide.length(pair.surface(o).gap(at))? != Side::Within {
            continue;
        }
        let normals = (pair.surface(r).normal(at), pair.surface(o).normal(at));
        if decide.angle(sine_between(normals.0, normals.1))? != Side::Within {
            continue;
        }
        if flat {
            return Ok(None);
        }
        if !found.iter().any(|t| (t.at - at).norm() <= 1e-9 * (1.0 + at.norm())) {
            found.push(Touch { at });
        }
    }
    Ok(Some(found))
}
