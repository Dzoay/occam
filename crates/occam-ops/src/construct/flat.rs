//! Source: original
//! A face's loops in its plane's parameters, which are lengths: straight pieces and arcs, how far apart two pieces come,
//! whether a point is inside a loop, and a point clear of every loop that is strictly inside one.
//!
//! How far apart two pieces come is the least distance between them, found among the places it can be: where they cross,
//! an end of one against the other, and where both curves share a normal (for a segment and an arc, the arc's points
//! square to the segment; for two arcs, their points on the line through both centres). A distance no greater than the
//! band is a touch (DETERMINISM.md rule 6); whether two segments cross is decided exactly on their numbers.

use core::f64::consts::TAU;

use occam_math::{Predicate, Ring, Side, Verdict, atan2, decide_stored, sin_cos};
use occam_units::UvPoint;

/// The most steps an arc is cut into for a winding number.
const MOST_STEPS: u32 = 1 << 16;

/// A point of the plane, `[u, v]`.
pub(crate) type P2 = [f64; 2];

/// A piece of a loop, in the order the loop runs.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Flat {
    /// The segment from `a` to `b`.
    Seg { a: P2, b: P2 },
    /// The arc about `c` of radius `r` from the angle `from`, turning by `sweep` (anticlockwise if positive).
    Arc { c: P2, r: f64, from: f64, sweep: f64 },
}

fn sub(a: P2, b: P2) -> P2 {
    [a[0] - b[0], a[1] - b[1]]
}

fn add(a: P2, b: P2) -> P2 {
    [a[0] + b[0], a[1] + b[1]]
}

fn scale(a: P2, k: f64) -> P2 {
    [a[0] * k, a[1] * k]
}

fn dot(a: P2, b: P2) -> f64 {
    a[0] * b[0] + a[1] * b[1]
}

fn cross(a: P2, b: P2) -> f64 {
    a[0] * b[1] - a[1] * b[0]
}

/// The smaller of `a` and `b`, the first if they are equal, and never `f64::min`'s choice between zeros.
pub(crate) fn least(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// The length of `a`.
pub(crate) fn norm(a: P2) -> f64 {
    dot(a, a).sqrt()
}

/// The distance between `a` and `b`.
pub(crate) fn apart(a: P2, b: P2) -> f64 {
    norm(sub(a, b))
}

/// The point at `angle` on the circle about `c` of radius `r`.
fn on_circle(c: P2, r: f64, angle: f64) -> P2 {
    let (sin, cos) = sin_cos(angle);
    [c[0] + r * cos, c[1] + r * sin]
}

impl Flat {
    /// Where it begins.
    pub(crate) fn start(self) -> P2 {
        match self {
            Self::Seg { a, .. } => a,
            Self::Arc { c, r, from, .. } => on_circle(c, r, from),
        }
    }

    /// Where it ends.
    pub(crate) fn end(self) -> P2 {
        match self {
            Self::Seg { b, .. } => b,
            Self::Arc { c, r, from, sweep } => on_circle(c, r, from + sweep),
        }
    }

    /// The same piece run the other way.
    pub(crate) fn reversed(self) -> Self {
        match self {
            Self::Seg { a, b } => Self::Seg { a: b, b: a },
            Self::Arc { c, r, from, sweep } => Self::Arc { c, r, from: from + sweep, sweep: -sweep },
        }
    }

    /// Its length.
    pub(crate) fn length(self) -> f64 {
        match self {
            Self::Seg { a, b } => apart(a, b),
            Self::Arc { r, sweep, .. } => r * sweep.abs(),
        }
    }

    /// The point a fraction `f` of the way along it, and the unit normal on its left there.
    pub(crate) fn at(self, f: f64) -> (P2, P2) {
        match self {
            Self::Seg { a, b } => {
                let d = sub(b, a);
                let n = scale([-d[1], d[0]], 1.0 / norm(d));
                (add(a, scale(d, f)), n)
            }
            Self::Arc { c, r, from, sweep } => {
                let angle = from + sweep * f;
                let p = on_circle(c, r, angle);
                let inward = scale(sub(c, p), 1.0 / r);
                (p, if sweep > 0.0 { inward } else { scale(inward, -1.0) })
            }
        }
    }

    /// Whether the direction `angle` from an arc's centre falls within it.
    fn spans(self, angle: f64) -> bool {
        match self {
            Self::Seg { .. } => false,
            Self::Arc { from, sweep, .. } => (sweep.signum() * (angle - from)).rem_euclid(TAU) <= sweep.abs(),
        }
    }

    /// The distance from `q` to it.
    pub(crate) fn distance(self, q: P2) -> f64 {
        match self {
            Self::Seg { a, b } => {
                let d = sub(b, a);
                let t = dot(sub(q, a), d) / dot(d, d);
                let t = if t > 1.0 {
                    1.0
                } else if t > 0.0 {
                    t
                } else {
                    0.0
                };
                apart(q, add(a, scale(d, t)))
            }
            Self::Arc { c, r, .. } => {
                let w = sub(q, c);
                if self.spans(atan2(w[1], w[0])) {
                    (norm(w) - r).abs()
                } else {
                    least(apart(q, self.start()), apart(q, self.end()))
                }
            }
        }
    }

    /// Points along it, its start first and its end last, close enough that the chords stray from it by at most `sag`.
    pub(crate) fn samples(self, sag: f64) -> Vec<P2> {
        match self {
            Self::Seg { a, b } => vec![a, b],
            Self::Arc { c, r, from, sweep } => {
                // 1 - cos x <= x^2 / 2, so steps of sqrt(8 sag / r) stray by at most `sag`.
                let step = (8.0 * sag / r).sqrt();
                let mut n: u32 = 8;
                while n < MOST_STEPS && f64::from(n) * step < sweep.abs() {
                    n = n.saturating_mul(2);
                }
                (0..=n).map(|k| on_circle(c, r, from + sweep * (f64::from(k) / f64::from(n)))).collect()
            }
        }
    }
}

/// Twice the signed area of `q`, `a`, `b`, from `[qu, qv, au, av, bu, bv]`: above zero when `b` is anticlockwise of `a`
/// seen from `q`.
struct Turn;

impl Predicate<6> for Turn {
    const DEGREE: u32 = 2;

    fn value<T: Ring>(&self, inputs: &[T; 6]) -> T {
        let [qu, qv, au, av, bu, bv] = inputs;
        let (du, dv) = (au.sub(qu), av.sub(qv));
        let (eu, ev) = (bu.sub(qu), bv.sub(qv));
        du.mul(&ev).sub(&dv.mul(&eu))
    }
}

/// Which side of the line through `q` and `a` the point `b` is on, decided exactly on these numbers.
fn side(q: P2, a: P2, b: P2) -> Option<Side> {
    let mut out = Vec::new();
    decide_stored(&Turn, &[[q[0], q[1], a[0], a[1], b[0], b[1]]], 0.0, &mut out);
    match out.first() {
        Some(Verdict::Decided(side)) => Some(*side),
        Some(Verdict::Undecided) | None => None,
    }
}

/// Whether `x` and `y` are certainly on opposite sides.
fn opposite(x: Option<Side>, y: Option<Side>) -> bool {
    matches!((x, y), (Some(Side::Above), Some(Side::Below)) | (Some(Side::Below), Some(Side::Above)))
}

/// Where the segments `a` to `b` and `c` to `d` cross, each strictly between its ends, if they do.
#[expect(clippy::many_single_char_names, reason = "the segments' ends, as geometry names them")]
fn segments_cross(a: P2, b: P2, c: P2, d: P2) -> Option<P2> {
    if !(opposite(side(a, b, c), side(a, b, d)) && opposite(side(c, d, a), side(c, d, b))) {
        return None;
    }
    let t = cross(sub(c, a), sub(d, c)) / cross(sub(b, a), sub(d, c));
    Some(add(a, scale(sub(b, a), t)))
}

/// Where the segment from `a` to `b` meets the circle about `c` of radius `r`, each place within the segment.
#[expect(clippy::many_single_char_names, reason = "the segment and the circle, as geometry names them")]
fn segment_circle(a: P2, b: P2, c: P2, r: f64) -> Vec<P2> {
    let length = apart(a, b);
    let d = scale(sub(b, a), 1.0 / length);
    let along = dot(sub(c, a), d);
    let h = cross(d, sub(c, a));
    if h.abs() > r {
        return Vec::new();
    }
    let half = (r * r - h * h).sqrt();
    let within = |s: f64| (0.0..=length).contains(&s);
    [along - half, along + half].into_iter().filter(|&s| within(s)).map(|s| add(a, scale(d, s))).collect()
}

/// Where the circles about `c1` and `c2`, of radii `r1` and `r2`, meet.
fn circles_meet(c1: P2, r1: f64, c2: P2, r2: f64) -> Vec<P2> {
    let gap = apart(c1, c2);
    if gap > r1 + r2 || gap < (r1 - r2).abs() || gap <= 0.0 {
        return Vec::new();
    }
    let u = scale(sub(c2, c1), 1.0 / gap);
    let a = (r1 * r1 - r2 * r2 + gap * gap) / (2.0 * gap);
    let h = (r1 * r1 - a * a).abs().sqrt();
    let foot = add(c1, scale(u, a));
    let across = [-u[1] * h, u[0] * h];
    vec![add(foot, across), sub(foot, across)]
}

/// Whether the arc `arc` reaches the point `p` of its circle.
fn reaches(arc: Flat, p: P2) -> bool {
    match arc {
        Flat::Seg { .. } => true,
        Flat::Arc { c, .. } => {
            let w = sub(p, c);
            arc.spans(atan2(w[1], w[0]))
        }
    }
}

/// Where the pieces `x` and `y` cross or meet, each place on both.
fn crossings(x: Flat, y: Flat) -> Vec<P2> {
    let found = match (x, y) {
        (Flat::Seg { a, b }, Flat::Seg { a: c, b: d }) => segments_cross(a, b, c, d).into_iter().collect(),
        (Flat::Seg { a, b }, Flat::Arc { c, r, .. }) | (Flat::Arc { c, r, .. }, Flat::Seg { a, b }) => {
            segment_circle(a, b, c, r)
        }
        (Flat::Arc { c: c1, r: r1, .. }, Flat::Arc { c: c2, r: r2, .. }) => circles_meet(c1, r1, c2, r2),
    };
    found.into_iter().filter(|&p| reaches(x, p) && reaches(y, p)).collect()
}

/// The points of the arc `arc`, if it is one, where its normal runs along `direction`: its points on the line through its
/// centre along `direction`, within it.
fn square_to(arc: Flat, direction: P2) -> Vec<P2> {
    let Flat::Arc { c, r, .. } = arc else { return Vec::new() };
    let length = norm(direction);
    if length <= 0.0 {
        return Vec::new();
    }
    let u = scale(direction, r / length);
    [add(c, u), sub(c, u)].into_iter().filter(|&p| reaches(arc, p)).collect()
}

/// The points of `x` and `y` where their normals are shared, each on the piece it is from, with that piece's index.
fn shared_normals(x: Flat, y: Flat) -> Vec<(P2, bool)> {
    let direction = |piece: Flat, other: Flat| match (piece, other) {
        (Flat::Arc { .. }, Flat::Seg { a, b }) => {
            let d = sub(b, a);
            [-d[1], d[0]]
        }
        (Flat::Arc { c, .. }, Flat::Arc { c: other, .. }) => sub(other, c),
        (Flat::Seg { .. }, _) => [0.0, 0.0],
    };
    let on_x = square_to(x, direction(x, y)).into_iter().map(|p| (p, true));
    on_x.chain(square_to(y, direction(y, x)).into_iter().map(|p| (p, false))).collect()
}

/// How close `x` and `y` come, and a place on `x` or `y` where they do.
pub(crate) fn closest(x: Flat, y: Flat) -> (f64, P2) {
    let mut best = crossings(x, y).first().map_or((f64::INFINITY, x.start()), |&p| (0.0, p));
    let mut keep = |d: f64, p: P2| {
        if d < best.0 {
            best = (d, p);
        }
    };
    for end in [x.start(), x.end()] {
        keep(y.distance(end), end);
    }
    for end in [y.start(), y.end()] {
        keep(x.distance(end), end);
    }
    for (p, on_x) in shared_normals(x, y) {
        keep(if on_x { y.distance(p) } else { x.distance(p) }, p);
    }
    best
}

/// Where `x` and `y`, which meet end to end at each point of `shared`, also meet or touch elsewhere: a place where they
/// cross away from every shared point, or an end of one, not a shared point, within `band` of the other.
pub(crate) fn meet_elsewhere(x: Flat, y: Flat, shared: &[P2], band: f64) -> Option<P2> {
    let away = |p: P2| shared.iter().all(|&s| apart(p, s) > 4.0 * band);
    if let Some(&p) = crossings(x, y).iter().find(|&&p| away(p)) {
        return Some(p);
    }
    let x_ends = [x.start(), x.end()].into_iter().filter(|&e| away(e) && y.distance(e) <= band);
    let y_ends = [y.start(), y.end()].into_iter().filter(|&e| away(e) && x.distance(e) <= band);
    x_ends.chain(y_ends).next()
}

/// The closed chain of segments through `pieces`, its arcs cut so their chords stray by at most `sag`; each piece begins
/// where the one before it ends, and the last ends where the first begins.
fn chain(pieces: &[Flat], sag: f64) -> Vec<[UvPoint; 2]> {
    let mut points: Vec<P2> = Vec::new();
    for (k, piece) in pieces.iter().enumerate() {
        let samples = piece.samples(sag);
        points.extend(samples.into_iter().skip(usize::from(k > 0)));
    }
    if let Some(&first) = points.first() {
        points.pop();
        points.push(first);
    }
    let uv = |p: &P2| UvPoint::new(p[0], p[1]);
    points.windows(2).filter_map(|pair| Some([uv(pair.first()?), uv(pair.get(1)?)])).collect()
}

/// The winding number of the closed loop `pieces` about `q`, which is further than `clear` from it.
pub(crate) fn winding(pieces: &[Flat], q: P2, clear: f64) -> Option<i32> {
    occam_topo::winding_number(&chain(pieces, clear / 4.0), UvPoint::new(q[0], q[1]))
}

/// How far `q` is from every piece of every loop of `face`.
fn clearance(face: &[Vec<Flat>], q: P2) -> f64 {
    face.iter().flatten().fold(f64::INFINITY, |so_far, piece| least(so_far, piece.distance(q)))
}

/// Whether `q`, further than `clear` from every loop of `face`, is a witness for its loop `k`: strictly inside it, and,
/// for the outer loop, outside every hole.
fn witnesses(face: &[Vec<Flat>], k: usize, q: P2, clear: f64) -> bool {
    let inside = |pieces: &[Flat]| winding(pieces, q, clear).is_some_and(|w| w != 0);
    let Some(own) = face.get(k) else { return false };
    inside(own) && (k > 0 || face.iter().skip(1).all(|hole| winding(hole, q, clear) == Some(0)))
}

/// A witness for loop `k` of `face` (the outer loop first): a point strictly inside it and, for the outer loop, outside
/// every hole, further than `clear` from every loop. The clearest of the points a quarter, a half and three quarters of
/// the way along each piece, moved off it either way by a half, an eighth and a thirty-second of its length.
pub(crate) fn witness(face: &[Vec<Flat>], k: usize, clear: f64) -> Option<(P2, f64)> {
    let mut best: Option<(P2, f64)> = None;
    for piece in face.get(k).into_iter().flatten() {
        for f in [0.5, 0.25, 0.75] {
            let (p, n) = piece.at(f);
            for d in [0.5, 0.125, 0.031_25].map(|share| share * piece.length()) {
                for q in [add(p, scale(n, d)), sub(p, scale(n, d))] {
                    let room = clearance(face, q);
                    let better = best.is_none_or(|(_, so_far)| room > so_far);
                    if room > clear && better && witnesses(face, k, q, room) {
                        best = Some((q, room));
                    }
                }
            }
        }
    }
    best
}

/// Whether the loop `inner`, which neither touches nor crosses `outer`, lies inside it: whether its start does.
pub(crate) fn inside(outer: &[Flat], inner: &[Flat]) -> bool {
    let Some(q) = inner.first().map(|piece| piece.start()) else { return false };
    let room = outer.iter().fold(f64::INFINITY, |so_far, piece| least(so_far, piece.distance(q)));
    winding(outer, q, room).is_some_and(|w| w != 0)
}

/// Twice the signed area the closed loop `pieces` bounds, anticlockwise positive, measured from its first point.
pub(crate) fn area(pieces: &[Flat], sag: f64) -> f64 {
    let segments = chain(pieces, sag);
    let Some(origin) = segments.first().map(|[a, _]| [a.u(), a.v()]) else { return 0.0 };
    let at = |p: UvPoint| sub([p.u(), p.v()], origin);
    segments.iter().map(|&[a, b]| cross(at(a), at(b))).sum()
}

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

/// The first two pieces of `runs`, each a run of pieces and whether it comes round, that touch or cross within `band`
/// other than where consecutive pieces of one run meet end to end: run by run, piece by piece.
pub(crate) fn first_touch(runs: &[(&[Flat], bool)], band: f64) -> Option<Touch> {
    for (r, &(pieces, closed)) in runs.iter().enumerate() {
        for (i, &x) in pieces.iter().enumerate() {
            for (j, &y) in pieces.iter().enumerate().skip(i.saturating_add(1)) {
                let meets = shared(pieces, i, j, closed);
                let at = if meets.is_empty() {
                    Some(closest(x, y)).filter(|&(d, _)| d <= band).map(|(_, p)| p)
                } else {
                    meet_elsewhere(x, y, &meets, band)
                };
                if let Some(at) = at {
                    return Some(Touch { first: (r, i), second: (r, j), at });
                }
            }
            for (s, &(others, _)) in runs.iter().enumerate().skip(r.saturating_add(1)) {
                for (j, &y) in others.iter().enumerate() {
                    if let Some((_, at)) = Some(closest(x, y)).filter(|&(d, _)| d <= band) {
                        return Some(Touch { first: (r, i), second: (s, j), at });
                    }
                }
            }
        }
    }
    None
}

#[cfg(test)]
#[expect(clippy::arithmetic_side_effects, reason = "small indices in tests")]
mod tests {
    use core::f64::consts::{FRAC_PI_2, PI};

    use super::*;

    fn seg(a: P2, b: P2) -> Flat {
        Flat::Seg { a, b }
    }

    /// The anticlockwise circle about `c` of radius `r`, from angle 0.
    fn circle(c: P2, r: f64) -> Flat {
        Flat::Arc { c, r, from: 0.0, sweep: TAU }
    }

    /// The square with corners `lo` and `hi`, anticlockwise.
    fn square(lo: f64, hi: f64) -> Vec<Flat> {
        let corners = [[lo, lo], [hi, lo], [hi, hi], [lo, hi]];
        (0..4).map(|k| seg(corners[k], corners[(k + 1) % 4])).collect()
    }

    fn near(a: P2, b: P2) -> bool {
        apart(a, b) < 1e-12
    }

    #[test]
    fn a_piece_has_ends_a_length_points_along_it_and_a_normal_on_its_left() {
        let s = seg([0.0, 0.0], [4.0, 0.0]);
        assert_eq!((s.start(), s.end(), s.length()), ([0.0, 0.0], [4.0, 0.0], 4.0));
        assert_eq!(s.at(0.25), ([1.0, 0.0], [0.0, 1.0]));
        let quarter = Flat::Arc { c: [0.0, 0.0], r: 2.0, from: 0.0, sweep: FRAC_PI_2 };
        assert!(near(quarter.start(), [2.0, 0.0]) && near(quarter.end(), [0.0, 2.0]));
        assert!((quarter.length() - PI).abs() < 1e-12);
        let (p, n) = quarter.at(0.5);
        assert!(near(p, [2.0_f64.sqrt(), 2.0_f64.sqrt()]) && near(n, scale(p, -0.5)), "inward, on the left");
        let back = quarter.reversed();
        assert!(near(back.start(), [0.0, 2.0]) && near(back.end(), [2.0, 0.0]));
        assert!(near(back.at(0.5).1, scale(p, 0.5)), "clockwise, the left is outward");
        assert_eq!(seg([0.0, 0.0], [1.0, 2.0]).reversed(), seg([1.0, 2.0], [0.0, 0.0]));
    }

    #[test]
    fn the_distance_to_a_piece_is_to_its_nearest_point() {
        let s = seg([0.0, 0.0], [4.0, 0.0]);
        assert_eq!((s.distance([2.0, 3.0]), s.distance([-3.0, 4.0]), s.distance([7.0, 4.0])), (3.0, 5.0, 5.0));
        let quarter = Flat::Arc { c: [0.0, 0.0], r: 2.0, from: 0.0, sweep: FRAC_PI_2 };
        assert!((quarter.distance([3.0, 3.0]) - (18.0_f64.sqrt() - 2.0)).abs() < 1e-12, "square to the arc");
        assert!((quarter.distance([0.0, -1.0]) - 5.0_f64.sqrt()).abs() < 1e-12, "beyond it, to its start");
        let clockwise = Flat::Arc { c: [0.0, 0.0], r: 2.0, from: 0.0, sweep: -FRAC_PI_2 };
        assert!((clockwise.distance([0.0, -3.0]) - 1.0).abs() < 1e-12, "a clockwise arc spans below");
        assert!((clockwise.distance([0.0, 3.0]) - 13.0_f64.sqrt()).abs() < 1e-12);
    }

    #[test]
    fn an_arc_is_cut_finely_enough_that_its_chords_stray_no_further_than_asked() {
        let arc = Flat::Arc { c: [1.0, 1.0], r: 10.0, from: 0.3, sweep: 2.0 };
        for sag in [1.0, 1e-2, 1e-4] {
            let points = arc.samples(sag);
            assert!(near(points[0], arc.start()) && near(*points.last().unwrap(), arc.end()));
            let worst = points
                .windows(2)
                .map(|w| arc.distance(scale(add(w[0], w[1]), 0.5)))
                .fold(0.0, |so_far, d| if d > so_far { d } else { so_far });
            assert!(worst <= sag, "{sag}: {worst}");
        }
        assert_eq!(arc.samples(1e3).len(), 9, "never fewer than eight steps");
        assert_eq!(seg([0.0, 0.0], [1.0, 0.0]).samples(1e-9), [[0.0, 0.0], [1.0, 0.0]]);
    }

    #[test]
    fn segments_cross_only_strictly_between_their_ends() {
        let cross_at = segments_cross([0.0, 0.0], [2.0, 2.0], [0.0, 2.0], [2.0, 0.0]).unwrap();
        assert!(near(cross_at, [1.0, 1.0]));
        assert_eq!(segments_cross([0.0, 0.0], [2.0, 0.0], [1.0, 0.0], [1.0, 2.0]), None, "an end on the other");
        assert_eq!(segments_cross([0.0, 0.0], [2.0, 0.0], [0.0, 1.0], [2.0, 1.0]), None, "parallel");
        assert_eq!(segments_cross([0.0, 0.0], [2.0, 0.0], [3.0, -1.0], [3.0, 1.0]), None, "beyond the end");
    }

    #[test]
    fn the_closest_two_pieces_come_is_found_wherever_it_is() {
        let s = seg([0.0, 0.0], [4.0, 0.0]);
        assert_eq!(closest(s, seg([1.0, -1.0], [1.0, 1.0])).0, 0.0, "crossing");
        assert_eq!(
            closest(s, seg([5.0, 1.0], [9.0, 1.0])),
            (2.0_f64.sqrt(), [4.0, 0.0]),
            "end to end, the first's end"
        );
        let (d, p) = closest(s, circle([2.0, 3.0], 1.0));
        assert!((d - 2.0).abs() < 1e-12 && near(p, [2.0, 2.0]), "the arc's point square to the segment: {d} {p:?}");
        let (d, _) = closest(circle([0.0, 0.0], 1.0), circle([5.0, 0.0], 2.0));
        assert!((d - 2.0).abs() < 1e-12, "on the line through the centres: {d}");
        assert_eq!(closest(circle([0.0, 0.0], 1.0), circle([1.0, 0.0], 1.0)).0, 0.0, "circles that meet");
        let (d, _) = closest(circle([0.0, 0.0], 3.0), circle([0.0, 0.0], 1.0));
        assert!((d - 2.0).abs() < 1e-12, "one inside the other: {d}");
        assert_eq!(closest(s, circle([2.0, 0.0], 1.0)).0, 0.0, "a segment through a circle");
        let upper = Flat::Arc { c: [2.0, 0.0], r: 1.0, from: 0.0, sweep: PI };
        assert!(
            (closest(seg([0.0, -2.0], [4.0, -2.0]), upper).0 - 2.0).abs() < 1e-12,
            "the arc's ends, not its far side"
        );
    }

    #[test]
    fn pieces_that_meet_end_to_end_meet_elsewhere_only_if_they_cross_or_fold_back() {
        let band = 1e-7;
        let (a, b) = (seg([0.0, 0.0], [2.0, 0.0]), seg([2.0, 0.0], [2.0, 2.0]));
        assert_eq!(meet_elsewhere(a, b, &[[2.0, 0.0]], band), None, "a corner");
        let back = seg([2.0, 0.0], [1.0, 0.0]);
        assert_eq!(meet_elsewhere(a, back, &[[2.0, 0.0]], band), Some([1.0, 0.0]), "folding back on itself");
        let over = seg([2.0, 0.0], [-1.0, 0.0]);
        assert_eq!(meet_elsewhere(a, over, &[[2.0, 0.0]], band), Some([0.0, 0.0]), "folding back past its start");
        let half = Flat::Arc { c: [1.0, 0.0], r: 1.0, from: 0.0, sweep: PI };
        assert_eq!(
            meet_elsewhere(seg([0.0, 0.0], [2.0, 0.0]), half, &[[2.0, 0.0], [0.0, 0.0]], band),
            None,
            "a half disc"
        );
        let away = Flat::Arc { c: [1.0, 1.0], r: 2.0_f64.sqrt(), from: -FRAC_PI_2 / 2.0, sweep: PI };
        assert_eq!(meet_elsewhere(a, away, &[[2.0, 0.0]], band), None, "an arc turning away");
        let back = Flat::Arc { c: [1.0, 1.0], r: 2.0_f64.sqrt(), from: -FRAC_PI_2 / 2.0, sweep: -PI };
        let met = meet_elsewhere(a, back, &[[2.0, 0.0]], band).unwrap();
        assert!(near(met, [0.0, 0.0]), "an arc back across the segment's start: {met:?}");
    }

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

    #[test]
    fn a_point_winds_once_inside_a_loop_of_arcs_and_not_outside() {
        let disc = vec![circle([0.0, 0.0], 1.0)];
        assert_eq!(winding(&disc, [0.0, 0.0], 1.0), Some(1));
        assert_eq!(winding(&disc, [0.9, 0.0], 0.1), Some(1), "near the edge, with the arc cut finely");
        assert_eq!(winding(&disc, [2.0, 0.0], 1.0), Some(0));
        let backwards: Vec<Flat> = disc.iter().map(|p| p.reversed()).collect();
        assert_eq!(winding(&backwards, [0.0, 0.0], 1.0), Some(-1));
        assert!(area(&disc, 1e-3) > 0.0 && area(&backwards, 1e-3) < 0.0);
        assert!((area(&square(0.0, 2.0), 1.0) - 8.0).abs() < 1e-12, "twice the area");
    }

    #[test]
    fn a_witness_is_strictly_inside_its_loop_outside_every_hole_and_clear_of_them_all() {
        let ell: Vec<Flat> = {
            let corners = [[0.0, 0.0], [4.0, 0.0], [4.0, 1.0], [1.0, 1.0], [1.0, 4.0], [0.0, 4.0]];
            (0..6).map(|k| seg(corners[k], corners[(k + 1) % 6])).collect()
        };
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
    }
}
