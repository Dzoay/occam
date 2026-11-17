//! Source: piegl-tiller-1997
//! Global interpolation (Piegl and Tiller, section 9.2.1, A9.1): the B-spline of a degree through points at the
//! parameters given, its knots the averages of the parameters (eq. 9.8), so that the function of each pole is nonzero
//! at one parameter at least and the system is banded and totally positive, which Gaussian elimination solves without
//! pivoting. The caller gives the parameters, so that curves through different points at the same parameters, such as
//! a curve in space and its images in two faces' parameters, share their knots and run together.

use occam_report::{Code, Fault, NamedValue, Quantity, Report};
use occam_units::{Point, Vec3, World};

use crate::basis::{basis, rows_len, span};
use crate::bspline::{BsplineCurve, checked_knots, checked_poles, wrong_count};

/// A count as a number.
fn number(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

/// The refusal of parameter `index`, `value`, which is not finite or not above the one before it.
fn bad_parameter(index: usize, value: f64) -> Report {
    let code = if value.is_finite() { Code::KnotsNotValid } else { Code::NotFinite };
    Report::new(code, Fault::InvalidInput)
        .with_value(NamedValue::new("index", Quantity::Count, number(index)))
        .with_value(NamedValue::new("value", Quantity::Ratio, value))
}

/// Checks that every parameter is finite and above the one before it.
fn increasing(params: &[f64]) -> Result<(), Report> {
    let mut before = f64::NEG_INFINITY;
    for (index, &t) in params.iter().enumerate() {
        if !(t.is_finite() && t > before) {
            return Err(bad_parameter(index, t));
        }
        before = t;
    }
    Ok(())
}

/// The knots of degree `p` for `params` by averaging (eq. 9.8): each end repeated `p + 1` times, and between them the
/// average of each run of `p` parameters but the first and the last. Empty where there are too few parameters.
fn averaged(params: &[f64], p: usize) -> Vec<f64> {
    let (Some(&first), Some(&last)) = (params.first(), params.last()) else { return Vec::new() };
    let n = params.len().saturating_sub(1);
    if p == 0 || n < p {
        return Vec::new();
    }
    let inner = params.windows(p).skip(1).take(n.saturating_sub(p)).map(|run| run.iter().sum::<f64>() / number(p));
    let ends = p.saturating_add(1);
    std::iter::repeat_n(first, ends).chain(inner).chain(std::iter::repeat_n(last, ends)).collect()
}

/// A row of the system: the functions nonzero at one parameter, from column `start`, and the point there.
struct Row {
    start: usize,
    values: Vec<f64>,
    rhs: Vec3,
}

impl Row {
    /// The entry in column `c`: zero outside the band.
    fn at(&self, c: usize) -> f64 {
        c.checked_sub(self.start).and_then(|i| self.values.get(i)).copied().unwrap_or(0.0)
    }

    /// The last column the row holds.
    const fn end(&self) -> usize {
        self.start.saturating_add(self.values.len()).saturating_sub(1)
    }
}

/// The rows of the system for degree `p`, `knots` and `params`, with `points` on the right.
fn system(knots: &[f64], p: usize, params: &[f64], points: &[Point<World>]) -> Vec<Row> {
    let (count, width) = (params.len(), p.saturating_add(1));
    let mut work = vec![0.0; rows_len(p, 0)];
    params
        .iter()
        .zip(points)
        .map(|(&t, point)| {
            let s = span(knots, p, count, t);
            let mut values = vec![0.0; width];
            basis(knots, (p, s), t, 0, &mut work, &mut values);
            Row { start: s.saturating_sub(p), values, rhs: point.to_vec3() }
        })
        .collect()
}

/// Gaussian elimination without pivoting, below the diagonal of each column in turn, then back substitution: the poles.
///
/// # Errors
///
/// The row whose pivot is zero or not finite, which totally positive rows never have.
fn solve(mut rows: Vec<Row>) -> Result<Vec<Vec3>, usize> {
    for j in 0..rows.len() {
        let (done, rest) = rows.split_at_mut(j.saturating_add(1));
        let Some(pivot_row) = done.last() else { return Err(j) };
        let pivot = pivot_row.at(j);
        if !(pivot != 0.0 && pivot.is_finite()) {
            return Err(j);
        }
        for row in rest.iter_mut().take_while(|row| row.start <= j) {
            let factor = row.at(j) / pivot;
            for c in j..=pivot_row.end() {
                if let Some(value) = c.checked_sub(row.start).and_then(|i| row.values.get_mut(i)) {
                    *value -= factor * pivot_row.at(c);
                }
            }
            row.rhs = row.rhs - pivot_row.rhs * factor;
        }
    }
    let mut poles = vec![Vec3::new(0.0, 0.0, 0.0); rows.len()];
    for (j, row) in rows.iter().enumerate().rev() {
        let known = (j.saturating_add(1)..=row.end())
            .fold(row.rhs, |sum, c| sum - poles.get(c).copied().unwrap_or(Vec3::new(0.0, 0.0, 0.0)) * row.at(c));
        if let Some(pole) = poles.get_mut(j) {
            *pole = known * (1.0 / row.at(j));
        }
    }
    Ok(poles)
}

/// The B-spline of `degree` through `points`, each at its parameter in `params`, which are finite and rise: clamped at
/// the first and last, its knots by averaging.
///
/// # Errors
///
/// `geom.wrong-count` for a parameter count other than the points' or too few points for the degree,
/// `geom.out-of-range` for a degree outside 1 to 25, `units.not-finite` for a parameter or point that is not finite,
/// `geom.knots-not-valid` for a parameter not above the one before it, and `geom.knots-not-valid`, fault
/// `numerically-sensitive`, where the parameters are so close that the system cannot be solved, naming its row.
pub fn interpolate(degree: u16, params: &[f64], points: &[Point<World>]) -> Result<BsplineCurve, Report> {
    if params.len() != points.len() {
        return Err(wrong_count(params.len(), points.len()));
    }
    increasing(params)?;
    checked_poles(points, None)?;
    let knots = averaged(params, usize::from(degree));
    let (p, _) = checked_knots(degree, points.len(), &knots)?;
    let poles = solve(system(&knots, p, params, points)).map_err(|row| {
        Report::new(Code::KnotsNotValid, Fault::NumericallySensitive).with_value(NamedValue::new(
            "index",
            Quantity::Count,
            number(row),
        ))
    })?;
    Ok(BsplineCurve { degree, knots, poles: poles.into_iter().map(Point::from_vec3).collect(), weights: None })
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::curve::Curve;
    use crate::spline_curve::SplineCurve;
    use occam_fixtures::point;

    /// A cubic's points, `(t, t^2 - t, 2 t^3 - t)`, at uneven parameters.
    fn on_a_cubic() -> (Vec<f64>, Vec<Point<World>>) {
        let params = vec![0.0, 0.3, 0.45, 1.1, 1.6, 2.0, 2.9, 3.0];
        let points = params.iter().map(|&t| point(t, t * t - t, 2.0 * t * t * t - t)).collect();
        (params, points)
    }

    fn through(degree: u16, params: &[f64], points: &[Point<World>]) -> SplineCurve {
        SplineCurve::new(&interpolate(degree, params, points).unwrap()).unwrap()
    }

    #[test]
    fn it_passes_through_every_point_at_its_parameter() {
        let (params, points) = on_a_cubic();
        for degree in 1..=5 {
            let c = through(degree, &params, &points);
            let range = Curve::range(&c);
            assert_eq!((range.lo, range.hi), (0.0, 3.0), "clamped at the first and last");
            for (t, p) in params.iter().zip(&points) {
                let off = (c.point(*t).to_vec3() - p.to_vec3()).norm();
                assert!(off < 1e-13, "degree {degree} at {t}: {off:e}");
            }
        }
    }

    #[test]
    fn a_cubic_through_points_of_a_cubic_is_that_cubic() {
        let (params, points) = on_a_cubic();
        let c = through(3, &params, &points);
        for k in 0..=60 {
            let t = 0.05 * f64::from(k);
            let want = Vec3::new(t, t * t - t, 2.0 * t * t * t - t);
            assert!((c.point(t).to_vec3() - want).norm() < 1e-12, "at {t}");
        }
        let line = through(1, &params[..2], &points[..2]);
        let middle = (points[0].to_vec3() + points[1].to_vec3()) * 0.5;
        assert!((line.point(0.15).to_vec3() - middle).norm() < 1e-15);
    }

    #[test]
    fn its_knots_are_the_averages_of_the_parameters() {
        let form = interpolate(2, &[0.0, 1.0, 3.0, 4.0, 8.0], &[point(0.0, 0.0, 0.0); 5]).unwrap();
        assert_eq!(form.knots, [0.0, 0.0, 0.0, 2.0, 3.5, 8.0, 8.0, 8.0]);
        assert_eq!(form.poles, [point(0.0, 0.0, 0.0); 5]);
        let form = interpolate(3, &[0.0, 1.0, 2.0, 6.0], &[point(1.0, 2.0, 3.0); 4]).unwrap();
        assert_eq!(form.knots, [0.0, 0.0, 0.0, 0.0, 6.0, 6.0, 6.0, 6.0], "as many points as a Bézier has poles");
    }

    #[test]
    fn what_cannot_be_interpolated_is_refused() {
        let (params, points) = on_a_cubic();
        let code =
            |degree: u16, params: &[f64], points: &[Point<World>]| interpolate(degree, params, points).unwrap_err();
        let code_of = |params: &[f64], points: &[Point<World>]| code(3, params, points);
        assert_eq!(code(3, &params[..7], &points).code, Code::WrongCount);
        assert_eq!(code(3, &params[..3], &points[..3]).code, Code::WrongCount, "too few for a cubic");
        assert_eq!(code(0, &params, &points).code, Code::OutOfRange);
        assert_eq!(code(26, &params, &points).code, Code::OutOfRange);
        // Each is refused as the parameter it is, by its index and value, before any knot or row is made of it.
        let named = |r: &Report| {
            (r.code, r.fault, r.value("index").and_then(NamedValue::number), r.value("value").map(|v| v.value))
        };
        let mut flat = params.clone();
        for (index, value) in [(4, flat[3]), (5, flat[4] - 0.1), (4, f64::NAN), (0, f64::NEG_INFINITY)] {
            flat.clone_from(&params);
            flat[index] = value;
            let code = if value.is_finite() { Code::KnotsNotValid } else { Code::NotFinite };
            let want = (code, Some(Fault::InvalidInput), Some(f64::from(u32::try_from(index).unwrap())));
            let got = named(&code_of(&flat, &points));
            assert_eq!((got.0, got.1, got.2), want, "parameter {index} at {value}");
            assert_eq!(format!("{:?}", got.3), format!("{:?}", Some(occam_report::Value::Scalar(value))));
        }
        let mut far = points.clone();
        far[2] = point(0.0, f64::INFINITY, 0.0);
        assert_eq!(code(3, &params, &far).code, Code::NotFinite);
    }

    #[test]
    fn a_zero_pivot_is_refused_not_divided_by() {
        let rows = vec![Row { start: 0, values: vec![0.0, 1.0], rhs: Vec3::new(1.0, 0.0, 0.0) }];
        assert_eq!(solve(rows).unwrap_err(), 0);
    }
}
