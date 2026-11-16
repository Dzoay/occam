//! Source: original
//! `occam skill`: the skill for Claude Code that this binary carries, `skills/occam/SKILL.md` in the repository
//! (docs/decisions/2026-11-14-the-agent-starts-the-viewer.md), printed, or with `--install` copied into the current
//! directory's `.claude/skills/occam/`, where Claude Code finds a project's skills. A skill other than this build's that
//! is there already is left as it was unless `--force` is given.

use std::path::{Path, PathBuf};

use occam::{Code, Fault, Input, Install, Installed, Remedy, RemedyKind, Report, skill_json};

use crate::args::SkillArgs;
use crate::commands::{Said, whole};

/// The skill, as the repository holds it.
pub const SKILL: &str = include_str!("../../../skills/occam/SKILL.md");

/// Where in a project Claude Code finds the skill, from the project's directory.
const PLACE: [&str; 4] = [".claude", "skills", "occam", "SKILL.md"];

/// `occam skill`.
pub fn skill(args: &SkillArgs, json: bool) -> Said {
    if !args.install {
        let text = if json { skill_json(SKILL) } else { SKILL.trim_end().to_string() };
        return Said { lines: vec![(true, text)], status: 0 };
    }
    let file: PathBuf = PLACE.iter().collect();
    let installed = install(&std::path::absolute(&file).unwrap_or(file), args.force);
    let status = u8::from(!installed.ok());
    Said { lines: if json { vec![(true, installed.json())] } else { installed.words() }, status }
}

/// The skill written to `file`, unless another is there and `force` is not given.
fn install(file: &Path, force: bool) -> Installed {
    let shown = file.display().to_string();
    let done = match std::fs::read(file) {
        Ok(there) if there == SKILL.as_bytes() => {
            return Installed { file: shown, done: Some(Install::Unchanged), reports: vec![] };
        }
        Ok(_) if !force => return Installed { done: None, reports: vec![present(&shown)], file: shown },
        Ok(_) => Install::Replaced,
        Err(_) => Install::Written,
    };
    let made = file.parent().map_or(Ok(()), std::fs::create_dir_all);
    if made.and_then(|()| whole(file, SKILL.as_bytes())).is_err() {
        return Installed { done: None, reports: vec![not_written(&shown)], file: shown };
    }
    Installed { file: shown, done: Some(done), reports: vec![] }
}

/// `cli.skill-present` for `file`, with the remedy of giving `--force`.
fn present(file: &str) -> Report {
    Report::new(Code::SkillPresent, Fault::InvalidInput)
        .with_input(Input::at("--install").given(file))
        .with_remedy(Remedy::new(RemedyKind::UseForce))
}

/// `cli.skill-not-written` for `file`.
fn not_written(file: &str) -> Report {
    Report::new(Code::SkillNotWritten, Fault::InvalidInput).with_input(Input::at("--install").given(file))
}
