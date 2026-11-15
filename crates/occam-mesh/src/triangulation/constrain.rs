//! Source: sloan-1993
//! The segments a face's boundary runs along, made edges of the triangulation and fixed (Sloan 1993): the edges a
//! segment crosses are found by walking along it from one end, and each is flipped where the two triangles at it make a
//! strictly convex quadrilateral, until none crosses; one that cannot be flipped yet waits its turn again. A segment
//! that runs through a point other than its ends, crosses a segment already fixed, or is given twice, is refused.
//! Every free edge is then flipped until the triangulation is Delaunay but for the fixed edges, and the triangles are
//! marked inside where reaching them from outside crosses an odd number of fixed edges.

use std::collections::{BTreeMap, VecDeque};

use occam_math::Side;

use super::{Fail, NONE, OUTER, Triangulation, at, edge_of, next, prev};

/// Makes each of `segments`, numbered as given, a fixed edge of `mesh`, and marks the triangles inside them.
///
/// # Errors
///
/// A [`Fail`] for a segment that touches or crosses another, or that is given twice.
pub(super) fn all(mesh: &mut Triangulation, segments: &[[usize; 2]]) -> Result<(), Fail> {
    let mut seen: BTreeMap<[usize; 2], [usize; 2]> = BTreeMap::new();
    for &[a, b] in segments {
        let (a, b) = (a.saturating_add(OUTER), b.saturating_add(OUTER));
        if a == b {
            continue;
        }
        if a >= mesh.points.len() || b >= mesh.points.len() {
            return Err(Fail::Broken);
        }
        if let Some(&earlier) = seen.get(&[a.min(b), a.max(b)]) {
            return Err(Fail::Crosses([a, b], earlier));
        }
        seen.insert([a.min(b), a.max(b)], [a, b]);
        mesh.enforce(a, b)?;
    }
    let every = (0..mesh.tris.len()).flat_map(|t| (0..3).map(move |k| (t, k))).collect();
    mesh.legalize(every)?;
    mesh.mark();
    Ok(())
}

/// Whether `v`, on the line through `a` and `b`, lies on the side of `a` that `b` does.
fn ahead(a: [f64; 2], b: [f64; 2], v: [f64; 2]) -> bool {
    let sign = |x: f64, from: f64| (x > from, x < from);
    sign(v[0], a[0]) == sign(b[0], a[0]) && sign(v[1], a[1]) == sign(b[1], a[1])
}

impl Triangulation {
    /// The triangles round point `a`, anticlockwise from the one it knows, each with the corner `a` is at.
    fn star(&self, a: usize) -> Vec<(usize, usize)> {
        let first = self.incident.get(a).copied().unwrap_or(NONE);
        let mut around = Vec::new();
        let mut t = first;
        while let Some(k) = (0..3).find(|&k| at(self.tri(t).v, k) == a) {
            around.push((t, k));
            t = at(self.tri(t).n, next(k));
            if t == first || around.len() > self.tris.len() {
                break;
            }
        }
        around
    }

    /// A triangle with an edge from `a` to `b`, either way, and the corner opposite it: found round whichever end's
    /// star holds it, as a corner of the outermost triangle's star is open.
    fn find_edge(&self, a: usize, b: usize) -> Option<(usize, usize)> {
        self.edge_round(a, b).or_else(|| self.edge_round(b, a))
    }

    /// A triangle round `a` with an edge from `a` to `b`, either way, and the corner opposite it.
    fn edge_round(&self, a: usize, b: usize) -> Option<(usize, usize)> {
        self.star(a).into_iter().find_map(|(t, k)| {
            let v = self.tri(t).v;
            if at(v, next(k)) == b {
                Some((t, prev(k)))
            } else if at(v, prev(k)) == b {
                Some((t, next(k)))
            } else {
                None
            }
        })
    }

    /// The triangle at `a` through which the segment to `b` leaves it, and the edge it leaves by, the end right of
    /// the segment first.
    fn leaving(&self, a: usize, b: usize) -> Result<(usize, [usize; 2]), Fail> {
        for (t, k) in self.star(a) {
            let v = self.tri(t).v;
            let (right, left) = (at(v, next(k)), at(v, prev(k)));
            for x in [right, left] {
                if self.orient(a, b, x)? == Side::Within && ahead(self.point(a), self.point(b), self.point(x)) {
                    return Err(Fail::Touches([a, b], x));
                }
            }
            if self.orient(a, b, right)? == Side::Below && self.orient(a, b, left)? == Side::Above {
                return Ok((t, [right, left]));
            }
        }
        Err(Fail::Broken)
    }

    /// The edges the segment from `a` to `b` crosses, in order from `a`, each with the end right of the segment first.
    fn crossing(&self, a: usize, b: usize) -> Result<Vec<[usize; 2]>, Fail> {
        let (mut t, mut edge) = self.leaving(a, b)?;
        let mut crossed = Vec::new();
        for _ in 0..=self.tris.len() {
            let k = edge_of(self.tri(t), edge).ok_or(Fail::Broken)?;
            if at(self.fixed(t), k) {
                return Err(Fail::Crosses([a, b], edge));
            }
            crossed.push(edge);
            let u = at(self.tri(t).n, k);
            let w = at(self.tri(u).v, self.across(u, t)?);
            if w == b {
                return Ok(crossed);
            }
            edge = match self.orient(a, b, w)? {
                Side::Above => [edge[0], w],
                Side::Below => [w, edge[1]],
                Side::Within => return Err(Fail::Touches([a, b], w)),
            };
            t = u;
        }
        Err(Fail::Broken)
    }

    /// Whether the edge opposite corner `k` of triangle `t` is the diagonal of a strictly convex quadrilateral.
    fn convex(&self, t: usize, k: usize) -> Result<bool, Fail> {
        let tri = self.tri(t);
        let (c, a, b) = (at(tri.v, k), at(tri.v, next(k)), at(tri.v, prev(k)));
        let u = at(tri.n, k);
        let d = at(self.tri(u).v, self.across(u, t)?);
        Ok(self.orient(c, a, d)? == Side::Above && self.orient(c, d, b)? == Side::Above)
    }

    /// Whether the segments from `a` to `b` and from `c` to `d` cross strictly inside both.
    fn cross(&self, [a, b]: [usize; 2], [c, d]: [usize; 2]) -> Result<bool, Fail> {
        if a == c || a == d || b == c || b == d {
            return Ok(false);
        }
        let apart = |x: Side, y: Side| matches!((x, y), (Side::Above, Side::Below) | (Side::Below, Side::Above));
        let (s, t) = (self.orient(a, b, c)?, self.orient(a, b, d)?);
        Ok(apart(s, t) && apart(self.orient(c, d, a)?, self.orient(c, d, b)?))
    }

    /// Makes the segment from `a` to `b` an edge, by flipping the edges that cross it, and fixes it.
    fn enforce(&mut self, a: usize, b: usize) -> Result<(), Fail> {
        if self.find_edge(a, b).is_none() {
            let mut queue: VecDeque<[usize; 2]> = self.crossing(a, b)?.into();
            let mut left = queue.len().saturating_mul(queue.len()).saturating_mul(8).saturating_add(64);
            while let Some([p, q]) = queue.pop_front() {
                left = left.checked_sub(1).ok_or(Fail::Broken)?;
                let (t, k) = self.find_edge(p, q).ok_or(Fail::Broken)?;
                if !self.convex(t, k)? {
                    queue.push_back([p, q]);
                    continue;
                }
                self.flip(t, k)?;
                let made = self.tri(t).v;
                let diagonal = [at(made, 0), at(made, 2)];
                if self.cross([a, b], diagonal)? {
                    queue.push_back(diagonal);
                }
            }
        }
        let (t, k) = self.find_edge(a, b).ok_or(Fail::Broken)?;
        let u = at(self.tri(t).n, k);
        self.fix(t, k);
        let j = self.across(u, t)?;
        self.fix(u, j);
        Ok(())
    }

    /// Fixes the edge opposite corner `k` of triangle `t`.
    fn fix(&mut self, t: usize, k: usize) {
        if let Some(tri) = self.tris.get_mut(t) {
            tri.fixed = [0, 1, 2].map(|j| j == k || at(tri.fixed, j));
        }
    }

    /// Marks each triangle inside where reaching it from the outermost triangle crosses an odd number of fixed edges,
    /// and counts them.
    fn mark(&mut self) {
        let mut depth = vec![u32::MAX; self.tris.len()];
        let start = self.incident.first().copied().unwrap_or(0);
        let mut level = vec![start];
        let mut crossings = 0_u32;
        while !level.is_empty() {
            for &t in &level {
                if let Some(d) = depth.get_mut(t) {
                    *d = crossings;
                }
            }
            let (mut stack, mut beyond) = (level, Vec::new());
            while let Some(t) = stack.pop() {
                let tri = self.tri(t);
                for k in 0..3 {
                    let u = at(tri.n, k);
                    match depth.get_mut(u) {
                        Some(d) if *d == u32::MAX && !at(tri.fixed, k) => {
                            *d = crossings;
                            stack.push(u);
                        }
                        Some(d) if *d == u32::MAX => beyond.push(u),
                        Some(_) | None => {}
                    }
                }
            }
            beyond.retain(|&u| depth.get(u) == Some(&u32::MAX));
            beyond.sort_unstable();
            beyond.dedup();
            (level, crossings) = (beyond, crossings.saturating_add(1));
        }
        self.inside = depth.iter().map(|&d| d != u32::MAX && d & 1 == 1).collect();
        self.count = self.inside.iter().filter(|&&inside| inside).count();
    }
}
