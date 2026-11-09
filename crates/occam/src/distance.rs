//! Source: original
//! The first operation and the simplest: how far apart two points are. It exists to prove the seams before there is any
//! geometry to hide behind.

use occam_math::Real;
use occam_report::{AngleUnit, Code, DisplayUnits, LengthUnit, NamedValue, Outcome, Remedy, RemedyKind, Report};
use occam_units::{Angle, Length, Point, Tolerance, World};

use crate::Model;

/// One request as flat numbers, in millimetres and radians, as a facade receives it.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct DistanceQuery {
    /// The first point.
    pub from: [f64; 3],
    /// The second point.
    pub to: [f64; 3],
    /// The length tolerance the model is built to.
    pub model_length_tolerance: f64,
    /// The angle tolerance the model is built to.
    pub model_angle_tolerance: f64,
    /// The tolerance asked for.
    pub tolerance: f64,
}

/// What a facade passes on.
#[derive(Clone, Debug, PartialEq)]
pub struct Answer {
    /// The result or the report, in its one JSON form.
    pub json: String,
    /// The same in words.
    pub words: String,
    /// Whether the operation was refused.
    pub refused: bool,
}

fn too_fine(asked: Length, model: Length) -> Report {
    Report::new(Code::ToleranceBelowModel)
        .with_value(NamedValue::length("requested", asked.millimetres()))
        .with_value(NamedValue::length("model", model.millimetres()))
        .with_remedy(
            Remedy::new(RemedyKind::UseModelTolerance).with_value(NamedValue::length("tolerance", model.millimetres())),
        )
}

/// The distance from `from` to `to`, worked to `tolerance`.
///
/// # Errors
///
/// [`Code::ToleranceBelowModel`] if `tolerance` is finer than the model's or is not a number, and [`Code::NotFinite`] if the
/// distance is too large to hold.
pub fn distance(
    model: Model,
    from: Point<World>,
    to: Point<World>,
    tolerance: Length,
) -> Result<Outcome<Length>, Report> {
    let finest = model.tolerance().length();
    let allowed = tolerance >= finest; // false for NaN, which is refused with the rest
    if !allowed {
        return Err(too_fine(tolerance, finest));
    }
    let gap = to - from;
    let (x, y, z) = (gap.x().millimetres(), gap.y().millimetres(), gap.z().millimetres());
    let length = Length::new(Real::sqrt(x * x + y * y + z * z), LengthUnit::Millimetre)?;
    let used = Report::new(Code::ToleranceUsed).with_value(NamedValue::length("tolerance", tolerance.millimetres()));
    Ok(Outcome::new(length).with_diagnostic(used))
}

/// Where numbers from outside become typed quantities, which is the only place they are checked.
fn answered(query: &DistanceQuery) -> Result<Outcome<f64>, Report> {
    let length = |value| Length::new(value, LengthUnit::Millimetre);
    let point =
        |[x, y, z]: [f64; 3]| -> Result<Point<World>, Report> { Ok(Point::new(length(x)?, length(y)?, length(z)?)) };
    let angle = Angle::new(query.model_angle_tolerance, AngleUnit::Radian)?;
    let model = Model::new(Tolerance::new(length(query.model_length_tolerance)?, angle)?);
    let outcome = distance(model, point(query.from)?, point(query.to)?, length(query.tolerance)?)?;
    Ok(Outcome { value: outcome.value.millimetres(), diagnostics: outcome.diagnostics })
}

/// Answers `query` for a facade, which only has to pass the answer on.
#[must_use]
pub fn distance_json(query: &DistanceQuery) -> Answer {
    let units = DisplayUnits::default();
    let written = answered(query).and_then(|outcome| Ok((outcome.to_json(units)?, format!("{} mm", outcome.value))));
    match written {
        Ok((json, words)) => Answer { json, words, refused: false },
        Err(report) => {
            let json = report.to_json(units).unwrap_or_else(|failed| failed.code.text().to_string());
            Answer { json, words: report.message(units), refused: true }
        }
    }
}

#[cfg(test)]
mod tests {
    use occam_report::followed_remedy;

    use super::*;

    fn mm(value: f64) -> Length {
        Length::from_millimetres(value)
    }

    fn model(length: f64) -> Model {
        Model::new(Tolerance::new(mm(length), Angle::from_radians(1e-6)).unwrap())
    }

    fn point(x: f64, y: f64, z: f64) -> Point<World> {
        Point::new(mm(x), mm(y), mm(z))
    }

    fn origin() -> Point<World> {
        point(0.0, 0.0, 0.0)
    }

    #[test]
    fn the_distance_uses_every_axis() {
        let got = distance(model(0.001), point(1.0, -2.0, 3.0), point(4.0, 2.0, 15.0), mm(0.001)).unwrap();
        assert_eq!(got.value, mm(13.0));
    }

    #[test]
    fn a_coarser_tolerance_than_the_models_is_accepted() {
        assert!(distance(model(0.001), origin(), origin(), mm(0.01)).is_ok());
    }

    #[test]
    fn a_tolerance_equal_to_the_models_is_accepted() {
        assert!(distance(model(0.001), origin(), origin(), mm(0.001)).is_ok());
    }

    #[test]
    fn success_says_what_tolerance_was_used() {
        let got = distance(model(0.001), origin(), point(3.0, 4.0, 0.0), mm(0.01)).unwrap();
        let note = Report::new(Code::ToleranceUsed).with_value(NamedValue::length("tolerance", 0.01));
        assert_eq!(got.diagnostics, [note]);
    }

    fn refusal() -> Report {
        distance(model(0.001), origin(), point(3.0, 4.0, 0.0), mm(0.0001)).unwrap_err()
    }

    #[test]
    fn a_tolerance_finer_than_the_models_is_refused_with_both_numbers() {
        let report = refusal();
        assert_eq!(report.code, Code::ToleranceBelowModel);
        assert_eq!(report.values, [NamedValue::length("requested", 0.0001), NamedValue::length("model", 0.001)]);
        let remedy = Remedy::new(RemedyKind::UseModelTolerance).with_value(NamedValue::length("tolerance", 0.001));
        assert_eq!(report.remedies, [remedy]);
    }

    #[test]
    fn a_tolerance_that_is_not_a_number_is_refused() {
        let got = distance(model(0.001), origin(), origin(), mm(f64::NAN)).unwrap_err();
        assert_eq!(got.code, Code::ToleranceBelowModel);
    }

    #[test]
    fn following_the_remedy_succeeds() {
        followed_remedy!(refusal(), Code::ToleranceBelowModel, RemedyKind::UseModelTolerance, |remedy| {
            let tolerance = remedy.value("tolerance").map_or(0.0, |value| value.value);
            let got = distance(
                model(0.001),
                origin(),
                point(3.0, 4.0, 0.0),
                Length::new(tolerance, LengthUnit::Millimetre)?,
            )?;
            assert_eq!(got.value, mm(5.0));
            Ok(got)
        });
    }

    #[test]
    fn a_distance_too_large_to_hold_is_refused() {
        let got = distance(model(0.001), origin(), point(1e200, 0.0, 0.0), mm(0.001)).unwrap_err();
        assert_eq!(got.code, Code::NotFinite);
    }

    fn query(tolerance: f64) -> DistanceQuery {
        DistanceQuery {
            from: [0.0, 0.0, 0.0],
            to: [3.0, 4.0, 0.0],
            model_length_tolerance: 0.001,
            model_angle_tolerance: 1e-6,
            tolerance,
        }
    }

    #[test]
    fn an_answer_gives_the_outcome_as_json_and_in_words() {
        let got = distance_json(&query(0.01));
        let json = concat!(
            r#"{"schema":1,"value":5.0,"diagnostics":[{"schema":1,"code":"measure.tolerance-used","severity":"note","#,
            r#""message":"The operation worked to a tolerance of 0.01 mm.","entities":[],"#,
            r#""values":[{"name":"tolerance","quantity":"length","value":0.01}],"remedies":[]}]}"#,
        );
        assert_eq!(got, Answer { json: json.to_string(), words: "5 mm".to_string(), refused: false });
    }

    #[test]
    fn a_refused_answer_gives_the_report_as_json_and_in_words() {
        let got = distance_json(&query(0.0001));
        let words = "The tolerance asked for, 0.0001 mm, is finer than the model's tolerance, 0.001 mm.";
        let json = format!(
            concat!(
                r#"{{"schema":1,"code":"measure.tolerance-below-model","severity":"error","message":"{}","entities":[],"#,
                r#""values":[{{"name":"requested","quantity":"length","value":0.0001}},"#,
                r#"{{"name":"model","quantity":"length","value":0.001}}],"#,
                r#""remedies":[{{"kind":"use-model-tolerance","entities":[],"#,
                r#""values":[{{"name":"tolerance","quantity":"length","value":0.001}}]}}]}}"#,
            ),
            words,
        );
        assert_eq!(got, Answer { json, words: words.to_string(), refused: true });
    }

    #[test]
    fn numbers_from_outside_are_checked_where_they_enter() {
        let code = |query: &DistanceQuery| Report::from_json(&distance_json(query).json).unwrap().code;
        for field in 0..3 {
            let mut bad = query(0.01);
            *bad.from.get_mut(field).unwrap() = f64::NAN;
            assert_eq!(code(&bad), Code::NotFinite, "from[{field}]");
            let mut bad = query(0.01);
            *bad.to.get_mut(field).unwrap() = f64::INFINITY;
            assert_eq!(code(&bad), Code::NotFinite, "to[{field}]");
        }
        assert_eq!(code(&query(f64::NAN)), Code::NotFinite);
        assert_eq!(code(&DistanceQuery { model_length_tolerance: f64::NAN, ..query(0.01) }), Code::NotFinite);
        assert_eq!(code(&DistanceQuery { model_angle_tolerance: f64::NAN, ..query(0.01) }), Code::NotFinite);
        assert_eq!(code(&DistanceQuery { model_length_tolerance: 0.0, ..query(0.01) }), Code::ToleranceOutOfRange);
        assert_eq!(code(&DistanceQuery { model_angle_tolerance: 0.0, ..query(0.01) }), Code::ToleranceOutOfRange);
    }
}
