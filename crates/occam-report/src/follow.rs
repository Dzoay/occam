//! Source: original
//! A suggested fix is a claim, so each one has a test that follows it (L2). `gates errors` finds every use of
//! [`followed_remedy!`](crate::followed_remedy) and holds the catalogue to them.

use crate::{Code, Remedy, RemedyKind, Report};

/// Follows the remedy of `kind` in `report` by calling `retry` with its parameters, and says what went wrong if the report is
/// not the one claimed or the retry fails. [`followed_remedy!`](crate::followed_remedy) is the way to use this in a test.
///
/// # Errors
///
/// A description of the problem if `report` does not have `code`, the catalogue does not pair `code` with `kind`, `report`
/// does not offer that remedy, or `retry` fails.
pub fn follow_remedy<T>(
    report: &Report,
    code: Code,
    kind: RemedyKind,
    retry: impl FnOnce(&Remedy) -> Result<T, Report>,
) -> Result<T, String> {
    if report.code != code {
        return Err(format!("the report's code is {}, not {}", report.code.text(), code.text()));
    }
    if !code.entry().remedies.contains(&kind) {
        return Err(format!("the catalogue does not pair {} with {}", code.text(), kind.text()));
    }
    let Some(remedy) = report.remedy(kind) else {
        return Err(format!("the report does not offer {}", kind.text()));
    };
    retry(remedy).map_err(|failed| format!("following {} failed again with {}", kind.text(), failed.code.text()))
}

/// In a test: receives a [`Report`], follows its remedy with the parameters it carries, and asserts that the retry succeeds.
/// `gates errors` requires one of these for every code and remedy pair in the catalogue (L2).
#[macro_export]
macro_rules! followed_remedy {
    ($report:expr, $code:expr, $kind:expr, $retry:expr $(,)?) => {{
        let followed = $crate::follow_remedy(&$report, $code, $kind, $retry);
        assert!(followed.is_ok(), "{:?}", followed.err());
    }};
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::NamedValue;

    fn report() -> Report {
        Report::new(Code::UnsupportedSchema)
            .with_remedy(Remedy::new(RemedyKind::UseSchema).with_value(NamedValue::count("version", 1)))
    }

    #[test]
    fn the_retry_receives_the_reports_parameters() {
        let version = follow_remedy(&report(), Code::UnsupportedSchema, RemedyKind::UseSchema, |remedy| {
            Ok(remedy.value("version").map(|v| v.value))
        });
        assert_eq!(version, Ok(Some(1.0)));
    }

    #[test]
    fn the_wrong_code_is_refused() {
        let got = follow_remedy(&report(), Code::Malformed, RemedyKind::UseSchema, |_| Ok(()));
        assert_eq!(got, Err("the report's code is report.unsupported-schema, not report.malformed".to_string()));
    }

    #[test]
    fn a_pair_the_catalogue_does_not_have_is_refused() {
        let plain = Report::new(Code::Malformed);
        let got = follow_remedy(&plain, Code::Malformed, RemedyKind::UseSchema, |_| Ok(()));
        assert_eq!(got, Err("the catalogue does not pair report.malformed with use-schema".to_string()));
    }

    #[test]
    fn a_report_that_does_not_offer_the_remedy_is_refused() {
        let bare = Report::new(Code::UnsupportedSchema);
        let got = follow_remedy(&bare, Code::UnsupportedSchema, RemedyKind::UseSchema, |_| Ok(()));
        assert_eq!(got, Err("the report does not offer use-schema".to_string()));
    }

    #[test]
    fn a_retry_that_fails_again_is_reported() {
        let got: Result<(), String> = follow_remedy(&report(), Code::UnsupportedSchema, RemedyKind::UseSchema, |_| {
            Err(Report::new(Code::Malformed))
        });
        assert_eq!(got, Err("following use-schema failed again with report.malformed".to_string()));
    }

    #[test]
    #[should_panic(expected = "the report does not offer use-schema")]
    fn the_macro_fails_the_test_when_the_remedy_does_not_work() {
        followed_remedy!(
            Report::new(Code::UnsupportedSchema),
            Code::UnsupportedSchema,
            RemedyKind::UseSchema,
            |_| Ok::<(), Report>(())
        );
    }
}
