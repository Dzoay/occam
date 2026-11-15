//! Source: original
//! The trace's decisions (S2, O11). Each compares a length formed from the trace's numbers with the band, through
//! occam-math's `decide_computed`, so that it is counted in the trace's tally and one whose enclosure reaches across an
//! edge of the band is undecided, reported, and never guessed: whether a trace closes, whether the surfaces touch or
//! cross, whether a parameter is on or past an end of its range, whether an edge is a seam or a pole, and whether a
//! projection moved the trace on. The numbers come from evaluation through the seam, which bounds none of its own
//! rounding, so each is taken as it is, and the enclosure is the interval arithmetic of the decision's own formula: a
//! length that rounding could put on either side of an edge of the band is undecided. `Ring` has no square root, so a
//! length is formed as an [`Interval`] and decided as a predicate of one input, its value.

use occam_math::{Interval, Predicate, Ring, Side, Tally, Verdict, decide_computed};
use occam_units::{Length, Point, Vec3};

use crate::trace::Uncertain;

/// A length already formed in interval arithmetic: the predicate's value is its input.
struct Formed;

impl Predicate<1> for Formed {
    const DEGREE: u32 = 1;

    fn value<T: Ring>(&self, [length]: &[T; 1]) -> T {
        length.clone()
    }
}

/// The trace's decisions as it makes them, against the band, and their tally.
#[derive(Debug)]
pub(crate) struct Decide {
    /// The band.
    band: f64,
    /// The decisions made.
    pub tally: Tally,
    /// The verdict of the last decision.
    verdicts: Vec<Verdict>,
}

impl Decide {
    /// No decision yet, against `band`.
    pub(crate) fn new(band: f64) -> Self {
        Self { band, tally: Tally::default(), verdicts: Vec::with_capacity(1) }
    }

    /// Where `length`, decided at the point `at`, lies against the band, counted in the tally.
    ///
    /// # Errors
    ///
    /// Where its enclosure reaches across an edge of the band: what the decision was on, and where.
    pub(crate) fn side(&mut self, length: Interval, at: Vec3) -> Result<Side, Uncertain> {
        self.verdicts.clear();
        self.tally = self.tally.merged(decide_computed(&Formed, &[[length]], self.band, &mut self.verdicts));
        match self.verdicts.first() {
            Some(Verdict::Decided(side)) => Ok(*side),
            Some(Verdict::Undecided) | None => Err(Uncertain {
                at: Point::from_vec3(at),
                length: Length::from_millimetres(length.mid()),
                radius: Length::from_millimetres(length.radius()),
            }),
        }
    }
}

/// `v` as three exact intervals.
fn exact(v: Vec3) -> [Interval; 3] {
    [Interval::exact(v.x), Interval::exact(v.y), Interval::exact(v.z)]
}

/// `a - b`.
fn less([ax, ay, az]: [Interval; 3], [bx, by, bz]: [Interval; 3]) -> [Interval; 3] {
    [ax - bx, ay - by, az - bz]
}

/// `a . b`.
fn dot([ax, ay, az]: [Interval; 3], [bx, by, bz]: [Interval; 3]) -> Interval {
    ax * bx + ay * by + az * bz
}

/// The distance from `a` to `b`.
pub(crate) fn distance(a: Vec3, b: Vec3) -> Interval {
    let d = less(exact(a), exact(b));
    dot(d, d).sqrt()
}

/// How far `to` is ahead of `from` along the unit vector `along`.
pub(crate) fn ahead(from: Vec3, to: Vec3, along: Vec3) -> Interval {
    dot(less(exact(to), exact(from)), exact(along))
}

/// The gap a touch of radius `longest` has where the unit normals of its two surfaces are `normals` apart:
/// `longest |n x m|^2 / 2`, its distance from the touch being `longest |n x m|`. It is within the band where the
/// surfaces cannot be told to cross at the trace's scale, and so touch.
pub(crate) fn touch(longest: f64, (n, m): (Vec3, Vec3)) -> Interval {
    let ([nx, ny, nz], [mx, my, mz]) = (exact(n), exact(m));
    let cross = [ny * mz - nz * my, nz * mx - nx * mz, nx * my - ny * mx];
    Interval::exact(longest) * Interval::exact(0.5) * dot(cross, cross)
}

/// How far a parameter at `t` is past `end`, the end of its range above the other where `upward`, in the length a
/// unit of it moves the point, `rate`: below the band inside the range, within it on the end.
pub(crate) fn past(t: f64, end: f64, rate: f64, upward: bool) -> Interval {
    let beyond =
        if upward { Interval::exact(t) - Interval::exact(end) } else { Interval::exact(end) - Interval::exact(t) };
    beyond * Interval::exact(rate)
}

#[cfg(test)]
mod tests {
    use super::*;

    const BAND: f64 = 1e-7;

    fn origin() -> Vec3 {
        Vec3::new(0.0, 0.0, 0.0)
    }

    #[test]
    fn each_length_clear_of_an_edge_of_the_band_is_decided_and_counted() {
        let mut decide = Decide::new(BAND);
        let sides: Vec<_> = [-3e-7, -5e-8, 0.0, 5e-8, 3e-7]
            .into_iter()
            .map(|length| decide.side(Interval::new(length, 1e-9), origin()))
            .collect();
        let want = [Side::Below, Side::Within, Side::Within, Side::Within, Side::Above].map(Ok);
        assert_eq!(sides, want);
        assert_eq!(decide.tally, Tally { decided: 5, exact: 0, undecided: 0 });
    }

    #[test]
    fn a_length_whose_enclosure_reaches_across_an_edge_of_the_band_is_undecided() {
        let mut decide = Decide::new(BAND);
        let at = Vec3::new(1.0, 2.0, 3.0);
        for length in [BAND, -BAND, BAND + 5e-10] {
            let got = decide.side(Interval::new(length, 1e-9), at);
            let want = Uncertain {
                at: Point::from_vec3(at),
                length: Length::from_millimetres(length),
                radius: Length::from_millimetres(1e-9),
            };
            assert_eq!(got, Err(want), "{length}");
        }
        assert_eq!(decide.tally, Tally { decided: 0, exact: 0, undecided: 3 });
    }

    #[test]
    fn the_lengths_are_enclosed() {
        let (a, b) = (Vec3::new(1.0, 2.0, 2.0), Vec3::new(0.0, 0.0, 0.0));
        let encloses = |x: Interval, want: f64| x.lower() <= want && want <= x.upper() && x.radius() < 1e-14;
        assert!(encloses(distance(a, b), 3.0));
        assert!(encloses(ahead(b, a, Vec3::new(0.0, 0.6, 0.8)), 2.8));
        let (up, across) = (Vec3::new(0.0, 0.0, 1.0), Vec3::new(0.0, 0.6, 0.8));
        assert!(encloses(touch(2.0, (up, across)), 0.36));
        assert!(encloses(past(3.0, 2.0, 0.5, true), 0.5) && encloses(past(3.0, 2.0, 0.5, false), -0.5));
    }

    #[test]
    fn a_touchs_gap_takes_every_part_of_the_normals_cross_product() {
        let (n, m) = (Vec3::new(0.48, 0.6, 0.64), Vec3::new(0.6, 0.64, 0.48));
        let cross = n.cross(m);
        let (got, want) = (touch(3.0, (n, m)), 1.5 * cross.dot(cross));
        assert!(got.lower() <= want && want <= got.upper() && got.radius() < 1e-15, "{got:?} against {want}");
    }
}
