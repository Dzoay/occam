//! Source: original
//! A grid over a rectangle of the parameters, each cell listing the boxes that reach it, so that finding what holds a
//! parameter reads one cell rather than every box. A value's cell is found among the cuts by bisection, the same way for
//! a box's corners as for a point, so a box holding a point is always listed in the point's cell.

use occam_units::UvPoint;

use crate::domain::UvBox;

/// The most cells along each side.
const MOST: usize = 1024;

/// A grid of cells over a rectangle of the parameters, each listing, in order, the boxes that reach it.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Cells {
    /// Where the cells meet in `u`, lowest first.
    cuts_u: Vec<f64>,
    /// Where they meet in `v`.
    cuts_v: Vec<f64>,
    /// The indices of the boxes reaching each cell, a row of constant `v` at a time.
    lists: Vec<Vec<u32>>,
}

/// The `n - 1` places that cut `lo` to `hi` into `n` equal parts.
fn cuts(lo: f64, hi: f64, n: usize) -> Vec<f64> {
    let parts = f64::from(u32::try_from(n).unwrap_or(1));
    (1..n).map(|k| lo + (hi - lo) * (f64::from(u32::try_from(k).unwrap_or(0)) / parts)).collect()
}

impl Cells {
    /// The grid over `window` for `boxes`: about one cell for each box, square in count, and at most [`MOST`] a side.
    pub(crate) fn new(window: UvBox, boxes: &[UvBox]) -> Self {
        let n = boxes.len().isqrt().clamp(1, MOST);
        let (lo, hi) = (window.lo, window.hi);
        let lists = vec![Vec::new(); n.saturating_mul(n)];
        let mut cells = Self { cuts_u: cuts(lo.u(), hi.u(), n), cuts_v: cuts(lo.v(), hi.v(), n), lists };
        for (index, b) in (0_u32..).zip(boxes) {
            let ((u0, v0), (u1, v1)) = (cells.cell(b.lo), cells.cell(b.hi));
            for row in v0..=v1 {
                for column in u0..=u1 {
                    if let Some(list) = cells.lists.get_mut(row.saturating_mul(n).saturating_add(column)) {
                        list.push(index);
                    }
                }
            }
        }
        cells
    }

    /// The column and row of the cell holding `uv`; a value beyond the grid is in its last cell that way.
    fn cell(&self, uv: UvPoint) -> (usize, usize) {
        (self.cuts_u.partition_point(|c| *c <= uv.u()), self.cuts_v.partition_point(|c| *c <= uv.v()))
    }

    /// The boxes listed in every cell that `b` reaches, cell by cell, so a box may come more than once.
    pub(crate) fn reaching(&self, b: UvBox) -> impl Iterator<Item = u32> + '_ {
        let (n, (u0, v0), (u1, v1)) = (self.cuts_u.len().saturating_add(1), self.cell(b.lo), self.cell(b.hi));
        (v0..=v1)
            .flat_map(move |row| (u0..=u1).map(move |column| row.saturating_mul(n).saturating_add(column)))
            .filter_map(|cell| self.lists.get(cell))
            .flatten()
            .copied()
    }

    /// The boxes that reach the cell holding `uv`, in order.
    pub(crate) fn near(&self, uv: UvPoint) -> &[u32] {
        let (column, row) = self.cell(uv);
        let n = self.cuts_u.len().saturating_add(1);
        self.lists.get(row.saturating_mul(n).saturating_add(column)).map_or(&[], Vec::as_slice)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn square(u0: f64, v0: f64, u1: f64, v1: f64) -> UvBox {
        UvBox::new(UvPoint::new(u0, v0), UvPoint::new(u1, v1))
    }

    #[test]
    fn the_cuts_divide_a_range_into_equal_parts_wherever_it_lies() {
        assert_eq!(cuts(-3.0, -1.0, 4), [-2.5, -2.0, -1.5]);
        assert_eq!(cuts(2.0, 5.0, 3), [3.0, 4.0]);
        assert_eq!(cuts(0.0, 1.0, 1), [0.0; 0]);
    }

    #[test]
    fn a_box_is_listed_in_every_cell_it_reaches_and_no_other() {
        let window = square(0.0, 0.0, 4.0, 2.0);
        let boxes = [
            square(0.0, 0.0, 1.0, 0.5),
            square(1.5, 0.2, 3.9, 1.9),
            square(3.0, 1.5, 4.0, 2.0),
            square(0.0, 1.0, 0.5, 2.0),
        ];
        let cells = Cells::new(window, &boxes);
        assert_eq!(
            (cells.cuts_u.clone(), cells.cuts_v.clone()),
            (vec![2.0], vec![1.0]),
            "two cells a side for four boxes"
        );
        assert_eq!(cells.near(UvPoint::new(0.5, 0.25)), [0, 1]);
        assert_eq!(cells.near(UvPoint::new(3.5, 0.5)), [1]);
        assert_eq!(cells.near(UvPoint::new(3.5, 1.5)), [1, 2]);
        assert_eq!(cells.near(UvPoint::new(0.2, 1.5)), [1, 3], "box 1 reaches the column from 1.5");
        assert_eq!(cells.near(UvPoint::new(9.0, -9.0)), [1], "beyond the grid, its last cell that way");
    }

    #[test]
    fn the_boxes_reaching_a_box_are_those_listed_in_its_cells() {
        let window = square(0.0, 0.0, 4.0, 2.0);
        let boxes = [
            square(0.0, 0.0, 1.0, 0.5),
            square(1.5, 0.2, 3.9, 1.9),
            square(3.0, 1.5, 4.0, 2.0),
            square(0.0, 1.0, 0.5, 2.0),
        ];
        let cells = Cells::new(window, &boxes);
        let reached = |b: UvBox| {
            let mut found: Vec<u32> = cells.reaching(b).collect();
            found.sort_unstable();
            found.dedup();
            found
        };
        assert_eq!(reached(square(0.1, 0.1, 0.2, 0.2)), [0, 1], "the lower left cell");
        assert_eq!(reached(square(2.5, 1.2, 3.5, 1.8)), [1, 2], "the upper right cell");
        assert_eq!(reached(square(0.5, 0.5, 3.0, 1.5)), [0, 1, 2, 3], "all four cells");
    }

    #[test]
    fn a_point_on_a_cut_is_in_the_cell_its_boxes_are_listed_in() {
        let window = square(0.0, 0.0, 1.0, 1.0);
        let boxes: Vec<UvBox> =
            (0..100).map(|k| f64::from(k) / 100.0).map(|x| square(x, x, x + 0.01, x + 0.01)).collect();
        let cells = Cells::new(window, &boxes);
        assert_eq!(cells.cuts_u.len(), 9);
        for (index, b) in (0_u32..).zip(&boxes) {
            for corner in b.corners() {
                assert!(cells.near(corner).contains(&index), "{index} at {corner:?}");
            }
        }
        assert_eq!(cuts(0.0, 1.0, 1), Vec::<f64>::new());
    }
}
