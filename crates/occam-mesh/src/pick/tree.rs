//! Source: original
//! The box tree a pick walks: the items' boxes, split at the median of their centres along the axis those spread
//! furthest on, until a leaf holds a few. It is built on one thread, with ties broken by the item's number, so it is
//! the same tree on every target; a walk visits every item whose box, grown by a reach, the ray may cross, and decides
//! nothing itself.

use occam_math::{larger, smaller};
use occam_units::Vec3;

use crate::pick::Ray;

/// How many items a leaf holds at most.
const LEAF: usize = 4;

/// An axis-aligned box: its least and greatest corners.
pub(crate) type Bounds = (Vec3, Vec3);

/// The origin, which stands in where nothing is.
pub(crate) const ZERO: Vec3 = Vec3::new(0.0, 0.0, 0.0);

/// A node: its box, and either the range of the items it holds, in the tree's order, or where its second child is,
/// its first being the node after it.
#[derive(Clone, Debug, PartialEq)]
struct Node {
    bounds: Bounds,
    first: u32,
    end: u32,
    right: Option<u32>,
}

/// A box tree over some items.
#[derive(Clone, Debug, PartialEq, Default)]
pub(crate) struct Tree {
    nodes: Vec<Node>,
    order: Vec<u32>,
}

/// The coordinates of `v`.
const fn axes(v: Vec3) -> [f64; 3] {
    [v.x, v.y, v.z]
}

/// The box holding both.
fn union((lo, hi): Bounds, (low, high): Bounds) -> Bounds {
    let pick = |a: Vec3, b: Vec3, f: fn(f64, f64) -> f64| Vec3::new(f(a.x, b.x), f(a.y, b.y), f(a.z, b.z));
    (pick(lo, low, smaller), pick(hi, high, larger))
}

/// The box of the points.
pub(crate) fn around(points: &[Vec3]) -> Bounds {
    let first = points.first().copied().unwrap_or(ZERO);
    points.iter().fold((first, first), |held, &p| union(held, (p, p)))
}

impl Tree {
    /// The tree over items whose boxes are `boxes`, each numbered by its place.
    pub(crate) fn new(boxes: &[Bounds]) -> Self {
        let count = u32::try_from(boxes.len()).unwrap_or(u32::MAX);
        let mut tree = Self { nodes: Vec::new(), order: (0..count).collect() };
        if count > 0 {
            tree.split(boxes, 0, count);
        }
        tree
    }

    /// Adds the node holding the items from `first` up to `end` in the tree's order, then its children.
    fn split(&mut self, boxes: &[Bounds], first: u32, end: u32) {
        let at = |k: u32| usize::try_from(k).unwrap_or(usize::MAX);
        let Some(items) = self.order.get_mut(at(first)..at(end)) else { return };
        let of = |item: &u32| boxes.get(at(*item)).copied().unwrap_or((ZERO, ZERO));
        let bounds = items.iter().map(of).reduce(union).unwrap_or((ZERO, ZERO));
        let node = self.nodes.len();
        self.nodes.push(Node { bounds, first, end, right: None });
        if items.len() <= LEAF {
            return;
        }
        let centre = |item: &u32| {
            let (lo, hi) = of(item);
            (lo + hi) * 0.5
        };
        let centres = items.iter().map(|item| (centre(item), centre(item))).reduce(union).unwrap_or((ZERO, ZERO));
        let spread = axes(centres.1 - centres.0);
        let axis = (0..3).fold(0, |best, k| if spread.get(k) > spread.get(best) { k } else { best });
        let key = |item: &u32| axes(centre(item)).get(axis).copied().unwrap_or_default();
        let middle = items.len() / 2;
        items.select_nth_unstable_by(middle, |a, b| key(a).total_cmp(&key(b)).then(a.cmp(b)));
        let half = first.saturating_add(u32::try_from(middle).unwrap_or(0));
        self.split(boxes, first, half);
        let right = u32::try_from(self.nodes.len()).ok();
        if let Some(held) = self.nodes.get_mut(node) {
            held.right = right;
        }
        self.split(boxes, half, end);
    }

    /// Calls `each` with every item whose box, grown by `reach` and a little more for rounding, the ray may cross at
    /// or after its start.
    pub(crate) fn visit(&self, ray: &Ray, reach: f64, mut each: impl FnMut(u32)) {
        let mut stack = if self.nodes.is_empty() { Vec::new() } else { vec![0_usize] };
        while let Some(at) = stack.pop() {
            let Some(node) = self.nodes.get(at) else { continue };
            if !crosses(ray, node.bounds, reach) {
                continue;
            }
            if let Some(right) = node.right {
                stack.extend(usize::try_from(right));
                stack.push(at.saturating_add(1));
            } else {
                let range = usize::try_from(node.first).unwrap_or(0)..usize::try_from(node.end).unwrap_or(0);
                self.order.get(range).into_iter().flatten().for_each(|&item| each(item));
            }
        }
    }
}

/// Whether the ray may cross `bounds` grown by `reach`, at or after its start: the slab test, with the box grown by
/// far more than its rounding can move it.
fn crosses(ray: &Ray, (lo, hi): Bounds, reach: f64) -> bool {
    let (origin, direction) = (axes(ray.origin), axes(ray.direction));
    let size = [lo, hi, ray.origin].into_iter().flat_map(axes).fold(1.0, |most, x| larger(most, x.abs()));
    let grown = reach + size * 1e-12;
    let (mut near, mut far) = (0.0_f64, f64::INFINITY);
    for ((o, d), (low, high)) in origin.into_iter().zip(direction).zip(axes(lo).into_iter().zip(axes(hi))) {
        let (low, high) = (low - grown, high + grown);
        if d == 0.0 {
            if o < low || o > high {
                return false;
            }
            continue;
        }
        let (a, b) = ((low - o) / d, (high - o) / d);
        let (enter, leave) = if a < b { (a, b) } else { (b, a) };
        near = larger(near, enter);
        far = smaller(far, leave);
        if far < near {
            return false;
        }
    }
    true
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Sixteen unit boxes along x, a gap between each.
    fn boxes() -> Vec<Bounds> {
        (0..16).map(|k| f64::from(k) * 2.0).map(|x| (Vec3::new(x, 0.0, 0.0), Vec3::new(x + 1.0, 1.0, 1.0))).collect()
    }

    fn visited(tree: &Tree, origin: [f64; 3], direction: [f64; 3], reach: f64) -> Vec<u32> {
        let mut seen = Vec::new();
        tree.visit(&Ray::new(origin, direction).unwrap(), reach, |item| seen.push(item));
        seen.sort_unstable();
        seen
    }

    #[test]
    fn a_tree_splits_at_the_median_down_to_leaves_of_four_and_a_walk_visits_only_the_leaves_the_ray_crosses() {
        let tree = Tree::new(&boxes());
        assert_eq!(tree.nodes.len(), 7, "sixteen, two of eight, four leaves of four");
        assert!(tree.nodes.iter().filter(|node| node.right.is_none()).all(|leaf| leaf.end - leaf.first == 4));
        assert_eq!(visited(&tree, [0.5, 0.5, 5.0], [0.0, 0.0, -1.0], 0.0), [0, 1, 2, 3], "the first box's leaf");
        assert_eq!(visited(&tree, [30.5, 0.5, 5.0], [0.0, 0.0, -1.0], 0.0), [12, 13, 14, 15], "the last box's leaf");
        assert_eq!(visited(&tree, [0.5, 0.5, 5.0], [0.0, 0.0, 1.0], 0.0), [] as [u32; 0], "away from every box");
        assert_eq!(visited(&tree, [0.5, 3.0, 5.0], [0.0, 0.0, -1.0], 0.0), [] as [u32; 0], "past every box");
        assert_eq!(visited(&tree, [0.5, 3.0, 5.0], [0.0, 0.0, -1.0], 2.5), [0, 1, 2, 3], "within reach of the first");
        assert_eq!(visited(&tree, [-1.0, 0.5, 0.5], [1.0, 0.0, 0.0], 0.0).len(), 16, "along all of them");
        assert_eq!(visited(&Tree::new(&[]), [0.0; 3], [1.0, 0.0, 0.0], 1.0), [] as [u32; 0], "an empty tree");
    }

    #[test]
    fn a_tree_splits_along_the_axis_its_centres_spread_furthest_on_the_first_where_two_tie_ordered_by_their_centres() {
        // Eight unit boxes, item `k` at x = 3k mod 8 and y = 100 + (5k + 2) mod 8: x and y spread alike, and their
        // orders and the items' own are three different orders. The first axis of the two splits them by x into
        // items {0, 1, 3, 6} and {2, 4, 5, 7}; by y they would go {0, 3, 5, 6} and {1, 2, 4, 7}, and by number
        // {0, 1, 2, 3} and {4, 5, 6, 7}.
        let corner = |k: u32| Vec3::new(f64::from(3 * k % 8), 100.0 + f64::from((5 * k + 2) % 8), 0.0);
        let boxes: Vec<Bounds> = (0..8).map(|k| (corner(k), corner(k) + Vec3::new(1.0, 1.0, 1.0))).collect();
        let tree = Tree::new(&boxes);
        assert_eq!(visited(&tree, [3.5, 107.5, 5.0], [0.0, 0.0, -1.0], 0.0), [0, 1, 3, 6], "down through item 1");
        assert_eq!(visited(&tree, [4.5, 106.5, 5.0], [0.0, 0.0, -1.0], 0.0), [2, 4, 5, 7], "down through item 4");
    }

    #[test]
    fn a_box_grown_by_the_reach_holds_its_faces_and_a_ray_that_starts_on_its_far_face_leaving_it_still_crosses_it() {
        // The box's tallest coordinate, 10, above every start's, sets how much more than the reach it grows, as the
        // code reckons it.
        let bounds = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 1.0, 10.0));
        let grown = 0.5 + 10.0 * 1e-12;
        let down = |x: f64| Ray::new([x, 0.5, 5.0], [0.0, 0.0, -1.0]).unwrap();
        assert!(crosses(&down(0.0 - grown), bounds, 0.5) && crosses(&down(1.0 + grown), bounds, 0.5), "on its faces");
        assert!(!crosses(&down(-0.6), bounds, 0.5) && !crosses(&down(1.6), bounds, 0.5), "beyond them");
        let leaving = Ray::new([1.0 + grown, 0.5, 5.0], [1.0, 0.0, 0.0]).unwrap();
        assert!(crosses(&leaving, bounds, 0.5), "where it starts, on the far face");
        let arriving = Ray::new([1.0 + grown, 0.5, 5.0], [-1.0, 0.0, 0.0]).unwrap();
        assert!(crosses(&arriving, bounds, 0.5));
        assert!(!crosses(&Ray::new([1.6, 0.5, 5.0], [1.0, 0.0, 0.0]).unwrap(), bounds, 0.5), "beyond it, leaving");
    }
}
