//! Source: original
//! Boxes in a plane, nested two by two so that a search reads only those near what it looks for: each group's box
//! holds the boxes below it, halved along its longer side at the middle box, so that a search that skips a group skips
//! everything in it. Which boxes meet a box, and the least of a distance that no box's contents come closer than, are
//! found in about the logarithm of the count, where the boxes are spread. A box that is not finite is in every answer,
//! so that a search never leaves out what a reading of every box would have found.

use super::flat::{P2, least};

/// A box, `[lo_u, lo_v, hi_u, hi_v]`.
pub(crate) type Bounds = [f64; 4];

/// The box that holds nothing, which every box grows from.
pub(crate) const EMPTY: Bounds = [f64::INFINITY, f64::INFINITY, f64::NEG_INFINITY, f64::NEG_INFINITY];

/// How many boxes a group holds before it is halved.
const LEAF: usize = 8;

/// The larger of `a` and `b`, the first if they are equal.
pub(crate) fn most(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// The box that holds `a` and `b`.
pub(crate) fn union(a: Bounds, b: Bounds) -> Bounds {
    [least(a[0], b[0]), least(a[1], b[1]), most(a[2], b[2]), most(a[3], b[3])]
}

/// The box that holds `a` and the point `p`.
pub(crate) fn with(a: Bounds, p: P2) -> Bounds {
    union(a, [p[0], p[1], p[0], p[1]])
}

/// `a` grown by `pad` on every side.
pub(crate) fn padded(a: Bounds, pad: f64) -> Bounds {
    [a[0] - pad, a[1] - pad, a[2] + pad, a[3] + pad]
}

/// Whether `a` and `b` meet, their edges included.
pub(crate) fn meet(a: Bounds, b: Bounds) -> bool {
    a[0] <= b[2] && b[0] <= a[2] && a[1] <= b[3] && b[1] <= a[3]
}

/// How far beyond rounding a box is padded: a billionth of the largest coordinate among `boxes`, and at least that of
/// one, where any distance or point computed from them is off by some ten thousandths of that.
pub(crate) fn slack(boxes: &[Bounds]) -> f64 {
    let largest = boxes.iter().flatten().filter(|x| x.is_finite()).fold(1.0, |so_far: f64, x| most(so_far, x.abs()));
    largest * 1e-9
}

/// How far `q` is from `a`: nothing inside it.
fn far(a: Bounds, q: P2) -> f64 {
    let off = |lo: f64, x: f64, hi: f64| most(most(lo - x, x - hi), 0.0);
    let (du, dv) = (off(a[0], q[0], a[2]), off(a[1], q[1], a[3]));
    (du * du + dv * dv).sqrt()
}

/// Whether every side of `a` is a finite number.
fn finite(a: Bounds) -> bool {
    a.iter().all(|x| x.is_finite())
}

/// What lies below a group: its two halves, or a run of the boxes in the order the groups take them.
#[derive(Clone, Copy)]
enum Below {
    Halves(usize, usize),
    Run(usize, usize),
}

/// A group of boxes: the box that holds them, and what lies below it.
#[derive(Clone, Copy)]
struct Group {
    bounds: Bounds,
    below: Below,
}

/// Boxes, each named by its index in the order given, nested for searching.
pub(crate) struct Boxes {
    /// Each box, in the order given.
    items: Vec<Bounds>,
    /// The finite boxes, in the order the groups take them.
    order: Vec<usize>,
    /// The groups, the one holding every finite box first.
    groups: Vec<Group>,
    /// The boxes that are not finite, which every search answers with.
    wild: Vec<usize>,
}

impl Boxes {
    /// `items` nested.
    pub(crate) fn new(items: Vec<Bounds>) -> Self {
        let (order, wild) = (0..items.len()).partition(|&k| items.get(k).is_some_and(|&b| finite(b)));
        let mut boxes = Self { items, order, groups: Vec::new(), wild };
        let centres: Vec<P2> = boxes.items.iter().map(|b| [b[0] + b[2], b[1] + b[3]]).collect();
        if !boxes.order.is_empty() {
            boxes.group(&centres, 0, boxes.order.len());
        }
        boxes
    }

    /// The box of item `k`.
    pub(crate) fn get(&self, k: usize) -> Bounds {
        self.items.get(k).copied().unwrap_or(EMPTY)
    }

    /// How many boxes there are.
    pub(crate) fn len(&self) -> usize {
        self.items.len()
    }

    /// Makes the group of the boxes `order[start..end]`, halving it while it holds more than [`LEAF`] at the middle of
    /// their `centres` (twice each box's centre) along its longer side, and gives its index.
    fn group(&mut self, centres: &[P2], start: usize, end: usize) -> usize {
        let run = self.order.get(start..end).unwrap_or_default();
        let bounds = run.iter().fold(EMPTY, |all, &k| union(all, self.get(k)));
        let index = self.groups.len();
        self.groups.push(Group { bounds, below: Below::Run(start, end) });
        let count = end.saturating_sub(start);
        if count > LEAF {
            let axis = usize::from(bounds[3] - bounds[1] > bounds[2] - bounds[0]);
            let at = |k: usize| centres.get(k).and_then(|c| c.get(axis)).copied().unwrap_or(0.0);
            let half = count / 2;
            if let Some(run) = self.order.get_mut(start..end) {
                run.select_nth_unstable_by(half, |&a, &b| at(a).total_cmp(&at(b)).then(a.cmp(&b)));
            }
            let middle = start.saturating_add(half);
            let halves = Below::Halves(self.group(centres, start, middle), self.group(centres, middle, end));
            if let Some(group) = self.groups.get_mut(index) {
                group.below = halves;
            }
        }
        index
    }

    /// Each box that meets `bounds`, and each that is not finite, each once, in no set order.
    pub(crate) fn meeting(&self, bounds: Bounds) -> Vec<usize> {
        let mut found = self.wild.clone();
        let mut todo: Vec<usize> = (!self.groups.is_empty()).then_some(0).into_iter().collect();
        while let Some(group) = todo.pop().and_then(|g| self.groups.get(g)) {
            if !meet(group.bounds, bounds) {
                continue;
            }
            match group.below {
                Below::Halves(a, b) => todo.extend([a, b]),
                Below::Run(start, end) => {
                    let run = self.order.get(start..end).unwrap_or_default();
                    found.extend(run.iter().copied().filter(|&k| meet(self.get(k), bounds)));
                }
            }
        }
        found
    }

    /// The least of `distance` over every box, as a reading of each in turn finds it, where no point in a box is nearer
    /// `q` than the box is, short by at most `slack`: a group further from `q` than the least found so far, by more than
    /// `slack`, holds nothing nearer, and is skipped. A distance that is not a number is passed over.
    pub(crate) fn least(&self, q: P2, slack: f64, distance: impl Fn(usize) -> f64) -> f64 {
        let mut nearest = self.wild.iter().fold(f64::INFINITY, |so_far, &k| least(so_far, distance(k)));
        let mut todo: Vec<usize> = (!self.groups.is_empty()).then_some(0).into_iter().collect();
        while let Some(group) = todo.pop().and_then(|g| self.groups.get(g)) {
            if far(group.bounds, q) - slack > nearest {
                continue;
            }
            match group.below {
                Below::Halves(a, b) => {
                    let to = |g: usize| self.groups.get(g).map_or(f64::INFINITY, |group| far(group.bounds, q));
                    todo.extend(if to(a) <= to(b) { [b, a] } else { [a, b] });
                }
                Below::Run(start, end) => {
                    for &k in self.order.get(start..end).unwrap_or_default() {
                        nearest = least(nearest, distance(k));
                    }
                }
            }
        }
        nearest
    }
}

#[cfg(test)]
#[expect(clippy::arithmetic_side_effects, reason = "small indices in tests")]
mod tests {
    use super::*;

    /// A grid of unit boxes `n` by `n`, a gap of one between each, row by row.
    fn grid(n: usize) -> Vec<Bounds> {
        let whole = |k: usize| f64::from(u32::try_from(k).unwrap());
        (0..n * n)
            .map(|k| [2.0 * whole(k % n), 2.0 * whole(k / n), 2.0 * whole(k % n) + 1.0, 2.0 * whole(k / n) + 1.0])
            .collect()
    }

    /// Every box that meets `bounds`, by reading each.
    fn read_all(items: &[Bounds], bounds: Bounds) -> Vec<usize> {
        (0..items.len()).filter(|&k| !finite(items[k]) || meet(items[k], bounds)).collect()
    }

    #[test]
    fn a_box_meets_another_where_they_overlap_or_touch_and_grows_to_hold_points() {
        let a = [0.0, 0.0, 1.0, 1.0];
        assert!(meet(a, [1.0, 1.0, 2.0, 2.0]), "a corner");
        assert!(meet(a, [0.25, 0.25, 0.5, 0.5]), "inside");
        assert!(!meet(a, [1.5, 0.0, 2.0, 1.0]) && !meet(a, [0.0, -2.0, 1.0, -1.0]));
        assert!(!meet(EMPTY, a) && !meet(a, EMPTY));
        assert_eq!(with(with(EMPTY, [1.0, 2.0]), [-1.0, 3.0]), [-1.0, 2.0, 1.0, 3.0]);
        assert_eq!(padded(a, 0.5), [-0.5, -0.5, 1.5, 1.5]);
        assert_eq!((far(a, [0.5, 0.5]), far(a, [4.0, 5.0]), far(a, [-2.0, 0.5])), (0.0, 5.0, 2.0));
        assert_eq!((most(1.0, 2.0), most(2.0, 1.0)), (2.0, 2.0));
        let equal = [most(0.0, -0.0), most(-0.0, 0.0)].map(f64::to_bits);
        assert_eq!(equal, [0.0, -0.0].map(f64::to_bits), "of two equal, the first, whatever the sign of a nought");
        let slacks = (slack(&[[0.0, 0.0, 0.5, -2000.0], EMPTY]), slack(&[EMPTY]));
        assert_eq!(slacks, (2000.0 * 1e-9, 1e-9), "a billionth of the largest coordinate, and of one at least");
    }

    #[test]
    fn the_boxes_that_meet_a_box_are_those_a_reading_of_every_box_finds() {
        let mut items = grid(12);
        items.push([f64::NAN, 0.0, 1.0, 1.0]);
        items.push([0.0, f64::NEG_INFINITY, 1.0, 1.0]);
        let boxes = Boxes::new(items.clone());
        assert_eq!(boxes.len(), items.len());
        for query in [[0.5, 0.5, 3.5, 2.5], [-10.0, 7.0, 40.0, 7.5], [100.0, 100.0, 101.0, 101.0], [5.0, 5.0, 5.0, 5.0]]
        {
            let mut found = boxes.meeting(query);
            found.sort_unstable();
            assert_eq!(found, read_all(&items, query), "{query:?}");
        }
        assert_eq!(Boxes::new(Vec::new()).meeting([0.0, 0.0, 1.0, 1.0]), Vec::<usize>::new());
    }

    #[test]
    fn a_search_for_the_least_reads_the_nearer_half_first_and_skips_a_half_too_far() {
        // Sixteen unit boxes in a column, listed out of order, and a point below them: the column is halved across its
        // length, so the lower eight are read first, and the upper eight, at least 9 away, are skipped once 1 is found,
        // but not while the least found is within the slack of them.
        let place = |k: usize| f64::from(u32::try_from((k * 7) % 16).unwrap());
        let items: Vec<Bounds> = (0..16).map(|k| [0.0, place(k), 1.0, place(k) + 1.0]).collect();
        let boxes = Boxes::new(items.clone());
        let q = [0.5, -1.0];
        let read = core::cell::Cell::new(0);
        let (counter, items) = (&read, &items);
        let lifted = |lift: f64| {
            move |k: usize| {
                counter.set(counter.get() + 1);
                let bottom = items[k][1];
                if bottom < 8.0 { bottom + 1.0 + lift } else { bottom + 1.0 }
            }
        };
        assert_eq!((boxes.least(q, 0.5, lifted(0.0)), read.replace(0)), (1.0, 8), "the upper half skipped");
        assert_eq!((boxes.least(q, 0.5, lifted(8.3)), read.replace(0)), (9.0, 16), "9.3 is within 0.5 of 9");
        assert_eq!((boxes.least(q, 0.5, lifted(9.0)), read.replace(0)), (9.0, 16), "and 10, but 9 is nearer");
    }

    #[test]
    fn a_square_group_is_halved_across_u() {
        // Sixteen unit boxes four by four, as wide as they are tall, and a point to their left: halved across u, the
        // nearer half is read and the farther skipped; halved across v, both halves would be as near, and both read.
        let items = grid(4);
        let boxes = Boxes::new(items.clone());
        let q = [-10.0, 3.5];
        let read = core::cell::Cell::new(0);
        let distance = |k: usize| {
            read.set(read.get() + 1);
            far(items[k], q)
        };
        assert_eq!((boxes.least(q, 0.5, distance), read.get()), (far(items[4], q), 8));
    }

    #[test]
    fn the_least_distance_is_the_one_a_reading_of_every_box_finds() {
        let items = grid(12);
        let boxes = Boxes::new(items.clone());
        let centre = |k: usize| [f64::midpoint(items[k][0], items[k][2]), f64::midpoint(items[k][1], items[k][3])];
        for q in [[0.0, 0.0], [7.3, 11.9], [-5.0, 30.0], [100.0, -3.0]] {
            let distance = |k: usize| {
                let c = centre(k);
                ((c[0] - q[0]) * (c[0] - q[0]) + (c[1] - q[1]) * (c[1] - q[1])).sqrt()
            };
            let all = (0..items.len()).fold(f64::INFINITY, |so_far, k| least(so_far, distance(k)));
            assert_eq!(boxes.least(q, 0.0, distance).to_bits(), all.to_bits(), "{q:?}");
        }
        let skipped = Boxes::new(vec![[0.0, 0.0, 1.0, 1.0], [f64::NAN; 4]]);
        assert_eq!(skipped.least([0.5, 0.5], 0.0, |k| if k == 0 { f64::NAN } else { 3.0 }), 3.0, "the wild box read");
        assert_eq!(Boxes::new(Vec::new()).least([0.0, 0.0], 0.0, |_| 1.0), f64::INFINITY);
    }
}
