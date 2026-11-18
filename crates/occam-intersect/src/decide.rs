//! Source: original
//! Every decision intersection makes, through occam-math's predicates (S2, O11): a computed value, carrying its
//! certified radius, against the band, so that it is counted in a tally, and one whose enclosure reaches across an edge
//! of the band is undecided, reported and never guessed. `Ring` has no square root, so a length is formed as an
//! [`Interval`] and decided as a predicate of one input, its value, or a polynomial is decided against the band scaled
//! by its first-order size.
//!
//! [`Decide`] is the trace's: whether a trace closes, whether the surfaces touch or cross, whether a parameter is on or
//! past an end of its range, whether an edge is a seam or a pole, and whether a projection moved the trace on. The
//! numbers come from evaluation through the seam, which bounds none of its own rounding, so each is taken as it is, and
//! the enclosure is the interval arithmetic of the decision's own formula; one undecided ends the trace with what it was
//! on, and where.
//!
//! [`Decisions`] is the closed forms' and the pencil method's: a length against the length band or an angle against the
//! angle band, each undecided one reported as `decide.undecided`, with the value, its radius and the band.

use occam_math::{Interval, Predicate, Ring, Side, Tally, Verdict, decide_computed};
use occam_report::{Code, Fault, NamedValue, Quantity, Report};
use occam_units::{Length, Point, Tolerance, Vec3};

use crate::interval3::{cross3, dot3, exact3, minus, norm3};
use crate::trace::Uncertain;

/// A length or an angle already formed in interval arithmetic: the predicate's value is its input.
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
            Some(Verdict::Undecided) | None => Err(uncertain(length, at)),
        }
    }

    /// Where each of `lengths` lies against the band, in order, all counted in the tally: `None` for one whose
    /// enclosure reaches across an edge of the band. They are decided together, four lanes at a time.
    pub(crate) fn sides(&mut self, lengths: &[Interval]) -> Vec<Option<Side>> {
        let inputs: Vec<[Interval; 1]> = lengths.iter().map(|length| [*length]).collect();
        self.verdicts.clear();
        self.tally = self.tally.merged(decide_computed(&Formed, &inputs, self.band, &mut self.verdicts));
        let side = |verdict: &Verdict| match verdict {
            Verdict::Decided(side) => Some(*side),
            Verdict::Undecided => None,
        };
        self.verdicts.iter().map(side).collect()
    }
}

/// The decision on `length` at `at` that the band could not make.
pub(crate) fn uncertain(length: Interval, at: Vec3) -> Uncertain {
    Uncertain {
        at: Point::from_vec3(at),
        length: Length::from_millimetres(length.mid()),
        radius: Length::from_millimetres(length.radius()),
    }
}

/// The report of a decision the band could not make: `decide.undecided`, fault `numerically-sensitive`, with the length
/// decided on, its radius, the band, where, and the tolerance in force. It offers no remedy.
pub(crate) fn undecided(uncertain: &Uncertain, band: f64) -> Report {
    let v = uncertain.at.to_vec3();
    Report::new(Code::Undecided, Fault::NumericallySensitive)
        .with_value(NamedValue::length("quantity", uncertain.length.millimetres()))
        .with_value(NamedValue::length("radius", uncertain.radius.millimetres()))
        .with_value(NamedValue::length("band", band))
        .with_value(NamedValue::point("at", [v.x, v.y, v.z]))
        .with_tolerance_in_force(band)
}

/// The distance from `a` to `b`.
pub(crate) fn distance(a: Vec3, b: Vec3) -> Interval {
    norm3(minus(exact3(a), exact3(b)))
}

/// How far `to` is ahead of `from` along the unit vector `along`.
pub(crate) fn ahead(from: Vec3, to: Vec3, along: Vec3) -> Interval {
    dot3(minus(exact3(to), exact3(from)), exact3(along))
}

/// The gap a touch of radius `longest` has where the unit normals of its two surfaces are `normals` apart:
/// `longest |n x m|^2 / 2`, its distance from the touch being `longest |n x m|`. It is within the band where the
/// surfaces cannot be told to cross at the trace's scale, and so touch.
pub(crate) fn touch(longest: f64, (n, m): (Vec3, Vec3)) -> Interval {
    let cross = cross3(exact3(n), exact3(m));
    Interval::exact(longest) * Interval::exact(0.5) * dot3(cross, cross)
}

/// How far a parameter at `t` is past `end`, the end of its range above the other where `upward`, in the length a
/// unit of it moves the point, `rate`: below the band inside the range, within it on the end.
pub(crate) fn past(t: f64, end: f64, rate: f64, upward: bool) -> Interval {
    let beyond =
        if upward { Interval::exact(t) - Interval::exact(end) } else { Interval::exact(end) - Interval::exact(t) };
    beyond * Interval::exact(rate)
}

/// What a decision compares: a length with the length band, or the sine of an angle with the angle band.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Measure {
    Length,
    Angle,
}

/// That a decision could not be made: its report is in [`Decisions::failures`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) struct Undecided;

/// The decisions a closed form made, and the reports of those the band could not make.
pub(crate) struct Decisions {
    tol: Tolerance,
    /// How every decision went.
    pub(crate) tally: Tally,
    /// A `decide.undecided` report for each decision left unmade.
    pub(crate) failures: Vec<Report>,
}

impl Decisions {
    pub(crate) const fn new(tol: Tolerance) -> Self {
        Self { tol, tally: Tally { decided: 0, exact: 0, undecided: 0 }, failures: Vec::new() }
    }

    /// The length band.
    pub(crate) fn band(&self) -> f64 {
        self.tol.band().millimetres()
    }

    /// Where the length `value` lies against the length band.
    ///
    /// # Errors
    ///
    /// [`Undecided`] where the band cannot tell, with a report in [`Decisions::failures`].
    pub(crate) fn length(&mut self, value: Interval) -> Result<Side, Undecided> {
        self.side(value, Measure::Length)
    }

    /// Where the sine `value` of an angle lies against the angle band.
    ///
    /// # Errors
    ///
    /// As [`Decisions::length`].
    pub(crate) fn angle(&mut self, value: Interval) -> Result<Side, Undecided> {
        self.side(value, Measure::Angle)
    }

    fn side(&mut self, value: Interval, measure: Measure) -> Result<Side, Undecided> {
        self.polynomial(&Formed, [value], 1.0, measure)
    }

    /// Where a length lies against the length band, as the value of `predicate` at `inputs`, certified intervals of
    /// the numbers it is a polynomial in, divided by `scale`: a quadric's value at a point over the size of its
    /// gradient there is the point's distance from it to first order.
    ///
    /// # Errors
    ///
    /// As [`Decisions::length`].
    pub(crate) fn length_of<const N: usize, P: Predicate<N>>(
        &mut self,
        predicate: &P,
        inputs: [Interval; N],
        scale: f64,
    ) -> Result<Side, Undecided> {
        self.polynomial(predicate, inputs, scale, Measure::Length)
    }

    /// Where an angle lies against the angle band, as [`Decisions::length_of`] gives a length.
    ///
    /// # Errors
    ///
    /// As [`Decisions::length`].
    pub(crate) fn angle_of<const N: usize, P: Predicate<N>>(
        &mut self,
        predicate: &P,
        inputs: [Interval; N],
        scale: f64,
    ) -> Result<Side, Undecided> {
        self.polynomial(predicate, inputs, scale, Measure::Angle)
    }

    /// `predicate`'s value at `inputs` against the band of `measure` times `scale`, through `decide_computed`.
    fn polynomial<const N: usize, P: Predicate<N>>(
        &mut self,
        predicate: &P,
        inputs: [Interval; N],
        scale: f64,
        measure: Measure,
    ) -> Result<Side, Undecided> {
        let band = match measure {
            Measure::Length => self.tol.band().millimetres(),
            Measure::Angle => self.tol.angle().radians(),
        };
        let mut verdicts = Vec::with_capacity(1);
        self.tally = self.tally.merged(decide_computed(predicate, &[inputs], band * scale, &mut verdicts));
        if let Some(Verdict::Decided(side)) = verdicts.first() {
            return Ok(*side);
        }
        let value = predicate.value(&inputs);
        let quantity = if measure == Measure::Length { Quantity::Length } else { Quantity::Angle };
        self.failures.push(
            Report::new(Code::Undecided, Fault::NumericallySensitive)
                .with_value(NamedValue::new("quantity", quantity, value.mid() / scale))
                .with_value(NamedValue::new("radius", quantity, value.radius() / scale))
                .with_value(NamedValue::new("band", quantity, band)),
        );
        Err(Undecided)
    }
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
    fn an_undecided_scaled_value_is_reported_as_the_length_it_stands_for() {
        let tol = Tolerance::new(Length::from_millimetres(BAND), occam_units::Angle::from_radians(1e-9)).unwrap();
        let mut decisions = Decisions::new(tol);
        // A value of twice the band, scaled by two, is the band itself, with half its radius.
        let got = decisions.length_of(&Formed, [Interval::new(2.0 * BAND, 1e-12)], 2.0);
        assert_eq!(got, Err(Undecided));
        let [report] = decisions.failures.as_slice() else { panic!("{:?}", decisions.failures) };
        let value = |name: &str| report.value(name).and_then(occam_report::NamedValue::number);
        assert_eq!((value("quantity"), value("radius"), value("band")), (Some(BAND), Some(5e-13), Some(BAND)));
        assert_eq!(decisions.length_of(&Formed, [Interval::new(BAND, 1e-12)], 2.0), Ok(Side::Within));
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
