//! Source: original
//! Every decision the closed forms make, through occam-math's predicates (S2): a computed value, carrying its
//! certified radius, against the band of the tolerance, a length's or an angle's. A value whose enclosure reaches
//! across an edge of the band is not guessed at. It is reported as `decide.undecided`, with the value, its radius and
//! the band, and the decision is left unmade (O11).

use occam_math::{Interval, Predicate, Ring, Side, Tally, Verdict, decide_computed};
use occam_report::{Code, Fault, NamedValue, Quantity, Report};
use occam_units::Tolerance;

/// The value itself: a decision on a computed value is where it lies against the band.
struct Value;

impl Predicate<1> for Value {
    const DEGREE: u32 = 1;

    fn value<T: Ring>(&self, [x]: &[T; 1]) -> T {
        x.clone()
    }
}

/// What a decision compares: a length with the length band, or the sine of an angle with the angle band.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Measure {
    Length,
    Angle,
}

/// That a decision could not be made: its report is in [`Decide::failures`].
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(super) struct Undecided;

/// The decisions a closed form made, and the reports of those the band could not make.
pub(super) struct Decide {
    tol: Tolerance,
    /// How every decision went.
    pub(super) tally: Tally,
    /// A `decide.undecided` report for each decision left unmade.
    pub(super) failures: Vec<Report>,
}

impl Decide {
    pub(super) const fn new(tol: Tolerance) -> Self {
        Self { tol, tally: Tally { decided: 0, exact: 0, undecided: 0 }, failures: Vec::new() }
    }

    /// Where the length `value` lies against the length band.
    ///
    /// # Errors
    ///
    /// [`Undecided`] where the band cannot tell, with a report in [`Decide::failures`].
    pub(super) fn length(&mut self, value: Interval) -> Result<Side, Undecided> {
        self.side(value, Measure::Length)
    }

    /// Where the sine `value` of an angle lies against the angle band.
    ///
    /// # Errors
    ///
    /// As [`Decide::length`].
    pub(super) fn angle(&mut self, value: Interval) -> Result<Side, Undecided> {
        self.side(value, Measure::Angle)
    }

    fn side(&mut self, value: Interval, measure: Measure) -> Result<Side, Undecided> {
        let band = match measure {
            Measure::Length => self.tol.band().millimetres(),
            Measure::Angle => self.tol.angle().radians(),
        };
        let mut verdicts = Vec::with_capacity(1);
        self.tally = self.tally.merged(decide_computed(&Value, &[[value]], band, &mut verdicts));
        if let Some(Verdict::Decided(side)) = verdicts.first() {
            return Ok(*side);
        }
        let quantity = if measure == Measure::Length { Quantity::Length } else { Quantity::Angle };
        self.failures.push(
            Report::new(Code::Undecided, Fault::NumericallySensitive)
                .with_value(NamedValue::new("quantity", quantity, value.mid()))
                .with_value(NamedValue::new("radius", quantity, value.radius()))
                .with_value(NamedValue::new("band", quantity, band)),
        );
        Err(Undecided)
    }
}
