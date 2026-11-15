//! Source: original
//! Which way a group is halved, and which groups a search for the least skips, as the count of boxes it reads shows: a
//! row is halved across u and a column across v wherever they lie, and a group exactly the slack further than the least
//! found so far is read.
#![expect(clippy::arithmetic_side_effects, reason = "small indices and counts in tests")]

use core::cell::Cell;

use super::*;

/// Sixteen unit boxes in a line from `start`, a gap of one between each, along u if `along_u` and along v if not,
/// listed out of order.
fn line(start: P2, along_u: bool) -> Vec<Bounds> {
    (0..16_u32)
        .map(|k| {
            let at = 2.0 * f64::from((k * 7) % 16);
            let [u, v] = if along_u { [start[0] + at, start[1]] } else { [start[0], start[1] + at] };
            [u, v, u + 1.0, v + 1.0]
        })
        .collect()
}

/// The least over `items` of each box's distance from `q` and `lift` more, searched with `slack`, and how many boxes
/// the search read.
fn search(items: &[Bounds], q: P2, slack: f64, lift: f64) -> (f64, usize) {
    let read = Cell::new(0);
    let least = Boxes::new(items.to_vec()).least(q, slack, |k| {
        read.set(read.get() + 1);
        far(items[k], q) + lift
    });
    (least, read.get())
}

#[test]
fn a_row_is_halved_across_u_and_a_column_across_v_wherever_they_lie() {
    // Halved across its length, a line's nearer half is read first and its farther half, 17 away, is skipped once 5 is
    // found. Halved the other way, every centre is level, so the halves are the boxes in the order given, the second of
    // them 5 away, which is not further than 5 by more than the slack: all sixteen would be read.
    let lines = [([0.0, 0.0], true), ([0.0, 100.0], true), ([0.0, 0.0], false), ([100.0, 0.0], false)];
    for (start, along_u) in lines {
        let q = if along_u { [start[0] - 1.0, start[1] + 0.5] } else { [start[0] + 0.5, start[1] - 1.0] };
        assert_eq!(search(&line(start, along_u), q, 0.5, 4.0), (5.0, 8), "{start:?}, along u: {along_u}");
    }
}

#[test]
fn a_group_exactly_the_slack_further_than_the_least_found_is_read() {
    // The column's upper half is 17 from the point below it, and 17 less the slack of 0.5 is 16.5.
    let column = line([0.0, 0.0], false);
    assert_eq!(search(&column, [0.5, -1.0], 0.5, 15.5), (16.5, 16), "16.5 found: no further than that");
    assert_eq!(search(&column, [0.5, -1.0], 0.5, 15.25), (16.25, 8), "16.25 found: further by more than the slack");
}
