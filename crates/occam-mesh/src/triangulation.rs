//! Source: lawson-1977
//! A Delaunay triangulation of points in a face's parameters, made by putting them one at a time into a triangle that
//! holds them all, and flipping each edge whose far corner lies certainly inside its triangle's circle until none does
//! (Lawson 1977). Each point is found by walking from the last triangle found, across the first edge it lies beyond,
//! which ends in a Delaunay triangulation. Which side of an edge a point lies on is decided exactly on the coordinates
//! as they are ([`crate::predicates`]), so a point on an edge splits it and a point on a corner is refused, whatever
//! rounding made them.
//!
//! The segments a face's boundary runs along are then made edges and fixed ([`constrain`]), no flip crosses a fixed
//! edge, and the triangles inside the boundary are marked. Points are numbered as they are given, from 0; the three
//! corners of the triangle that holds them all come before them inside, and are never shown.

#![expect(clippy::many_single_char_names, reason = "triangles' corners and numbers, as the papers name them")]

mod constrain;

use occam_math::Side;

use crate::limits::{higher, lower};
use crate::predicates::{P2, incircle, orient};

/// No triangle: what lies across an edge of the outermost triangle.
pub(crate) const NONE: usize = usize::MAX;

/// How many points come before the ones given: the corners of the triangle that holds them.
const OUTER: usize = 3;

/// The most flips any one change may make before the triangulation is taken as broken: far more than valid input
/// needs, so that a mistake is a report and not a hang.
const FLIPS: usize = 1 << 26;

/// A triangle: its corners anticlockwise, and across each edge, the edge opposite the corner at the same place, the
/// triangle there and whether the edge is fixed.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Tri {
    pub v: [usize; 3],
    pub n: [usize; 3],
    pub fixed: [bool; 3],
}

/// What a link that does not exist reads as: a triangle whose corners are no points.
const DEAD: Tri = Tri { v: [NONE; 3], n: [NONE; 3], fixed: [false; 3] };

/// Why a triangulation could not be made, with the points involved, numbered as given.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Fail {
    /// Two points are at one place.
    Coincide(usize, usize),
    /// A segment runs through a point other than its ends.
    Touches([usize; 2], usize),
    /// Two segments cross, or one is given twice.
    Crosses([usize; 2], [usize; 2]),
    /// A decision could not be made: a coordinate is not finite, or is beyond the range the exact stage is exact for.
    Undecided,
    /// A change took more steps than valid input needs, or a link was broken.
    Broken,
}

impl Fail {
    /// The same failure with its points numbered as given rather than inside.
    fn given(self) -> Self {
        let out = |k: usize| k.saturating_sub(OUTER);
        match self {
            Self::Coincide(a, b) => Self::Coincide(out(a), out(b)),
            Self::Touches([a, b], c) => Self::Touches([out(a), out(b)], out(c)),
            Self::Crosses([a, b], [c, d]) => Self::Crosses([out(a), out(b)], [out(c), out(d)]),
            Self::Undecided | Self::Broken => self,
        }
    }
}

/// The corner after `k`, anticlockwise.
const fn next(k: usize) -> usize {
    match k {
        0 => 1,
        1 => 2,
        _ => 0,
    }
}

/// The corner before `k`.
const fn prev(k: usize) -> usize {
    match k {
        0 => 2,
        1 => 0,
        _ => 1,
    }
}

/// The item of `items` at corner `k`.
const fn at<T: Copy>(items: [T; 3], k: usize) -> T {
    match k {
        0 => items[0],
        1 => items[1],
        _ => items[2],
    }
}

/// `tri` turned so that its corner `k` comes first.
fn turned(tri: Tri, k: usize) -> Tri {
    let order = |items: [usize; 3]| [at(items, k), at(items, next(k)), at(items, prev(k))];
    let (j, l) = (next(k), prev(k));
    Tri { v: order(tri.v), n: order(tri.n), fixed: [at(tri.fixed, k), at(tri.fixed, j), at(tri.fixed, l)] }
}

/// The corner of `tri` opposite which its edge runs between `a` and `b`, either way.
fn edge_of(tri: Tri, [a, b]: [usize; 2]) -> Option<usize> {
    (0..3).find(|&k| {
        let (p, q) = (at(tri.v, next(k)), at(tri.v, prev(k)));
        (p == a && q == b) || (p == b && q == a)
    })
}

/// Where a point lies in a triangle.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Place {
    /// Strictly inside it.
    Inside,
    /// On the edge opposite this corner, strictly between its ends.
    Edge(usize),
    /// On this corner.
    Corner(usize),
    /// Strictly beyond the edge opposite this corner.
    Beyond(usize),
}

/// A constrained Delaunay triangulation of points in a plane, and which of its triangles lie inside the boundary.
pub(crate) struct Triangulation {
    points: Vec<P2>,
    tris: Vec<Tri>,
    /// One triangle at each point.
    incident: Vec<usize>,
    inside: Vec<bool>,
    count: usize,
    /// The triangle the last walk ended in.
    last: usize,
    /// The triangles each change has made or changed since they were last taken.
    touched: Vec<usize>,
}

impl Triangulation {
    /// The triangulation of `points` whose edges include each of `segments`, the pairs of points a face's boundary
    /// runs between, each loop closed; a triangle is inside where it lies within an odd number of loops.
    ///
    /// # Errors
    ///
    /// A [`Fail`] naming the points of what made it impossible.
    pub(crate) fn new(points: &[P2], segments: &[[usize; 2]]) -> Result<Self, Fail> {
        Self::made(points, segments).map_err(Fail::given)
    }

    fn made(points: &[P2], segments: &[[usize; 2]]) -> Result<Self, Fail> {
        let mut mesh = Self::around(points);
        for p in OUTER..mesh.points.len() {
            mesh.insert(p)?;
        }
        constrain::all(&mut mesh, segments)?;
        mesh.touched.clear();
        Ok(mesh)
    }

    /// The one triangle holding every point of `points`, far beyond them, with none of them in it yet.
    fn around(points: &[P2]) -> Self {
        let (mut lo, mut hi) = ([f64::INFINITY; 2], [f64::NEG_INFINITY; 2]);
        for p in points {
            lo = [lower(lo[0], p[0]), lower(lo[1], p[1])];
            hi = [higher(hi[0], p[0]), higher(hi[1], p[1])];
        }
        let size = higher(hi[0] - lo[0], hi[1] - lo[1]);
        let size = if size > 0.0 { size } else { 1.0 };
        let (x, y, far) = (lo[0].midpoint(hi[0]), lo[1].midpoint(hi[1]), 16.0 * size);
        let mut all = vec![[x - far, y - far], [x + far, y - far], [x, y + far]];
        all.extend_from_slice(points);
        let first = Tri { v: [0, 1, 2], n: [NONE; 3], fixed: [false; 3] };
        let incident = vec![0; all.len()];
        Self { points: all, tris: vec![first], incident, inside: vec![false], count: 0, last: 0, touched: Vec::new() }
    }

    fn tri(&self, t: usize) -> Tri {
        self.tris.get(t).copied().unwrap_or(DEAD)
    }

    fn point(&self, v: usize) -> P2 {
        self.points.get(v).copied().unwrap_or([f64::NAN; 2])
    }

    fn orient(&self, a: usize, b: usize, c: usize) -> Result<Side, Fail> {
        orient(self.point(a), self.point(b), self.point(c)).ok_or(Fail::Undecided)
    }

    /// Writes `tri` as triangle `t`, and makes it the one each of its corners knows.
    fn put(&mut self, t: usize, tri: Tri) {
        if let Some(slot) = self.tris.get_mut(t) {
            *slot = tri;
        }
        for v in tri.v {
            if let Some(slot) = self.incident.get_mut(v) {
                *slot = t;
            }
        }
        self.touched.push(t);
    }

    /// Adds `tri`, inside if `inside`, and gives its number.
    fn add(&mut self, tri: Tri, inside: bool) -> usize {
        let t = self.tris.len();
        self.tris.push(DEAD);
        self.inside.push(inside);
        if inside {
            self.count = self.count.saturating_add(1);
        }
        self.put(t, tri);
        t
    }

    /// Makes `u`'s link across its edge between `edge`'s ends point to `to`.
    fn relink(&mut self, u: usize, edge: [usize; 2], to: usize) {
        let Some(slot) = self.tris.get_mut(u) else { return };
        if let Some(k) = edge_of(*slot, edge) {
            slot.n = [0, 1, 2].map(|j| if j == k { to } else { at(slot.n, j) });
        }
    }

    /// The corner of triangle `u` across from triangle `t`.
    fn across(&self, u: usize, t: usize) -> Result<usize, Fail> {
        let tri = self.tri(u);
        (0..3).find(|&k| at(tri.n, k) == t).ok_or(Fail::Broken)
    }

    /// Where point `q` lies in triangle `t`.
    fn place(&self, t: usize, q: P2) -> Result<Place, Fail> {
        let tri = self.tri(t);
        let mut on = Vec::new();
        for k in 0..3 {
            let (a, b) = (self.point(at(tri.v, next(k))), self.point(at(tri.v, prev(k))));
            match orient(a, b, q).ok_or(Fail::Undecided)? {
                Side::Below => return Ok(Place::Beyond(k)),
                Side::Within => on.push(k),
                Side::Above => {}
            }
        }
        match on.as_slice() {
            [] => Ok(Place::Inside),
            [k] => Ok(Place::Edge(*k)),
            [j, k] => Ok(Place::Corner(at(tri.v, (0..3).find(|c| c != j && c != k).unwrap_or(0)))),
            _ => Err(Fail::Broken),
        }
    }

    /// The triangle point `q` lies in, and where in it, found by walking from the last triangle found. A walk that
    /// crosses more edges than there are triangles is taken as broken.
    fn locate(&mut self, q: P2) -> Result<(usize, Place), Fail> {
        let mut t = self.last;
        for _ in 0..=self.tris.len() {
            match self.place(t, q)? {
                Place::Beyond(k) => t = at(self.tri(t).n, k),
                place => {
                    self.last = t;
                    return Ok((t, place));
                }
            }
        }
        Err(Fail::Broken)
    }

    /// Puts point `p` in, wherever it lies.
    fn insert(&mut self, p: usize) -> Result<(), Fail> {
        match self.locate(self.point(p))? {
            (t, Place::Inside) => self.split(t, p),
            (t, Place::Edge(k)) => self.split_edge(t, k, p),
            (_, Place::Corner(v)) => Err(Fail::Coincide(v, p)),
            (_, Place::Beyond(_)) => Err(Fail::Broken),
        }
    }

    /// Splits triangle `t` into three at point `p`, strictly inside it, and flips what that makes need.
    fn split(&mut self, t: usize, p: usize) -> Result<(), Fail> {
        let Tri { v: [a, b, c], n: [na, nb, nc], fixed: [fa, fb, fc] } = self.tri(t);
        let inside = self.inside.get(t).copied().unwrap_or(false);
        let (t1, t2) = (self.tris.len(), self.tris.len().saturating_add(1));
        self.put(t, Tri { v: [b, c, p], n: [t1, t2, na], fixed: [false, false, fa] });
        self.add(Tri { v: [c, a, p], n: [t2, t, nb], fixed: [false, false, fb] }, inside);
        self.add(Tri { v: [a, b, p], n: [t, t1, nc], fixed: [false, false, fc] }, inside);
        self.relink(nb, [c, a], t1);
        self.relink(nc, [a, b], t2);
        self.legalize(vec![(t, 2), (t1, 2), (t2, 2)])
    }

    /// Splits the edge opposite corner `k` of triangle `t` at point `p`, strictly between its ends, and the triangles
    /// either side of it, each in two, and flips what that makes need.
    fn split_edge(&mut self, t: usize, k: usize, p: usize) -> Result<(), Fail> {
        let s = turned(self.tri(t), k);
        let u = at(s.n, 0);
        let r = turned(self.tri(u), self.across(u, t)?);
        let ([c, a, b], d) = (s.v, at(r.v, 0));
        let inside = |x: usize| self.inside.get(x).copied().unwrap_or(false);
        let (inside_t, inside_u, t2) = (inside(t), inside(u), self.tris.len());
        let u2 = t2.saturating_add(1);
        let split = at(s.fixed, 0);
        self.put(t, Tri { v: [c, a, p], n: [u2, t2, at(s.n, 2)], fixed: [split, false, at(s.fixed, 2)] });
        self.add(Tri { v: [c, p, b], n: [u, at(s.n, 1), t], fixed: [split, at(s.fixed, 1), false] }, inside_t);
        self.put(u, Tri { v: [d, b, p], n: [t2, u2, at(r.n, 2)], fixed: [split, false, at(r.fixed, 2)] });
        self.add(Tri { v: [d, p, a], n: [t, at(r.n, 1), u], fixed: [split, at(r.fixed, 1), false] }, inside_u);
        self.relink(at(s.n, 1), [b, c], t2);
        self.relink(at(r.n, 1), [a, d], u2);
        self.legalize(vec![(t, 2), (t2, 1), (u, 2), (u2, 1)])
    }

    /// Whether the edge opposite corner `k` of triangle `t` is free and the corner across it lies certainly inside
    /// `t`'s circle, so that the two triangles make a strictly convex quadrilateral and flipping the edge makes both
    /// Delaunay.
    fn flippable(&self, t: usize, k: usize) -> Result<bool, Fail> {
        let s = turned(self.tri(t), k);
        let u = at(s.n, 0);
        if u == NONE || at(s.fixed, 0) || u >= self.tris.len() {
            return Ok(false);
        }
        let d = at(self.tri(u).v, self.across(u, t)?);
        let [c, a, b] = s.v.map(|v| self.point(v));
        Ok(incircle(c, a, b, self.point(d)) == Some(Side::Above))
    }

    /// Flips the edge opposite corner `k` of triangle `t`, whose two triangles make a quadrilateral strictly convex
    /// at it, and gives the four edges round the quadrilateral.
    fn flip(&mut self, t: usize, k: usize) -> Result<[(usize, usize); 4], Fail> {
        let s = turned(self.tri(t), k);
        let u = at(s.n, 0);
        let r = turned(self.tri(u), self.across(u, t)?);
        let ([c, a, b], d) = (s.v, at(r.v, 0));
        self.put(
            t,
            Tri { v: [c, a, d], n: [at(r.n, 1), u, at(s.n, 2)], fixed: [at(r.fixed, 1), false, at(s.fixed, 2)] },
        );
        self.put(
            u,
            Tri { v: [d, b, c], n: [at(s.n, 1), t, at(r.n, 2)], fixed: [at(s.fixed, 1), false, at(r.fixed, 2)] },
        );
        self.relink(at(s.n, 1), [b, c], u);
        self.relink(at(r.n, 1), [a, d], t);
        Ok([(t, 0), (t, 2), (u, 0), (u, 2)])
    }

    /// Flips edges from `stack` until none of them, nor any edge a flip makes, needs it.
    fn legalize(&mut self, mut stack: Vec<(usize, usize)>) -> Result<(), Fail> {
        let mut flips = 0_usize;
        while let Some((t, k)) = stack.pop() {
            if self.flippable(t, k)? {
                flips = flips.saturating_add(1);
                if flips > FLIPS {
                    return Err(Fail::Broken);
                }
                stack.extend(self.flip(t, k)?);
            }
        }
        Ok(())
    }

    /// The inside triangles' corners, numbered as given, anticlockwise.
    pub(crate) fn triangles(&self) -> Vec<[usize; 3]> {
        self.inside().into_iter().filter_map(|t| self.corners(t)).collect()
    }

    /// The numbers of the inside triangles, in order.
    pub(crate) fn inside(&self) -> Vec<usize> {
        self.inside.iter().enumerate().filter(|&(_, &inside)| inside).map(|(t, _)| t).collect()
    }

    /// Whether triangle `t` is inside.
    pub(crate) fn is_inside(&self, t: usize) -> bool {
        self.inside.get(t).copied().unwrap_or(false)
    }

    /// How many triangles there are, inside or not: one more than the highest number of one.
    pub(crate) const fn slots(&self) -> usize {
        self.tris.len()
    }

    /// How many triangles are inside.
    pub(crate) const fn count(&self) -> usize {
        self.count
    }

    /// The corners of triangle `t`, numbered as given, if it is inside.
    pub(crate) fn corners(&self, t: usize) -> Option<[usize; 3]> {
        let [a, b, c] = self.tri(t).v.map(|v| v.checked_sub(OUTER));
        self.is_inside(t).then_some([a?, b?, c?])
    }

    /// Which edges of triangle `t` are fixed, each opposite the corner at the same place.
    pub(crate) fn fixed(&self, t: usize) -> [bool; 3] {
        self.tri(t).fixed
    }

    /// Puts a new point at `q` in triangle `t`, inside, and gives its number; or nothing, and changes nothing, unless
    /// `q` lies strictly inside `t` or strictly inside an edge of it that is not fixed.
    ///
    /// # Errors
    ///
    /// [`Fail::Undecided`] where `q` cannot be placed, and [`Fail::Broken`] where the flips do not end.
    pub(crate) fn place_in(&mut self, t: usize, q: P2) -> Result<Option<usize>, Fail> {
        if !self.is_inside(t) {
            return Ok(None);
        }
        let (place, p) = (self.place(t, q)?, self.points.len());
        let free = |k: usize| !at(self.fixed(t), k);
        match place {
            Place::Inside => {
                self.points.push(q);
                self.incident.push(t);
                self.split(t, p)?;
            }
            Place::Edge(k) if free(k) => {
                self.points.push(q);
                self.incident.push(t);
                self.split_edge(t, k, p)?;
            }
            Place::Edge(_) | Place::Corner(_) | Place::Beyond(_) => return Ok(None),
        }
        Ok(Some(p.saturating_sub(OUTER)))
    }

    /// The triangles made or changed since this was last asked, each once, in order.
    pub(crate) fn take_touched(&mut self) -> Vec<usize> {
        let mut touched = std::mem::take(&mut self.touched);
        touched.sort_unstable();
        touched.dedup();
        touched
    }
}

#[cfg(test)]
mod tests;
