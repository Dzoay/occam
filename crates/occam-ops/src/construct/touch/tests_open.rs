//! Source: original
//! An open run meets itself end to end only where consecutive pieces do: its last piece's end, where no piece begins,
//! touches the piece it comes to.

use super::*;
use crate::construct::flat::tests::seg;

#[test]
fn the_end_of_an_open_run_touches_the_piece_it_comes_to() {
    // Round three sides of a square and back across its middle, to the second side.
    let corners = [[0.0, 0.0], [10.0, 0.0], [10.0, 10.0], [0.0, 10.0], [0.0, 5.0], [10.0, 5.0]];
    let open: Vec<Flat> = corners.windows(2).map(|pair| seg(pair[0], pair[1])).collect();
    let found = first_touch(&[(&open, false)], 1e-7).map(|t| (t.first, t.second, t.at));
    assert_eq!(found, Some(((0, 1), (0, 4), [10.0, 5.0])));
}
