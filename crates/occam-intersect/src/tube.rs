//! Source: original
//! The tube about the curves traced so far (P5): a start point on a curve already found is not traced again, and a
//! trace whose first, middle and last stations are on curves found before it found one of them again. About each chord
//! between two stations it is as wide as twice the chord's sag from its curve, which the circles through neighbouring
//! stations give, since a trace's steps keep to its sag only roughly, and never narrower than that sag; about a touch
//! it is as wide as the square root of the band, which near a tangency is as close as the band places the curve (P5: a
//! loop of radius about `sqrt(eps)` at a gap of `eps`).
//!
//! Its width is not a resolution of its own. A second curve may pass inside it, as the two lines the steep sides of a
//! narrow ridge cut a plane in do, so a point inside the tube about a chord is on the chord's curve only where [`same`]
//! confirms it: the corrector, from the point of the cubic through the chord's stations along the curve's direction at
//! each, onto the plane through the point across its curve, lands on the point, within the band over the sine between
//! the surfaces there, as a trace closes on its start. Two curves the band tells apart are then both traced, however
//! close, and two it cannot are one, as they are to the band. The cubic matters: from a station, along its tangent, the
//! corrector starts as far off the curve as the chord sags, and beside a second curve that near it lands on the second.
//! The cubic strays from the curve by a term of the fourth order in the step, where a station's tangent strays by one of
//! the second, so the landing tells apart curves far closer together than the trace's own steps do. A touch, and a
//! trace's lone station, hold what is within their radius, as the contact rule bounds what is near a touch.
//!
//! Whether a point is inside, and whether a landing is on its mark, are lengths decided against the band, counted, and
//! one the band cannot decide is given back. The chords are kept in a grid of cubes, so that a point is measured
//! against those near it alone; the grid is only looked up, so nothing depends on its order.

use std::collections::BTreeMap;

use occam_math::{Interval, Side};
use occam_units::Vec3;

use crate::correct::{Here, Other, Pair, foot};
use crate::decide::{Decide, distance, dot, exact, less};
use crate::march::{Frame, frame};
use crate::trace::{End, Station, Trace, Tracer, Uncertain};

/// The most cubes along an axis a chord is filed under; a longer one is measured against every point.
const SPAN: f64 = 3.0;

/// A chord between two points and the tube's radius about it.
#[derive(Clone, Copy, Debug)]
struct Chord {
    from: Vec3,
    to: Vec3,
    radius: f64,
    /// The stations it runs between, against which a point inside it is confirmed; none about a touch or a lone station.
    ends: Option<[Station; 2]>,
}

impl Chord {
    /// How far along the chord, from nought at `from` to one at `to`, the point of it nearest `at` is: nought where the
    /// chord has no length.
    fn along(&self, at: Vec3) -> f64 {
        let along = self.to - self.from;
        let length = along.dot(along);
        let t = if length > 0.0 { (at - self.from).dot(along) / length } else { 0.0 };
        if t > 1.0 {
            1.0
        } else if t > 0.0 {
            t
        } else {
            0.0
        }
    }

    /// The point of the chord nearest `at`, or near it: the end where the chord has no length.
    fn foot(&self, at: Vec3) -> Vec3 {
        self.from + (self.to - self.from) * self.along(at)
    }
}

/// The tube about the traces it holds.
pub(crate) struct Tube {
    /// The radius about a chord, the tracer's sag, and no less than the band.
    sag: f64,
    /// The radius about a touch: the square root of the band, and no less than the sag.
    touch: f64,
    /// The side of the grid's cubes.
    side: f64,
    /// The chords.
    chords: Vec<Chord>,
    /// The chords reaching into each cube, by the bits of its lowest corner's coordinates in cubes.
    grid: BTreeMap<[u64; 3], Vec<usize>>,
    /// The chords too long to file, measured against every point.
    long: Vec<usize>,
    /// How many traces it holds.
    pub held: usize,
}

/// The cube coordinate of `x`, for cubes of `side`, with no negative zero.
fn cube(x: f64, side: f64) -> f64 {
    (x / side).floor() + 0.0
}

/// The cubes along an axis from `lo` to `hi`, in order: whole numbers fewer than [`SPAN`] apart, so at most three,
/// counted off a fixed list rather than a counter, so that no change to it can make the walk endless.
fn along(lo: f64, hi: f64) -> impl Iterator<Item = f64> {
    [0.0, 1.0, 2.0].into_iter().map(move |step| lo + step).take_while(move |at| *at <= hi)
}

impl Tube {
    /// An empty tube for the traces of `tracer`.
    pub(crate) fn new(tracer: &Tracer) -> Self {
        let band = tracer.tol.band().millimetres();
        let sag = tracer.sag.millimetres();
        let sag = if sag > band { sag } else { band };
        let touch = band.sqrt();
        let touch = if touch > sag { touch } else { sag };
        let longest = tracer.longest.millimetres();
        let side = (if longest > 0.0 { longest } else { touch }) + 2.0 * touch;
        Self { sag, touch, side, chords: Vec::new(), grid: BTreeMap::new(), long: Vec::new(), held: 0 }
    }

    /// Files `chord` under every cube it reaches into, or as long.
    fn file(&mut self, chord: Chord) {
        let index = self.chords.len();
        self.chords.push(chord);
        let r = chord.radius;
        let ends = |a: f64, b: f64| {
            if a < b {
                (cube(a - r, self.side), cube(b + r, self.side))
            } else {
                (cube(b - r, self.side), cube(a + r, self.side))
            }
        };
        let axes = [ends(chord.from.x, chord.to.x), ends(chord.from.y, chord.to.y), ends(chord.from.z, chord.to.z)];
        if !axes.iter().all(|(lo, hi)| hi - lo < SPAN) {
            self.long.push(index);
            return;
        }
        let [(x0, x1), (y0, y1), (z0, z1)] = axes;
        for x in along(x0, x1) {
            for y in along(y0, y1) {
                for z in along(z0, z1) {
                    self.grid.entry([x.to_bits(), y.to_bits(), z.to_bits()]).or_default().push(index);
                }
            }
        }
    }

    /// The tube's radius about the chord from station `at` to the next of `points`, which come round where `closed`:
    /// twice the chord's sag, `length^2 bend / 8`, the most bend of the circles through it and the station either side
    /// of it, and never less than the tracer's sag. Doubling leaves as much again for a bend that changes along it.
    fn radius(&self, points: &[Vec3], at: usize, closed: bool) -> f64 {
        let count = points.len();
        let point = |offset: isize| {
            let index = at.checked_add_signed(offset)?;
            match (closed, count) {
                (_, 0) => None,
                (true, _) => points.get(index.checked_add(count)?.checked_sub(1)?.checked_rem(count)?),
                (false, _) => points.get(index.checked_sub(1)?),
            }
        };
        let bend = |a: Option<&Vec3>, b: Option<&Vec3>, c: Option<&Vec3>| match (a, b, c) {
            (Some(&a), Some(&b), Some(&c)) => {
                let lengths = (b - a).norm() * (c - b).norm() * (a - c).norm();
                if lengths > 0.0 { 2.0 * (b - a).cross(c - b).norm() / lengths } else { 0.0 }
            }
            _ => 0.0,
        };
        let (before, from, to, after) = (point(0), point(1), point(2), point(3));
        let (first, second) = (bend(before, from, to), bend(from, to, after));
        let length = match (from, to) {
            (Some(&from), Some(&to)) => (to - from).norm(),
            _ => 0.0,
        };
        let sag = length * length * (if first > second { first } else { second }) / 4.0;
        if sag > self.sag { sag } else { self.sag }
    }

    /// Adds `trace`: a chord between each two stations in order, and the one that closes a loop; a point at an end
    /// that is a touch, with the touch's radius; and a trace's only station, if it has one.
    pub(crate) fn add(&mut self, trace: &Trace) {
        self.held = self.held.saturating_add(1);
        let (stations, closed) = (&trace.stations, trace.ends.contains(&End::Closed));
        let points: Vec<Vec3> = stations.iter().map(|station| station.at.to_vec3()).collect();
        let chord = |from: Vec3, to: Vec3, radius: f64| Chord { from, to, radius, ends: None };
        let spans = if closed && points.len() > 1 { points.len() } else { points.len().saturating_sub(1) };
        let mut chords: Vec<Chord> = (0..spans)
            .filter_map(|at| {
                let next = if closed { at.saturating_add(1).checked_rem(points.len())? } else { at.saturating_add(1) };
                let ends = [*stations.get(at)?, *stations.get(next)?];
                let radius = self.radius(&points, at, closed);
                Some(Chord { ends: Some(ends), ..chord(ends[0].at.to_vec3(), ends[1].at.to_vec3(), radius) })
            })
            .collect();
        for (end, at) in trace.ends.iter().zip([points.first(), points.last()]) {
            chords.extend(at.filter(|_| *end == End::Touch).map(|at| chord(*at, *at, self.touch)));
        }
        if let ([only], true) = (points.as_slice(), chords.is_empty()) {
            chords.push(chord(*only, *only, self.sag));
        }
        for chord in chords {
            self.file(chord);
        }
    }

    /// Whether the curve through `at` is one the tube holds, decided against the band by `decide`: `at` is within a
    /// chord's radius of it, and where the chord runs between two stations, `same` confirms that the curve through `at`
    /// is the one through them, given how far along the chord its foot is. About a touch or a lone station, within the
    /// radius is held.
    ///
    /// # Errors
    ///
    /// The first decision the band could not make, where no chord holds `at`.
    pub(crate) fn holds(
        &self,
        at: Vec3,
        decide: &mut Decide,
        mut same: impl FnMut(&[Station; 2], f64, &mut Decide) -> Result<bool, Uncertain>,
    ) -> Result<bool, Uncertain> {
        let key = [cube(at.x, self.side), cube(at.y, self.side), cube(at.z, self.side)].map(f64::to_bits);
        let near = self.grid.get(&key).map_or(&[][..], Vec::as_slice);
        let mut open = None;
        for chord in near.iter().chain(&self.long).filter_map(|&index| self.chords.get(index)) {
            let gap = distance(at, chord.foot(at)) - Interval::exact(chord.radius);
            let held = match decide.side(gap, at) {
                Ok(Side::Above) => Ok(false),
                Ok(_) => chord.ends.map_or(Ok(true), |ends| same(&ends, chord.along(at), decide)),
                Err(uncertain) => Err(uncertain),
            };
            match held {
                Ok(true) => return Ok(true),
                Ok(false) => {}
                Err(uncertain) => open = open.or(Some(uncertain)),
            }
        }
        open.map_or(Ok(false), Err)
    }
}

/// Whether the curve through `at` is the one a trace found through the stations `ends`, on `pair`, within the band:
/// the corrector, from the point `along` of the way along the cubic between them, onto the plane through `at` across
/// its curve, lands as far across the curve from `at` as the band over the sine between the surfaces there, or less.
/// Where `at` or a station has no curve's direction, or the landing finds nothing, it is not.
///
/// # Errors
///
/// A landing the band could not place.
pub(crate) fn same(
    pair: &Pair<'_>,
    at: &Here,
    ends: [Here; 2],
    along: f64,
    decide: &mut Decide,
) -> Result<bool, Uncertain> {
    let (Some(here), Some(start)) = (frame(pair, *at, Vec3::new(0.0, 0.0, 0.0)), between(pair, ends, along)) else {
        return Ok(false);
    };
    let Some(landed) = pair.correct(&start, here.here.at, here.along) else { return Ok(false) };
    Ok(decide.side(across(landed.at, &here), landed.at)? != Side::Above)
}

/// The point `along` of the way along the cubic from `from` to `to` that leaves each along the curve's direction there,
/// with tangents as long as those of the circle through both, so that it strays from a circle by a term of the sixth
/// order in the turn between them; with the parameters of its foot on each surface held by them, from `from`'s. `None`
/// where either station has no curve's direction.
fn between(pair: &Pair<'_>, [from, to]: [Here; 2], along: f64) -> Option<Here> {
    let chord = to.at - from.at;
    let (first, second) = (frame(pair, from, chord)?, frame(pair, to, chord)?);
    let half = f64::midpoint(1.0, first.along.dot(second.along));
    let length = 2.0 * chord.norm() / (1.0 + (if half > 0.0 { half } else { 0.0 }).sqrt());
    let (t, t2, t3) = (along, along * along, along * along * along);
    let at = from.at * (2.0 * t3 - 3.0 * t2 + 1.0)
        + first.along * (length * (t3 - 2.0 * t2 + t))
        + to.at * (3.0 * t2 - 2.0 * t3)
        + second.along * (length * (t3 - t2));
    let q = match pair.other {
        Other::Implicit(_) => from.q,
        Other::Parametric => foot(pair.q, at, from.q),
    };
    Some(Here { at, p: foot(pair.p, at, from.p), q })
}

/// How far `landed` is from the station of `onto` across its curve, times the sine between the surfaces there: the gap
/// that far across makes between them.
fn across(landed: Vec3, onto: &Frame) -> Interval {
    let (off, along) = (less(exact(landed), exact(onto.here.at)), exact(onto.along));
    let ahead = dot(off, along);
    let aside = less(off, along.map(|part| part * ahead));
    let sine = onto.normals.0.cross(onto.normals.1).norm();
    dot(aside, aside).sqrt() * Interval::exact(sine)
}

#[cfg(test)]
mod tests {
    use occam_geom::Torus;
    use occam_math::{Tally, sin_cos};
    use occam_units::{Length, Point, UvPoint};

    use super::*;
    use crate::trace_tests::{BAND, level, mm, moved, point, start_near, tol, tracer};

    /// A tube for a tracer of sag 1e-4 and longest step 0.1, at a band of 1e-7: cubes 0.1 and twice the square root of
    /// the band on a side.
    fn tube() -> Tube {
        Tube::new(&tracer(1e-4, 0.1, 100))
    }

    fn at(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3::new(x, y, z)
    }

    /// A trace through `points` in order, ending as `ends`.
    fn trace(points: &[Vec3], ends: [End; 2]) -> Trace {
        let station =
            |p: &Vec3| Station { at: Point::from_vec3(*p), on_a: UvPoint::new(0.0, 0.0), on_b: UvPoint::new(0.0, 0.0) };
        let stations = points.iter().map(station).collect();
        Trace { stations, crossings: vec![], ends, steps: 0, tally: Tally::default(), undecided: vec![] }
    }

    /// Whether `tube` holds `point` where every chord's curve is confirmed, and how many chords it measured to say so.
    fn holds(tube: &Tube, point: Vec3) -> (bool, u64) {
        let mut decide = Decide::new(BAND);
        let held = tube.holds(point, &mut decide, |_, _, _| Ok(true)).unwrap();
        (held, decide.tally.decided)
    }

    #[test]
    fn the_cubes_along_an_axis_run_from_the_lower_end_to_the_upper_in_order() {
        let cubes = |lo: f64, hi: f64| along(lo, hi).collect::<Vec<f64>>();
        assert_eq!(
            (cubes(-1.0, 1.0), cubes(4.0, 4.0), cubes(9.0, 10.0)),
            (vec![-1.0, 0.0, 1.0], vec![4.0], vec![9.0, 10.0])
        );
    }

    #[test]
    fn a_cube_is_counted_from_nought_and_has_no_negative_zero() {
        assert_eq!((cube(0.75, 0.5), cube(-0.25, 0.5), cube(2.0, 0.5)), (1.0, -1.0, 4.0));
        assert_eq!(cube(-0.0, 0.5).to_bits(), 0.0_f64.to_bits());
    }

    #[test]
    fn a_point_is_measured_only_against_the_chords_near_it_and_the_long_ones() {
        let mut tube = tube();
        // Two chords far from the origin, one across the first cube's edge, and one three cubes long, which is long.
        for (from, to) in [
            (at(1.0, 1.0, 1.0), at(1.05, 1.0, 1.0)),
            (at(1.5, 1.5, 1.5), at(1.55, 1.5, 1.5)),
            (at(0.09, 0.5, 0.5), at(0.11, 0.5, 0.5)),
            (at(0.05, 0.9, 0.9), at(0.32, 0.9, 0.9)),
        ] {
            tube.file(Chord { from, to, radius: 1e-4, ends: None });
        }
        assert_eq!((tube.long.len(), tube.grid.values().flatten().count()), (1, 2 + 2 + 2));
        assert_eq!(holds(&tube, at(5.0, 5.0, 5.0)), (false, 1), "far from all: only the long chord");
        assert_eq!(holds(&tube, at(1.02, 1.0, 1.00005)), (true, 1), "beside the first");
        assert_eq!(
            holds(&tube, at(1.02, 1.0, 1.001)),
            (false, 2),
            "in the first's cube but outside its tube, and the long"
        );
        assert!(holds(&tube, at(0.2, 0.9, 0.90005)).0, "beside the long one");
    }

    #[test]
    fn a_point_on_a_chord_across_nought_is_found_from_a_negative_zero() {
        let mut tube = tube();
        tube.file(Chord { from: at(0.0, 1.0, 1.0), to: at(0.0, 1.05, 1.0), radius: 1e-4, ends: None });
        assert_eq!(holds(&tube, at(-0.0, 1.02, 1.0)), (true, 1));
    }

    #[test]
    fn a_chords_radius_is_twice_its_sag_from_the_circles_either_side_and_no_less_than_the_tracers() {
        let tube = tube();
        // Stations 0.3 apart in angle on a circle of radius 2: each chord sags 2 - 2 cos(0.15), its bend a half.
        let circle: Vec<Vec3> = (0..5)
            .map(|k| {
                let (sin, cos) = occam_math::sin_cos(0.3 * f64::from(k));
                at(2.0 * cos, 2.0 * sin, 0.0)
            })
            .collect();
        let length = (circle[2] - circle[1]).norm();
        let want = length * length * 0.5 / 4.0;
        assert!(
            (tube.radius(&circle, 1, false) - want).abs() < 1e-12 * want,
            "{} {want}",
            tube.radius(&circle, 1, false)
        );
        // The bend before a chord counts where it is the larger, and a station repeated after it bends nothing.
        let turning = [circle[0], circle[1], circle[2], circle[2] + (circle[2] - circle[1])];
        assert!((tube.radius(&turning, 1, false) - want).abs() < 1e-12 * want, "the bend before");
        let repeated = [circle[0], circle[1], circle[2], circle[2]];
        assert!((tube.radius(&repeated, 1, false) - want).abs() < 1e-12 * want, "a station repeated after");
        let straight = [at(0.0, 0.0, 0.0), at(0.1, 0.0, 0.0), at(0.2, 0.0, 0.0), at(0.3, 0.0, 0.0)];
        assert_eq!(tube.radius(&straight, 1, false), 1e-4, "the tracer's sag");
    }

    #[test]
    fn a_touch_at_an_end_is_held_to_the_square_root_of_the_band() {
        let line = [at(0.0, 0.0, 0.0), at(0.05, 0.0, 0.0), at(0.1, 0.0, 0.0)];
        let beyond = |x: f64| at(x, 2e-4, 0.0);
        for (ends, first, last) in [([End::Edge, End::Touch], false, true), ([End::Touch, End::Edge], true, false)] {
            let mut tube = tube();
            tube.add(&trace(&line, ends));
            assert_eq!((holds(&tube, beyond(-1e-4)).0, holds(&tube, beyond(0.1001)).0), (first, last), "{ends:?}");
        }
    }

    #[test]
    fn a_chords_foot_is_the_nearest_point_of_it() {
        let chord = Chord { from: Vec3::new(0.0, 0.0, 0.0), to: Vec3::new(2.0, 0.0, 0.0), radius: 0.1, ends: None };
        assert_eq!(chord.foot(Vec3::new(0.5, 1.0, 0.0)), Vec3::new(0.5, 0.0, 0.0));
        assert_eq!(chord.foot(Vec3::new(3.0, 1.0, 0.0)), Vec3::new(2.0, 0.0, 0.0), "past its end");
        assert_eq!(chord.foot(Vec3::new(-1.0, 1.0, 0.0)), Vec3::new(0.0, 0.0, 0.0), "before its start");
        let point = Chord { to: chord.from, ..chord };
        assert_eq!(point.foot(Vec3::new(3.0, 1.0, 0.0)), chord.from, "a chord of no length");
    }

    #[test]
    fn a_point_inside_a_chord_between_two_stations_is_held_only_where_its_curve_is_confirmed_there() {
        // A line traced through three stations, ending in a touch: a point beside its first chord, four fifths along.
        let line = [at(0.0, 0.0, 0.0), at(0.05, 0.0, 0.0), at(0.1, 0.0, 0.0)];
        let traced = trace(&line, [End::Edge, End::Touch]);
        let mut tube = tube();
        tube.add(&traced);
        let beside = at(0.04, 5e-5, 0.0);
        for answer in [true, false] {
            let (mut asked, mut decide) = (Vec::new(), Decide::new(BAND));
            let held = tube.holds(beside, &mut decide, |ends, along, _| {
                asked.push((*ends, along));
                Ok(answer)
            });
            assert_eq!(held, Ok(answer));
            let [(ends, along)] = asked.as_slice() else { panic!("{asked:?}") };
            assert!(*ends == [traced.stations[0], traced.stations[1]] && (along - 0.8).abs() < 1e-12, "{asked:?}");
        }
        let open = Uncertain {
            at: Point::from_vec3(beside),
            length: Length::from_millimetres(BAND),
            radius: Length::from_millimetres(0.0),
        };
        assert_eq!(tube.holds(beside, &mut Decide::new(BAND), |_, _, _| Err(open)), Err(open), "given back");
        // About the touch, within its radius is held, and nothing is asked.
        let mut asked = 0_u32;
        let near_touch = tube.holds(at(0.1002, 1e-4, 0.0), &mut Decide::new(BAND), |_, _, _| {
            asked = asked.saturating_add(1);
            Ok(false)
        });
        assert_eq!((near_touch, asked), (Ok(true), 0));
    }

    #[test]
    fn a_point_on_a_second_curve_beside_a_found_one_is_told_from_it_by_a_landing_from_the_cubic_through_its_stations() {
        // A torus of major radius 10 and minor 1 about Z, cut near the top of its tube in two circles 0.02 apart, of radii
        // 9.99 and 10.01, at a sine of 0.01. A trace of the inner one has stations a fifth of a radian apart.
        let torus = Torus::placed(&moved([0.0; 3]), mm(10.0), mm(1.0)).unwrap();
        let height = (1.0 - 1e-4_f64).sqrt();
        let plane = level(height);
        let pair = Pair::new(&plane, &torus, tol());
        let on = |radius: f64, angle: f64| {
            let (sin, cos) = sin_cos(angle);
            pair.seed(start_near(&plane, &torus, point(radius * cos, radius * sin, height)))
        };
        let ends = [on(9.99, 0.0), on(9.99, 0.2)];
        let mut decide = Decide::new(BAND);
        assert_eq!(same(&pair, &on(9.99, 0.1), ends, 0.5, &mut decide), Ok(true), "on the inner circle");
        assert_eq!(same(&pair, &on(10.01, 0.1), ends, 0.5, &mut decide), Ok(false), "on the outer");
        assert_eq!((decide.tally.decided, decide.tally.undecided), (2, 0));
        // The cubic's middle is on the inner circle; a station's tangent, a unit along, is 0.05 outside it, and the
        // corrector from there onto the plane across the outer circle lands on the outer, as if it were the same.
        let middle = between(&pair, ends, 0.5).unwrap();
        let off = |at: Vec3| (at.x * at.x + at.y * at.y).sqrt();
        assert!((off(middle.at) - 9.99).abs() < 1e-7, "{}", off(middle.at) - 9.99);
        let outer = frame(&pair, on(10.01, 0.1), Vec3::new(0.0, 0.0, 0.0)).unwrap();
        let landed = |start: &Here| off(pair.correct(start, outer.here.at, outer.along).unwrap().at);
        let (from_cubic, from_station) = (landed(&middle), landed(&ends[0]));
        assert!((from_cubic - 9.99).abs() < 1e-5 && (from_station - 10.01).abs() < 1e-5, "{from_cubic} {from_station}");
    }

    #[test]
    fn the_cubics_point_has_the_parameters_of_its_feet_on_both_surfaces_held_by_them() {
        // Two curved graphs, each held by its parameters, which meet along one curve through a corner of both: a quarter
        // of the way along the cubic between two stations of its trace, the feet on both are as near as the cubic is to
        // the curve.
        let (a, b) = crate::hybrid_tests::crossing_at_a_corner();
        let corner = crate::hybrid_tests::placed(0.0, 0.0, 0.0);
        let trace = tracer(1e-4, 0.05, 1000).trace(&a, &b, start_near(&a, &b, corner));
        let pair = Pair::new(&a, &b, tol());
        let ends = [pair.here(&trace.stations[3]), pair.here(&trace.stations[4])];
        let middle = between(&pair, ends, 0.25).unwrap();
        let off = |surface: &dyn occam_geom::Surface, uv: UvPoint| (surface.point(uv).to_vec3() - middle.at).norm();
        let (along, chord) = ((middle.at - ends[0].at).norm(), (ends[1].at - ends[0].at).norm());
        assert!(off(&a, middle.p) < 1e-7 && off(&b, middle.q) < 1e-7, "{middle:?}");
        assert!((along / chord - 0.25).abs() < 0.01, "{along} of {chord}");
    }
}
