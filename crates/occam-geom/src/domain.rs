//! Source: original
//! Where a curve or surface is defined: each parameter's range, what it measures and whether it comes round again, and
//! the lines where a surface collapses to a point, which are declared when it is made (S4).

use occam_report::{Code, Fault, NamedValue, Quantity, Reason, Report};
use occam_units::{Direction, ParamKind, Point, UvPoint, World};

/// Whether a parameter's ends meet.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Wrap {
    /// They do not.
    Open,
    /// They meet at a seam, and the parameter stops there.
    Closed,
    /// They meet at a seam, and the parameter carries on round, so every finite value is defined.
    Periodic,
}

/// One parameter's range.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct ParamRange {
    /// The lowest value, or minus infinity.
    pub lo: f64,
    /// The highest value, or infinity. Where the ends meet, the value at which `lo` comes round again.
    pub hi: f64,
    /// Whether the ends meet.
    pub wrap: Wrap,
    /// What the parameter measures.
    pub kind: ParamKind,
}

impl ParamRange {
    /// A parameter defined at every finite value, as a plane's and a line's are.
    #[must_use]
    pub const fn unbounded(kind: ParamKind) -> Self {
        Self { lo: f64::NEG_INFINITY, hi: f64::INFINITY, wrap: Wrap::Open, kind }
    }

    /// Whether `t` is a parameter here: finite, and inside the range unless the parameter comes round again.
    #[must_use]
    pub fn contains(self, t: f64) -> bool {
        t.is_finite() && (self.wrap == Wrap::Periodic || (self.lo <= t && t <= self.hi))
    }

    /// What its values measure, as a report names it.
    #[must_use]
    pub const fn quantity(self) -> Quantity {
        match self.kind {
            ParamKind::Length => Quantity::Length,
            ParamKind::Angle => Quantity::Angle,
            ParamKind::Knot => Quantity::Ratio,
        }
    }

    /// Refuses a parameter that is not [contained](ParamRange::contains).
    ///
    /// # Errors
    ///
    /// [`Code::UndefinedParameter`], with the parameter and the range in the units it measures.
    pub fn check(self, t: f64) -> Result<(), Report> {
        if self.contains(t) {
            return Ok(());
        }
        let quantity = self.quantity();
        Err(Report::new(Code::UndefinedParameter, Fault::InvalidInput)
            .with_reason(Reason::OutsideDomain)
            .with_value(NamedValue::new("parameter", quantity, t))
            .with_value(NamedValue::new("low", quantity, self.lo))
            .with_value(NamedValue::new("high", quantity, self.hi)))
    }
}

impl ParamRange {
    /// The part of this range from `lo` to `hi`, as a curve trimmed by parameter has (PARITY 1.4). For a periodic
    /// parameter, `hi` may be up to a period beyond `lo`, and a whole period makes the ends meet again.
    ///
    /// # Errors
    ///
    /// [`Code::UndefinedParameter`] for an end outside the range, or for `hi` not above `lo`.
    pub fn trimmed(self, lo: f64, hi: f64) -> Result<Self, Report> {
        self.check(lo)?;
        let periodic = self.wrap == Wrap::Periodic;
        let top = if periodic { lo + (self.hi - self.lo) } else { self.hi };
        let part = Self { lo, hi: top, wrap: Wrap::Open, kind: self.kind };
        Self { lo: lo.next_up(), ..part }.check(hi)?;
        let wrap = if periodic && hi >= top { Wrap::Closed } else { Wrap::Open };
        Ok(Self { hi, wrap, ..part })
    }

    /// `t` moved to the nearer end where it is beyond one. A periodic parameter is never beyond an end.
    #[must_use]
    pub fn clamp(self, t: f64) -> f64 {
        if self.wrap == Wrap::Periodic {
            t
        } else if t < self.lo {
            self.lo
        } else if t > self.hi {
            self.hi
        } else {
            t
        }
    }
}

/// A surface's two parameter ranges.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Domain {
    /// The first parameter's.
    pub u: ParamRange,
    /// The second parameter's.
    pub v: ParamRange,
}

impl Domain {
    /// Whether both parameters are [contained](ParamRange::contains).
    #[must_use]
    pub fn contains(self, uv: UvPoint) -> bool {
        self.u.contains(uv.u()) && self.v.contains(uv.v())
    }

    /// Refuses a point outside the domain, `u` first.
    ///
    /// # Errors
    ///
    /// As [`ParamRange::check`].
    pub fn check(self, uv: UvPoint) -> Result<(), Report> {
        self.u.check(uv.u())?;
        self.v.check(uv.v())
    }
}

/// A rectangle in a surface's parameters, edges included.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct UvBox {
    /// The corner with the lowest parameters.
    pub lo: UvPoint,
    /// The corner with the highest.
    pub hi: UvPoint,
}

impl UvBox {
    /// The rectangle between two corners.
    #[must_use]
    pub const fn new(lo: UvPoint, hi: UvPoint) -> Self {
        Self { lo, hi }
    }

    /// Its four corners: low `u` and low `v` first, then `u` changes first.
    #[must_use]
    pub const fn corners(self) -> [UvPoint; 4] {
        let (lo, hi) = (self.lo, self.hi);
        [lo, UvPoint::new(hi.u(), lo.v()), UvPoint::new(lo.u(), hi.v()), hi]
    }
}

/// One of a surface's two parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Param {
    /// The first.
    U,
    /// The second.
    V,
}

/// A line of constant parameter that collapses to one point, declared when the surface is made. A sphere's pole has a
/// normal there and a cone's apex does not (S4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Degeneracy {
    /// The parameter that is constant along the line.
    pub param: Param,
    /// Its value there.
    pub at: f64,
    /// The point the line collapses to.
    pub point: Point<World>,
    /// The surface's normal at that point, where it has one.
    pub normal: Option<Direction<World>>,
}

#[cfg(test)]
mod tests {
    use super::*;

    fn closed(lo: f64, hi: f64, wrap: Wrap) -> ParamRange {
        ParamRange { lo, hi, wrap, kind: ParamKind::Angle }
    }

    #[test]
    fn a_range_holds_finite_values_between_its_ends() {
        let range = closed(-1.0, 2.0, Wrap::Closed);
        assert!(range.contains(-1.0) && range.contains(0.5) && range.contains(2.0));
        assert!(!range.contains(-1.000_001) && !range.contains(2.000_001) && !range.contains(f64::NAN));
        let unbounded = ParamRange::unbounded(ParamKind::Length);
        assert!(unbounded.contains(-1e300) && unbounded.contains(1e300));
        assert!(!unbounded.contains(f64::INFINITY) && !unbounded.contains(f64::NEG_INFINITY));
    }

    #[test]
    fn a_periodic_range_holds_every_finite_value() {
        let range = closed(0.0, 1.0, Wrap::Periodic);
        assert!(range.contains(-7.5) && range.contains(12.25));
        assert!(!range.contains(f64::INFINITY) && !range.contains(f64::NAN));
    }

    #[test]
    fn a_parameter_outside_the_range_is_refused_in_its_own_units() {
        for (kind, quantity) in [
            (ParamKind::Length, Quantity::Length),
            (ParamKind::Angle, Quantity::Angle),
            (ParamKind::Knot, Quantity::Ratio),
        ] {
            let range = ParamRange { lo: 0.0, hi: 1.0, wrap: Wrap::Open, kind };
            assert!(range.check(0.25).is_ok());
            let report = range.check(1.5).unwrap_err();
            assert_eq!(
                (report.code, report.fault, report.reason),
                (Code::UndefinedParameter, Some(Fault::InvalidInput), Some(Reason::OutsideDomain))
            );
            let named = |name: &str| report.value(name).map(|value| (value.number(), value.quantity));
            assert_eq!(named("parameter"), Some((Some(1.5), quantity)));
            assert_eq!(named("low"), Some((Some(0.0), quantity)));
            assert_eq!(named("high"), Some((Some(1.0), quantity)));
        }
    }

    #[test]
    fn a_trimmed_range_holds_from_one_end_to_the_other() {
        let line = ParamRange::unbounded(ParamKind::Length).trimmed(-2.0, 5.0).unwrap();
        assert_eq!(line, ParamRange { lo: -2.0, hi: 5.0, wrap: Wrap::Open, kind: ParamKind::Length });
        let turn = closed(0.0, 4.0, Wrap::Periodic);
        assert_eq!(turn.trimmed(3.0, 6.5).unwrap(), closed(3.0, 6.5, Wrap::Open), "past the seam, within a period");
        assert_eq!(turn.trimmed(-1.0, 3.0).unwrap(), closed(-1.0, 3.0, Wrap::Closed), "a whole period meets itself");
        let shifted = closed(-1.0, 3.0, Wrap::Periodic);
        assert_eq!(
            shifted.trimmed(2.0, 5.5).unwrap(),
            closed(2.0, 5.5, Wrap::Open),
            "a period is its length, not its end"
        );
        let arc = closed(1.0, 2.0, Wrap::Open);
        assert_eq!(arc.trimmed(1.25, 2.0).unwrap(), closed(1.25, 2.0, Wrap::Open));
    }

    #[test]
    fn a_trim_outside_the_range_or_back_to_front_is_refused() {
        let turn = closed(0.0, 4.0, Wrap::Periodic);
        let parameter = |report: Report| report.value("parameter").and_then(NamedValue::number);
        assert_eq!(parameter(turn.trimmed(3.0, 7.5).unwrap_err()), Some(7.5), "longer than a period");
        assert_eq!(turn.trimmed(f64::NAN, 1.0).unwrap_err().code, Code::UndefinedParameter);
        let arc = closed(1.0, 2.0, Wrap::Open);
        assert_eq!(parameter(arc.trimmed(0.5, 1.5).unwrap_err()), Some(0.5));
        assert_eq!(parameter(arc.trimmed(1.5, 2.5).unwrap_err()), Some(2.5));
        for (lo, hi) in [(1.5, 1.5), (1.75, 1.25)] {
            assert_eq!(parameter(arc.trimmed(lo, hi).unwrap_err()), Some(hi), "{lo} {hi}");
        }
    }

    #[test]
    fn a_parameter_beyond_an_end_is_clamped_to_it() {
        let arc = closed(1.0, 2.0, Wrap::Open);
        assert_eq!(
            [arc.clamp(0.5), arc.clamp(1.5), arc.clamp(2.5), arc.clamp(1.0), arc.clamp(2.0)],
            [1.0, 1.5, 2.0, 1.0, 2.0]
        );
        assert_eq!(closed(0.0, 1.0, Wrap::Periodic).clamp(7.25), 7.25);
    }

    #[test]
    fn a_domain_checks_both_parameters() {
        let domain = Domain { u: closed(0.0, 1.0, Wrap::Open), v: closed(10.0, 20.0, Wrap::Open) };
        assert!(domain.contains(UvPoint::new(0.5, 15.0)) && domain.check(UvPoint::new(0.5, 15.0)).is_ok());
        for outside in [UvPoint::new(1.5, 15.0), UvPoint::new(0.5, 25.0)] {
            assert!(!domain.contains(outside));
            assert_eq!(domain.check(outside).unwrap_err().code, Code::UndefinedParameter);
        }
        let in_u = domain.check(UvPoint::new(0.5, 25.0)).unwrap_err();
        assert_eq!(in_u.value("parameter").and_then(NamedValue::number), Some(25.0), "v is checked once u passes");
    }

    #[test]
    fn a_box_has_four_corners_u_first() {
        let corners = UvBox::new(UvPoint::new(1.0, 2.0), UvPoint::new(3.0, 4.0)).corners();
        let pairs = corners.map(|c| (c.u(), c.v()));
        assert_eq!(pairs, [(1.0, 2.0), (3.0, 2.0), (1.0, 4.0), (3.0, 4.0)]);
    }
}
