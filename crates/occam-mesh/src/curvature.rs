//! Source: do-carmo-1976
//! How sharply a face bends, which is what it asks of its edges: the largest principal curvature of its surface over an
//! 8 by 8 grid of its box in the parameters (P9's phase 0), from the first and second fundamental forms (do Carmo,
//! chapter 3). Its edges' curves in its parameters bound the box.
//!
//! No estimate is read where the surface collapses (P9: on a collapsed row the derivative is rounding noise, its
//! "normal" points anywhere, and the curvature read near 1e15). A grid point is skipped where a neighbour along either
//! parameter lies on it within the band, as along a collapsed row or at a pole, and where its curvature is not a number,
//! as where its derivatives span no normal at all.

use occam_geom::{Jet2, Surface, UvBox};
use occam_topo::{Face, Id, Loop, Snapshot, View};
use occam_units::{Point, UvPoint, World};

use crate::limits::{higher, lower};

/// How many grid points along each parameter, the box's edges included.
const GRID: usize = 8;

/// The largest principal curvature of `face`'s surface, per millimetre, over its grid: 0 for a face on a flat surface,
/// one with no curve in its parameters to bound its box, or one whose every grid point is skipped.
pub(crate) fn of_face(model: &Snapshot, face: Id<Face>) -> f64 {
    let Some(view) = model.view(face) else { return 0.0 };
    let Some(surface) = view.surface() else { return 0.0 };
    let uses = view.loops().into_iter().filter_map(|l| model.view(l)).flat_map(View::<Loop>::uses);
    let mut boxes = uses.filter_map(|u| {
        let side = model.view(u)?;
        let (lo, hi) = model.view(side.edge())?.range();
        Some(side.pcurve()?.bounds(lo, hi))
    });
    let Some(first) = boxes.next() else { return 0.0 };
    let patch = boxes.fold(first, joined);
    largest(surface, patch, model.tolerance().band().millimetres())
}

/// The box holding both.
fn joined(a: UvBox, b: UvBox) -> UvBox {
    let lo = UvPoint::new(lower(a.lo.u(), b.lo.u()), lower(a.lo.v(), b.lo.v()));
    let hi = UvPoint::new(higher(a.hi.u(), b.hi.u()), higher(a.hi.v(), b.hi.v()));
    UvBox::new(lo, hi)
}

/// The grid's points on `patch`, a row of constant `v` at a time.
fn grid(patch: UvBox) -> Vec<UvPoint> {
    let count = u32::try_from(GRID).unwrap_or(0);
    let last = f64::from(count.saturating_sub(1));
    let along = |lo: f64, hi: f64, k: u32| lo + (hi - lo) * (f64::from(k) / last);
    let steps = 0..count;
    let rows = steps.clone().map(|j| along(patch.lo.v(), patch.hi.v(), j));
    rows.flat_map(|v| steps.clone().map(move |i| UvPoint::new(along(patch.lo.u(), patch.hi.u(), i), v))).collect()
}

/// Which of the grid's `points` lie within `band` of a neighbour along either parameter.
fn collapsed(points: &[Point<World>], band: f64) -> Vec<bool> {
    // A point's neighbour along `u` is the next in its row, if the row goes on; along `v`, the same in the next row.
    let along_u = |k: usize| Some(k.saturating_add(1)).filter(|next| !next.is_multiple_of(GRID));
    let neighbours = |k: usize| [along_u(k), Some(k.saturating_add(GRID))].into_iter().flatten().map(move |n| (k, n));
    let mut skipped = vec![false; points.len()];
    for (a, b) in (0..points.len()).flat_map(neighbours) {
        let (Some(&p), Some(&q)) = (points.get(a), points.get(b)) else { continue };
        if (p - q).to_vec3().norm() <= band {
            skipped.iter_mut().enumerate().filter(|(k, _)| *k == a || *k == b).for_each(|(_, s)| *s = true);
        }
    }
    skipped
}

/// The largest principal curvature of `surface` over the grid on `patch`, skipping where a neighbour lies within
/// `band`.
pub(crate) fn largest(surface: &dyn Surface, patch: UvBox, band: f64) -> f64 {
    let jets: Vec<Jet2> = grid(patch).into_iter().map(|uv| surface.second(uv)).collect();
    let points: Vec<Point<World>> = jets.iter().map(|jet| jet.first.point).collect();
    let kept = jets.iter().zip(collapsed(&points, band)).filter(|(_, skipped)| !skipped);
    kept.fold(0.0, |most, (jet, _)| higher(most, principal(jet)))
}

/// The larger of the principal curvatures' sizes at `jet`, `|H| + sqrt(H^2 - K)`: NaN where its derivatives span no
/// normal.
fn principal(jet: &Jet2) -> f64 {
    let (su, sv) = (jet.first.du.rate(), jet.first.dv.rate());
    let across = su.cross(sv);
    let normal = across * (1.0 / across.norm());
    // The first fundamental form's coefficients, and the second's.
    let (uu, uv, vv) = (su.dot(su), su.dot(sv), sv.dot(sv));
    let (nuu, nuv, nvv) = (jet.duu.rate().dot(normal), jet.duv.rate().dot(normal), jet.dvv.rate().dot(normal));
    let first = uu * vv - uv * uv;
    let gauss = (nuu * nvv - nuv * nuv) / first;
    let mean = (uu * nvv - 2.0 * uv * nuv + vv * nuu) / (2.0 * first);
    // H^2 - K is never below zero, so a negative value is rounding, and its size is as small.
    mean.abs() + (mean * mean - gauss).abs().sqrt()
}

#[cfg(test)]
mod tests;
