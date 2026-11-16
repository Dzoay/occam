//! Source: original
//! `occam skill` as a person or an agent runs it in a project (docs/decisions/2026-11-14-the-agent-starts-the-viewer.md):
//! it prints the skill the repository holds, installs it where Claude Code finds a project's skills and says where,
//! says so when it is there already, leaves another skill as it was unless told to replace it, and names a project it
//! cannot write into.
#![cfg(not(target_family = "wasm"))]
#![allow(clippy::unwrap_used, reason = "a failed step should fail the test")]
#![allow(clippy::indexing_slicing, reason = "the answers a test reads, where a step that fails should fail the test")]

use std::path::{Path, PathBuf};
use std::process::{Command, Output};

use occam::{Code, RemedyKind, Report, followed_remedy};
use serde_json::{Value, json};

/// The skill, as the repository holds it.
const SKILL: &str = include_str!("../../../skills/occam/SKILL.md");

/// A skill of the person's own, where occam's would go.
const THEIRS: &str = "---\nname: occam\ndescription: The person's own.\n---\n\nTheirs.\n";

/// What `occam` says to `arguments`, run in the project `dir`.
fn occam_in(dir: &Path, arguments: &[&str]) -> Output {
    Command::new(env!("CARGO_BIN_EXE_occam")).args(arguments).current_dir(dir).output().unwrap()
}

/// What `occam` answers to `arguments` with `--json`, run in the project `dir`: its exit status and its JSON.
fn json_in(dir: &Path, arguments: &[&str]) -> (Option<i32>, Value) {
    let output = occam_in(dir, &[arguments, &["--json"]].concat());
    let text = String::from_utf8(output.stdout).unwrap();
    (output.status.code(), serde_json::from_str(&text).map_err(|e| format!("{e}: {text}")).unwrap())
}

/// An empty project directory of the test's own.
fn project(name: &str) -> PathBuf {
    let dir = Path::new(env!("CARGO_TARGET_TMPDIR")).join(format!("skill-{name}"));
    let _ = std::fs::remove_dir_all(&dir);
    std::fs::create_dir_all(&dir).unwrap();
    dir
}

/// Where the skill goes in the project `dir`.
fn skill_file(dir: &Path) -> PathBuf {
    dir.join(".claude").join("skills").join("occam").join("SKILL.md")
}

/// `file`'s text, as a string for the JSON it is named in.
fn shown(file: &Path) -> String {
    file.display().to_string()
}

#[test]
fn occam_skill_prints_the_skill_the_repository_holds_in_claude_codes_form() {
    let dir = project("print");
    let output = occam_in(&dir, &["skill"]);
    assert_eq!(output.status.code(), Some(0));
    assert_eq!(String::from_utf8(output.stdout).unwrap(), format!("{}\n", SKILL.trim_end()));
    let front = SKILL.strip_prefix("---\n").and_then(|rest| rest.split_once("\n---\n")).unwrap().0;
    let fields: Vec<(&str, &str)> = front.lines().map(|line| line.split_once(": ").unwrap()).collect();
    assert_eq!(fields.iter().map(|(name, _)| *name).collect::<Vec<_>>(), ["name", "description"]);
    let (name, description) = (fields[0].1, fields[1].1);
    assert_eq!(name, "occam");
    assert!(description.len() <= 1024 && description.contains("viewer"), "{description}");
    assert_eq!(json_in(&dir, &["skill"]), (Some(0), json!({"schema": 2, "ok": true, "skill": SKILL})));
    assert!(std::fs::read_dir(&dir).unwrap().next().is_none(), "printing writes nothing");
}

#[test]
fn install_writes_the_skill_where_claude_code_finds_it_and_says_where() {
    let dir = project("install");
    let file = skill_file(&dir);
    let output = occam_in(&dir, &["skill", "--install"]);
    assert_eq!(output.status.code(), Some(0), "{}", String::from_utf8_lossy(&output.stderr));
    let said = String::from_utf8(output.stdout).unwrap();
    assert_eq!(said, format!("The occam skill is installed at {}, where Claude Code finds it.\n", file.display()));
    assert_eq!(std::fs::read_to_string(&file).unwrap(), SKILL);
    let again = json!({"schema": 2, "ok": true, "file": shown(&file), "installed": "unchanged", "reports": []});
    assert_eq!(json_in(&dir, &["skill", "--install"]), (Some(0), again), "there already, so nothing is written");
    let names: Vec<_> = std::fs::read_dir(file.parent().unwrap()).unwrap().map(|e| e.unwrap().file_name()).collect();
    assert_eq!(names, ["SKILL.md"], "and nothing is left beside it");
}

#[test]
fn another_skill_is_left_as_it_was_and_following_the_remedy_replaces_it() {
    let dir = project("present");
    let file = skill_file(&dir);
    std::fs::create_dir_all(file.parent().unwrap()).unwrap();
    std::fs::write(&file, THEIRS).unwrap();
    let output = occam_in(&dir, &["skill", "--install"]);
    assert_eq!((output.status.code(), output.stdout.as_slice()), (Some(1), &b""[..]));
    let want = format!(
        "{} holds a skill other than this build's occam skill, so it was left as it was; --force replaces it.\n",
        file.display()
    );
    assert_eq!(String::from_utf8(output.stderr).unwrap(), want);
    let (status, said) = json_in(&dir, &["skill", "--install"]);
    assert_eq!((status, &said["ok"], &said["installed"]), (Some(1), &json!(false), &Value::Null), "{said}");
    assert_eq!(std::fs::read_to_string(&file).unwrap(), THEIRS, "left as it was");
    let report = Report::from_json(&said["reports"][0].to_string()).unwrap();
    assert_eq!(report.input.as_ref().and_then(|input| input.given.clone()), Some(shown(&file)));
    followed_remedy!(report, Code::SkillPresent, RemedyKind::UseForce, |_| {
        let (status, said) = json_in(&dir, &["skill", "--install", "--force"]);
        match status {
            Some(0) if said["installed"] == "replaced" => Ok(()),
            _ => Err(Report::from_json(&said["reports"][0].to_string()).unwrap()),
        }
    });
    assert_eq!(std::fs::read_to_string(&file).unwrap(), SKILL, "replaced with this build's");
    std::fs::write(&file, THEIRS).unwrap();
    let output = occam_in(&dir, &["skill", "--install", "--force"]);
    let replaced = format!("The skill at {} is replaced with this build's occam skill.\n", file.display());
    assert_eq!((output.status.code(), String::from_utf8(output.stdout).unwrap()), (Some(0), replaced));
    assert_eq!(
        occam_in(&dir, &["skill", "--force"]).status.code(),
        Some(2),
        "--force replaces only what --install would"
    );
}

#[test]
fn a_project_the_skill_cannot_be_written_into_is_named() {
    let dir = project("not-written");
    let file = skill_file(&dir);
    let place = file.parent().unwrap();
    std::fs::create_dir_all(place.parent().unwrap()).unwrap();
    std::fs::write(place, "a file where the skill's directory would be").unwrap();
    let (status, said) = json_in(&dir, &["skill", "--install"]);
    assert_eq!((status, &said["file"], &said["installed"]), (Some(1), &json!(shown(&file)), &Value::Null), "{said}");
    assert_eq!(said["reports"][0]["code"], "cli.skill-not-written", "{said}");
    assert_eq!(said["reports"][0]["remedies"], json!([]), "the directory is the person's to mend");
    let output = occam_in(&dir, &["skill", "--install"]);
    let want = format!("The occam skill could not be written to {}.\n", file.display());
    assert_eq!(String::from_utf8(output.stderr).unwrap(), want);
}
