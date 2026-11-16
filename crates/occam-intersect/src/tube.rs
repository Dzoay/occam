//! Source: original
//! The tube about the curves traced so far (P5): a start point inside it is on a curve already found, and a trace
//! whose stations are all inside it found that curve again. About each chord between two stations it is as wide as
//! twice the chord's sag from its curve, which the circles through neighbouring stations give, since a trace's steps
//! keep to its sag only roughly, and never narrower than that sag; about a touch it is as wide as the square root of the
//! band, which near a tangency is as close as the band places the curve (P5: a loop of radius about `sqrt(eps)` at a
//! gap of `eps`). Whether a point is
//! inside is a length decided against the band, counted, and one the band cannot decide is given back. The chords are
//! kept in a grid of cubes, so that a point is measured against those near it alone; the grid is only looked up, so
//! nothing depends on its order.

use std::collections::BTreeMap;

use occam_units::Vec3;

use crate::decide::{Decide, distance};
use crate::trace::{End, Trace, Tracer, Uncertain};

/// The most cubes along an axis a chord is filed under; a longer one is measured against every point.
const SPAN: f64 = 3.0;

/// A chord between two points and the tube's radius about it.
#[derive(Clone, Copy, Debug)]
struct Chord {
    from: Vec3,
    to: Vec3,
    radius: f64,
}

impl Chord {
    /// The point of the chord nearest `at`, or near it: the end where the chord has no length.
    fn foot(&self, at: Vec3) -> Vec3 {
        let along = self.to - self.from;
        let length = along.dot(along);
        let t = if length > 0.0 { (at - self.from).dot(along) / length } else { 0.0 };
        let t = if t > 1.0 {
            1.0
        } else if t > 0.0 {
            t
        } else {
            0.0
        };
        self.from + along * t
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
        let mut x = x0;
        while x <= x1 {
            let mut y = y0;
            while y <= y1 {
                let mut z = z0;
                while z <= z1 {
                    self.grid.entry([x.to_bits(), y.to_bits(), z.to_bits()]).or_default().push(index);
                    z += 1.0;
                }
                y += 1.0;
            }
            x += 1.0;
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
        let points: Vec<Vec3> = trace.stations.iter().map(|station| station.at.to_vec3()).collect();
        let closed = trace.ends.contains(&End::Closed);
        let chord = |from: Vec3, to: Vec3, radius: f64| Chord { from, to, radius };
        let spans = if closed && points.len() > 1 { points.len() } else { points.len().saturating_sub(1) };
        let mut chords: Vec<Chord> = (0..spans)
            .filter_map(|at| {
                let next = if closed { at.saturating_add(1).checked_rem(points.len())? } else { at.saturating_add(1) };
                Some(chord(*points.get(at)?, *points.get(next)?, self.radius(&points, at, closed)))
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

    /// Whether `at` is inside the tube, decided against the band by `decide`: within it of a chord's radius is inside.
    ///
    /// # Errors
    ///
    /// The first decision the band could not make, where no chord holds `at`.
    pub(crate) fn holds(&self, at: Vec3, decide: &mut Decide) -> Result<bool, Uncertain> {
        let key = [cube(at.x, self.side), cube(at.y, self.side), cube(at.z, self.side)].map(f64::to_bits);
        let near = self.grid.get(&key).map_or(&[][..], Vec::as_slice);
        let mut open = None;
        for chord in near.iter().chain(&self.long).filter_map(|&index| self.chords.get(index)) {
            let gap = distance(at, chord.foot(at)) - occam_math::Interval::exact(chord.radius);
            match decide.side(gap, at) {
                Ok(occam_math::Side::Above) => {}
                Ok(_) => return Ok(true),
                Err(uncertain) => open = open.or(Some(uncertain)),
            }
        }
        open.map_or(Ok(false), Err)
    }
}

#[cfg(test)]
mod tests {
    use occam_math::Tally;
    use occam_units::{Point, UvPoint};

    use super::*;
    use crate::trace::Station;
    use crate::trace_tests::{BAND, tracer};

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

    /// Whether `tube` holds `point`, and how many chords it measured to say so.
    fn holds(tube: &Tube, point: Vec3) -> (bool, u64) {
        let mut decide = Decide::new(BAND);
        let held = tube.holds(point, &mut decide).unwrap();
        (held, decide.tally.decided)
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
            tube.file(Chord { from, to, radius: 1e-4 });
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
        tube.file(Chord { from: at(0.0, 1.0, 1.0), to: at(0.0, 1.05, 1.0), radius: 1e-4 });
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
        let chord = Chord { from: Vec3::new(0.0, 0.0, 0.0), to: Vec3::new(2.0, 0.0, 0.0), radius: 0.1 };
        assert_eq!(chord.foot(Vec3::new(0.5, 1.0, 0.0)), Vec3::new(0.5, 0.0, 0.0));
        assert_eq!(chord.foot(Vec3::new(3.0, 1.0, 0.0)), Vec3::new(2.0, 0.0, 0.0), "past its end");
        assert_eq!(chord.foot(Vec3::new(-1.0, 1.0, 0.0)), Vec3::new(0.0, 0.0, 0.0), "before its start");
        let point = Chord { to: chord.from, ..chord };
        assert_eq!(point.foot(Vec3::new(3.0, 1.0, 0.0)), chord.from, "a chord of no length");
    }
}
