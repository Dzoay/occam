//! What the type system must refuse (L22). Each directory in `fixtures/compile_fail` holds `fails.rs`, whose first line names
//! the one error code it must fail with, and `compiles.rs`, a positive twin that must compile, so that a fixture cannot pass
//! just because a type does not exist yet.

use std::collections::BTreeSet;
use std::path::{Path, PathBuf};
use std::process::Command;
use std::{env, fs, process};

/// The expected code on a fixture's first line.
const EXPECT: &str = "// expect: ";

fn root() -> &'static Path {
    Path::new(concat!(env!("CARGO_MANIFEST_DIR"), "/../.."))
}

/// What compiling one file gave: nothing wrong, or the error codes it failed with.
#[derive(Debug, PartialEq, Eq)]
enum Compiled {
    Ok,
    Failed(BTreeSet<String>),
}

/// The error codes in rustc's `--error-format=json` output. Its closing "aborting due to" line is an error with no code, so
/// only the coded ones count, and a syntax error, which has none, can never match an expected code.
fn error_codes(stderr: &str) -> BTreeSet<String> {
    let diagnostics = stderr.lines().filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok());
    let errors = diagnostics.filter(|value| value["level"] == "error");
    errors.filter_map(|value| value["code"]["code"].as_str().map(str::to_string)).collect()
}

/// What is wrong with a fixture, given its expected code and what compiling its two files gave.
fn judge(expected: &str, fails: &Compiled, compiles: &Compiled) -> Option<String> {
    if let Compiled::Failed(codes) = compiles {
        return Some(format!("its positive twin must compile, but failed with {codes:?}"));
    }
    match fails {
        Compiled::Ok => Some(format!("it must fail with {expected}, but it compiled")),
        Compiled::Failed(codes) if codes.len() == 1 && codes.contains(expected) => None,
        Compiled::Failed(codes) => Some(format!("it must fail with {expected} alone, but failed with {codes:?}")),
    }
}

/// The compiled libraries the fixtures name, and the directory holding what those depend on.
struct Libraries {
    units: PathBuf,
    report: PathBuf,
    deps: PathBuf,
}

/// The `.rlib` cargo reported for the crate called `name` in its `--message-format=json` output.
fn rlib(stdout: &str, name: &str) -> PathBuf {
    let artifacts = stdout.lines().filter_map(|line| serde_json::from_str::<serde_json::Value>(line).ok());
    let wanted = artifacts.filter(|value| value["reason"] == "compiler-artifact" && value["target"]["name"] == name);
    let files: Vec<String> = wanted
        .flat_map(|value| value["filenames"].as_array().unwrap().clone())
        .map(|file| file.as_str().unwrap().to_string())
        .collect();
    PathBuf::from(
        files.iter().find(|file| Path::new(file).extension().is_some_and(|extension| extension == "rlib")).unwrap(),
    )
}

fn libraries() -> Libraries {
    let output = Command::new(env!("CARGO"))
        .args(["build", "--quiet", "-p", "occam-units", "--message-format=json"])
        .current_dir(root())
        .output()
        .unwrap();
    assert!(output.status.success(), "{}", String::from_utf8_lossy(&output.stderr));
    let stdout = String::from_utf8(output.stdout).unwrap();
    let report = rlib(&stdout, "occam_report");
    let deps = report.parent().unwrap().to_path_buf();
    Libraries { units: rlib(&stdout, "occam_units"), report, deps }
}

fn compile(file: &Path, libraries: &Libraries, out: &Path) -> Compiled {
    let rustc = env::var("RUSTC").unwrap_or_else(|_| "rustc".to_string());
    let externs =
        [format!("occam_units={}", libraries.units.display()), format!("occam_report={}", libraries.report.display())];
    let output = Command::new(rustc)
        .args(["--edition", "2024", "--crate-type", "lib", "--crate-name", "fixture", "--emit=metadata"])
        .args(["--error-format=json", "--out-dir"])
        .arg(out)
        .args(externs.iter().flat_map(|name| ["--extern", name]))
        .arg("-L")
        .arg(format!("dependency={}", libraries.deps.display()))
        .arg(file)
        .output()
        .unwrap();
    if output.status.success() {
        Compiled::Ok
    } else {
        Compiled::Failed(error_codes(&String::from_utf8_lossy(&output.stderr)))
    }
}

fn fixtures() -> Vec<PathBuf> {
    let directory = root().join("crates/occam-xtask/fixtures/compile_fail");
    let mut found: Vec<PathBuf> = fs::read_dir(directory).unwrap().map(|entry| entry.unwrap().path()).collect();
    found.sort();
    found
}

/// The problem with one fixture, named.
fn problem(fixture: &Path, libraries: &Libraries, out: &Path) -> Option<String> {
    let fails = fixture.join("fails.rs");
    let text = fs::read_to_string(&fails).unwrap();
    let expected = text.lines().next().and_then(|line| line.strip_prefix(EXPECT)).unwrap();
    let verdict =
        judge(expected, &compile(&fails, libraries, out), &compile(&fixture.join("compiles.rs"), libraries, out));
    verdict.map(|message| format!("{}: {message}", fixture.file_name().unwrap().to_string_lossy()))
}

#[test]
fn every_fixture_fails_with_its_code_and_its_twin_compiles() {
    let libraries = libraries();
    let out = env::temp_dir().join(format!("occam-compile-fail-{}", process::id()));
    fs::create_dir_all(&out).unwrap();
    let problems: Vec<String> = fixtures().iter().filter_map(|fixture| problem(fixture, &libraries, &out)).collect();
    fs::remove_dir_all(&out).unwrap();
    assert!(problems.is_empty(), "{}", problems.join("\n"));
}

#[test]
fn the_fixtures_the_plan_names_exist() {
    let names: Vec<String> =
        fixtures().iter().map(|path| path.file_name().unwrap().to_string_lossy().into_owned()).collect();
    for name in ["adding_a_world_point_to_a_uv_point", "passing_a_bare_f64_as_a_length"] {
        assert!(names.iter().any(|found| found == name), "{name} is missing from {names:?}");
    }
}

fn failed(codes: &[&str]) -> Compiled {
    Compiled::Failed(codes.iter().map(ToString::to_string).collect())
}

#[test]
fn the_expected_failure_and_a_compiling_twin_pass() {
    assert_eq!(judge("E0308", &failed(&["E0308"]), &Compiled::Ok), None);
}

#[test]
fn a_fixture_that_compiles_is_a_problem() {
    assert_eq!(
        judge("E0308", &Compiled::Ok, &Compiled::Ok),
        Some("it must fail with E0308, but it compiled".to_string())
    );
}

#[test]
fn the_wrong_code_is_a_problem_so_a_missing_type_cannot_pass() {
    let got = judge("E0308", &failed(&["E0433"]), &Compiled::Ok).unwrap();
    assert!(got.contains("alone") && got.contains("E0433"), "{got}");
}

#[test]
fn a_second_code_alongside_the_expected_one_is_a_problem() {
    assert!(judge("E0308", &failed(&["E0308", "E0599"]), &Compiled::Ok).is_some());
}

#[test]
fn a_twin_that_does_not_compile_is_a_problem() {
    let got = judge("E0308", &failed(&["E0308"]), &failed(&["E0433"])).unwrap();
    assert!(got.contains("positive twin") && got.contains("E0433"), "{got}");
}

#[test]
fn only_coded_errors_count() {
    let stderr = concat!(
        r#"{"$message_type":"diagnostic","level":"warning","code":{"code":"unused_imports"}}"#,
        "\n",
        r#"{"$message_type":"diagnostic","level":"error","code":{"code":"E0277"}}"#,
        "\n",
        r#"{"$message_type":"diagnostic","level":"error","code":null,"message":"aborting due to 1 previous error"}"#,
        "\nnot json\n",
    );
    assert_eq!(error_codes(stderr), BTreeSet::from(["E0277".to_string()]));
}
