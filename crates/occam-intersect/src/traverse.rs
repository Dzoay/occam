//! Source: houghton-emnett-factor-sabharwal-1985
//! The hybrid's subdivision, divide and conquer as Houghton, Emnett, Factor and Sabharwal intersect parametric
//! surfaces: the parameters are halved, a part that cannot meet the other surface is dropped, and what is left is
//! halved again until each part is small, a leaf. It goes breadth first, a level at a time, so that a level's boxes are
//! made together through the pair's runner and its tests decided together, in the order the level holds them.
//!
//! Where neither surface presents an analytic form, both are subdivided and a pair of parts is dropped where their
//! boxes miss each other by more than the band. Where one does, the other alone is subdivided, and a part is dropped
//! where its box misses the analytic surface's box, or where the analytic surface's implicit function cannot change
//! sign over its box (P8): the function's size at the box's middle, less the box's half diagonal, is above the band.
//! Each test is a length decided against the band through occam-math's predicates and counted; the boxes are rounded
//! outward and the lengths formed in interval arithmetic, so the rounding margin is in both. A part is kept where any
//! test cannot be decided, and the decision is given back. A cell still larger than a leaf whose parts can be halved
//! no further, each [`DEPTH`] times halved or as fine as its numbers go, is unresolved.
//!
//! Each cell is paid for from the budget as it is queued, before it is made, and a level holds at most [`FRONTIER`]
//! cells: a level that would hold more stops the subdivision as spent. So its memory is bounded by the budget and by
//! that, whatever the pair, and a subdivision that never ends runs out of steps at once.

use std::sync::Arc;

use occam_geom::{Surface, UvBox};
use occam_math::{Grain, Interval, Side};
use occam_units::{Bounds, Tolerance, UvPoint, Vec3};

use crate::decide::{Decide, uncertain};
use crate::implicit::{Implicit, magnitude};
use crate::interval3::norm3;
use crate::methods::{Spent, Work};
use crate::on::On;
use crate::pieces::Piece;
use crate::trace::{Uncertain, Which};

pub(crate) use crate::pieces::DEPTH;

/// The most cells the subdivision queues at once, about 60 MB of them. A level that would hold more stops the
/// subdivision as spent: no budget meant for one pair should reach it, so its memory stays bounded whatever the budget.
pub(crate) const FRONTIER: usize = 1 << 18;

/// How a pair is subdivided.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Held {
    /// Both surfaces' parameters, a pair of parts at a time.
    Both,
    /// `side`'s parameters alone, the other surface held by its implicit form.
    One {
        /// The surface subdivided.
        side: Which,
        /// The other's implicit form.
        form: Implicit,
    },
}

/// A part of each surface subdivided, `None` for one held whole.
#[derive(Clone, Copy)]
pub(crate) struct Cell {
    /// The first surface's part.
    pub a: Option<Piece>,
    /// The second's.
    pub b: Option<Piece>,
}

/// What the subdivision left.
#[derive(Default)]
pub(crate) struct Traversal {
    /// The cells whose parts are each at most a leaf across, in the order they were reached.
    pub leaves: Vec<Cell>,
    /// The cells still larger than a leaf that could be halved no further.
    pub unresolved: Vec<Cell>,
    /// The decisions the band could not make.
    pub uncertain: Vec<Uncertain>,
    /// The most cells it queued at once, for the next level.
    pub widest: usize,
}

/// Whether every parameter of `surface`'s domain has finite ends.
fn bounded(surface: &dyn Surface) -> bool {
    let d = surface.domain();
    [d.u.lo, d.u.hi, d.v.lo, d.v.hi].into_iter().all(f64::is_finite)
}

/// The whole of `surface`'s domain.
pub(crate) fn whole(surface: &dyn Surface) -> UvBox {
    let d = surface.domain();
    UvBox::new(UvPoint::new(d.u.lo, d.v.lo), UvPoint::new(d.u.hi, d.v.hi))
}

/// How `a` and `b` are subdivided: the one that presents no analytic form against the other's implicit form, the second
/// held where both do, as the trace holds it; both where neither does. `None` where a surface to be subdivided has a
/// parameter without finite ends, which no box of parts can cover.
pub(crate) fn held(a: &dyn Surface, b: &dyn Surface, tol: Tolerance) -> Option<Held> {
    let form = |surface: &dyn Surface| surface.analytic(tol).map(Implicit::of);
    match (form(a), form(b)) {
        (_, Some(form)) if bounded(a) => Some(Held::One { side: Which::A, form }),
        (Some(form), _) if bounded(b) => Some(Held::One { side: Which::B, form }),
        (None, None) if bounded(a) && bounded(b) => Some(Held::Both),
        _ => None,
    }
}

/// Whether every corner of `bounds` is finite.
fn finite(bounds: &Bounds) -> bool {
    let (low, high) = (bounds.low().to_vec3(), bounds.high().to_vec3());
    [low.x, low.y, low.z, high.x, high.y, high.z].into_iter().all(f64::is_finite)
}

/// How far apart two boxes are along the axis and the way they are furthest apart: below nought where they overlap.
fn apart(p: &Bounds, q: &Bounds) -> Interval {
    let (pl, ph, ql, qh) = (p.low().to_vec3(), p.high().to_vec3(), q.low().to_vec3(), q.high().to_vec3());
    let gap = |low: f64, high: f64| Interval::exact(low) - Interval::exact(high);
    let [first, rest @ ..] =
        [gap(ql.x, ph.x), gap(pl.x, qh.x), gap(ql.y, ph.y), gap(pl.y, qh.y), gap(ql.z, ph.z), gap(pl.z, qh.z)];
    rest.into_iter().fold(first, |best, next| if next.mid() > best.mid() { next } else { best })
}

/// The middle of a box.
fn centre(bounds: &Bounds) -> Vec3 {
    bounds.low().to_vec3() * 0.5 + bounds.high().to_vec3() * 0.5
}

/// How far at least every point of `bounds` is from where `form` is nought: the size of its value at the box's middle
/// less the furthest a corner is from there, as the value changes no faster than the distance (P8).
fn clear(form: &Implicit, bounds: &Bounds) -> Interval {
    let (low, high, mid) = (bounds.low().to_vec3(), bounds.high().to_vec3(), centre(bounds));
    let reach = |lo: f64, m: f64, hi: f64| {
        let (up, down) =
            ((Interval::exact(hi) - Interval::exact(m)).upper(), (Interval::exact(m) - Interval::exact(lo)).upper());
        Interval::exact(if up > down { up } else { down })
    };
    let half = [reach(low.x, mid.x, high.x), reach(low.y, mid.y, high.y), reach(low.z, mid.z, high.z)];
    magnitude(form.enclosed(mid)) - norm3(half)
}

/// The lengths that drop `cell`, each above the band where the cell cannot meet the other surface, and where they were
/// formed.
fn tests(cell: &Cell, held: Held, other: Option<&Bounds>) -> Vec<(Interval, Vec3)> {
    let mut out = Vec::with_capacity(2);
    match (held, &cell.a, &cell.b) {
        (Held::Both, Some(a), Some(b)) if finite(&a.bounds) && finite(&b.bounds) => {
            out.push((apart(&a.bounds, &b.bounds), centre(&a.bounds)));
        }
        (Held::One { form, .. }, Some(piece), None) | (Held::One { form, .. }, None, Some(piece))
            if finite(&piece.bounds) =>
        {
            let at = centre(&piece.bounds);
            if let Some(other) = other.filter(|other| finite(other)) {
                out.push((apart(&piece.bounds, other), at));
            }
            out.push((clear(&form, &piece.bounds), at));
        }
        _ => {}
    }
    out
}

/// The cells of `level` that may meet the other surface, in order. One whose tests the band cannot all decide is kept,
/// and the decision given back.
fn kept(level: Vec<Cell>, held: Held, other: Option<&Bounds>, decide: &mut Decide, out: &mut Traversal) -> Vec<Cell> {
    let tested: Vec<Vec<(Interval, Vec3)>> = level.iter().map(|cell| tests(cell, held, other)).collect();
    let lengths: Vec<Interval> = tested.iter().flatten().map(|(length, _)| *length).collect();
    let mut sides = decide.sides(&lengths).into_iter();
    let mut keep = Vec::with_capacity(level.len());
    for (cell, tests) in level.into_iter().zip(tested) {
        let mine: Vec<Option<Side>> = sides.by_ref().take(tests.len()).collect();
        if mine.contains(&Some(Side::Above)) {
            continue;
        }
        let open = tests.iter().zip(&mine).find(|(_, side)| side.is_none());
        out.uncertain.extend(open.map(|((length, at), _)| uncertain(*length, *at)));
        keep.push(cell);
    }
    keep
}

/// Whether every part of `cell` is at most `leaf` across.
fn small(cell: &Cell, leaf: f64) -> bool {
    [&cell.a, &cell.b].into_iter().flatten().all(|piece| piece.size <= leaf)
}

/// A part to make: `which` surface's `patch`, a half of `parent`, beside the cell's other part.
struct Job {
    which: Which,
    parent: Piece,
    patch: UvBox,
    beside: Option<Piece>,
}

/// The halves of `cell`: of its larger part where both are subdivided and it can be halved, else of the other.
fn split(cell: &Cell) -> Option<[Job; 2]> {
    let halves = |which: Which, piece: &Option<Piece>| piece.as_ref().and_then(|p| Some((which, *p, p.halves()?)));
    let (first, second) = (halves(Which::A, &cell.a), halves(Which::B, &cell.b));
    let second_larger = matches!((&first, &second), (Some((_, a, _)), Some((_, b, _))) if b.size > a.size);
    let larger = if second_larger || first.is_none() { second } else { first };
    let (which, parent, [lo, hi]) = larger?;
    let beside = match which {
        Which::A => cell.b,
        Which::B => cell.a,
    };
    let job = |patch| Job { which, parent, patch, beside };
    Some([job(lo), job(hi)])
}

/// The cells `jobs` make, their parts' boxes made through `on`. A level of a pair subdivided on both sides halves one
/// of them, so the levels are at most twice [`DEPTH`].
fn made(on: On<'_>, jobs: Vec<Job>) -> Vec<Cell> {
    on.map(Arc::from(jobs), Grain::Auto, |a, b, job| {
        let piece = Some(job.parent.part(if job.which == Which::A { a } else { b }, job.patch));
        match job.which {
            Which::A => Cell { a: piece, b: job.beside },
            Which::B => Cell { a: job.beside, b: piece },
        }
    })
}

/// Queues `halves` for the next level, paid for from `work` as they are queued, so that no level holds a cell its budget
/// has not paid for, nor more than `most`.
///
/// # Errors
///
/// [`Spent`] when `work` cannot pay for them, or the level would hold more than `most`, which spends what is left.
fn queue(jobs: &mut Vec<Job>, halves: [Job; 2], work: &mut Work, most: usize) -> Result<(), Spent> {
    if jobs.len().saturating_add(halves.len()) > most {
        return Err(work.exhaust());
    }
    work.spend(2)?;
    jobs.extend(halves);
    Ok(())
}

/// The subdivision of `on`'s pair, held as `held`, to leaves at most `leaf` across, into `out`. Each cell spends a step
/// of `work` as it is queued, and a level holds at most `most` cells.
///
/// # Errors
///
/// [`Spent`] when `work` cannot pay for a cell, or a level would hold more than `most`. `out` keeps what was found.
pub(crate) fn traverse(
    on: On<'_>,
    (held, leaf, most): (Held, f64, usize),
    work: &mut Work,
    decide: &mut Decide,
    out: &mut Traversal,
) -> Result<(), Spent> {
    let (a, b) = on.surfaces();
    let root = |surface: &dyn Surface| Some(Piece::whole(surface, whole(surface)));
    let (mut level, other) = match held {
        Held::Both => (vec![Cell { a: root(a), b: root(b) }], None),
        Held::One { side: Which::A, .. } => {
            (vec![Cell { a: root(a), b: None }], bounded(b).then(|| b.bounds(whole(b))))
        }
        Held::One { side: Which::B, .. } => {
            (vec![Cell { a: None, b: root(b) }], bounded(a).then(|| a.bounds(whole(a))))
        }
    };
    work.spend(1)?;
    for _ in 0..=DEPTH.saturating_mul(2) {
        let mut jobs = Vec::new();
        for cell in kept(level, held, other.as_ref(), decide, out) {
            match (small(&cell, leaf), split(&cell)) {
                (true, _) => out.leaves.push(cell),
                (false, Some(halves)) => queue(&mut jobs, halves, work, most)?,
                (false, _) => out.unresolved.push(cell),
            }
        }
        out.widest = out.widest.max(jobs.len());
        level = made(on, jobs);
        if level.is_empty() {
            break;
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    use std::f64::consts::TAU;

    use occam_geom::{Circle, Cylinder, Extrusion};
    use occam_units::{Direction, Length, Point};

    use super::*;
    use crate::methods::Budget;
    use crate::trace_tests::{level, mm, moved, nurbs_cylinder, tol};

    fn cube(low: [f64; 3], high: [f64; 3]) -> Bounds {
        let corner = |[x, y, z]: [f64; 3]| Point::from_vec3(Vec3::new(x, y, z));
        Bounds::of([corner(low), corner(high)]).unwrap()
    }

    #[test]
    fn a_box_is_finite_or_not_and_its_middle_is_halfway() {
        assert!(finite(&cube([0.0; 3], [1.0; 3])));
        let everything =
            Bounds::around(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)), Length::from_millimetres(f64::INFINITY));
        assert!(!finite(&everything));
        assert_eq!(centre(&cube([1.0, 2.0, -4.0], [3.0, 6.0, 0.0])), Vec3::new(2.0, 4.0, -2.0));
    }

    #[test]
    fn two_boxes_are_as_far_apart_as_along_the_axis_they_are_furthest_apart_on() {
        // The second box is 2 below the first: the gap along z, the last of the six, is the largest.
        let gap = apart(&cube([0.0; 3], [1.0; 3]), &cube([0.0, 0.0, -3.0], [1.0, 1.0, -2.0]));
        assert_eq!(gap.mid(), 2.0);
        let overlap = apart(&cube([0.0; 3], [2.0; 3]), &cube([1.0; 3], [3.0; 3]));
        assert_eq!(overlap.mid(), -1.0);
    }

    #[test]
    fn a_surface_without_ends_and_no_analytic_form_cannot_be_subdivided() {
        let circle = Circle::placed(&moved([0.0; 3]), mm(1.0)).unwrap();
        let tube = Extrusion::new(circle, Direction::normalised(Vec3::new(0.0, 0.0, 1.0))).unwrap();
        let spline = nurbs_cylinder((0.0, TAU));
        assert!(held(&tube, &spline, tol()).is_none() && held(&spline, &tube, tol()).is_none());
        assert!(matches!(held(&spline, &nurbs_cylinder((0.0, 3.0)), tol()), Some(Held::Both)));
    }

    /// A part over the unit square whose box is `bounds`, `size` across.
    fn part(bounds: Bounds, size: f64) -> Piece {
        let patch = UvBox::new(UvPoint::new(0.0, 0.0), UvPoint::new(1.0, 1.0));
        Piece { patch, bounds, size, wide_u: true, depth: 0 }
    }

    fn everything() -> Bounds {
        Bounds::around(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)), Length::from_millimetres(f64::INFINITY))
    }

    #[test]
    fn a_box_is_as_clear_of_a_plane_as_its_middle_less_its_half_diagonal() {
        let plane = Implicit::of(level(0.0).analytic(tol()).unwrap());
        let gap = clear(&plane, &cube([-1.0, -2.0, 3.0], [1.0, 2.0, 5.0]));
        // Each reach is the upper end of its difference's enclosure, so the half diagonal is a rounding or two long.
        assert!((gap.mid() - (4.0 - 6.0_f64.sqrt())).abs() < 1e-14, "{gap:?}");
        // A box one double wide, whose middle rounds to its low end: its half diagonal is its whole width.
        let width = 1.0_f64.next_up() - 1.0;
        let thin = clear(&plane, &cube([1.0, 0.0, 0.0], [1.0_f64.next_up(), 0.0, 0.0]));
        assert!((thin.mid() + width).abs() < 1e-6 * width, "{thin:?}");
    }

    #[test]
    fn a_part_whose_box_is_not_finite_is_not_tested() {
        let (held, open) = (part(cube([0.0; 3], [1.0; 3]), 1.0), part(everything(), 1.0));
        let one = Held::One { side: Which::A, form: Implicit::of(level(0.0).analytic(tol()).unwrap()) };
        let both = |a: Piece, b: Piece| tests(&Cell { a: Some(a), b: Some(b) }, Held::Both, None).len();
        assert_eq!((both(held, held), both(held, open), both(open, held)), (1, 0, 0));
        let alone = |a: Piece, other: Option<&Bounds>| tests(&Cell { a: Some(a), b: None }, one, other).len();
        let (near, far) = (cube([0.0; 3], [2.0; 3]), everything());
        assert_eq!(
            (alone(held, None), alone(held, Some(&near)), alone(held, Some(&far)), alone(open, Some(&near))),
            (1, 2, 1, 0)
        );
    }

    #[test]
    fn a_cell_is_a_leaf_when_every_part_is_at_most_a_leaf_across() {
        let box_ = cube([0.0; 3], [1.0; 3]);
        let (small_part, large_part) = (part(box_, 0.5), part(box_, 1.5));
        assert!(small(&Cell { a: Some(small_part), b: Some(part(box_, 1.0)) }, 1.0));
        assert!(!small(&Cell { a: Some(small_part), b: Some(large_part) }, 1.0));
        assert!(
            small(&Cell { a: None, b: Some(small_part) }, 1.0) && !small(&Cell { a: Some(large_part), b: None }, 1.0)
        );
    }

    #[test]
    fn a_subdivision_without_end_stops_spent_holding_no_more_cells_than_it_paid_for_or_may_hold() {
        // A rational cylinder on its analytic twin with leaves of nothing: every part meets the twin and none is ever
        // small, so every cell is halved and each level holds twice the cells of the one before.
        let (twin, rational) = (Cylinder::placed(&moved([0.0; 3]), mm(2.0)).unwrap(), nurbs_cylinder((0.0, TAU)));
        let held = held(&twin, &rational, tol()).unwrap();
        let widest = |steps: u32, most: usize| {
            let (mut decide, mut out) = (Decide::new(1e-7), Traversal::default());
            let got = traverse(
                On::Here(&twin, &rational),
                (held, 0.0, most),
                &mut Work::of(Budget { steps }),
                &mut decide,
                &mut out,
            );
            assert!(got.is_err(), "it ends spent");
            out.widest
        };
        let paid = widest(100, FRONTIER);
        assert!(paid > 16 && paid <= 100, "no more than the budget pays for: {paid}");
        let held = widest(100_000, 32);
        assert!(held > 16 && held <= 32, "no more than it may hold: {held}");
    }

    #[test]
    fn a_halved_part_is_boxed_on_its_own_surface() {
        let (a, b) = (level(0.0), level(5.0));
        let whole = UvBox::new(UvPoint::new(0.0, 0.0), UvPoint::new(2.0, 1.0));
        for (cell, which) in [
            (Cell { a: Some(Piece::whole(&a, whole)), b: None }, Which::A),
            (Cell { a: None, b: Some(Piece::whole(&b, whole)) }, Which::B),
        ] {
            let halves = made(On::Here(&a, &b), split(&cell).unwrap().into());
            let surface: &dyn Surface = if which == Which::A { &a } else { &b };
            for half in halves {
                let piece = if which == Which::A { half.a } else { half.b }.unwrap();
                assert_eq!(piece.bounds, surface.bounds(piece.patch), "{which:?}");
            }
        }
    }
}
