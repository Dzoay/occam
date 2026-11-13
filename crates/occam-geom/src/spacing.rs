//! Source: original
//! A curve cut into points (PARITY 2.1): uniformly by distance, each point the same length along the curve from the one
//! before, to a tolerance; and by deflection, so that no part of the curve between two neighbours is further from the
//! chord joining them than asked. Both read only `dyn Curve`.
//!
//! The deflection is measured, not estimated from curvature: on each piece, the distance from the curve to its chord is
//! sampled, each sample nearer the chord than both its neighbours' further ones is skipped, and each local largest is
//! refined by golden-section search; a piece whose largest exceeds the deflection is cut there. A piece is first cut
//! into [`START`] so that the chord of a closed or nearly closed curve is never a point.

use occam_report::Report;
use occam_units::{Length, Vec3};

use crate::arc::{integral, length, not_converged};
use crate::curve::Curve;
use crate::support::positive;

/// How many pieces a curve is cut into before any is measured.
const START: u32 = 8;

/// How many samples measure a piece's distance from its chord.
const SAMPLES: u32 = 16;

/// The most pieces a discretisation may make before it gives up.
const BUDGET: usize = 1 << 20;

/// The value a fraction `f` of the way from `a` to `b`.
fn lerp(a: f64, b: f64, f: f64) -> f64 {
    a + (b - a) * f
}

/// Halfway from `lo` to `hi`.
fn middle(lo: f64, hi: f64) -> f64 {
    0.5 * lo + 0.5 * hi
}

/// Whether a discretisation with `entries` pieces made or waiting is within [`BUDGET`].
const fn within_budget(entries: usize) -> bool {
    entries <= BUDGET
}

/// How far `p` is from the segment from `a` to `b`.
fn from_chord(p: Vec3, (a, b): (Vec3, Vec3)) -> f64 {
    let along = b - a;
    let reach = along.dot(along);
    let f = if reach > 0.0 { (p - a).dot(along) / reach } else { 0.0 };
    let past_start = if f > 0.0 { f } else { 0.0 };
    let within = if past_start < 1.0 { past_start } else { 1.0 };
    (p - (a + along * within)).norm()
}

/// The ends `lo` and `hi` in order and checked against the range.
fn ends(c: &dyn Curve, (lo, hi): (f64, f64)) -> Result<(f64, f64), Report> {
    let range = c.range();
    range.check(lo)?;
    range.check(hi)?;
    Ok(if hi < lo { (hi, lo) } else { (lo, hi) })
}

/// `count` parameters from `lo` to `hi`, ends included, the length along `c` between each and the next the same to
/// within `tolerance`.
///
/// # Errors
///
/// `geom.size-not-positive` for a tolerance that is not a finite length above zero, `geom.undefined-parameter` for an
/// end outside the range, `geom.wrong-count` for fewer than two points, and `measure.not-converged` where a length
/// cannot reach the tolerance.
pub fn by_count(c: &dyn Curve, (lo, hi): (f64, f64), count: usize, tolerance: Length) -> Result<Vec<f64>, Report> {
    let (lo, hi) = ends(c, (lo, hi))?;
    let tol = positive(tolerance)?;
    if count < 2 {
        return Err(crate::bspline::wrong_count(count, 2));
    }
    let total = length(c, (lo, hi), Length::from_millimetres(0.25 * tol))?.millimetres();
    let pieces = f64::from(u32::try_from(count.saturating_sub(1)).unwrap_or(u32::MAX));
    let mut out = Vec::with_capacity(count);
    out.push(lo);
    let (mut at, mut reached) = (lo, 0.0);
    for k in 1..count.saturating_sub(1) {
        let target = f64::from(u32::try_from(k).unwrap_or(u32::MAX)) * total / pieces;
        let (next, step, _) = advance(c, (at, hi), target - reached, 0.25 * tol)?;
        (at, reached) = (next, reached + step);
        out.push(at);
    }
    out.push(hi);
    Ok(out)
}

/// As [`by_count`], with as few points as keep each length along the curve between neighbours no more than `spacing`.
///
/// # Errors
///
/// As [`by_count`], and `geom.size-not-positive` for a spacing that is not a finite length above zero.
pub fn by_spacing(c: &dyn Curve, (lo, hi): (f64, f64), spacing: Length, tolerance: Length) -> Result<Vec<f64>, Report> {
    let (lo, hi) = ends(c, (lo, hi))?;
    let (step, tol) = (positive(spacing)?, positive(tolerance)?);
    let total = length(c, (lo, hi), Length::from_millimetres(0.25 * tol))?.millimetres();
    let mut pieces = 1_u32;
    while f64::from(pieces) * step < total {
        pieces = pieces.checked_add(1).ok_or_else(|| not_converged(total, step))?;
        if !usize::try_from(pieces).is_ok_and(|p| p <= BUDGET) {
            return Err(not_converged(total, step));
        }
    }
    by_count(c, (lo, hi), usize::try_from(pieces).unwrap_or(1).saturating_add(1), tolerance)
}

/// The parameter after `from`, at most `hi`, where the length along `c` from `from` is `step`, to within `tol`, the
/// length measured there, and how many lengths were measured: Newton's method on the length, from a guess by the speed
/// at `from`, kept between `from` and `hi`, halving the bracket where Newton's step would leave it. Each point aims at
/// its length from the start, so errors do not add up along the curve.
fn advance(c: &dyn Curve, (from, hi): (f64, f64), step: f64, tol: f64) -> Result<(f64, f64, u32), Report> {
    let speed = |t: f64| c.first(t).1.rate().norm();
    let along = |t: f64| integral(&speed, (from, t), 0.25 * tol).map(|(s, _)| s);
    let (mut low, mut high, guess) = (from, hi, from + step / speed(from));
    let mut at = if guess > from && guess < hi { guess } else { middle(from, hi) };
    for steps in 1..=200 {
        let measured = along(at)?;
        let gap = measured - step;
        if gap.abs() <= tol {
            return Ok((at, measured, steps));
        }
        if gap < 0.0 {
            low = at;
        } else {
            high = at;
        }
        let newton = at - gap / speed(at);
        let next = if low < newton && newton < high { newton } else { middle(low, high) };
        if next.to_bits() == at.to_bits() {
            return Ok((at, measured, steps));
        }
        at = next;
    }
    Err(not_converged(step, tol))
}

/// The distance from the chord `(a, b)` of the curve at `t`.
fn deviation(c: &dyn Curve, chord: (Vec3, Vec3), t: f64) -> f64 {
    from_chord(c.point(t).to_vec3(), chord)
}

/// The largest distance of `c` from the chord between its points at `a` and `b`, and where: the samples' local largest,
/// each refined by golden-section search between its neighbours, the first of two as far. Where nothing is further than
/// nought, at `a`.
fn furthest(c: &dyn Curve, (a, b): (f64, f64)) -> (f64, f64) {
    let chord = (c.point(a).to_vec3(), c.point(b).to_vec3());
    let at = |k: u32| lerp(a, b, f64::from(k) / f64::from(SAMPLES));
    let values: Vec<f64> = (0..=SAMPLES).map(|k| deviation(c, chord, at(k))).collect();
    let mut best = (0.0, a);
    for (k, window) in (1_u32..).zip(values.windows(3)) {
        let (before, here, after) = (window.first(), window.get(1), window.get(2));
        let (Some(&before), Some(&here), Some(&after)) = (before, here, after) else { continue };
        if here >= before && here >= after {
            let peak = golden(c, chord, (at(k.saturating_sub(1)), at(k.saturating_add(1))));
            if peak.0 > best.0 {
                best = peak;
            }
        }
    }
    best
}

/// The largest distance from `chord` of `c` between `lo` and `hi`, and where, by golden-section search: of a stretch
/// as far, its end towards `hi`.
fn golden(c: &dyn Curve, chord: (Vec3, Vec3), (mut lo, mut hi): (f64, f64)) -> (f64, f64) {
    let ratio = 0.5 * (5.0_f64.sqrt() - 1.0);
    for _ in 0..80 {
        let (x, y) = (hi - ratio * (hi - lo), lo + ratio * (hi - lo));
        if deviation(c, chord, x) > deviation(c, chord, y) {
            hi = y;
        } else {
            lo = x;
        }
    }
    let t = 0.5 * lo + 0.5 * hi;
    (deviation(c, chord, t), t)
}

/// What becomes of a piece of a discretisation by deflection.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Piece {
    /// Near enough its chord: its end is a point.
    Keep,
    /// Too far: cut it where it is furthest.
    Cut,
    /// Too far, and no point inside it to cut at, or no finite distance: refused.
    Refuse,
}

/// The fate of the piece from `a` to `b` whose furthest point from its chord is `far` away, at `at`, where no point may
/// be more than `most` away.
fn judge(far: f64, most: f64, (a, at, b): (f64, f64, f64)) -> Piece {
    let splittable = a < at && at < b;
    if far > most && splittable {
        Piece::Cut
    } else if far > most || !far.is_finite() {
        Piece::Refuse
    } else {
        Piece::Keep
    }
}

/// Parameters from `lo` to `hi`, ends included and in order, such that no part of `c` between two neighbours is
/// further than `deflection` from the chord joining their points.
///
/// # Errors
///
/// `geom.size-not-positive` for a deflection that is not a finite length above zero, `geom.undefined-parameter` for an
/// end outside the range, and `measure.not-converged` where it would take more than a million pieces.
pub fn by_deflection(c: &dyn Curve, (lo, hi): (f64, f64), deflection: Length) -> Result<Vec<f64>, Report> {
    let (lo, hi) = ends(c, (lo, hi))?;
    let most = positive(deflection)?;
    let start: Vec<f64> = (0..=START).map(|k| lerp(lo, hi, f64::from(k) / f64::from(START))).collect();
    let mut pending: Vec<(f64, f64)> = start.windows(2).rev().filter_map(|w| Some((*w.first()?, *w.get(1)?))).collect();
    let mut out = vec![lo];
    while let Some((a, b)) = pending.pop() {
        let (far, at) = furthest(c, (a, b));
        match judge(far, most, (a, at, b)) {
            Piece::Keep => out.push(b),
            Piece::Cut => pending.extend([(at, b), (a, at)]),
            Piece::Refuse => return Err(not_converged(far, most)),
        }
        if !within_budget(out.len().saturating_add(pending.len())) {
            return Err(not_converged(far, most));
        }
    }
    Ok(out)
}

#[cfg(test)]
mod tests {
    use occam_report::Code;
    use occam_units::{Direction, Point, World};

    use super::*;
    use crate::conformance::placement;
    use crate::{Circle, Ellipse, Hyperbola, Line, Parabola, SplineCurve};

    fn mm(x: f64) -> Length {
        Length::from_millimetres(x)
    }

    fn point(x: f64, y: f64, z: f64) -> Point<World> {
        Point::from_vec3(Vec3::new(x, y, z))
    }

    /// Every analytic family over a part, and two splines: an S with an inflection and a sharp hook.
    fn curves() -> Vec<(Box<dyn Curve>, (f64, f64))> {
        let at = placement();
        let line = Line::new(point(1.0, 2.0, 3.0), Direction::normalised(Vec3::new(1.0, -2.0, 0.5)));
        let s = [point(0.0, 0.0, 0.0), point(4.0, 3.0, 0.0), point(-1.0, 3.0, 1.0), point(3.0, 6.0, 0.0)];
        let hook = [point(0.0, 0.0, 0.0), point(10.0, 0.1, 0.0), point(10.0, -0.1, 0.0), point(0.0, 0.5, 0.2)];
        vec![
            (Box::new(line), (-4.0, 7.5)),
            (Box::new(Circle::placed(&at, mm(10.0)).unwrap()), (0.5, 5.5)),
            (Box::new(Ellipse::placed(&at, mm(10.0), mm(0.5)).unwrap()), (-0.5, 3.5)),
            (Box::new(Hyperbola::placed(&at, mm(2.0), mm(5.0)).unwrap()), (-1.5, 2.0)),
            (Box::new(Parabola::placed(&at, mm(1.5)).unwrap()), (-6.0, 4.0)),
            (Box::new(SplineCurve::bezier(s.to_vec(), None).unwrap()), (0.0, 1.0)),
            (Box::new(SplineCurve::bezier(hook.to_vec(), Some(vec![1.0, 3.0, 3.0, 1.0])).unwrap()), (0.0, 1.0)),
        ]
    }

    fn lengths(c: &dyn Curve, params: &[f64]) -> Vec<f64> {
        params.windows(2).map(|w| length(c, (w[0], w[1]), mm(1e-12)).unwrap().millimetres()).collect()
    }

    #[test]
    fn a_discretisation_is_never_further_from_the_curve_than_asked() {
        for (c, (lo, hi)) in curves() {
            for deflection in [0.05, 1e-3] {
                let params = by_deflection(c.as_ref(), (lo, hi), mm(deflection)).unwrap();
                assert_eq!((params.first(), params.last()), (Some(&lo), Some(&hi)));
                assert!(params.windows(2).all(|w| w[0] < w[1]) && params.len() < 20_000, "{}", params.len());
                for w in params.windows(2) {
                    let chord = (c.point(w[0]).to_vec3(), c.point(w[1]).to_vec3());
                    for i in 1..400 {
                        let t = lerp(w[0], w[1], f64::from(i) / 400.0);
                        let far = deviation(c.as_ref(), chord, t);
                        assert!(far <= deflection * (1.0 + 1e-9), "{:?} at {t}: {far} beyond {deflection}", c.family());
                    }
                }
            }
        }
    }

    #[test]
    fn a_line_needs_no_more_points_than_it_starts_with() {
        let (line, range) = curves().swap_remove(0);
        assert_eq!(by_deflection(line.as_ref(), range, mm(1e-9)).unwrap().len(), 9);
    }

    #[test]
    fn points_by_count_and_by_spacing_are_equally_far_along_the_curve() {
        let at = placement();
        let ellipse = Ellipse::placed(&at, mm(7.0), mm(3.0)).unwrap();
        let tol = 1e-9;
        let total = length(&ellipse, (0.3, 4.0), mm(1e-12)).unwrap().millimetres();
        let params = by_count(&ellipse, (0.3, 4.0), 9, mm(tol)).unwrap();
        assert_eq!(params.len(), 9);
        for piece in lengths(&ellipse, &params) {
            assert!((piece - total / 8.0).abs() <= tol, "{piece} against {}", total / 8.0);
        }
        let spaced = by_spacing(&ellipse, (0.3, 4.0), mm(1.3), mm(tol)).unwrap();
        let pieces = lengths(&ellipse, &spaced);
        assert!(pieces.iter().all(|p| *p <= 1.3 + tol), "{pieces:?}");
        let fewer = f64::from(u32::try_from(pieces.len()).unwrap() - 1) * 1.3;
        assert!(fewer < total, "no fewer points would do: {} pieces for {total}", pieces.len());
        let circle = Circle::placed(&at, mm(10.0)).unwrap();
        let even = by_count(&circle, (0.0, 2.0), 5, mm(tol)).unwrap();
        assert!(even.iter().zip([0.0, 0.5, 1.0, 1.5, 2.0]).all(|(t, want)| (t - want).abs() < 1e-9), "{even:?}");
    }

    #[test]
    fn a_piece_is_kept_cut_or_refused_by_its_furthest_point() {
        assert_eq!(judge(0.5, 1.0, (0.0, 0.5, 1.0)), Piece::Keep, "near enough, though it could be cut");
        assert_eq!(judge(1.0, 1.0, (0.0, 0.5, 1.0)), Piece::Keep, "exactly as far as allowed");
        assert_eq!(judge(2.0, 1.0, (0.0, 0.5, 1.0)), Piece::Cut);
        for at in [0.0, 1.0] {
            assert_eq!(judge(2.0, 1.0, (0.0, at, 1.0)), Piece::Refuse, "too far, with nowhere inside to cut at {at}");
        }
        assert_eq!(judge(f64::NAN, 1.0, (0.0, 0.5, 1.0)), Piece::Refuse);
    }

    #[test]
    fn a_bracket_is_halved_at_its_middle_and_the_budget_holds_a_million_pieces() {
        assert_eq!((middle(0.5, 1.0), middle(2.0, 6.0), middle(-3.0, 1.0)), (0.75, 4.0, -1.0));
        assert!(within_budget(BUDGET) && !within_budget(BUDGET + 1));
    }

    #[test]
    fn two_points_are_the_ends_and_a_spacing_of_the_whole_length_needs_no_more() {
        let (c, (lo, hi)) = curves().swap_remove(5);
        assert_eq!(by_count(c.as_ref(), (lo, hi), 2, mm(1e-9)).unwrap(), [lo, hi]);
        // The length by_spacing measures, to the last bit: one piece of exactly that is enough.
        let total = length(c.as_ref(), (lo, hi), mm(0.25e-9)).unwrap();
        assert_eq!(by_spacing(c.as_ref(), (lo, hi), total, mm(1e-9)).unwrap(), [lo, hi]);
    }

    #[test]
    fn a_spacing_a_hair_either_side_of_a_fifth_of_a_hook_makes_five_pieces_or_six() {
        // The hook's speed varies sharply, so its length is only right to the tolerance asked if it is measured so.
        let (hook, range) = curves().swap_remove(6);
        let total = length(hook.as_ref(), range, mm(1e-13)).unwrap().millimetres();
        for (scale, points) in [(1.0 + 1e-9, 6), (1.0 - 1e-9, 7)] {
            let spaced = by_spacing(hook.as_ref(), range, mm(total / 5.0 * scale), mm(1e-12)).unwrap();
            assert_eq!(spaced.len(), points, "a fifth times {scale}");
        }
    }

    #[test]
    fn a_range_given_backwards_is_cut_as_it_is_forwards() {
        let (c, (lo, hi)) = curves().swap_remove(5);
        let deflected = |range| by_deflection(c.as_ref(), range, mm(1e-3)).unwrap();
        assert_eq!(deflected((hi, lo)), deflected((lo, hi)));
        let counted = |range| by_count(c.as_ref(), range, 5, mm(1e-9)).unwrap();
        assert_eq!(counted((hi, lo)), counted((lo, hi)));
    }

    #[test]
    fn the_furthest_point_from_a_chord_is_found_between_the_samples() {
        for (c, (lo, hi)) in curves().into_iter().skip(1) {
            let (a, b) = (lo, lerp(lo, hi, 0.3));
            let (far, at) = furthest(c.as_ref(), (a, b));
            let chord = (c.point(a).to_vec3(), c.point(b).to_vec3());
            let dense = (0..=100_000_u32)
                .map(|i| deviation(c.as_ref(), chord, lerp(a, b, f64::from(i) / 1e5)))
                .fold(0.0, |most: f64, d| if d > most { d } else { most });
            assert!(far >= dense * (1.0 - 1e-12), "{:?}: {far} against {dense}", c.family());
            assert_eq!(deviation(c.as_ref(), chord, at), far, "{:?}", c.family());
        }
    }

    #[test]
    fn of_points_as_far_the_first_is_taken_and_a_level_stretch_at_its_end_towards_hi() {
        // A line on its own chord, from 0 to 4 along x: every distance is nought to the last bit, as a quarter is exact.
        let along_x = Line::new(point(0.0, 0.0, 0.0), Direction::normalised(Vec3::new(1.0, 0.0, 0.0)));
        assert_eq!(furthest(&along_x, (0.0, 4.0)), (0.0, 0.0), "nothing further than nought: at the start");
        // A line a unit from a chord along it is a unit from it everywhere, to the last bit.
        let above = Line::new(point(0.0, 1.0, 0.0), Direction::normalised(Vec3::new(1.0, 0.0, 0.0)));
        let chord = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(4.0, 0.0, 0.0));
        let (level, end) = golden(&above, chord, (1.5, 2.5));
        assert!(level.to_bits() == 1f64.to_bits() && end > 2.5 - 1e-9, "{level} at {end}");
    }

    #[test]
    fn newtons_step_does_the_work_from_a_guess_by_the_speed() {
        // Along an ellipse, from a guess by its speed at the start; halving the bracket alone would take some thirty.
        let ellipse = Ellipse::placed(&placement(), mm(7.0), mm(3.0)).unwrap();
        let (at, measured, steps) = advance(&ellipse, (0.3, 4.0), 2.5, 1e-10).unwrap();
        let along = length(&ellipse, (0.3, at), mm(1e-12)).unwrap().millimetres();
        assert!((measured - 2.5).abs() <= 1e-10 && (along - 2.5).abs() <= 1e-10, "{at}: {measured} {along}");
        assert!(steps <= 5, "{steps} steps");
        let (_, _, near) = advance(&ellipse, (0.3, 6.0), 0.4, 1e-10).unwrap();
        assert!(near <= 4, "a short step over a long range, which the middle is far from: {near} steps");
    }

    #[test]
    fn a_newton_step_that_would_leave_the_bracket_is_not_taken() {
        // Along (t^3, 0, 0), at rest at its start so that the guess fails and the search starts at the middle, 0.9 long:
        // the root is past the middle, and Newton's step from there lands past the end, so the bracket is halved.
        let origin = point(0.0, 0.0, 0.0);
        let cubic = SplineCurve::bezier(vec![origin, origin, origin, point(1.0, 0.0, 0.0)], None).unwrap();
        let (at, measured, steps) = advance(&cubic, (0.0, 1.0), 0.9, 1e-12).unwrap();
        assert!((measured - 0.9).abs() <= 1e-12 && (at * at * at - 0.9).abs() <= 1e-11, "{at}: {measured}");
        assert!(steps <= 7, "{steps} steps");
    }

    #[test]
    fn points_by_count_are_as_far_along_a_hook_and_a_curve_that_starts_at_rest() {
        // A hook's speed varies sharply, and a curve whose first two poles are one point has none at its start.
        let (hook, range) = curves().swap_remove(6);
        let rest = [point(0.0, 0.0, 0.0), point(0.0, 0.0, 0.0), point(2.0, 1.0, 0.0), point(4.0, 0.0, 0.0)];
        let rest: Box<dyn Curve> = Box::new(SplineCurve::bezier(rest.to_vec(), None).unwrap());
        for (c, (lo, hi)) in [(hook, range), (rest, (0.0, 1.0))] {
            let total = length(c.as_ref(), (lo, hi), mm(1e-12)).unwrap().millimetres();
            let params = by_count(c.as_ref(), (lo, hi), 7, mm(1e-9)).unwrap();
            for piece in lengths(c.as_ref(), &params) {
                assert!((piece - total / 6.0).abs() <= 1e-9, "{:?}: {piece} against {}", c.family(), total / 6.0);
            }
        }
    }

    #[test]
    fn a_discretisation_asked_for_the_impossible_is_refused() {
        let (c, range) = curves().swap_remove(1);
        let code = |r: Result<Vec<f64>, Report>| r.unwrap_err().code;
        assert_eq!(code(by_count(c.as_ref(), range, 1, mm(1e-6))), Code::WrongCount);
        assert_eq!(code(by_deflection(c.as_ref(), range, mm(0.0))), Code::SizeNotPositive);
        assert_eq!(code(by_spacing(c.as_ref(), range, mm(-1.0), mm(1e-6))), Code::SizeNotPositive);
        let arc = Circle::placed(&placement(), mm(1.0)).unwrap().trimmed(0.5, 1.0).unwrap();
        assert_eq!(code(by_deflection(&arc, (0.5, 2.0), mm(1e-3))), Code::UndefinedParameter);
        assert_eq!(from_chord(Vec3::new(3.0, 4.0, 0.0), (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0))), 5.0);
    }
}
