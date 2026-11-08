//! What a gate says, for people and for agents (L1, L8): every finding has a stable code and says what to do about it.

use serde::Serialize;

/// The version of the `--json` form.
const SCHEMA: u32 = 1;

/// One violation.
#[derive(Clone, Debug, PartialEq, Eq, Serialize)]
pub struct Finding {
    /// A stable identifier, `<gate>.<kind>`.
    pub code: &'static str,
    /// The file it concerns, relative to the repository root.
    pub file: String,
    /// The line, where there is one.
    #[serde(skip_serializing_if = "Option::is_none")]
    pub line: Option<usize>,
    /// What is wrong.
    pub message: String,
    /// What to do about it.
    pub remedy: String,
}

impl Finding {
    pub fn new(code: &'static str, file: &str, message: impl Into<String>, remedy: impl Into<String>) -> Self {
        Self { code, file: file.to_string(), line: None, message: message.into(), remedy: remedy.into() }
    }

    pub fn at(self, line: usize) -> Self {
        Self { line: Some(line), ..self }
    }

    fn render(&self) -> String {
        let place = self.line.map_or_else(|| self.file.clone(), |line| format!("{}:{line}", self.file));
        format!("{place}: [{}] {}. Fix: {}", self.code, self.message, self.remedy)
    }
}

/// A gate's result: a one-line summary when it passes, the findings when it fails.
pub type Verdict = Result<String, Vec<Finding>>;

/// `Ok(summary)` when nothing was found.
pub fn verdict(summary: String, findings: Vec<Finding>) -> Verdict {
    if findings.is_empty() { Ok(summary) } else { Err(findings) }
}

#[derive(Serialize)]
struct GateReport<'a> {
    gate: &'a str,
    ok: bool,
    summary: &'a str,
    findings: &'a [Finding],
}

#[derive(Serialize)]
struct Report<'a> {
    schema: u32,
    ok: bool,
    gates: Vec<GateReport<'a>>,
}

/// Whether every gate passed.
pub fn all_ok(results: &[(&str, Verdict)]) -> bool {
    results.iter().all(|(_, verdict)| verdict.is_ok())
}

/// The `--json` form of `results`.
pub fn to_json(results: &[(&str, Verdict)]) -> Result<String, serde_json::Error> {
    let gates = results
        .iter()
        .map(|(gate, verdict)| match verdict {
            Ok(summary) => GateReport { gate, ok: true, summary, findings: &[] },
            Err(findings) => GateReport { gate, ok: false, summary: "", findings },
        })
        .collect();
    serde_json::to_string_pretty(&Report { schema: SCHEMA, ok: all_ok(results), gates })
}

/// The text form of `results`.
pub fn to_text(results: &[(&str, Verdict)]) -> String {
    let mut lines = Vec::new();
    for (gate, verdict) in results {
        match verdict {
            Ok(summary) => lines.push(format!("gates {gate}: ok ({summary})\n")),
            Err(findings) => {
                lines.push(format!("gates {gate}: FAILED\n"));
                lines.extend(findings.iter().map(|finding| format!("  {}\n", finding.render())));
            }
        }
    }
    lines.concat()
}

/// The codes of a failing verdict, in order; empty when it passes.
#[cfg(test)]
pub fn codes(verdict: &Verdict) -> Vec<&'static str> {
    verdict.as_ref().err().map_or_else(Vec::new, |findings| findings.iter().map(|f| f.code).collect())
}

#[cfg(test)]
mod tests {
    use super::*;

    fn results() -> Vec<(&'static str, Verdict)> {
        let finding = Finding::new("a.bad", "x.rs", "it is bad", "make it good").at(7);
        vec![
            ("a", Err(vec![finding, Finding::new("a.worse", "y.rs", "worse", "better")])),
            ("b", Ok("fine".to_string())),
        ]
    }

    #[test]
    fn the_verdict_is_ok_only_with_no_findings() {
        assert_eq!(verdict("s".to_string(), vec![]), Ok("s".to_string()));
        assert_eq!(codes(&verdict("s".to_string(), vec![Finding::new("a.b", "f", "m", "r")])), ["a.b"]);
        assert_eq!(codes(&Ok(String::new())), Vec::<&str>::new());
    }

    #[test]
    fn json_names_every_finding_with_its_remedy() {
        let value: serde_json::Value = serde_json::from_str(&to_json(&results()).unwrap()).unwrap();
        let want = serde_json::json!({
            "schema": 1,
            "ok": false,
            "gates": [
                {"gate": "a", "ok": false, "summary": "", "findings": [
                    {"code": "a.bad", "file": "x.rs", "line": 7, "message": "it is bad", "remedy": "make it good"},
                    {"code": "a.worse", "file": "y.rs", "message": "worse", "remedy": "better"},
                ]},
                {"gate": "b", "ok": true, "summary": "fine", "findings": []},
            ],
        });
        assert_eq!(value, want);
    }

    #[test]
    fn json_is_ok_when_every_gate_passes() {
        let value: serde_json::Value = serde_json::from_str(&to_json(&[("b", Ok("s".into()))]).unwrap()).unwrap();
        assert_eq!(value["ok"], true);
    }

    #[test]
    fn all_ok_needs_every_gate() {
        assert!(!all_ok(&results()));
        assert!(all_ok(&[("b", Ok(String::new()))]));
    }

    #[test]
    fn text_shows_place_code_message_and_remedy() {
        let want = "gates a: FAILED\n  x.rs:7: [a.bad] it is bad. Fix: make it good\n  y.rs: [a.worse] worse. Fix: better\n\
                    gates b: ok (fine)\n";
        assert_eq!(to_text(&results()), want);
    }
}
