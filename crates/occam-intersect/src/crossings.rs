//! Source: original
//! Where the curves through a leaf of the hybrid's subdivision cross its edges, so that a leaf holding more than one
//! curve gives a start on each, not one alone (P5). Newton's method from a leaf's middle settles onto one curve, and a
//! second through the same leaves, as the two lines a plane cuts a ridge's steep sides in where the crest lies inside
//! them, would otherwise be found by none of them.
//!
//! The gap between the surfaces is read signed, along the implicit form's gradient or the other surface's normal at
//! the foot, at the leaf's corners, with its rate along the part's parameters; the leaf's own gaps there are read from
//! the same points. Along each edge the gap is taken to have at most one extremum, as a smooth gap, near a quadratic
//! over a leaf, and a B-spline's crease across it have. Then an edge whose ends' gaps differ in sign crosses one curve,
//! and one whose ends agree crosses two where the gap turns back towards nought from both ends and is past it between
//! them. Every curve through the leaf crosses its edges twice, a closed loop inside it aside, so a leaf whose edges are
//! crossed more than twice holds more than one curve, and gives back each crossing. Settling them is the hybrid's.
//!
//! An edge where the gap turns back is probed towards where its rate along the edge changes sign, at the turn a rate
//! changing evenly would give, as a smooth gap's nearly does, else halfway, until a point past nought is found; or what
//! is left is too short for the tube to tell two crossings in it apart; or the box over it is nearer than the gap is
//! to nought, as the gap changes no faster than its point moves. A crossing is found by Newton's
//! method along the edge, kept between ends whose gaps differ in sign, to well within the band. Edges are searched only
//! while more than two crossings could still be found, so a leaf one curve crosses costs little more than its corners.
//!
//! Signs here are of the numbers as computed, not decisions: they choose where starts are looked for, as Newton's
//! method from a leaf's middle does, and every start is decided against the band as the tube holds it or a trace runs.

use occam_geom::{Domain, ParamRange, UvBox};
use occam_units::{UvPoint, Vec3};

use crate::correct::{Here, Other, Pair, foot};
use crate::pieces::diagonal;

/// The most probes along an edge looking for where the gap turns back across nought, and the most steps taken to where
/// it crosses: a piece this many times halved is a part in `2^48` of its edge, near the precision of its parameters.
const HALVINGS: u32 = 48;

/// How near nought, as a part of the band, a crossing is halved to: within the quarter of it where settling takes a
/// point as on both surfaces and moves it no further, with room for the rounding of a foot, so that a crossing on an
/// edge of the domain stays inside it.
const CLOSE: f64 = 0.125;

/// The gap between the surfaces at a point of the part of `p` a leaf holds, signed, with what it was read from.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Gap {
    /// The gap: the implicit form's value, or the distance along `q`'s unit normal at the point's foot on it.
    pub value: f64,
    /// The unit normal it is read along: the implicit form's gradient, or `q`'s normal at the foot.
    normal: Vec3,
    /// `p`'s derivatives at the point.
    rates: [Vec3; 2],
    /// The point, its parameters on `p`, and on `q` its foot's where `q` is held by its parameters, else the seed's.
    pub here: Here,
    /// `q`'s point at the foot where `q` is held by its parameters, else the point.
    pub on_q: Vec3,
}

impl Gap {
    /// The gap at `uv` on `pair`'s `p`, its point's foot on `q` found from `seed` where `q` is held by its parameters.
    /// Its numbers may not be finite, as on an implicit form's axis or where `q` has no normal.
    pub(crate) fn at(pair: &Pair<'_>, uv: UvPoint, seed: UvPoint) -> Self {
        let jet = pair.p.first(uv);
        let at = jet.point.to_vec3();
        let (value, normal, q, on_q) = match pair.other {
            Other::Implicit(form) => {
                let (value, gradient) = form.at(at);
                (value, gradient, seed, at)
            }
            Other::Parametric => {
                let q = foot(pair.q, at, seed);
                let jet = pair.q.first(q);
                let (on_q, cross) = (jet.point.to_vec3(), jet.du.rate().cross(jet.dv.rate()));
                let normal = cross * (1.0 / cross.norm());
                ((at - on_q).dot(normal), normal, q, on_q)
            }
        };
        Self { value, normal, rates: [jet.du.rate(), jet.dv.rate()], here: Here { at, p: uv, q }, on_q }
    }

    /// Whether its value and how fast it changes along `p`'s parameters are finite.
    fn finite(&self) -> bool {
        let [du, dv] = self.rates;
        [self.value, self.normal.dot(du), self.normal.dot(dv)].iter().all(|x| x.is_finite())
    }

    /// Whether the gap is above nought.
    fn above(&self) -> bool {
        self.value > 0.0
    }

    /// How far, to first order, `p`'s point moves along `way`, a change of its parameters.
    fn along(&self, way: (f64, f64)) -> Vec3 {
        let [du, dv] = self.rates;
        du * way.0 + dv * way.1
    }

    /// The sine of the angle between `p`'s normal and the normal the gap is read along.
    fn sine(&self) -> f64 {
        let [du, dv] = self.rates;
        let across = du.cross(dv);
        across.cross(self.normal).norm() / across.norm()
    }

    /// How fast the gap changes along `way`, a change of `p`'s parameters: the normal's part of the move.
    fn slope(&self, way: (f64, f64)) -> f64 {
        self.normal.dot(self.along(way))
    }

    /// Whether a move along `way` takes the gap towards nought: its slope has the sign the gap lacks.
    fn towards(&self, way: (f64, f64)) -> bool {
        (self.slope(way) > 0.0) != self.above()
    }
}

/// The gap `part` of the way from `lo` to `hi` along `p`'s parameters, its foot found from `lo`'s, where its numbers
/// are finite.
fn part_way(pair: &Pair<'_>, (lo, hi): (&Gap, &Gap), part: f64) -> Option<Gap> {
    let (a, b) = (lo.here.p, hi.here.p);
    let uv = UvPoint::new(a.u() + (b.u() - a.u()) * part, a.v() + (b.v() - a.v()) * part);
    Some(Gap::at(pair, uv, lo.here.q)).filter(Gap::finite)
}

/// The change of `p`'s parameters from `lo` to `hi`.
fn between(lo: &Gap, hi: &Gap) -> (f64, f64) {
    (hi.here.p.u() - lo.here.p.u(), hi.here.p.v() - lo.here.p.v())
}

/// Whether the piece of an edge from `lo` to `hi`, neither past nought, is too short for two crossings inside it to be
/// told apart: to first order from either end, within the band over the sine between the surfaces, as the tube tells
/// curves apart ([`tube`](crate::tube)), each the larger of the two ends'. Where the gap turns back between them and is
/// past nought anywhere, it is so only strictly between them, so the crossings either side would be one to the band;
/// where it turns back as a smooth gap or a crease does, the surfaces meet there at a smaller angle than at its ends.
fn within(band: f64, lo: &Gap, hi: &Gap) -> bool {
    let way = between(lo, hi);
    let larger = |a: f64, b: f64| if a > b { a } else { b };
    larger(lo.along(way).norm(), hi.along(way).norm()) * larger(lo.sine(), hi.sine()) <= band
}

/// Whether the gaps at `from` and `to` agree in sign and the gap goes towards nought from each into the edge between
/// them, so that it turns back between them and may cross nought twice.
fn turns(from: &Gap, to: &Gap) -> bool {
    let way = between(from, to);
    from.above() == to.above() && from.towards(way) && !to.towards(way)
}

/// How many times at most the gap crosses nought along the edge from `from` to `to`: once where their gaps differ in
/// sign, twice where it turns back between them, else not.
fn most(from: &Gap, to: &Gap) -> usize {
    if from.above() != to.above() {
        1
    } else if turns(from, to) {
        2
    } else {
        0
    }
}

/// The parameters within `wide` of `at` each way, inside `domain`.
fn around(at: UvPoint, wide: f64, domain: Domain) -> UvBox {
    let side = |x: f64, range: ParamRange| {
        let (from, to) = (x - wide, x + wide);
        (if from < range.lo { range.lo } else { from }, if to > range.hi { range.hi } else { to })
    };
    let (u, v) = (side(at.u(), domain.u), side(at.v(), domain.v));
    UvBox::new(UvPoint::new(u.0, v.0), UvPoint::new(u.1, v.1))
}

/// Whether the gap cannot reach nought between `lo` and `hi`, on an edge along one of `p`'s parameters: it changes no
/// faster than its point moves, each gap here being a distance or changing no faster than one (P8), and every point of
/// `p` between them lies in `p`'s box over the parameters [`around`] `lo` within the piece's length, which shrinks with
/// the piece both along the edge and across it. So the gap keeps the side of nought it has at `lo` where it is further
/// from nought than the box is across.
fn clear(pair: &Pair<'_>, lo: &Gap, hi: &Gap) -> bool {
    let (du, dv) = between(lo, hi);
    let wide = if du.abs() > dv.abs() { du.abs() } else { dv.abs() };
    diagonal(&pair.p.bounds(around(lo.here.p, wide, pair.p.domain()))) < lo.value.abs()
}

/// Where between `lo` and `hi`, as a part of the way, the gap's rate along `way` is nought if it changes evenly between
/// them, as a smooth gap's nearly does: its turn. Kept to the middle half of the way, so that each probe takes at least
/// a quarter off what is left, and halfway where it is not there or not a number.
fn turn(lo: &Gap, hi: &Gap, way: (f64, f64)) -> f64 {
    let (from, to) = (lo.slope(way), hi.slope(way));
    let part = from / (from - to);
    if part > 0.25 && part < 0.75 { part } else { 0.5 }
}

/// The pairs of points along the edge from `from` to `to`, each either side of where the gap crosses nought: the ends
/// where their gaps differ in sign; where it turns back between them, the two either side of the first point found past
/// nought in at most `probes` probes, each at the [`turn`] of what is left, if one is found before what is left is
/// [`within`] the tube's reach or [`clear`] of nought; none otherwise.
fn brackets(pair: &Pair<'_>, from: Gap, to: Gap, probes: u32) -> Vec<[Gap; 2]> {
    if from.above() != to.above() {
        return vec![[from, to]];
    }
    if !turns(&from, &to) {
        return Vec::new();
    }
    let (way, mut lo, mut hi) = (between(&from, &to), from, to);
    for _ in 0..probes {
        if within(pair.band(), &lo, &hi) || clear(pair, &lo, &hi) {
            break;
        }
        let Some(mid) = part_way(pair, (&lo, &hi), turn(&lo, &hi, way)) else { break };
        if mid.above() != from.above() {
            return vec![[lo, mid], [mid, hi]];
        }
        if mid.towards(way) {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    Vec::new()
}

/// The point between `ends`, whose gaps differ in sign, where the gap crosses nought, in at most `steps` steps: each
/// of Newton's method along the edge from the end nearer nought where it stays in the half of what is left on that
/// end's side, so that a step can neither leave the ends nor creep along to the other, or else halfway; the new point
/// takes the place of the end its gap agrees with, until its gap is within [`CLOSE`] of the band. Else the first end
/// of what is left.
fn crossing(pair: &Pair<'_>, [mut lo, mut hi]: [Gap; 2], steps: u32) -> Here {
    for _ in 0..steps {
        let way = between(&lo, &hi);
        let (near, from, inward) = if lo.value.abs() < hi.value.abs() { (&lo, 0.0, 1.0) } else { (&hi, 1.0, -1.0) };
        let step = -near.value / near.slope(way) * inward;
        let part = if step > 0.0 && step < 0.5 { from + step * inward } else { 0.5 };
        let Some(mid) = part_way(pair, (&lo, &hi), part) else { break };
        if mid.value.abs() <= pair.band() * CLOSE {
            return mid.here;
        }
        if mid.above() == lo.above() {
            lo = mid;
        } else {
            hi = mid;
        }
    }
    lo.here
}

/// Where the curves cross the edges of a leaf's part of `pair`'s `p`, whose corners' gaps are `corners`, in the order
/// [`UvBox::corners`] gives them, where more than one curve does: each crossing, in order round the edges.
/// None where they are crossed twice or less, or where a corner's gap is not finite. An edge is searched only while the
/// crossings found and the most the edges left could add come to more than two, so that a leaf one curve at most
/// crosses costs little more than its corners.
pub(crate) fn crossings(pair: &Pair<'_>, corners: &[Gap; 4]) -> Vec<Here> {
    let [c0, c1, c2, c3] = *corners;
    let round = [c0, c1, c3, c2];
    if !round.iter().all(Gap::finite) {
        return Vec::new();
    }
    let edges: Vec<(&Gap, &Gap)> = round.iter().zip(round.iter().cycle().skip(1)).collect();
    let mut left: usize = edges.iter().map(|(from, to)| most(from, to)).sum();
    let mut found: Vec<[Gap; 2]> = Vec::new();
    for (from, to) in edges {
        if found.len().saturating_add(left) <= 2 {
            return Vec::new();
        }
        left = left.saturating_sub(most(from, to));
        found.extend(brackets(pair, *from, *to, HALVINGS));
    }
    if found.len() <= 2 {
        return Vec::new();
    }
    found.into_iter().map(|ends| crossing(pair, ends, HALVINGS)).collect()
}

#[cfg(test)]
pub(crate) mod tests {
    use occam_geom::{BsplineSurface, Cylinder, Plane, SplineSurface, Surface, UvBox};
    use occam_units::{Point, UvPoint, Vec3};

    use super::*;
    use crate::trace_tests::{BAND, ball, level, mm, moved, tol};

    fn patch(u: (f64, f64), v: (f64, f64)) -> UvBox {
        UvBox::new(UvPoint::new(u.0, v.0), UvPoint::new(u.1, v.1))
    }

    impl Gap {
        /// This gap said to be `value`, read from where it was.
        pub(crate) fn said(self, value: f64) -> Self {
            Self { value, ..self }
        }
    }

    /// The bilinear B-spline over `v` from -1 to 1 whose height runs straight between `(u, z)` of `points` in turn, the
    /// same at every `v`: its parameters are `x` and `y`.
    pub(crate) fn profile(points: &[(f64, f64)]) -> SplineSurface {
        let corner = |y: f64| points.iter().map(move |&(x, z)| Point::from_vec3(Vec3::new(x, y, z)));
        let us: Vec<f64> = points.iter().map(|&(x, _)| x).collect();
        let mut knots_u = vec![us[0]];
        knots_u.extend(&us);
        knots_u.extend(us.last());
        SplineSurface::new(&BsplineSurface {
            degree: (1, 1),
            count: (u32::try_from(points.len()).unwrap(), 2),
            knots_u,
            knots_v: vec![-1.0, -1.0, 1.0, 1.0],
            poles: corner(-1.0).chain(corner(1.0)).collect(),
            weights: None,
        })
        .unwrap()
    }

    /// A ridge with its crest at `x = 0.3`, its sides falling 10 a unit, and the plane cutting it in two lines `apart`
    /// apart: below the crest by `5 apart`.
    fn ridge(apart: f64) -> (Plane, SplineSurface) {
        (level(-5.0 * apart), profile(&[(-1.0, -13.0), (0.3, 0.0), (1.0, -7.0)]))
    }

    /// The gap on `pair` at `(u, v)`.
    fn gap_at(pair: &Pair<'_>, u: f64, v: f64) -> Gap {
        Gap::at(pair, UvPoint::new(u, v), UvPoint::new(0.0, 0.0))
    }

    /// How fast `gap` changes along `p`'s parameters.
    fn rate(gap: &Gap) -> [f64; 2] {
        gap.rates.map(|along| gap.normal.dot(along))
    }

    /// The crossings of the leaf of `patch` on `pair`.
    fn of(pair: &Pair<'_>, patch: UvBox) -> Vec<Here> {
        crossings(pair, &patch.corners().map(|uv| Gap::at(pair, uv, UvPoint::new(0.0, 0.0))))
    }

    /// Where along x from the crest each of `found` is, in order.
    fn xs(found: &[Here]) -> Vec<f64> {
        let mut out: Vec<f64> = found.iter().map(|here| here.at.x - 0.3).collect();
        out.sort_by(f64::total_cmp);
        out
    }

    #[test]
    fn the_gap_is_signed_and_its_rate_is_along_the_parts_parameters_on_either_hold() {
        // 0.2 left of the crest the ridge is 2 down, rising 10 a unit of x towards the plane cutting it 5e-3 down, and
        // level along y; on a B-spline level with that plane until it rises far off, the foot is straight above.
        let (plane, ridge) = ridge(1e-3);
        let flat = profile(&[(-2.0, -5e-3), (1.5, -5e-3), (2.0, 1.0)]);
        let (implicit, parametric) = (Pair::new(&ridge, &plane, tol()), Pair::new(&ridge, &flat, tol()));
        for pair in [&implicit, &parametric] {
            let gap = gap_at(pair, 0.1, 0.5);
            assert!((gap.value + 1.995).abs() < 1e-12 && !gap.above(), "{gap:?}");
            let [along_u, along_v] = rate(&gap);
            assert!((along_u - 10.0).abs() < 1e-9 && along_v.abs() < 1e-9 && gap.finite(), "{gap:?}");
            assert!(gap.here.p == UvPoint::new(0.1, 0.5) && (gap.here.at - Vec3::new(0.1, 0.5, -2.0)).norm() < 1e-12);
            assert_eq!(gap.along((0.5, 2.0)), gap.rates[0] * 0.5 + gap.rates[1] * 2.0);
        }
        assert!(matches!((implicit.other, parametric.other), (Other::Implicit(_), Other::Parametric)));
        let (foot, on_q) = (gap_at(&parametric, 0.1, 0.5).here.q, gap_at(&parametric, 0.1, 0.5).on_q);
        assert!((on_q - Vec3::new(0.1, 0.5, -5e-3)).norm() < 1e-12 && on_q == flat.first(foot).point.to_vec3());
        let held = gap_at(&implicit, 0.1, 0.5);
        assert_eq!((held.here.q, held.on_q), (UvPoint::new(0.0, 0.0), held.here.at), "held implicitly: the seed's");
        // Against a sheet tilted to a kink, whose derivatives' cross product is not a unit long, the gap is the distance
        // to the foot, signed.
        let tilted = profile(&[(-2.0, 0.0), (1.5, 1.75), (2.0, 3.0)]);
        let gap = gap_at(&Pair::new(&ridge, &tilted, tol()), 0.1, 0.5);
        assert!(gap.value < 0.0 && (gap.value.abs() - (gap.here.at - gap.on_q).norm()).abs() < 1e-12, "{gap:?}");
        // Nought is not above it, and a move along the rate is towards nought from below, away from it from above.
        let on = Gap { value: 0.0, ..gap_at(&implicit, 0.1, 0.5) };
        assert!(!on.above());
        assert!(gap_at(&implicit, 0.1, 0.5).towards((1.0, 0.0)) && !gap_at(&implicit, 0.1, 0.5).towards((-1.0, 0.0)));
        let low = level(-3.0);
        let high = Pair::new(&ridge, &low, tol());
        assert!(gap_at(&high, 0.1, 0.5).towards((-1.0, 0.0)) && !gap_at(&high, 0.1, 0.5).towards((1.0, 0.0)));
    }

    #[test]
    fn a_gap_on_an_implicit_forms_axis_is_not_finite() {
        // The plane z = 0 through the axis of a cylinder about Z: at the axis the form has no gradient.
        let (plane, tube) = (level(0.0), Cylinder::placed(&moved([0.0; 3]), mm(0.5)).unwrap());
        let pair = Pair::new(&plane, &tube, tol());
        assert!(!gap_at(&pair, 0.0, 0.0).finite() && gap_at(&pair, 0.1, 0.0).finite());
        let gap = gap_at(&pair, 0.1, 0.0);
        let bad = |value: f64, normal: Vec3| !Gap { value, normal, ..gap }.finite();
        assert!(bad(f64::NAN, gap.normal) && bad(gap.value, Vec3::new(f64::INFINITY, 0.0, 0.0)));
    }

    #[test]
    fn an_edge_crosses_once_where_its_ends_differ_twice_where_the_gap_turns_back_across_nought_and_else_not() {
        let (plane, ridge) = ridge(1e-3);
        let pair = Pair::new(&ridge, &plane, tol());
        let count = |a: f64, b: f64| brackets(&pair, gap_at(&pair, a, -1.0), gap_at(&pair, b, -1.0), HALVINGS).len();
        // Across one line, across both with the crest between, either way, and along one side.
        assert_eq!((count(0.29, 0.2998), count(0.29, 0.31), count(0.31, 0.29), count(0.2, 0.25)), (1, 2, 2, 0));
        // Over the crest of a ridge that stays below the plane, the gap turns back towards nought and never reaches it.
        let (above, ridge) = (level(1e-3), profile(&[(-1.0, -13.0), (0.3, 0.0), (1.0, -7.0)]));
        let pair = Pair::new(&ridge, &above, tol());
        assert_eq!(brackets(&pair, gap_at(&pair, 0.29, -1.0), gap_at(&pair, 0.31, -1.0), HALVINGS).len(), 0);
        // A valley, then a ridge above the plane: from 0.1 the gap turns back towards nought and crosses it at 0.4,
        // but from -0.8 it goes away from nought into the edge, so the edge is taken to cross nothing.
        let wave = profile(&[(-1.0, -1.0), (0.0, -2.0), (0.5, 1.0), (1.0, -1.0)]);
        let flat = level(0.0);
        let pair = Pair::new(&wave, &flat, tol());
        assert_eq!(brackets(&pair, gap_at(&pair, -0.8, 0.0), gap_at(&pair, 1.0, 0.0), HALVINGS).len(), 0);
        assert_eq!(brackets(&pair, gap_at(&pair, 0.1, 0.0), gap_at(&pair, 1.0, 0.0), HALVINGS).len(), 2);
    }

    #[test]
    fn a_smooth_dip_is_probed_at_its_turn_and_a_crease_halfway() {
        // A sphere of radius 10 about (0.3, 0, 10 - 1e-3), held by its implicit form, dips 1e-3 below the plane z = 0
        // within 0.14 of x = 0.3. Along x from 0 to 1 the gap's rate is -0.03 and 0.07, nought at 0.3 if it changed
        // evenly: one probe there finds the gap past nought, where one halfway, at 0.5, would not.
        let (plane, sphere) = (level(0.0), ball(10.0, [0.3, 0.0, 10.0 - 1e-3]));
        let pair = Pair::new(&plane, &sphere, tol());
        let edge = |a: f64, b: f64| (gap_at(&pair, a, 0.0), gap_at(&pair, b, 0.0));
        let (from, to) = edge(0.0, 1.0);
        assert!((turn(&from, &to, between(&from, &to)) - 0.3).abs() < 1e-3);
        let found = brackets(&pair, from, to, 1);
        assert_eq!(found.len(), 2);
        assert!((found[0][1].here.p.u() - 0.3).abs() < 1e-3, "{:?}", found[0][1].here.p);
        assert!(gap_at(&pair, 0.5, 0.0).above(), "halfway is not past nought");
        // A turn outside the middle half of the way is taken halfway.
        for (a, b) in [(0.0, 0.35), (0.25, 1.0)] {
            let (from, to) = edge(a, b);
            assert_eq!(turn(&from, &to, between(&from, &to)), 0.5, "{a} {b}");
        }
        // Along the ridge, whose sides fall alike, the turn is halfway, wherever the crest is.
        let (plane, ridge) = ridge(1e-3);
        let pair = Pair::new(&ridge, &plane, tol());
        let (from, to) = (gap_at(&pair, 0.29, 0.0), gap_at(&pair, 0.312_3, 0.0));
        assert!((turn(&from, &to, between(&from, &to)) - 0.5).abs() < 1e-12);
    }

    #[test]
    fn halving_stops_where_two_crossings_would_be_one_to_the_band() {
        // Lines 1e-9 apart about the crest, 1e-8 apart along the ridge's side: halving stops once what is left is
        // within the band across, before it comes between them; lines 1e-6 apart are found.
        let count = |apart: f64| {
            let (plane, ridge) = ridge(apart);
            let pair = Pair::new(&ridge, &plane, tol());
            brackets(&pair, gap_at(&pair, 0.29, 0.0), gap_at(&pair, 0.312_3, 0.0), HALVINGS).len()
        };
        assert_eq!((count(1e-9), count(1e-6)), (0, 2));
        let (plane, ridge) = ridge(1e-3);
        let pair = Pair::new(&ridge, &plane, tol());
        let near = |u: f64| (gap_at(&pair, 0.29, 0.0), gap_at(&pair, u, 0.0));
        // Across, 101 times the step squared, to first order from either end.
        // Across, its side moving the root of 101 a unit of x and its normal 10 over that root from the plane's: within
        // the band over that sine, 1e-8 of x.
        let (lo, hi) = near(0.29 + 0.99e-8);
        assert!(within(BAND, &lo, &hi) && within(BAND, &hi, &lo));
        let (lo, hi) = near(0.29 + 1.01e-8);
        assert!(!within(BAND, &lo, &hi) && !within(BAND, &hi, &lo));
        assert!((lo.sine() - 10.0 / 101.0_f64.sqrt()).abs() < 1e-12, "{}", lo.sine());
        // Across a kink from a side level with the plane to a steep one, the steep end's reach and sine count, from
        // either end; on the level side alone, the surfaces are tangent, and nothing is told apart.
        let (kinked, low) = (profile(&[(-1.0, 0.0), (0.3, 0.0), (1.0, -7.0)]), level(-1.0));
        let pair = Pair::new(&kinked, &low, tol());
        let across = |half: f64| (gap_at(&pair, 0.3 - half, 0.0), gap_at(&pair, 0.3 + half, 0.0));
        let ((lo, hi), (near_lo, near_hi)) = (across(2.5e-8), across(0.45e-8));
        assert!(!within(BAND, &lo, &hi) && !within(BAND, &hi, &lo));
        assert!(within(BAND, &near_lo, &near_hi) && within(BAND, &near_hi, &near_lo));
        assert!(lo.sine() == 0.0 && within(BAND, &lo, &gap_at(&pair, -0.5, 0.0)), "level with the plane");
    }

    #[test]
    fn the_parameters_around_a_point_are_as_wide_each_way_inside_the_domain() {
        let range = ParamRange { lo: -1.0, hi: 1.0, wrap: occam_geom::Wrap::Open, kind: occam_units::ParamKind::Knot };
        let domain = Domain { u: range, v: range };
        let got = |u: f64, v: f64| around(UvPoint::new(u, v), 0.125, domain);
        assert_eq!(got(0.5, 0.0), patch((0.375, 0.625), (-0.125, 0.125)));
        assert_eq!(got(0.9375, -0.9375), patch((0.8125, 1.0), (-1.0, -0.8125)));
        assert_eq!(got(-0.9375, 0.9375), patch((-1.0, -0.8125), (0.8125, 1.0)));
    }

    #[test]
    fn a_piece_whose_box_is_nearer_than_its_gap_is_clear_even_on_the_domains_edge() {
        // 0.2 down the ridge's side the gap is nearly 1; a piece 0.01 of x long, its box widened as much along y, is
        // about 0.1 across, the side falling 10 a unit. One about the crest, its gap 0.095 at 0.29, is not.
        let (plane, ridge) = ridge(1e-3);
        let pair = Pair::new(&ridge, &plane, tol());
        let piece = |a: f64, b: f64, v: f64| clear(&pair, &gap_at(&pair, a, v), &gap_at(&pair, b, v));
        assert!(piece(0.1, 0.11, 0.0) && piece(0.11, 0.1, 0.0), "either way");
        assert!(piece(0.1, 0.11, 1.0) && piece(0.1, 0.11, -1.0), "widened inside the domain at its edges");
        assert!(!piece(0.29, 0.31, 0.0) && !piece(0.1, 0.3, 0.0));
        // Along y the piece's own length widens it along x.
        let along = |v: f64, w: f64| clear(&pair, &gap_at(&pair, 0.1, v), &gap_at(&pair, 0.1, w));
        assert!(along(0.0, 0.01) && along(1.0, 0.99) && !along(-1.0, 1.0));
    }

    #[test]
    fn each_crossing_is_found_within_an_eighth_of_the_band_of_its_line() {
        for apart in [1.5e-7, 1e-5, 1e-3] {
            let (plane, ridge) = ridge(apart);
            let pair = Pair::new(&ridge, &plane, tol());
            let ends = brackets(&pair, gap_at(&pair, 0.29, -0.5), gap_at(&pair, 0.31, -0.5), HALVINGS);
            let got = xs(&ends.into_iter().map(|ends| crossing(&pair, ends, HALVINGS)).collect::<Vec<_>>());
            let near = |x: f64, want: f64| (x - want).abs() <= BAND * CLOSE / 10.0;
            assert!(got.len() == 2 && near(got[0], -apart / 2.0) && near(got[1], apart / 2.0), "{apart}: {got:?}");
        }
    }

    #[test]
    fn a_crossing_on_a_straight_side_takes_one_step_and_one_a_step_would_overshoot_is_halved_first() {
        // Along the ridge's side the gap is straight, so one step of Newton's method from either end lands on the line.
        let (plane, ridge) = ridge(1e-3);
        let pair = Pair::new(&ridge, &plane, tol());
        for (from, to) in [(0.1, 0.2999), (0.2999, 0.1), (0.2, 0.2999), (0.2999, 0.2)] {
            let got = crossing(&pair, [gap_at(&pair, from, 0.0), gap_at(&pair, to, 0.0)], 1);
            assert!((got.at.x - 0.2995).abs() <= BAND * CLOSE / 10.0, "{from} {to}: {got:?}");
        }
        // Over a crest from 0.3 up to 1/3 and on to 0.72: from 0.72, nearer nought, a step would run past the crest's
        // other side to 0.75, so the first step halves the edge, and the second is from 0.3 to 1/3.
        let wave = profile(&[(-1.0, -1.0), (0.0, -2.0), (0.5, 1.0), (1.0, -1.0)]);
        let flat = level(0.0);
        let pair = Pair::new(&wave, &flat, tol());
        let (lo, hi) = (gap_at(&pair, 0.3, 0.0), gap_at(&pair, 0.72, 0.0));
        assert!(hi.value.abs() < lo.value.abs(), "{lo:?} {hi:?}");
        let got = crossing(&pair, [lo, hi], 2);
        assert!((got.at.x - 1.0 / 3.0).abs() < 1e-12, "{got:?}");
        let after_one = crossing(&pair, [lo, hi], 1);
        assert!((after_one.at.x - 0.3).abs() < 1e-12, "what is left after one runs from 0.3 to 0.51");
    }

    #[test]
    fn an_edge_may_be_crossed_once_where_its_ends_differ_twice_where_the_gap_turns_back_and_else_not() {
        let (plane, ridge) = ridge(1e-3);
        let pair = Pair::new(&ridge, &plane, tol());
        let most_of = |a: f64, b: f64| most(&gap_at(&pair, a, 0.0), &gap_at(&pair, b, 0.0));
        assert_eq!((most_of(0.29, 0.2998), most_of(0.29, 0.31), most_of(0.31, 0.29), most_of(0.2, 0.25)), (1, 2, 2, 0));
        assert_eq!((most_of(0.25, 0.2), most_of(0.4, 0.5)), (0, 0));
        // Level with the plane at one end, the gap's slope exactly nought there, and falling away at the other: no turn.
        let (kinked, above) = (profile(&[(-1.0, 0.0), (0.3, 0.0), (1.0, -7.0)]), level(0.5));
        let pair = Pair::new(&kinked, &above, tol());
        assert_eq!(most(&gap_at(&pair, 0.2, 0.0), &gap_at(&pair, 0.4, 0.0)), 0);
    }

    #[test]
    fn a_crossing_that_never_reaches_the_band_is_the_first_end_of_what_is_left() {
        // Ends said to differ in sign, though the gap between them never reaches nought: the second end comes halfway
        // to the first each time.
        let (plane, ridge) = ridge(1e-3);
        let pair = Pair::new(&ridge, &plane, tol());
        let lo = Gap { value: 1.0, ..gap_at(&pair, 0.1, 0.0) };
        let got = crossing(&pair, [lo, gap_at(&pair, 0.2, 0.0)], HALVINGS);
        assert_eq!(got, lo.here);
        let hi = Gap { value: 1.0, ..gap_at(&pair, 0.2, 0.0) };
        let got = crossing(&pair, [gap_at(&pair, 0.1, 0.0), hi], HALVINGS);
        assert!(got.at.x > 0.2 - 1e-12 && got.at.x < 0.2, "{got:?}");
    }

    #[test]
    fn a_leaf_gives_its_crossings_only_where_its_edges_are_crossed_more_than_twice_and_its_corners_are_finite() {
        // The leaf about the crest holds both lines 1e-3 apart, each crossing its two edges along x; the leaf beside it
        // holds the line on its side alone, and one far down a side none.
        let (plane, ridge) = ridge(1e-3);
        let pair = Pair::new(&ridge, &plane, tol());
        let about = patch((0.296_875, 0.312_5), (-1.0, -0.875));
        let got = xs(&of(&pair, about));
        let want = [-5e-4, -5e-4, 5e-4, 5e-4];
        assert!(got.len() == 4 && got.iter().zip(want).all(|(x, w)| (x - w).abs() < 1e-8), "{got:?}");
        assert!(of(&pair, patch((0.296_875, 0.3), (-1.0, -0.875))).is_empty(), "one line");
        assert!(of(&pair, patch((0.5, 0.6), (-1.0, -0.875))).is_empty(), "no line");
        let [c0, c1, c2, c3] = about.corners().map(|uv| Gap::at(&pair, uv, UvPoint::new(0.0, 0.0)));
        assert!(crossings(&pair, &[c0, c1, c2, Gap { value: f64::NAN, ..c3 }]).is_empty(), "a corner not finite");
    }
}
