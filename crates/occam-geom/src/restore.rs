//! Source: original
//! Reading back the numbers a curve or surface's content holds after its family's name, in the order the family pushed
//! them, so that each family can restore itself from what was saved (PARITY 2.17). A family's `restore` reads its own
//! numbers with this and refuses what none of its constructors would have made, so a file is trusted no more than a
//! request is.

use occam_report::{Code, Fault, NamedValue, Quantity, Report};
use occam_units::Vec3;

use crate::bspline::{bezier_knots, wrong_count};
use crate::domain::{ParamRange, Wrap};
use crate::knots::canonical;

/// The number a range's wrap is written as.
pub(crate) const fn wrap_code(wrap: Wrap) -> f64 {
    match wrap {
        Wrap::Open => 0.0,
        Wrap::Closed => 1.0,
        Wrap::Periodic => 2.0,
    }
}

/// The numbers after a family's name, read in order.
pub(crate) struct Numbers<'a> {
    all: &'a [f64],
    read: usize,
}

impl<'a> Numbers<'a> {
    /// Ready to read `all` from the first.
    pub(crate) const fn new(all: &'a [f64]) -> Self {
        Self { all, read: 0 }
    }

    /// The next `n` numbers.
    ///
    /// # Errors
    ///
    /// `geom.wrong-count` if fewer are left, with how many were given and how many reading needed.
    pub(crate) fn take(&mut self, n: usize) -> Result<&'a [f64], Report> {
        let end = self.read.saturating_add(n);
        let taken = self.all.get(self.read..end).ok_or_else(|| wrong_count(self.all.len(), end))?;
        self.read = end;
        Ok(taken)
    }

    /// The next number.
    ///
    /// # Errors
    ///
    /// As [`Numbers::take`].
    pub(crate) fn next(&mut self) -> Result<f64, Report> {
        self.take(1).map(|taken| taken.first().copied().unwrap_or(f64::NAN))
    }

    /// The next three, `x` first.
    ///
    /// # Errors
    ///
    /// As [`Numbers::take`].
    pub(crate) fn vec3(&mut self) -> Result<Vec3, Report> {
        Ok(Vec3::new(self.next()?, self.next()?, self.next()?))
    }

    /// How many are left to read.
    pub(crate) const fn left(&self) -> usize {
        self.all.len().saturating_sub(self.read)
    }

    /// A flag, written as 0 or 1.
    ///
    /// # Errors
    ///
    /// `geom.out-of-range` for any other number.
    pub(crate) fn flag(&mut self) -> Result<bool, Report> {
        match self.next()? {
            0.0 => Ok(false),
            1.0 => Ok(true),
            other => Err(out_of_range(other, 1.0)),
        }
    }

    /// A count, written as a whole number from 0 to `most`.
    ///
    /// # Errors
    ///
    /// `geom.out-of-range` for a number that is not one.
    pub(crate) fn count(&mut self, most: u32) -> Result<usize, Report> {
        let x = self.next()?;
        if !((0.0..=f64::from(most)).contains(&x) && x.fract() == 0.0) {
            return Err(out_of_range(x, f64::from(most)));
        }
        #[expect(
            clippy::cast_possible_truncation,
            clippy::cast_sign_loss,
            reason = "a whole number within a u32's range"
        )]
        let n = x as u32;
        usize::try_from(n).map_err(|_| out_of_range(x, f64::from(most)))
    }

    /// A range's two ends, and then its wrap if `wrapped`, restored as `whole` trimmed to them: `whole` itself where
    /// they are its own.
    ///
    /// # Errors
    ///
    /// As [`ParamRange::trimmed`], and `geom.out-of-range` for a wrap that trimming would not give.
    pub(crate) fn range(&mut self, whole: ParamRange, wrapped: bool) -> Result<ParamRange, Report> {
        let (lo, hi) = (self.next()?, self.next()?);
        let wrap = if wrapped { self.next()? } else { wrap_code(Wrap::Open) };
        let same = |a: f64, b: f64| a.to_bits() == b.to_bits();
        if same(lo, whole.lo) && same(hi, whole.hi) && same(wrap, wrap_code(whole.wrap)) {
            return Ok(whole);
        }
        let part = whole.trimmed(lo, hi)?;
        if same(wrap, wrap_code(part.wrap)) { Ok(part) } else { Err(out_of_range(wrap, wrap_code(Wrap::Periodic))) }
    }

    /// Every number has been read.
    ///
    /// # Errors
    ///
    /// `geom.wrong-count` if some are left, with how many were given and how many were read.
    pub(crate) fn done(self) -> Result<(), Report> {
        if self.left() == 0 { Ok(()) } else { Err(wrong_count(self.all.len(), self.read)) }
    }
}

/// `n` knots read back, made canonical as a constructor makes them, and, for a Bézier form, refused unless they are its
/// knots of degree `p`.
///
/// # Errors
///
/// As [`Numbers::take`], and `geom.knots-not-valid` for a Bézier form's knots that are not its own.
pub(crate) fn knots(numbers: &mut Numbers<'_>, n: usize, bezier: Option<usize>) -> Result<Vec<f64>, Report> {
    let knots = canonical(numbers.take(n)?);
    match bezier {
        Some(p) if knots != bezier_knots(p) => Err(Report::new(Code::KnotsNotValid, Fault::InvalidInput)
            .with_value(NamedValue::new("index", Quantity::Count, 0.0))
            .with_value(NamedValue::new("knot", Quantity::Ratio, knots.first().copied().unwrap_or(f64::NAN)))),
        _ => Ok(knots),
    }
}

/// `n` homogeneous poles read back, `(w x, w y, w z, w)`: every number finite, every weight above zero, and every
/// weight exactly 1 unless `rational`.
///
/// # Errors
///
/// As [`Numbers::take`], `units.not-finite` for a number that is not, `geom.weight-not-positive` for a weight, and
/// `geom.out-of-range` for a weight other than 1 in a form that is not rational.
pub(crate) fn poles(numbers: &mut Numbers<'_>, n: usize, rational: bool) -> Result<Vec<[f64; 4]>, Report> {
    let read = numbers.take(n.saturating_mul(4))?;
    let poles: Vec<[f64; 4]> = read.as_chunks::<4>().0.to_vec();
    for (index, pole) in (0_u32..).zip(&poles) {
        if let Some(x) = pole.iter().find(|x| !x.is_finite()) {
            return Err(Report::new(Code::NotFinite, Fault::InvalidInput).with_value(NamedValue::length("value", *x)));
        }
        let [_, _, _, w] = *pole;
        if w <= 0.0 {
            return Err(Report::new(Code::WeightNotPositive, Fault::InvalidInput)
                .with_value(NamedValue::new("index", Quantity::Count, f64::from(index)))
                .with_value(NamedValue::new("weight", Quantity::Ratio, w)));
        }
        if !rational && w.to_bits() != 1.0_f64.to_bits() {
            return Err(Report::new(Code::OutOfRange, Fault::InvalidInput)
                .with_value(NamedValue::new("value", Quantity::Ratio, w))
                .with_value(NamedValue::new("low", Quantity::Ratio, 1.0))
                .with_value(NamedValue::new("high", Quantity::Ratio, 1.0)));
        }
    }
    Ok(poles)
}

/// The refusal of `x` where a whole number from 0 to `high` was wanted.
pub(crate) fn out_of_range(x: f64, high: f64) -> Report {
    Report::new(Code::OutOfRange, Fault::InvalidInput)
        .with_value(NamedValue::new("value", Quantity::Count, x))
        .with_value(NamedValue::new("low", Quantity::Count, 0.0))
        .with_value(NamedValue::new("high", Quantity::Count, high))
}

#[cfg(test)]
mod tests {
    use occam_units::ParamKind;

    use super::*;

    fn turn() -> ParamRange {
        ParamRange { lo: 0.0, hi: core::f64::consts::TAU, wrap: Wrap::Periodic, kind: ParamKind::Angle }
    }

    #[test]
    fn numbers_are_read_in_order_and_every_one_must_be() {
        let mut r = Numbers::new(&[1.0, 2.0, 3.0, 4.0]);
        assert_eq!(r.vec3().unwrap(), Vec3::new(1.0, 2.0, 3.0));
        assert_eq!(r.left(), 1);
        let left = r.done().unwrap_err();
        assert_eq!(left.code, Code::WrongCount);
        assert_eq!(
            (left.value("found").and_then(NamedValue::number), left.value("needed").and_then(NamedValue::number)),
            (Some(4.0), Some(3.0))
        );
        let mut r = Numbers::new(&[1.0]);
        let short = r.take(2).unwrap_err();
        assert_eq!(
            (short.value("found").and_then(NamedValue::number), short.value("needed").and_then(NamedValue::number)),
            (Some(1.0), Some(2.0))
        );
        assert_eq!(r.next().unwrap(), 1.0, "a failed read reads nothing");
        assert!(r.done().is_ok());
    }

    #[test]
    fn a_flag_is_0_or_1_and_a_count_a_whole_number_in_range() {
        let mut r = Numbers::new(&[0.0, 1.0, 0.5, 3.0, 2.5, -1.0, 4.0]);
        assert_eq!((r.flag().unwrap(), r.flag().unwrap()), (false, true));
        assert_eq!(r.flag().unwrap_err().code, Code::OutOfRange);
        assert_eq!(r.count(3).unwrap(), 3);
        for _ in 0..3 {
            assert_eq!(r.count(3).unwrap_err().code, Code::OutOfRange);
        }
    }

    #[test]
    fn a_range_is_the_whole_or_the_whole_trimmed_with_the_wrap_trimming_gives() {
        let whole = turn();
        let tau = core::f64::consts::TAU;
        assert_eq!(Numbers::new(&[0.0, tau, 2.0]).range(whole, true).unwrap(), whole);
        let closed = Numbers::new(&[0.0, tau, 1.0]).range(whole, true).unwrap();
        assert_eq!((closed.wrap, closed.lo, closed.hi), (Wrap::Closed, 0.0, tau));
        let arc = Numbers::new(&[1.0, 2.0, 0.0]).range(whole, true).unwrap();
        assert_eq!((arc.wrap, arc.lo, arc.hi), (Wrap::Open, 1.0, 2.0));
        assert_eq!(Numbers::new(&[1.0, 2.0, 1.0]).range(whole, true).unwrap_err().code, Code::OutOfRange);
        assert_eq!(Numbers::new(&[2.0, 1.0, 0.0]).range(whole, true).unwrap_err().code, Code::UndefinedParameter);
        let line = ParamRange::unbounded(ParamKind::Length);
        assert_eq!(Numbers::new(&[f64::NEG_INFINITY, f64::INFINITY]).range(line, false).unwrap(), line);
        let segment = Numbers::new(&[-1.0, 3.0]).range(line, false).unwrap();
        assert_eq!((segment.lo, segment.hi, segment.wrap), (-1.0, 3.0, Wrap::Open));
        assert_eq!(wrap_code(Wrap::Open), 0.0);
    }
}
