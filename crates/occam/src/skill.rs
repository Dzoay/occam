//! Source: original
//! `occam skill`'s answers in their one JSON form and in words: the skill for Claude Code that the command carries, and
//! what installing it into a project did (docs/decisions/2026-11-14-the-agent-starts-the-viewer.md). The command line
//! reads and writes the files; this is only what it says.

use occam_report::{DisplayUnits, Report, SCHEMA};
use serde_json::{Value, json};

use crate::run::report_value;

/// What `occam skill --install` did with the project's skill file.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Install {
    /// There was none, and this build's skill was written.
    Written,
    /// This build's skill was there already, so nothing was written.
    Unchanged,
    /// Another was there, and `--force` replaced it with this build's.
    Replaced,
}

impl Install {
    /// Its name in the JSON form.
    #[must_use]
    pub const fn text(self) -> &'static str {
        match self {
            Self::Written => "written",
            Self::Unchanged => "unchanged",
            Self::Replaced => "replaced",
        }
    }
}

/// What `occam skill --install` answered: the skill file, and what was done with it or why nothing was.
#[derive(Clone, Debug, PartialEq)]
pub struct Installed {
    /// The skill file, as an absolute path.
    pub file: String,
    /// What was done, unless the command was refused.
    pub done: Option<Install>,
    /// What refused it.
    pub reports: Vec<Report>,
}

impl Installed {
    /// Whether it was installed, or was there already.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.done.is_some() && self.reports.is_empty()
    }

    /// Its one JSON form: `{"schema", "ok", "file", "installed", "reports"}`, `installed` null when refused.
    #[must_use]
    pub fn json(&self) -> String {
        let reports: Vec<Value> = self.reports.iter().map(|r| report_value(r, DisplayUnits::default())).collect();
        let installed = self.done.map(Install::text);
        json!({"schema": SCHEMA, "ok": self.ok(), "file": self.file, "installed": installed, "reports": reports})
            .to_string()
    }

    /// It in words, a line each, with whether each goes to standard output: what was done, or each report, for
    /// standard error.
    #[must_use]
    pub fn words(&self) -> Vec<(bool, String)> {
        let file = &self.file;
        let said = self.done.map(|done| match done {
            Install::Written => format!("The occam skill is installed at {file}, where Claude Code finds it."),
            Install::Unchanged => format!("The occam skill at {file} is this build's already, so nothing was written."),
            Install::Replaced => format!("The skill at {file} is replaced with this build's occam skill."),
        });
        let refused = self.reports.iter().map(|report| (false, report.message(DisplayUnits::default())));
        said.map(|line| (true, line)).into_iter().chain(refused).collect()
    }
}

/// `occam skill --json`: the skill's text in the one JSON form, `{"schema", "ok", "skill"}`.
#[must_use]
pub fn skill_json(skill: &str) -> String {
    json!({"schema": SCHEMA, "ok": true, "skill": skill}).to_string()
}

#[cfg(test)]
mod tests {
    use occam_report::{Code, Fault, Input, Remedy, RemedyKind};

    use super::*;

    fn json_of(text: &str) -> Value {
        serde_json::from_str(text).unwrap_or_else(|e| panic!("{e}: {text}"))
    }

    fn installed(done: Option<Install>, reports: Vec<Report>) -> Installed {
        Installed { file: "/p/.claude/skills/occam/SKILL.md".to_string(), done, reports }
    }

    #[test]
    fn what_was_done_is_said_in_json_and_in_words() {
        let file = "/p/.claude/skills/occam/SKILL.md";
        for (done, line) in [
            (Install::Written, format!("The occam skill is installed at {file}, where Claude Code finds it.")),
            (Install::Unchanged, format!("The occam skill at {file} is this build's already, so nothing was written.")),
            (Install::Replaced, format!("The skill at {file} is replaced with this build's occam skill.")),
        ] {
            let answer = installed(Some(done), vec![]);
            assert!(answer.ok());
            let want = json!({"schema": 2, "ok": true, "file": file, "installed": done.text(), "reports": []});
            assert_eq!(json_of(&answer.json()), want);
            assert_eq!(answer.words(), [(true, line)]);
        }
    }

    #[test]
    fn a_refusal_names_no_installation_and_its_report_goes_to_standard_error() {
        let report = Report::new(Code::SkillPresent, Fault::InvalidInput)
            .with_input(Input::at("--install").given("/p/.claude/skills/occam/SKILL.md"))
            .with_remedy(Remedy::new(RemedyKind::UseForce));
        let answer = installed(None, vec![report.clone()]);
        assert!(!answer.ok());
        let said = json_of(&answer.json());
        assert_eq!((&said["ok"], &said["installed"]), (&json!(false), &Value::Null));
        assert_eq!(said["reports"][0]["code"], "cli.skill-present");
        assert_eq!(said["reports"][0]["remedies"][0]["kind"], "use-force");
        assert_eq!(answer.words(), [(false, report.message(DisplayUnits::default()))]);
    }

    #[test]
    fn the_skill_is_given_whole_in_json() {
        let said = json_of(&skill_json("---\nname: occam\n---\n# A skill\n"));
        assert_eq!(said, json!({"schema": 2, "ok": true, "skill": "---\nname: occam\n---\n# A skill\n"}));
    }
}
