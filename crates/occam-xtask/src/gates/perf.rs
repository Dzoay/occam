//! `gates perf`: native instruction counts per operation class against a record (L31), and the browser build's simd128
//! speed-up against scalar, both measured in the same run (L30). Every other speed number is a goal, not a gate (L46).

use std::collections::BTreeMap;
use std::fs;
use std::path::{Path, PathBuf};

use serde::Deserialize;
use serde_json::Value;

use crate::repo::Repo;
use crate::verdict::{Finding, Verdict, verdict};

const RECORD: &str = "ci/perf-baseline.txt";
const BENCH: &str = "crates/occam/benches/operations.rs";
const WORKLOAD: &str = "crates/occam-wasm/examples/perf.rs";
const RUNNER: &str = "tools/browser-perf/run.mjs";
const UPDATE: &str = "if the change is intended, run `just perf --update` and say why in the PR";
const MUSL: &str = "x86_64-unknown-linux-musl";
const WASM: &str = "wasm32-unknown-unknown";
const SIMD128: &str = r#"target.wasm32-unknown-unknown.rustflags=["-Ctarget-feature=+simd128"]"#;
/// A count may reach this percentage of the record before it fails, and fall to `FLOOR` before the record must be lowered.
const CEILING: u64 = 102;
const FLOOR: u64 = 98;
/// The simd128 build must be at least this many times as fast as scalar. A build with no vector instructions runs at about
/// 1.0, so "not slower" would not notice lanes that stop vectorising. Timing the two builds in one run leaves a few percent of
/// noise, and the workload measures about 1.8, so this is well clear of both.
const MIN_GAIN: f64 = 1.25;

/// One browser run: how long each build took, and what each computed.
#[derive(Debug, PartialEq, Deserialize)]
struct Timing {
    scalar_ms: f64,
    simd128_ms: f64,
    scalar_result: f64,
    simd128_result: f64,
}

fn is_entry(line: &str) -> bool {
    !line.trim().is_empty() && !line.starts_with('#')
}

fn entry(line: &str) -> Option<(&str, u64)> {
    let mut words = line.split_whitespace();
    let (class, count) = (words.next()?, words.next()?);
    if words.next().is_some() {
        return None;
    }
    Some((class, count.parse().ok()?))
}

fn compared(class: &str, recorded: u64, count: u64) -> Option<Finding> {
    let hundredfold = count.saturating_mul(100);
    if hundredfold > recorded.saturating_mul(CEILING) {
        let message = format!("{class} takes {count} instructions; the record says {recorded}");
        return Some(Finding::new("perf.regression", BENCH, message, UPDATE));
    }
    if hundredfold < recorded.saturating_mul(FLOOR) {
        let message = format!("{class} takes {count} instructions, well under the record of {recorded}");
        return Some(Finding::new("perf.stale", RECORD, message, "lower the record: run `just perf --update`"));
    }
    None
}

fn check(record: &str, measured: &BTreeMap<String, u64>) -> Verdict {
    if measured.is_empty() {
        return Err(vec![Finding::new(
            "perf.no-measurements",
            BENCH,
            "no benchmark reported a count",
            "run `just perf` and read its output",
        )]);
    }
    let mut findings = Vec::new();
    let mut recorded = BTreeMap::new();
    for (line, number) in record.lines().zip(1..).filter(|(line, _)| is_entry(line)) {
        match entry(line) {
            Some((class, count)) => {
                recorded.insert(class, count);
            }
            None => findings.push(
                Finding::new(
                    "perf.malformed",
                    RECORD,
                    format!("cannot read `{line}`"),
                    "write `<class> <instructions>`",
                )
                .at(number),
            ),
        }
    }
    if !findings.is_empty() {
        return Err(findings);
    }
    for (class, count) in measured {
        match recorded.get(class.as_str()) {
            Some(want) => findings.extend(compared(class, *want, *count)),
            None => findings.push(Finding::new("perf.unrecorded", RECORD, format!("{class} has no record"), UPDATE)),
        }
    }
    for class in recorded.keys().filter(|class| !measured.contains_key(**class)) {
        findings.push(Finding::new(
            "perf.not-measured",
            BENCH,
            format!("{class} is recorded but was not measured"),
            UPDATE,
        ));
    }
    verdict(format!("{} operation classes within 2% of the record", measured.len()), findings)
}

/// Comparing the two builds of one run, so that the machine and its load cancel out (L30).
fn check_browser(timing: &Timing) -> Vec<Finding> {
    let mut findings = Vec::new();
    let gained = timing.scalar_ms >= timing.simd128_ms * MIN_GAIN;
    if !gained {
        let message = format!(
            "in the browser, simd128 took {} ms and scalar {} ms; simd128 must be {MIN_GAIN} times as fast",
            timing.simd128_ms, timing.scalar_ms
        );
        findings.push(Finding::new(
            "perf.simd128-no-gain",
            WORKLOAD,
            message,
            "find out why lanes do not vectorise (L30)",
        ));
    }
    if timing.simd128_result.to_bits() != timing.scalar_result.to_bits() {
        let message = format!("simd128 computed {} and scalar {}", timing.simd128_result, timing.scalar_result);
        findings.push(Finding::new("perf.simd128-differs", WORKLOAD, message, "see docs/DETERMINISM.md (L23)"));
    }
    findings
}

/// The class and the instructions from one of gungraun's `summary.json` files.
fn parse_summary(text: &str) -> Option<(String, u64)> {
    let value: Value = serde_json::from_str(text).ok()?;
    let count = value.get("profiles")?.get(0)?.pointer("/data/total/metrics/Ir/values/new")?.as_u64()?;
    Some((value.get("function_name")?.as_str()?.to_string(), count))
}

fn parse_timing(text: &str) -> Option<Timing> {
    serde_json::from_str(text).ok()
}

/// The record with its comments kept and every class rewritten, in order.
fn updated(record: &str, measured: &BTreeMap<String, u64>) -> String {
    let comments = record.lines().filter(|line| !is_entry(line)).map(|line| format!("{line}\n"));
    let classes = measured.iter().map(|(class, count)| format!("{class} {count}\n"));
    comments.chain(classes).collect()
}

fn summaries(dir: &Path, found: &mut Vec<PathBuf>) -> std::io::Result<()> {
    for item in fs::read_dir(dir)? {
        let path = item?.path();
        if path.is_dir() {
            summaries(&path, found)?;
        } else if path.file_name().is_some_and(|name| name == "summary.json") {
            found.push(path);
        }
    }
    Ok(())
}

/// Instruction counts of a static musl build, so that no machine's libc or CPU features change them.
fn counts(repo: &Repo) -> Result<BTreeMap<String, u64>, Finding> {
    let home = repo.nested_target("gungraun-perf");
    drop(fs::remove_dir_all(&home));
    let home_var = format!("GUNGRAUN_HOME={home}");
    let args = [
        home_var.as_str(),
        "GUNGRAUN_SAVE_SUMMARY=json",
        env!("CARGO"),
        "bench",
        "--quiet",
        "-p",
        "occam",
        "--bench",
        "operations",
        "--target",
        MUSL,
    ];
    repo.checked("perf.run-failed", "env", &args)?;
    let mut found = Vec::new();
    let failed = |e: std::io::Error| {
        Finding::new("perf.run-failed", BENCH, format!("reading {home}: {e}"), "run `just toolchain`")
    };
    summaries(Path::new(&home), &mut found).map_err(failed)?;
    let mut measured = BTreeMap::new();
    for path in found {
        let shown = path.display().to_string();
        let text = Repo::read_outside(&shown)?;
        let Some((class, count)) = parse_summary(&text) else {
            return Err(Finding::new(
                "perf.bad-summary",
                BENCH,
                format!("cannot read {shown}"),
                "check the gungraun version",
            ));
        };
        measured.insert(class, count);
    }
    Ok(measured)
}

/// Builds the workload one way and returns where its JavaScript went.
fn built(repo: &Repo, name: &str, config: Option<&str>) -> Result<String, Finding> {
    let dir = repo.nested_target(name);
    let mut args = vec![
        "build",
        "--quiet",
        "--release",
        "-p",
        "occam-wasm",
        "--example",
        "perf",
        "--target",
        WASM,
        "--target-dir",
        dir.as_str(),
    ];
    if let Some(config) = config {
        args.extend(["--config", config]);
    }
    repo.checked("perf.build-failed", env!("CARGO"), &args)?;
    let js = format!("{dir}/js");
    let wasm = format!("{dir}/{WASM}/release/examples/perf.wasm");
    repo.checked("perf.build-failed", "wasm-bindgen", &["--target", "web", "--out-dir", js.as_str(), wasm.as_str()])?;
    Ok(js)
}

fn browser(repo: &Repo) -> Result<Timing, Finding> {
    let scalar = built(repo, "perf-scalar", None)?;
    let simd = built(repo, "perf-simd128", Some(SIMD128))?;
    repo.checked("perf.browser-failed", "npm", &["ci", "--silent", "--prefix", "tools/browser-perf"])?;
    let (ok, output, errors) = repo.capture("node", &[RUNNER, scalar.as_str(), simd.as_str()])?;
    if !ok {
        return Err(Finding::new("perf.browser-failed", RUNNER, errors, "run `just setup` for Playwright's browser"));
    }
    parse_timing(&output).ok_or_else(|| {
        Finding::new("perf.browser-failed", RUNNER, format!("cannot read `{output}`"), "check the runner's output")
    })
}

fn measure(repo: &Repo) -> Verdict {
    let measured = counts(repo).map_err(|e| vec![e])?;
    let native = check(&repo.read(RECORD).map_err(|e| vec![e])?, &measured);
    let timing = browser(repo).map_err(|e| vec![e])?;
    let mut findings = native.as_ref().err().cloned().unwrap_or_default();
    findings.extend(check_browser(&timing));
    let ratio = timing.scalar_ms / timing.simd128_ms;
    let summary = format!(
        "{}; in the browser, scalar {:.1} ms, simd128 {:.1} ms, {ratio:.2} times as fast",
        native.unwrap_or_default(),
        timing.scalar_ms,
        timing.simd128_ms
    );
    verdict(summary, findings)
}

pub fn run(repo: &Repo, args: &[String]) -> Verdict {
    match args {
        [] => measure(repo),
        [flag] if flag == "--update" => {
            let measured = counts(repo).map_err(|e| vec![e])?;
            let record = repo.read(RECORD).map_err(|e| vec![e])?;
            repo.write(RECORD, &updated(&record, &measured)).map_err(|e| vec![e])?;
            Ok(format!("{RECORD} updated"))
        }
        _ => Err(vec![Finding::new(
            "gates.usage",
            "",
            format!("unexpected arguments: {}", args.join(" ")),
            "pass --update or nothing",
        )]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    const RECORD_TEXT: &str = "# machine: anything\nalpha 1000\nbeta 2000\n";

    fn measured(rows: &[(&str, u64)]) -> BTreeMap<String, u64> {
        rows.iter().map(|(name, count)| (name.to_string(), *count)).collect()
    }

    fn codes(result: &Verdict) -> Vec<&'static str> {
        match result {
            Ok(_) => Vec::new(),
            Err(findings) => findings.iter().map(|finding| finding.code).collect(),
        }
    }

    #[test]
    fn counts_within_two_percent_pass() {
        assert_eq!(codes(&check(RECORD_TEXT, &measured(&[("alpha", 1020), ("beta", 1960)]))), [""; 0]);
    }

    #[test]
    fn a_passing_run_says_how_many_classes_it_held() {
        let got = check(RECORD_TEXT, &measured(&[("alpha", 1000), ("beta", 2000)]));
        assert_eq!(got, Ok("2 operation classes within 2% of the record".to_string()));
    }

    #[test]
    fn a_count_more_than_two_percent_over_fails() {
        assert_eq!(codes(&check(RECORD_TEXT, &measured(&[("alpha", 1021), ("beta", 2000)]))), ["perf.regression"]);
    }

    #[test]
    fn a_count_more_than_two_percent_under_needs_the_record_lowered() {
        assert_eq!(codes(&check(RECORD_TEXT, &measured(&[("alpha", 979), ("beta", 2000)]))), ["perf.stale"]);
    }

    #[test]
    fn the_exact_limits_pass() {
        assert_eq!(codes(&check(RECORD_TEXT, &measured(&[("alpha", 980), ("beta", 2040)]))), [""; 0]);
    }

    #[test]
    fn a_class_with_no_record_fails() {
        let found = measured(&[("alpha", 1000), ("beta", 2000), ("gamma", 5)]);
        assert_eq!(codes(&check(RECORD_TEXT, &found)), ["perf.unrecorded"]);
    }

    #[test]
    fn a_recorded_class_that_was_not_measured_fails() {
        assert_eq!(codes(&check(RECORD_TEXT, &measured(&[("alpha", 1000)]))), ["perf.not-measured"]);
    }

    #[test]
    fn measuring_nothing_fails() {
        assert_eq!(codes(&check("# nothing\n", &measured(&[]))), ["perf.no-measurements"]);
    }

    #[test]
    fn a_line_that_cannot_be_read_fails() {
        for bad in ["alpha\n", "alpha lots\n", "alpha 1 2\n"] {
            assert_eq!(codes(&check(bad, &measured(&[("alpha", 1)]))), ["perf.malformed"], "{bad}");
        }
    }

    fn timing(scalar_ms: f64, simd128_ms: f64) -> Timing {
        Timing { scalar_ms, simd128_ms, scalar_result: 1.5, simd128_result: 1.5 }
    }

    fn browser_codes(timing: &Timing) -> Vec<&'static str> {
        check_browser(timing).iter().map(|finding| finding.code).collect()
    }

    #[test]
    fn simd128_clearly_faster_than_scalar_passes() {
        assert_eq!(browser_codes(&timing(100.0, 50.0)), [""; 0]);
        assert_eq!(browser_codes(&timing(125.0, 100.0)), [""; 0]);
    }

    #[test]
    fn a_build_that_gains_nothing_from_simd128_fails() {
        assert_eq!(browser_codes(&timing(124.9, 100.0)), ["perf.simd128-no-gain"]);
        assert_eq!(browser_codes(&timing(100.0, 100.0)), ["perf.simd128-no-gain"]);
        assert_eq!(browser_codes(&timing(100.0, 105.1)), ["perf.simd128-no-gain"]);
    }

    #[test]
    fn the_two_builds_must_compute_the_same_thing() {
        let differ = Timing { simd128_result: 1.25, ..timing(100.0, 50.0) };
        assert_eq!(browser_codes(&differ), ["perf.simd128-differs"]);
    }

    #[test]
    fn a_timing_that_is_not_a_number_fails() {
        assert_eq!(browser_codes(&timing(f64::NAN, 50.0)), ["perf.simd128-no-gain"]);
        assert_eq!(browser_codes(&timing(100.0, f64::NAN)), ["perf.simd128-no-gain"]);
    }

    const SUMMARY: &str = r#"{"function_name": "alpha", "profiles": [{"data": {"total": {"metrics": {"Ir": {"values": {"new": 2632}}}}}}]}"#;

    #[test]
    fn a_summary_gives_the_class_and_its_instructions() {
        assert_eq!(parse_summary(SUMMARY), Some(("alpha".to_string(), 2632)));
    }

    #[test]
    fn a_summary_without_the_count_is_refused() {
        assert_eq!(parse_summary(r#"{"function_name": "alpha", "profiles": []}"#), None);
        assert_eq!(
            parse_summary(r#"{"profiles": [{"data": {"total": {"metrics": {"Ir": {"values": {"new": 1}}}}}}]}"#),
            None
        );
        assert_eq!(parse_summary("not json"), None);
    }

    #[test]
    fn a_timing_is_read_from_the_runners_json() {
        let text = r#"{"scalar_ms": 12.5, "simd128_ms": 6.25, "scalar_result": 1.5, "simd128_result": 1.5}"#;
        let want = Timing { scalar_ms: 12.5, simd128_ms: 6.25, scalar_result: 1.5, simd128_result: 1.5 };
        assert_eq!(parse_timing(text), Some(want));
        assert_eq!(parse_timing(r#"{"scalar_ms": 12.5}"#), None);
        assert_eq!(parse_timing("not json"), None);
    }

    #[test]
    fn an_update_keeps_the_comments_and_writes_the_classes_in_order() {
        let got = updated("# machine: here\nold 1\n", &measured(&[("beta", 2), ("alpha", 1)]));
        assert_eq!(got, "# machine: here\nalpha 1\nbeta 2\n");
    }
}
