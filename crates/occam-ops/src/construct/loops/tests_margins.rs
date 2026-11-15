//! Source: original
//! A face's loops at their margins: the box of a loop's last piece holds where its chain comes round to the first, a
//! clearance from one loop is from that loop alone, a winding cuts an arc finer than the point is from it, and a witness
//! is clearer than asked, not only as clear.

use core::f64::consts::PI;

use occam_math::sin_cos;

use super::*;
use crate::construct::flat::tests::{circle, seg, square};

#[test]
fn each_piece_s_box_holds_its_own_chain_and_no_more() {
    // The last piece ends just short of where the first begins, where its chain ends.
    let pieces = vec![seg([0.0, 0.0], [2.0, 0.0]), seg([2.0, 0.0], [1.0, 1.0]), seg([1.0, 1.0], [0.0, 1e-8])];
    let boxes: Vec<Bounds> = (0..4).map(|k| chain_bounds(&pieces, k)).collect();
    assert_eq!(boxes, [[0.0, 0.0, 2.0, 0.0], [1.0, 0.0, 2.0, 1.0], [0.0, 0.0, 1.0, 1.0], EMPTY]);
}

#[test]
fn the_clearance_from_one_loop_is_from_its_pieces_alone() {
    let hole: Vec<Flat> = square(4.0, 6.0).iter().rev().map(|piece| piece.reversed()).collect();
    let face = vec![square(0.0, 10.0), hole];
    let loops = Loops::new(&face);
    let q = [1.0, 5.0];
    assert_eq!((loops.clearance(q, Some(0)), loops.clearance(q, Some(1)), loops.clearance(q, None)), (1.0, 3.0, 1.0));
}

#[test]
fn a_point_near_an_arc_is_inside_it_as_its_clearance_allows() {
    // 0.015 inside a circle of radius 1, facing the middle of the first of the chords a cut into sixteen gives, which
    // stray 0.019 from the arc: cut so that a chord strays by a quarter of the clearance at most, the circle holds it.
    let (sin, cos) = sin_cos(PI / 16.0);
    let q = [0.985 * cos, 0.985 * sin];
    let face =
        vec![vec![circle([0.0, 0.0], 1.0)], vec![seg(q, [0.5, 0.0]), seg([0.5, 0.0], [0.5, 0.1]), seg([0.5, 0.1], q)]];
    let loops = Loops::new(&face);
    assert!((loops.clearance(q, Some(0)) - 0.015).abs() < 1e-12);
    assert_eq!(loops.winding(0, &loops.crossing(q), q, 0.014), Some(1));
    assert!(loops.inside(0, 1), "the hole that begins there is inside");
}

#[test]
fn a_witness_is_clearer_than_asked_not_only_as_clear() {
    // The clearest point of a square 4 across is its middle, 2 from every side, a candidate the search tries first.
    let face = vec![square(0.0, 4.0)];
    let loops = Loops::new(&face);
    assert_eq!(loops.witness(0, 1.5), Some(([2.0, 2.0], 2.0)));
    assert_eq!(loops.witness(0, 2.0), None, "nowhere is clearer than 2");
}
