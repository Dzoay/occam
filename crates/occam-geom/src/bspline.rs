//! Source: original
//! The B-spline forms: the numbers of a rational B-spline curve or surface, as a family presents itself where it has one
//! exactly (PARITY 2.1, 2.2), and as a spline family is made from. They are data, as [`crate::Analytic`] is, and name no
//! family. What makes a valid one is checked here, once, for every family that is made from one.

use occam_report::{Code, Fault, NamedValue, Quantity, Report};
use occam_units::{Point, World};

/// The highest degree a B-spline may have.
pub(crate) const MAX_DEGREE: u16 = 25;

/// A rational B-spline curve, or a polynomial one where it has no weights.
#[derive(Clone, Debug, PartialEq)]
pub struct BsplineCurve {
    /// The degree.
    pub degree: u16,
    /// The knots, repeats included: as many as the poles and the degree and one more.
    pub knots: Vec<f64>,
    /// The poles.
    pub poles: Vec<Point<World>>,
    /// A weight for each pole, or `None` where every weight is one.
    pub weights: Option<Vec<f64>>,
}

/// A rational B-spline surface, or a polynomial one where it has no weights.
#[derive(Clone, Debug, PartialEq)]
pub struct BsplineSurface {
    /// The degree in each parameter.
    pub degree: (u16, u16),
    /// How many poles in each parameter.
    pub count: (u32, u32),
    /// The knots in `u`, repeats included.
    pub knots_u: Vec<f64>,
    /// The knots in `v`, repeats included.
    pub knots_v: Vec<f64>,
    /// The poles, `count.0 * count.1` of them, a row of constant `v` at a time.
    pub poles: Vec<Point<World>>,
    /// A weight for each pole, in the same order, or `None` where every weight is one.
    pub weights: Option<Vec<f64>>,
}

/// The knots of a Bézier of `degree`: zero and one, each repeated once more than the degree.
pub(crate) fn bezier_knots(degree: usize) -> Vec<f64> {
    let ends = degree.saturating_add(1);
    std::iter::repeat_n(0.0, ends).chain(std::iter::repeat_n(1.0, ends)).collect()
}

/// A count as a number a report holds.
fn count(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

/// The refusal of `found` items where `needed` are needed.
pub(crate) fn wrong_count(found: usize, needed: usize) -> Report {
    Report::new(Code::WrongCount, Fault::InvalidInput)
        .with_value(NamedValue::new("found", Quantity::Count, count(found)))
        .with_value(NamedValue::new("needed", Quantity::Count, count(needed)))
}

/// The refusal of knot `index`, `knot`.
fn bad_knot(index: usize, knot: f64) -> Report {
    Report::new(Code::KnotsNotValid, Fault::InvalidInput)
        .with_value(NamedValue::new("index", Quantity::Count, count(index)))
        .with_value(NamedValue::new("knot", Quantity::Ratio, knot))
}

/// The refusal of a number that is not finite.
fn not_finite(x: f64) -> Report {
    Report::new(Code::NotFinite, Fault::InvalidInput).with_value(NamedValue::length("value", x))
}

/// Checks one parameter of a B-spline: a degree from 1 to [`MAX_DEGREE`], at least one more pole than the degree, and
/// `knots` finite, never decreasing, as many as the poles and the degree and one more, and repeated at most `degree`
/// times inside, or once more where a run reaches an end. Gives the degree and the pole count as indices.
///
/// # Errors
///
/// `geom.out-of-range` for the degree, `geom.wrong-count` for the poles or the knots, `units.not-finite` for a knot,
/// and `geom.knots-not-valid` for a knot out of order or repeated too often.
pub(crate) fn checked_knots(degree: u16, poles: usize, knots: &[f64]) -> Result<(usize, usize), Report> {
    if degree == 0 || degree > MAX_DEGREE {
        return Err(Report::new(Code::OutOfRange, Fault::InvalidInput)
            .with_value(NamedValue::new("value", Quantity::Count, f64::from(degree)))
            .with_value(NamedValue::new("low", Quantity::Count, 0.0))
            .with_value(NamedValue::new("high", Quantity::Count, f64::from(MAX_DEGREE.saturating_add(1)))));
    }
    let p = usize::from(degree);
    if poles <= p {
        return Err(wrong_count(poles, p.saturating_add(1)));
    }
    let needed = poles.saturating_add(p).saturating_add(1);
    if knots.len() != needed {
        return Err(wrong_count(knots.len(), needed));
    }
    if let Some(x) = knots.iter().find(|k| !k.is_finite()) {
        return Err(not_finite(*x));
    }
    let last = needed.saturating_sub(1);
    let mut run_start = 0;
    for (index, pair) in (1..).zip(knots.windows(2)) {
        let [before, knot] = [pair.first().copied().unwrap_or(0.0), pair.get(1).copied().unwrap_or(0.0)];
        if knot < before {
            return Err(bad_knot(index, knot));
        }
        if knot > before {
            run_start = index;
        }
        let run = index.saturating_sub(run_start).saturating_add(1);
        let at_an_end = run_start == 0 || index == last;
        if run > p.saturating_add(1) || (run > p && !at_an_end) {
            return Err(bad_knot(index, knot));
        }
    }
    let (low, high) = (knots.get(p).copied().unwrap_or(0.0), knots.get(poles).copied().unwrap_or(0.0));
    if low >= high {
        return Err(bad_knot(poles, high));
    }
    Ok((p, poles))
}

/// Checks the poles and the weights: every pole finite, and either no weights or one for each pole, each a finite number
/// greater than zero.
///
/// # Errors
///
/// `units.not-finite` for a pole, `geom.wrong-count` for the weights, and `geom.weight-not-positive` for a weight.
pub(crate) fn checked_poles(poles: &[Point<World>], weights: Option<&[f64]>) -> Result<(), Report> {
    for p in poles {
        let v = p.to_vec3();
        if let Some(x) = [v.x, v.y, v.z].into_iter().find(|x| !x.is_finite()) {
            return Err(not_finite(x));
        }
    }
    let Some(weights) = weights else { return Ok(()) };
    if weights.len() != poles.len() {
        return Err(wrong_count(weights.len(), poles.len()));
    }
    match (0..).zip(weights).find(|(_, w)| !(**w > 0.0 && w.is_finite())) {
        None => Ok(()),
        Some((index, weight)) => Err(Report::new(Code::WeightNotPositive, Fault::InvalidInput)
            .with_value(NamedValue::new("index", Quantity::Count, count(index)))
            .with_value(NamedValue::new("weight", Quantity::Ratio, *weight))),
    }
}

/// The poles in homogeneous form, `(w x, w y, w z, w)`, with a weight of one where there are none.
pub(crate) fn homogeneous(poles: &[Point<World>], weights: Option<&[f64]>) -> Vec<[f64; 4]> {
    let weight = |index: usize| weights.and_then(|w| w.get(index)).copied().unwrap_or(1.0);
    poles
        .iter()
        .enumerate()
        .map(|(index, p)| {
            let (v, w) = (p.to_vec3(), weight(index));
            [v.x * w, v.y * w, v.z * w, w]
        })
        .collect()
}

/// The poles and weights of homogeneous poles, the weights `None` where `rational` is false.
pub(crate) fn cartesian(poles: &[[f64; 4]], rational: bool) -> (Vec<Point<World>>, Option<Vec<f64>>) {
    let points = poles
        .iter()
        .map(|[x, y, z, w]| {
            let v = occam_units::Vec3::new(x / w, y / w, z / w);
            Point::from_vec3(v)
        })
        .collect();
    (points, rational.then(|| poles.iter().map(|[.., w]| *w).collect()))
}

#[cfg(test)]
mod tests {
    use occam_units::Vec3;

    use super::*;

    fn point(x: f64) -> Point<World> {
        Point::from_vec3(Vec3::new(x, 0.0, 0.0))
    }

    fn named(report: &Report, name: &str) -> Option<f64> {
        report.value(name).and_then(NamedValue::number)
    }

    #[test]
    fn a_valid_knot_vector_gives_the_degree_and_the_pole_count() {
        assert_eq!(checked_knots(2, 4, &[0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0]), Ok((2, 4)));
        assert_eq!(checked_knots(2, 4, &[0.0, 1.0, 2.0, 3.0, 4.0, 5.0, 6.0]), Ok((2, 4)), "unclamped");
        assert_eq!(
            checked_knots(3, 5, &[0.0, 0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 1.0, 1.0]),
            Ok((3, 5)),
            "the degree's repeats"
        );
        assert_eq!(checked_knots(1, 2, &bezier_knots(1)), Ok((1, 2)));
        assert_eq!(checked_knots(25, 26, &bezier_knots(25)), Ok((25, 26)), "the highest degree");
        assert_eq!(bezier_knots(2), [0.0, 0.0, 0.0, 1.0, 1.0, 1.0]);
    }

    #[test]
    fn a_degree_outside_one_to_twenty_five_is_refused() {
        for degree in [0, 26] {
            let report = checked_knots(degree, 40, &vec![0.0; 70]).unwrap_err();
            assert_eq!((report.code, named(&report, "value")), (Code::OutOfRange, Some(f64::from(degree))));
            assert_eq!((named(&report, "low"), named(&report, "high")), (Some(0.0), Some(26.0)));
        }
    }

    #[test]
    fn too_few_poles_or_the_wrong_number_of_knots_is_refused() {
        let few = checked_knots(3, 3, &[0.0; 7]).unwrap_err();
        assert_eq!((few.code, named(&few, "found"), named(&few, "needed")), (Code::WrongCount, Some(3.0), Some(4.0)));
        let knots = checked_knots(2, 3, &[0.0, 0.0, 0.0, 1.0, 1.0]).unwrap_err();
        assert_eq!((named(&knots, "found"), named(&knots, "needed")), (Some(5.0), Some(6.0)));
    }

    #[test]
    fn knots_out_of_order_too_often_repeated_or_not_finite_are_refused() {
        let at = |degree, poles, knots: &[f64]| {
            let report = checked_knots(degree, poles, knots).unwrap_err();
            (report.code, named(&report, "index"), named(&report, "knot"))
        };
        assert_eq!(at(1, 3, &[0.0, 0.0, 2.0, 1.0, 3.0]), (Code::KnotsNotValid, Some(3.0), Some(1.0)));
        assert_eq!(at(2, 4, &[0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0]), (Code::KnotsNotValid, Some(3.0), Some(0.0)));
        assert_eq!(at(2, 5, &[0.0, 0.0, 0.0, 0.5, 0.5, 0.5, 1.0, 1.0]), (Code::KnotsNotValid, Some(5.0), Some(0.5)));
        assert!(checked_knots(2, 5, &[0.0, 0.0, 0.0, 0.5, 0.5, 1.0, 1.0, 1.0]).is_ok(), "the degree's repeats inside");
        assert_eq!(at(1, 2, &[0.0, 1.0, 1.0, 1.0]), (Code::KnotsNotValid, Some(2.0), Some(1.0)), "past the end's");
        let report = checked_knots(1, 2, &[0.0, f64::NAN, 1.0, 1.0]).unwrap_err();
        assert_eq!(report.code, Code::NotFinite);
    }

    #[test]
    fn an_empty_domain_is_refused() {
        // Degree 2 and three poles: the domain runs from knot 2 to knot 3, here both 1, which repeats no more than twice.
        let report = checked_knots(2, 3, &[0.0, 0.5, 1.0, 1.0, 1.5, 2.0]).unwrap_err();
        assert_eq!(
            (report.code, named(&report, "index"), named(&report, "knot")),
            (Code::KnotsNotValid, Some(3.0), Some(1.0))
        );
    }

    #[test]
    fn poles_must_be_finite_and_weights_one_each_finite_and_positive() {
        let poles = [point(0.0), point(1.0), point(2.0)];
        assert!(checked_poles(&poles, None).is_ok() && checked_poles(&poles, Some(&[1.0, 2.0, 0.5])).is_ok());
        assert_eq!(checked_poles(&[point(f64::INFINITY)], None).unwrap_err().code, Code::NotFinite);
        let short = checked_poles(&poles, Some(&[1.0, 2.0])).unwrap_err();
        assert_eq!(
            (short.code, named(&short, "found"), named(&short, "needed")),
            (Code::WrongCount, Some(2.0), Some(3.0))
        );
        for bad in [0.0, -1.0, f64::NAN, f64::INFINITY] {
            let report = checked_poles(&poles, Some(&[1.0, bad, 1.0])).unwrap_err();
            assert_eq!((report.code, named(&report, "index")), (Code::WeightNotPositive, Some(1.0)));
            assert_eq!(named(&report, "weight").map(f64::to_bits), Some(bad.to_bits()));
        }
    }

    #[test]
    fn homogeneous_poles_carry_their_weights_and_come_back() {
        let poles = [point(1.0), Point::from_vec3(Vec3::new(2.0, 3.0, 4.0))];
        let weighted = homogeneous(&poles, Some(&[2.0, 0.5]));
        assert_eq!(weighted, [[2.0, 0.0, 0.0, 2.0], [1.0, 1.5, 2.0, 0.5]]);
        assert_eq!(cartesian(&weighted, true), (poles.to_vec(), Some(vec![2.0, 0.5])));
        assert_eq!(homogeneous(&poles, None), [[1.0, 0.0, 0.0, 1.0], [2.0, 3.0, 4.0, 1.0]]);
        assert_eq!(cartesian(&homogeneous(&poles, None), false), (poles.to_vec(), None));
    }
}
