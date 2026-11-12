//! Source: original
//! The foot of a point on a surface where no formula gives it: a grid of samples over a window, each sample nearer than
//! its neighbours refined by Newton's method kept inside the window, and the nearest of those. It needs nothing of the
//! family but its second derivatives, so every surface family without a closed form shares it, as curves share theirs.

use occam_units::{Point, UvPoint, Vec3, World};

use crate::batch::Scratch;
use crate::domain::UvBox;
use crate::surface::Parametric;

/// The most Newton steps from one sample nearer than its neighbours, or from a seed. Each step must bring the point
/// nearer, so it stops sooner where none can.
const STEPS: u32 = 64;

/// The most Newton steps from any other sample. Such a sample is refined only in case the foot is in a cell beside it,
/// and a step reaches a cell, so a few steps find a foot there; one further away is found from its own cell's sample.
const NEARBY: u32 = 6;

/// The most times a step is halved in search of one that brings the point nearer.
const HALVINGS: u32 = 48;

/// The value a fraction `f` of the way from `lo` to `hi`.
fn lerp(lo: f64, hi: f64, f: f64) -> f64 {
    lo + (hi - lo) * f
}

/// `x` moved to the nearer end of `lo` to `hi` where it is beyond one.
fn inside(x: f64, lo: f64, hi: f64) -> f64 {
    if x < lo {
        lo
    } else if x > hi {
        hi
    } else {
        x
    }
}

/// The squared distance from `q` of the point of `surface` at `uv`.
fn gap<S: Parametric>(surface: &S, q: Vec3, uv: UvPoint) -> f64 {
    let d = surface.eval_point(uv).to_vec3() - q;
    d.dot(d)
}

/// Whether a parameter at `x`, from `lo` to `hi`, may move: not at an end where the slope `g` of the squared distance
/// would take it out.
fn free(x: f64, (lo, hi): (f64, f64), g: f64) -> bool {
    !((x <= lo && g > 0.0) || (x >= hi && g < 0.0))
}

/// The step `d` with `h d = -g` in the parameters that are free, for `h = ((a, b), (b, c))`, where `h` is positive
/// definite on them; `None` where it is not, or where neither parameter may move.
fn newton((a, b, c): (f64, f64, f64), (gu, gv): (f64, f64), free: (bool, bool)) -> Option<(f64, f64)> {
    match free {
        (true, true) => {
            let det = a * c - b * b;
            (a > 0.0 && det > 0.0).then(|| ((b * gv - c * gu) / det, (b * gu - a * gv) / det))
        }
        (true, false) => (a > 0.0).then(|| (-gu / a, 0.0)),
        (false, true) => (c > 0.0).then(|| (0.0, -gv / c)),
        (false, false) => None,
    }
}

/// From `at`, where the squared distance is `here`, the first of `step`, its half, its quarter and on, kept inside
/// `window`, that brings the point nearer, and the squared distance there.
fn nearer<S: Parametric>(
    surface: &S,
    q: Vec3,
    (at, here): (UvPoint, f64),
    step: (f64, f64),
    window: UvBox,
) -> Option<(UvPoint, f64)> {
    let mut scale = 1.0;
    for _ in 0..HALVINGS {
        let u = inside(at.u() + scale * step.0, window.lo.u(), window.hi.u());
        let v = inside(at.v() + scale * step.1, window.lo.v(), window.hi.v());
        let next = UvPoint::new(u, v);
        let there = gap(surface, q, next);
        if there < here {
            return Some((next, there));
        }
        scale *= 0.5;
    }
    None
}

/// `step` shortened, its direction kept, so that it moves no further than `reach` in either parameter.
fn within((du, dv): (f64, f64), reach: (f64, f64)) -> (f64, f64) {
    let over = |d: f64, most: f64| if d.abs() > most { most / d.abs() } else { 1.0 };
    let (a, b) = (over(du, reach.0), over(dv, reach.1));
    let scale = if b < a { b } else { a };
    (du * scale, dv * scale)
}

/// The point of `surface` nearest `q` that Newton's method reaches from `start` without leaving `window`, and its
/// squared distance. Where the Hessian of the squared distance is not positive definite it takes the Gauss-Newton
/// step, which is wherever the derivatives are independent, and a parameter at an end that would leave stays there.
/// It takes at most `steps` steps, and none moves further than `reach`, a cell of the grid the start came from, so that a start refines its own valley:
/// a full step can land on another sheet of a surface that passes near itself, nearer than the start but not the foot
/// of that valley, and from there the valley the start was in is lost.
fn refine<S: Parametric>(
    surface: &S,
    q: Vec3,
    (start, steps): (UvPoint, u32),
    (window, reach): (UvBox, (f64, f64)),
) -> (UvPoint, f64) {
    let mut best = (start, gap(surface, q, start));
    for _ in 0..steps {
        let at = best.0;
        let jet = surface.eval_second(at);
        let (r, su, sv) = (jet.first.point.to_vec3() - q, jet.first.du.rate(), jet.first.dv.rate());
        let g = (r.dot(su), r.dot(sv));
        let gauss = (su.dot(su), su.dot(sv), sv.dot(sv));
        let full = (gauss.0 + r.dot(jet.duu.rate()), gauss.1 + r.dot(jet.duv.rate()), gauss.2 + r.dot(jet.dvv.rate()));
        let open =
            (free(at.u(), (window.lo.u(), window.hi.u()), g.0), free(at.v(), (window.lo.v(), window.hi.v()), g.1));
        let Some(step) = newton(full, g, open).or_else(|| newton(gauss, g, open)) else { break };
        let Some(next) = nearer(surface, q, best, within(step, reach), window) else { break };
        best = next;
    }
    best
}

/// Whether sample `index` of `gaps`, a grid `columns` wide, is nearer than every neighbour before it and no further than
/// every neighbour after it, so that of a run of equal samples only the first counts.
fn lowest(gaps: &[f64], columns: usize, index: usize) -> bool {
    let Some(&here) = gaps.get(index) else { return false };
    let (row, column) = (index.checked_div(columns).unwrap_or(0), index.checked_rem(columns).unwrap_or(0));
    let last_row = gaps.len().div_ceil(columns.max(1)).saturating_sub(1);
    let last_column = columns.saturating_sub(1);
    let mut holds = true;
    for r in row.saturating_sub(1)..=row.saturating_add(1).min(last_row) {
        for c in column.saturating_sub(1)..=column.saturating_add(1).min(last_column) {
            let Some(&there) = gaps.get(r.saturating_mul(columns).saturating_add(c)) else { continue };
            if (r, c) < (row, column) {
                holds &= here < there;
            } else if (r, c) > (row, column) {
                holds &= here <= there;
            }
        }
    }
    holds
}

/// The parameters in `window` of the point of `surface` nearest `p`: from a grid of `steps.0 + 1` by `steps.1 + 1`
/// samples, edges included, the nearest of what Newton's method reaches from `seed`, where there is one, from each sample
/// nearer than its neighbours, and then from every sample that could lie within a cell of a nearer point. The foot lies
/// in some cell, within that cell's corner sample's [`reach`] of it, so the sample is no further than the foot and its
/// reach: a valley narrower than a cell, as where a surface passes near itself, has no sample nearer than its
/// neighbours, but it is not missed.
pub(crate) fn foot<S: Parametric>(
    surface: &S,
    p: Point<World>,
    window: UvBox,
    steps: (u32, u32),
    seed: Option<UvPoint>,
    scratch: &mut Scratch,
) -> UvPoint {
    let (q, (lo, hi)) = (p.to_vec3(), (window.lo, window.hi));
    let (nu, nv) = (steps.0.max(1), steps.1.max(1));
    let cell = cell(window, (nu, nv));
    let columns = nu.saturating_add(1);
    let at = |index: usize| {
        let index = u32::try_from(index).unwrap_or(u32::MAX);
        let (i, j) = (index.checked_rem(columns).unwrap_or(0), index.checked_div(columns).unwrap_or(0));
        UvPoint::new(
            lerp(lo.u(), hi.u(), f64::from(i) / f64::from(nu)),
            lerp(lo.v(), hi.v(), f64::from(j) / f64::from(nv)),
        )
    };
    let count = usize::try_from(columns.saturating_mul(nv.saturating_add(1))).unwrap_or(0);
    let (gaps, reaches) = scratch.reals(count.saturating_mul(2)).split_at_mut(count);
    for (index, (gap, reached)) in gaps.iter_mut().zip(reaches.iter_mut()).enumerate() {
        let jet = surface.eval_first(at(index));
        let d = jet.point.to_vec3() - q;
        (*gap, *reached) = (d.dot(d), reach((jet.du.rate().norm(), jet.dv.rate().norm()), cell));
    }
    let width = usize::try_from(columns).unwrap_or(1);
    let lowest = |index: &usize| lowest(gaps, width, *index);
    let seed = seed.map(|s| UvPoint::new(inside(s.u(), lo.u(), hi.u()), inside(s.v(), lo.v(), hi.v())));
    let from = |start: UvPoint| refine(surface, q, (start, STEPS), (window, cell));
    let nearer = |a: (UvPoint, f64), b: (UvPoint, f64)| if b.1 < a.1 { b } else { a };
    let mut best =
        seed.into_iter().chain((0..count).filter(lowest).map(at)).map(from).fold((lo, f64::INFINITY), nearer);
    for (index, (gap, reached)) in gaps.iter().zip(reaches.iter()).enumerate() {
        if !lowest(&index) && could_be_nearer(*gap, *reached, best.1) {
            best = nearer(best, refine(surface, q, (at(index), NEARBY), (window, cell)));
        }
    }
    best.0
}

/// The size in each parameter of a cell of a grid of `steps` cells a side over `window`.
fn cell(window: UvBox, steps: (u32, u32)) -> (f64, f64) {
    let (lo, hi) = (window.lo, window.hi);
    ((hi.u() - lo.u()) / f64::from(steps.0), (hi.v() - lo.v()) / f64::from(steps.1))
}

/// Whether a sample at the square root of `gap` from the point, whose cells the surface keeps within `reached` of it,
/// could have in them a point nearer than the square root of `best`. NaN could.
fn could_be_nearer(gap: f64, reached: f64, best: f64) -> bool {
    let far = gap.sqrt() - reached >= best.sqrt();
    !far
}

/// How far the surface can be from a sample anywhere in a cell beside it, from its first derivatives `rates` there and
/// the cell's size: twice their sum times the cell, as a derivative within the cell can exceed those at its corner. Not
/// finite where a derivative is not, which leaves that sample in.
fn reach(rates: (f64, f64), cell: (f64, f64)) -> f64 {
    2.0 * (rates.0 * cell.0 + rates.1 * cell.1)
}

#[cfg(test)]
mod tests {
    use occam_units::Placement;

    use super::*;
    use crate::conformance::placement;
    use crate::plane::Plane;
    use crate::sphere::Sphere;

    #[test]
    fn only_the_first_of_equal_neighbours_is_lowest() {
        let flat = [1.0; 9];
        let lowest_of = |gaps: &[f64]| (0..gaps.len()).filter(|i| lowest(gaps, 3, *i)).collect::<Vec<_>>();
        assert_eq!(lowest_of(&flat), [0]);
        assert_eq!(lowest_of(&[3.0, 2.0, 3.0, 2.0, 1.0, 2.0, 3.0, 2.0, 0.5]), [8]);
        assert_eq!(lowest_of(&[1.0, 2.0, 3.0, 2.0, 5.0, 2.0, 3.0, 2.0, 1.0]), [0, 8]);
        assert!(!lowest(&flat, 3, 9), "past the grid");
    }

    #[test]
    fn a_cell_is_the_window_over_the_steps() {
        assert_eq!(cell(UvBox::new(UvPoint::new(1.0, 2.0), UvPoint::new(4.0, 10.0)), (3, 4)), (1.0, 2.0));
    }

    #[test]
    fn a_sample_is_refined_only_where_its_cells_could_hold_a_nearer_point() {
        // At 3 from the point, reaching 1 about it: 2 is the nearest its cells can be, so a best of 2 needs nothing.
        assert!(!could_be_nearer(9.0, 1.0, 4.0), "as near as the best, not nearer");
        assert!(could_be_nearer(9.0, 1.5, 4.0) && !could_be_nearer(9.0, 0.5, 4.0));
        assert!(could_be_nearer(9.0, f64::NAN, 4.0) && could_be_nearer(9.0, 0.0, f64::INFINITY));
    }

    #[test]
    fn the_reach_is_twice_the_derivatives_times_the_cell_and_a_nan_reaches_everywhere() {
        assert_eq!(reach((2.0, 3.0), (0.5, 0.25)), 3.5);
        assert!(reach((f64::NAN, 1.0), (1.0, 1.0)).is_nan());
    }

    #[test]
    fn a_step_is_shortened_to_its_reach_in_its_own_direction() {
        assert_eq!(within((0.5, -0.25), (1.0, 1.0)), (0.5, -0.25));
        assert_eq!(within((4.0, -1.0), (1.0, 1.0)), (1.0, -0.25), "u reaches furthest");
        assert_eq!(within((0.5, -3.0), (1.0, 1.5)), (0.25, -1.5), "v reaches furthest");
        assert_eq!(within((-2.0, 0.0), (0.5, 0.1)), (-0.5, 0.0));
    }

    #[test]
    fn a_step_too_long_is_halved_until_it_brings_the_point_nearer() {
        // On the plane, from (1, 0) towards the origin: the whole step of 3 overshoots to 4 away, the half to a quarter.
        let plane = Plane::placed(&Placement::identity());
        let window = UvBox::new(UvPoint::new(-10.0, -10.0), UvPoint::new(10.0, 10.0));
        let q = Vec3::new(0.0, 0.0, 0.0);
        let (uv, gap) = nearer(&plane, q, (UvPoint::new(1.0, 0.0), 1.0), (-3.0, 0.0), window).unwrap();
        assert_eq!((uv.u(), uv.v(), gap), (-0.5, 0.0, 0.25));
        assert_eq!(nearer(&plane, q, (UvPoint::new(0.0, 0.0), 0.0), (1.0, 0.0), window), None, "nothing is nearer");
    }

    #[test]
    fn a_parameter_beyond_an_end_is_moved_to_it() {
        assert_eq!([inside(-1.0, 0.0, 1.0), inside(2.0, 0.0, 1.0), inside(0.25, 0.0, 1.0)], [0.0, 1.0, 0.25]);
    }

    #[test]
    fn newton_refuses_a_pivot_of_zero_in_any_case() {
        assert_eq!(newton((1.0, 1.0, 1.0), (1.0, 2.0), (true, true)), None, "singular");
        assert_eq!(newton((0.0, 0.0, 1.0), (1.0, 0.0), (true, false)), None);
        assert_eq!(newton((1.0, 0.0, 0.0), (0.0, 1.0), (false, true)), None);
    }

    #[test]
    fn newtons_full_step_reaches_a_foot_off_a_curved_surface_in_a_few_steps() {
        // Two off a sphere of radius 5: the residual's curvature is in the Hessian, so the rate is quadratic; without it,
        // as Gauss and Newton's step, it is linear at about two fifths a step.
        let sphere = Sphere::placed(&placement(), occam_units::Length::from_millimetres(5.0)).unwrap();
        let target = UvPoint::new(1.1, 0.3);
        let on = sphere.eval_first(target);
        let normal = on.du.rate().cross(on.dv.rate());
        let q = on.point.to_vec3() + normal * (2.0 / normal.norm());
        let window = UvBox::new(UvPoint::new(0.0, -1.5), UvPoint::new(3.0, 1.5));
        let (uv, _) = refine(&sphere, q, (UvPoint::new(1.4, 0.1), 5), (window, (1.0, 1.0)));
        assert!((uv.u() - 1.1).abs() < 1e-10 && (uv.v() - 0.3).abs() < 1e-10, "{uv:?}");
    }

    #[test]
    fn newton_steps_in_the_free_parameters_where_it_can() {
        assert_eq!(newton((2.0, 0.0, 4.0), (2.0, -4.0), (true, true)), Some((-1.0, 1.0)));
        assert_eq!(newton((2.0, 1.0, 4.0), (2.0, -4.0), (true, false)), Some((-1.0, 0.0)));
        assert_eq!(newton((2.0, 1.0, 4.0), (2.0, -4.0), (false, true)), Some((0.0, 1.0)));
        assert_eq!(newton((1.0, 2.0, 1.0), (1.0, 1.0), (true, true)), None, "not positive definite");
        assert_eq!(newton((-1.0, 0.0, 1.0), (1.0, 1.0), (true, false)), None);
        assert_eq!(newton((1.0, 0.0, 1.0), (1.0, 1.0), (false, false)), None);
        assert!(free(0.0, (0.0, 1.0), -1.0) && !free(0.0, (0.0, 1.0), 1.0) && !free(1.0, (0.0, 1.0), -1.0));
    }

    #[test]
    fn the_foot_on_a_patch_is_found_inside_it_or_on_its_edge() {
        let plane = Plane::placed(&Placement::identity());
        let window = UvBox::new(UvPoint::new(-1.0, -2.0), UvPoint::new(3.0, 4.0));
        let mut scratch = Scratch::default();
        let p = Point::from_vec3(Vec3::new(1.25, 0.5, 7.0));
        let uv = foot(&plane, p, window, (4, 4), None, &mut scratch);
        assert!((uv.u() - 1.25).abs() < 1e-12 && (uv.v() - 0.5).abs() < 1e-12, "{uv:?}");
        let beyond = foot(&plane, Point::from_vec3(Vec3::new(9.0, 9.0, 1.0)), window, (4, 4), None, &mut scratch);
        assert_eq!((beyond.u(), beyond.v()), (3.0, 4.0), "the corner");
        let edge = foot(&plane, Point::from_vec3(Vec3::new(1.0, -5.0, 1.0)), window, (4, 4), None, &mut scratch);
        assert!((edge.u() - 1.0).abs() < 1e-12, "{edge:?}");
        assert_eq!(edge.v(), -2.0);
    }

    #[test]
    fn of_several_valleys_the_deepest_is_found() {
        // A point just outside a sphere: the grid holds a valley by it and another across the pole.
        let sphere = Sphere::placed(&placement(), occam_units::Length::from_millimetres(5.0)).unwrap();
        let window = UvBox::new(UvPoint::new(0.0, -1.5), UvPoint::new(6.0, 1.5));
        let target = UvPoint::new(4.1, 0.3);
        let on = sphere.eval_first(target);
        let normal = on.du.rate().cross(on.dv.rate());
        let p = Point::from_vec3(on.point.to_vec3() + normal * (0.5 / normal.norm()));
        let mut scratch = Scratch::default();
        let uv = foot(&sphere, p, window, (6, 6), Some(UvPoint::new(1.0, 0.0)), &mut scratch);
        assert!((uv.u() - 4.1).abs() < 1e-9 && (uv.v() - 0.3).abs() < 1e-9, "{uv:?}");
    }
}
