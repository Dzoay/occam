//! Source: original
//! A face's loops in its plane's parameters, searched through the boxes of their pieces ([`Boxes`]): how far a point is
//! from them, how many times each winds about it, a witness for each loop, and whether one loop lies inside another.
//! A winding number counts only the segments that cross the line through its point along `u`, so it reads only the
//! pieces whose boxes reach that line; a distance reads only the pieces whose boxes are nearer than the nearest found.
//! Each answer is the one a reading of every piece gives.

use std::collections::BTreeSet;

use occam_units::UvPoint;

use super::boxes::{Bounds, Boxes, EMPTY, padded, slack, union, with};
use super::flat::{Flat, P2, add, chain_of, scale, sub};

/// The box that holds what piece `k` of the closed loop `pieces` gives its chain: the piece, where the piece before it
/// ends, and for the last, where the first begins.
fn chain_bounds(pieces: &[Flat], k: usize) -> Bounds {
    let Some(piece) = pieces.get(k) else { return EMPTY };
    let before = k.checked_sub(1).and_then(|j| pieces.get(j)).map(|p| p.sampled_ends().1);
    let closing = (k.saturating_add(1) == pieces.len()).then(|| pieces.first().map(|p| p.sampled_ends().0)).flatten();
    before.into_iter().chain(closing).fold(piece.bounds(), with)
}

/// A face's loops in its plane, the outer first, with every piece's box nested ([`Boxes`]) so that how far a point is
/// from them, and how many times each winds about it, read only the pieces near it.
pub(crate) struct Loops<'a> {
    face: &'a [Vec<Flat>],
    /// Each piece's loop, and its place there, in the order of `boxes`.
    places: Vec<(usize, usize)>,
    /// The box of what each piece gives its loop's chain, padded by `slack`.
    boxes: Boxes,
    /// Each loop's box, padded by `slack`.
    whole: Boxes,
    /// How far each box is padded beyond rounding.
    slack: f64,
}

impl<'a> Loops<'a> {
    /// The loops of `face`, the outer first.
    pub(crate) fn new(face: &'a [Vec<Flat>]) -> Self {
        let places: Vec<(usize, usize)> =
            face.iter().enumerate().flat_map(|(l, pieces)| (0..pieces.len()).map(move |k| (l, k))).collect();
        let bare: Vec<Bounds> =
            places.iter().map(|&(l, k)| face.get(l).map_or(EMPTY, |pieces| chain_bounds(pieces, k))).collect();
        let slack = slack(&bare);
        let mut loops = vec![EMPTY; face.len()];
        for (&(l, _), &b) in places.iter().zip(&bare) {
            if let Some(whole) = loops.get_mut(l) {
                *whole = union(*whole, padded(b, slack));
            }
        }
        let boxes = Boxes::new(bare.into_iter().map(|b| padded(b, slack)).collect());
        Self { face, places, boxes, whole: Boxes::new(loops), slack }
    }

    /// The piece at `k` in the order of the boxes.
    fn piece(&self, k: usize) -> Option<Flat> {
        let &(l, i) = self.places.get(k)?;
        self.face.get(l)?.get(i).copied()
    }

    /// How far `q` is from every piece of loop `only`, or of every loop for `None`.
    fn clearance(&self, q: P2, only: Option<usize>) -> f64 {
        let ours = |k: usize| only.is_none_or(|l| self.places.get(k).is_some_and(|&(own, _)| own == l));
        let distance = |k: usize| self.piece(k).filter(|_| ours(k)).map_or(f64::INFINITY, |piece| piece.distance(q));
        self.boxes.least(q, self.slack, distance)
    }

    /// The pieces whose segments may cross the line through `q` along `u`, in their loops' order.
    fn crossing(&self, q: P2) -> Vec<usize> {
        let mut found = self.boxes.meeting([f64::NEG_INFINITY, q[1], f64::INFINITY, q[1]]);
        found.sort_unstable();
        found
    }

    /// The winding number about `q` of loop `l`, further than `clear` from it, read from those of `crossing` that are
    /// its own: the segments of the others cannot cross the line through `q`, which is all the count reads.
    fn winding(&self, l: usize, crossing: &[usize], q: P2, clear: f64) -> Option<i32> {
        let pieces = self.face.get(l).map_or(&[][..], Vec::as_slice);
        let own = crossing.iter().filter_map(|&k| self.places.get(k)).filter(|&&(of, _)| of == l);
        let segments: Vec<[UvPoint; 2]> = own.flat_map(|&(_, i)| chain_of(pieces, i, clear / 4.0)).collect();
        occam_topo::winding_number(&segments, UvPoint::new(q[0], q[1]))
    }

    /// Whether `q`, further than `clear` from every loop, is a witness for loop `k`: strictly inside it, and, for the
    /// outer loop, outside every hole.
    fn witnesses(&self, k: usize, q: P2, clear: f64) -> bool {
        if k >= self.face.len() {
            return false;
        }
        let crossing = self.crossing(q);
        let holes: BTreeSet<usize> =
            crossing.iter().filter_map(|&p| self.places.get(p)).map(|&(l, _)| l).filter(|&l| l > 0).collect();
        let inside = self.winding(k, &crossing, q, clear).is_some_and(|w| w != 0);
        inside && (k > 0 || holes.into_iter().all(|hole| self.winding(hole, &crossing, q, clear) == Some(0)))
    }

    /// A witness for loop `k` (the outer loop first): a point strictly inside it and, for the outer loop, outside every
    /// hole, further than `clear` from every loop. The clearest of the points a quarter, a half and three quarters of the
    /// way along each piece, moved off it either way by a half, an eighth and a thirty-second of its length, the first of
    /// them that is as clear: each point's clearance is found first, and the clearest are tried in turn.
    pub(crate) fn witness(&self, k: usize, clear: f64) -> Option<(P2, f64)> {
        let mut found: Vec<(P2, f64)> = Vec::new();
        for piece in self.face.get(k).into_iter().flatten() {
            for f in [0.5, 0.25, 0.75] {
                let (p, n) = piece.at(f);
                for d in [0.5, 0.125, 0.031_25].map(|share| share * piece.length()) {
                    let points = [add(p, scale(n, d)), sub(p, scale(n, d))];
                    found.extend(
                        points.into_iter().map(|q| (q, self.clearance(q, None))).filter(|&(_, room)| room > clear),
                    );
                }
            }
        }
        found.sort_by(|a, b| b.1.total_cmp(&a.1));
        found.into_iter().find(|&(q, room)| self.witnesses(k, q, room))
    }

    /// Whether loop `inner`, which neither touches nor crosses loop `outer`, lies inside it: whether its start does.
    pub(crate) fn inside(&self, outer: usize, inner: usize) -> bool {
        let Some(q) = self.face.get(inner).and_then(|pieces| pieces.first()).map(|piece| piece.start()) else {
            return false;
        };
        let room = self.clearance(q, Some(outer));
        self.winding(outer, &self.crossing(q), q, room).is_some_and(|w| w != 0)
    }

    /// The first hole other than `k` that hole `k` lies inside: only a hole whose box holds `k`'s start can.
    pub(crate) fn nested(&self, k: usize) -> Option<usize> {
        let q = self.face.get(k)?.first()?.start();
        let mut holding = self.whole.meeting([q[0], q[1], q[0], q[1]]);
        holding.sort_unstable();
        holding.into_iter().filter(|&j| j > 0 && j != k).find(|&j| self.inside(j, k))
    }
}

#[cfg(test)]
#[expect(clippy::arithmetic_side_effects, reason = "small indices in tests")]
mod tests {
    use core::f64::consts::PI;

    use super::*;
    use crate::construct::boxes::meet;
    use crate::construct::flat::least;
    use crate::construct::flat::tests::{circle, polygon, regular, seg, square, winding};

    /// A witness for loop `k` of `face`.
    fn witness(face: &[Vec<Flat>], k: usize, clear: f64) -> Option<(P2, f64)> {
        Loops::new(face).witness(k, clear)
    }

    /// Whether the loop `inner` lies inside the loop `outer`.
    fn inside(outer: &[Flat], inner: &[Flat]) -> bool {
        Loops::new(&[outer.to_vec(), inner.to_vec()]).inside(0, 1)
    }

    /// How far `q` is from every piece of `face`, reading each.
    fn clearance_reading_all(face: &[Vec<Flat>], q: P2) -> f64 {
        face.iter().flatten().fold(f64::INFINITY, |so_far, piece| least(so_far, piece.distance(q)))
    }

    /// Whether `q` is a witness for loop `k` of `face`, winding every loop's whole chain about it.
    fn witnesses_reading_all(face: &[Vec<Flat>], k: usize, q: P2, clear: f64) -> bool {
        let inside = |pieces: &[Flat]| winding(pieces, q, clear).is_some_and(|w| w != 0);
        let Some(own) = face.get(k) else { return false };
        inside(own) && (k > 0 || face.iter().skip(1).all(|hole| winding(hole, q, clear) == Some(0)))
    }

    /// A witness for loop `k` of `face`, trying each point in turn against every piece.
    fn witness_reading_all(face: &[Vec<Flat>], k: usize, clear: f64) -> Option<(P2, f64)> {
        let mut best: Option<(P2, f64)> = None;
        for piece in face.get(k).into_iter().flatten() {
            for f in [0.5, 0.25, 0.75] {
                let (p, n) = piece.at(f);
                for d in [0.5, 0.125, 0.031_25].map(|share| share * piece.length()) {
                    for q in [add(p, scale(n, d)), sub(p, scale(n, d))] {
                        let room = clearance_reading_all(face, q);
                        let better = best.is_none_or(|(_, so_far)| room > so_far);
                        if room > clear && better && witnesses_reading_all(face, k, q, room) {
                            best = Some((q, room));
                        }
                    }
                }
            }
        }
        best
    }

    /// Whether `inner` lies inside `outer`, winding the whole of `outer` about its start.
    fn inside_reading_all(outer: &[Flat], inner: &[Flat]) -> bool {
        let Some(q) = inner.first().map(|piece| piece.start()) else { return false };
        let room = outer.iter().fold(f64::INFINITY, |so_far, piece| least(so_far, piece.distance(q)));
        winding(outer, q, room).is_some_and(|w| w != 0)
    }

    /// The bits of a witness, so that two compare equal only if they are the same numbers.
    fn bits(found: Option<(P2, f64)>) -> Option<[u64; 3]> {
        found.map(|(q, room)| [q[0].to_bits(), q[1].to_bits(), room.to_bits()])
    }

    /// A star of `n` points about the origin, its corners at radii 10 and 4 by turns.
    fn star(n: usize) -> Vec<Flat> {
        let corners: Vec<P2> = regular(2 * n, [0.0, 0.0], 10.0, 0.0)
            .iter()
            .enumerate()
            .map(|(k, piece)| if k % 2 == 0 { piece.start() } else { scale(piece.start(), 0.4) })
            .collect();
        polygon(&corners)
    }

    /// A square 24 across with a grid of holes four by four, polygons and circles by turns, each turning clockwise.
    fn perforated() -> Vec<Vec<Flat>> {
        let mut face = vec![square(0.0, 24.0)];
        for k in 0_u32..16 {
            let c = [3.0 + 6.0 * f64::from(k % 4), 3.0 + 6.0 * f64::from(k / 4)];
            let hole = if k % 2 == 0 { regular(9, c, 2.0, 0.3) } else { vec![circle(c, 2.0)] };
            face.push(hole.iter().rev().map(|piece| piece.reversed()).collect());
        }
        face
    }

    #[test]
    fn a_piece_s_chain_lies_in_its_box() {
        let pieces = vec![seg([0.0, 0.0], [2.0, 0.0]), Flat::Arc { c: [1.0, 0.0], r: 1.0, from: 0.0, sweep: PI }];
        for k in 0..2 {
            let b = chain_bounds(&pieces, k);
            let points = chain_of(&pieces, k, 1e-2);
            assert!(points.iter().flatten().all(|p| meet(b, [p.u(), p.v(), p.u(), p.v()])), "{k}");
        }
        assert_eq!(chain_bounds(&pieces, 2), EMPTY);
    }

    #[test]
    fn a_witness_is_strictly_inside_its_loop_outside_every_hole_and_clear_of_them_all() {
        let ell = polygon(&[[0.0, 0.0], [4.0, 0.0], [4.0, 1.0], [1.0, 1.0], [1.0, 4.0], [0.0, 4.0]]);
        let (q, room) = witness(std::slice::from_ref(&ell), 0, 1e-6).unwrap();
        assert!(winding(&ell, q, room) == Some(1) && room >= 0.25, "{q:?} {room}");
        let hole: Vec<Flat> = vec![circle([2.0, 2.0], 1.0).reversed()];
        let face = vec![square(0.0, 4.0), hole.clone()];
        let (outer, room) = witness(&face, 0, 1e-6).unwrap();
        assert_eq!((winding(&face[0], outer, room), winding(&hole, outer, room)), (Some(1), Some(0)));
        let (inner, room) = witness(&face, 1, 1e-6).unwrap();
        assert_eq!(winding(&hole, inner, room), Some(-1));
        assert_eq!(witness(&face, 0, 10.0), None, "nothing is that clear");
        assert_eq!(witness(&face, 2, 1e-6), None, "no such loop");
        assert!(inside(&face[0], &hole) && !inside(&hole, &face[0]));
        assert!(!inside(&square(10.0, 11.0), &hole));
        assert!(!inside(&square(0.0, 4.0), &[]) && !inside(&[], &hole), "no loop, nothing inside");
    }

    #[test]
    fn each_witness_is_the_one_trying_every_point_against_every_piece_finds() {
        let ell = polygon(&[[0.0, 0.0], [4.0, 0.0], [4.0, 1.0], [1.0, 1.0], [1.0, 4.0], [0.0, 4.0]]);
        let disc = vec![square(0.0, 4.0), vec![circle([2.0, 2.0], 1.0).reversed()]];
        let faces = [vec![regular(300, [1.0, -2.0], 10.0, 0.2)], vec![ell], disc, vec![star(40)], perforated()];
        for (n, face) in faces.iter().enumerate() {
            let loops = Loops::new(face);
            for k in 0..=face.len() {
                for clear in [1e-6, 0.1] {
                    let found = loops.witness(k, clear);
                    assert_eq!(bits(found), bits(witness_reading_all(face, k, clear)), "face {n}, loop {k}, {clear}");
                    assert_eq!(found.is_some(), k < face.len(), "face {n}, loop {k}");
                }
            }
        }
    }

    #[test]
    fn a_hole_lies_inside_another_as_winding_the_whole_of_it_finds() {
        let mut face = perforated();
        face.push(regular(5, [9.0, 9.0], 0.5, 0.0));
        face.push(regular(7, [30.0, 3.0], 1.0, 0.0));
        face.push(Vec::new());
        let loops = Loops::new(&face);
        for k in 1..face.len() {
            assert_eq!(loops.inside(0, k), inside_reading_all(&face[0], &face[k]), "{k}");
            let first = (1..face.len()).find(|&j| j != k && inside_reading_all(&face[j], &face[k]));
            assert_eq!(loops.nested(k), first, "{k}");
        }
        assert_eq!((loops.inside(0, 17), loops.nested(17)), (true, Some(6)), "the small hole in the hole at (9, 9)");
        assert_eq!((loops.inside(0, 18), loops.nested(18)), (false, None), "outside the square");
        assert_eq!((loops.inside(0, 19), loops.nested(19)), (false, None), "an empty loop");
        assert!(!loops.witnesses(face.len(), [1.0, 1.0], 0.1), "no such loop");
    }
}

#[cfg(test)]
mod tests_margins;
