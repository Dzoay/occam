//! Source: original
//! Where the pieces of runs in a plane touch or cross within the band, other than where consecutive pieces of a run
//! meet end to end: the check that a wire, and a face's loops, neither touch nor cross. Two pieces can touch only if
//! their boxes, padded by the band and beyond rounding, meet, so only those pairs are tried ([`Boxes`]), in the order a
//! trial of every pair takes them, and the first that touches is the one that trial finds.

use super::boxes::{Bounds, Boxes, padded, slack};
use super::flat::{Flat, P2, closest, meet_elsewhere};

/// Two pieces that touch or cross, each as its run and its place in it, and a place where they do.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Touch {
    /// The first piece's run and place.
    pub first: (usize, usize),
    /// The second's.
    pub second: (usize, usize),
    /// Where.
    pub at: P2,
}

/// The points where pieces `i` and `j > i` of a run of `n` meet end to end: after `i` if `j` follows it, and where the
/// run comes round if it is `closed` and they are its last and first.
fn shared(pieces: &[Flat], i: usize, j: usize, closed: bool) -> Vec<P2> {
    let (Some(x), Some(y)) = (pieces.get(i), pieces.get(j)) else { return Vec::new() };
    let mut found = Vec::new();
    if j == i.saturating_add(1) {
        found.push(x.end());
    }
    if closed && i == 0 && j.saturating_add(1) == pieces.len() {
        found.push(y.end());
    }
    found
}

/// Where pieces `i` and `j > i` of run `pieces`, which comes round if `closed`, touch or cross within `band` other than
/// where they meet end to end.
fn touch_within(pieces: &[Flat], (i, j): (usize, usize), closed: bool, band: f64) -> Option<P2> {
    let (&x, &y) = (pieces.get(i)?, pieces.get(j)?);
    let meets = shared(pieces, i, j, closed);
    if meets.is_empty() {
        Some(closest(x, y)).filter(|&(d, _)| d <= band).map(|(_, p)| p)
    } else {
        meet_elsewhere(x, y, &meets, band)
    }
}

/// The first two pieces of `runs`, each a run of pieces and whether it comes round, that touch or cross within `band`
/// other than where consecutive pieces of one run meet end to end: run by run, piece by piece. Only pieces whose boxes,
/// padded by the band and beyond rounding, meet can touch, so only those are tried, in that order.
pub(crate) fn first_touch(runs: &[(&[Flat], bool)], band: f64) -> Option<Touch> {
    let places: Vec<(usize, usize)> =
        runs.iter().enumerate().flat_map(|(r, &(pieces, _))| (0..pieces.len()).map(move |i| (r, i))).collect();
    let bare: Vec<Bounds> = runs.iter().flat_map(|&(pieces, _)| pieces.iter().map(|p| p.bounds())).collect();
    let pad = band + slack(&bare);
    let boxes = Boxes::new(bare.into_iter().map(|b| padded(b, pad)).collect());
    let mut pairs: Vec<(usize, usize)> = Vec::new();
    for g in 0..boxes.len() {
        let met = boxes.meeting(boxes.get(g)).into_iter().filter(|&h| h != g);
        pairs.extend(met.map(|h| (g.min(h), g.max(h))));
    }
    pairs.sort_unstable();
    pairs.dedup();
    pairs.into_iter().find_map(|(g, h)| {
        let (&first, &second) = (places.get(g)?, places.get(h)?);
        touch_between(runs, first, second, band).map(|at| Touch { first, second, at })
    })
}

/// Where the piece at `first` of `runs`, a run and a place in it, touches or crosses the later piece at `second` within
/// `band`, other than where consecutive pieces of one run meet end to end.
fn touch_between(runs: &[(&[Flat], bool)], first: (usize, usize), second: (usize, usize), band: f64) -> Option<P2> {
    let &(pieces, closed) = runs.get(first.0)?;
    if first.0 == second.0 {
        return touch_within(pieces, (first.1, second.1), closed, band);
    }
    let (&x, &y) = (pieces.get(first.1)?, runs.get(second.0)?.0.get(second.1)?);
    Some(closest(x, y)).filter(|&(d, _)| d <= band).map(|(_, p)| p)
}

#[cfg(test)]
#[expect(clippy::arithmetic_side_effects, reason = "small indices in tests")]
mod tests {
    use core::f64::consts::PI;

    use super::*;
    use crate::construct::flat::tests::{circle, near, polygon, regular, seg, square};

    #[test]
    fn the_first_touch_is_found_within_a_run_and_between_runs() {
        let band = 1e-7;
        let bow = [[0.0, 0.0], [2.0, 2.0], [2.0, 0.0], [0.0, 2.0]];
        let bow: Vec<Flat> = (0..4).map(|k| seg(bow[k], bow[(k + 1) % 4])).collect();
        let touch = first_touch(&[(&bow, true)], band).unwrap();
        assert_eq!((touch.first, touch.second), ((0, 0), (0, 2)));
        assert!(near(touch.at, [1.0, 1.0]));
        let square = square(0.0, 4.0);
        assert_eq!(first_touch(&[(&square, true)], band), None);
        let open: Vec<Flat> = square.iter().take(3).copied().collect();
        assert_eq!(first_touch(&[(&open, false)], band), None, "an open run does not come round");
        let ring = vec![circle([2.0, 2.0], 1.0)];
        assert_eq!(first_touch(&[(&square, true), (&ring, true)], band), None);
        let touching = vec![circle([3.0, 2.0], 1.0)];
        let found = first_touch(&[(&square, true), (&touching, true)], band).unwrap();
        assert_eq!((found.first, found.second), ((0, 1), (1, 0)));
        let two = [seg([0.0, 0.0], [2.0, 0.0]), Flat::Arc { c: [1.0, 0.0], r: 1.0, from: 0.0, sweep: PI }];
        assert_eq!(first_touch(&[(&two, true)], band), None, "two pieces that meet at both ends");
    }

    /// The first touch of `runs`, found by trying every pair of pieces in turn.
    fn first_touch_of_every_pair(runs: &[(&[Flat], bool)], band: f64) -> Option<Touch> {
        for (r, &(pieces, closed)) in runs.iter().enumerate() {
            for i in 0..pieces.len() {
                for j in i + 1..pieces.len() {
                    if let Some(at) = touch_within(pieces, (i, j), closed, band) {
                        return Some(Touch { first: (r, i), second: (r, j), at });
                    }
                }
                for (s, &(others, _)) in runs.iter().enumerate().skip(r + 1) {
                    for (j, &y) in others.iter().enumerate() {
                        if let Some((_, at)) = Some(closest(pieces[i], y)).filter(|&(d, _)| d <= band) {
                            return Some(Touch { first: (r, i), second: (s, j), at });
                        }
                    }
                }
            }
        }
        None
    }

    #[test]
    fn a_touch_within_the_band_far_from_the_origin_is_found() {
        // Far out, rounding is larger than the band: the boxes are padded by both, so two pieces closer than the band
        // still meet.
        let band = 1e-7;
        let (low, high) = (1000.0, 1000.0 + 0.5 * band);
        let under = polygon(&[[1000.0, low], [1010.0, low], [1010.0, low - 5.0]]);
        let over = polygon(&[[1005.0, high], [1004.0, high + 5.0], [1006.0, high + 5.0]]);
        let found = first_touch(&[(&under, true), (&over, true)], band).map(|t| (t.first, t.second));
        assert_eq!(found, Some(((0, 0), (1, 0))));
        assert_eq!(
            first_touch(&[(&under, true), (&over, true)], band),
            first_touch_of_every_pair(&[(&under, true), (&over, true)], band)
        );
    }

    #[test]
    fn the_first_touch_tries_only_pieces_whose_boxes_meet_and_finds_what_trying_every_pair_finds() {
        let band = 1e-7;
        let ring = regular(200, [0.0, 0.0], 10.0, 0.1);
        let mut folded = ring.clone();
        folded[120] = seg(folded[120].start(), [0.0, 0.0]);
        folded[121] = seg([0.0, 0.0], folded[121].end());
        let mut grazed = regular(64, [0.0, 0.0], 10.0, 0.0);
        grazed[40] = seg(grazed[40].start(), grazed[8].at(0.5).0);
        grazed[41] = seg(grazed[8].at(0.5).0, grazed[41].end());
        let holes: Vec<Vec<Flat>> = (0..6)
            .map(|k| {
                let x = f64::from(u32::try_from(k).unwrap()) * 2.5 - 6.0;
                if k == 4 { vec![circle([x, 0.0], 1.5)] } else { regular(12, [x, 0.0], 1.0, 0.0) }
            })
            .collect();
        let mut cases: Vec<Vec<(&[Flat], bool)>> = vec![
            vec![(&ring, true)],
            vec![(&ring, false)],
            vec![(&folded, true)],
            vec![(&grazed, true)],
            vec![(&ring, true), (&holes[0], true), (&holes[1], true), (&holes[2], true)],
        ];
        cases.push(std::iter::once((&ring[..], true)).chain(holes.iter().map(|h| (&h[..], true))).collect());
        for (k, runs) in cases.iter().enumerate() {
            assert_eq!(first_touch(runs, band), first_touch_of_every_pair(runs, band), "case {k}");
        }
        assert_eq!(first_touch(&cases[2], band), None, "a spike in to the middle and out again");
        assert_eq!(first_touch(&cases[3], band).map(|t| (t.first, t.second)), Some(((0, 8), (0, 40))), "a graze");
        let found = first_touch(&cases[5], band).map(|t| (t.first, t.second));
        assert_eq!(found, Some(((4, 0), (5, 0))), "the circle between holes 3 and 5 touches both");
    }
}

#[cfg(test)]
mod tests_open;
