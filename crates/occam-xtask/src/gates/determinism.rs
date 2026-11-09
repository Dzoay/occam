//! `gates determinism`: the same inputs give identical bits on every build the contract covers (L23,
//! `docs/DETERMINISM.md`). The stream's probe is built and run as each of them, and every hash is compared.

use std::collections::{BTreeMap, BTreeSet};
use std::thread;

use super::no_args;
use crate::repo::Repo;
use crate::verdict::{Finding, Verdict, verdict};

const STREAM: &str = "crates/occam-math/src/stream.rs";
const RECORDED: &str = "ci/determinism-hashes.txt";
const FIX: &str = "see docs/DETERMINISM.md; run `just gate determinism` to reproduce";
const RECORD_FIX: &str = "a result changed, which docs/DETERMINISM.md does not allow; if the stream's definition changed on purpose, re-derive the hashes separately and say so in the PR";
const WASM: &str = "wasm32-wasip2";
const SIMD128: &str = r#"target.wasm32-wasip2.rustflags=["-Ctarget-feature=+simd128"]"#;

/// Every way one operation is run in one build.
const EXECUTORS: [&str; 4] = ["scalar", "lanes2", "lanes4", "lanes8"];

/// One way the probe is built and run.
struct Build {
    name: String,
    /// The target triple, or the host's when `None`.
    triple: Option<&'static str>,
    arch: &'static str,
    simd128: bool,
    threads: usize,
}

/// Every build the contract covers. They are release builds, as what ships is; the test suite covers the debug ones.
fn builds(threads: usize) -> [Build; 4] {
    let native = |threads: usize| Build {
        name: format!("x86-64, {threads} thread(s)"),
        triple: None,
        arch: "x86_64",
        simd128: false,
        threads,
    };
    let wasm = |name: &str, simd128: bool| Build {
        name: name.to_string(),
        triple: Some(WASM),
        arch: "wasm32",
        simd128,
        threads: 1,
    };
    [native(1), native(threads), wasm("wasm32, baseline", false), wasm("wasm32, simd128", true)]
}

/// Cargo's arguments for `build`. The simd128 build has a target directory of its own, because its flags differ.
fn cargo_args<'a>(build: &'a Build, simd_dir: &'a str, threads: &'a str) -> Vec<&'a str> {
    let mut args = vec!["run", "--quiet", "--release", "-p", "occam-math", "--example", "determinism"];
    if let Some(triple) = build.triple {
        args.extend(["--target", triple]);
    }
    if build.simd128 {
        args.extend(["--target-dir", simd_dir, "--config", SIMD128]);
    }
    args.extend(["--", "--threads", threads]);
    args
}

pub fn run(repo: &Repo, args: &[String]) -> Verdict {
    no_args(args)?;
    let threads = thread::available_parallelism().map_or(2, |count| count.get().max(2));
    let simd_dir = repo.nested_target("simd");
    let mut runs = Vec::new();
    for build in builds(threads) {
        let count = build.threads.to_string();
        let (ok, output, errors) =
            repo.capture(env!("CARGO"), &cargo_args(&build, &simd_dir, &count)).map_err(|e| vec![e])?;
        if !ok {
            let message = format!("{} did not run: {errors}", build.name);
            return Err(vec![Finding::new(
                "determinism.run-failed",
                STREAM,
                message,
                "run `just toolchain`, then fix what it reports",
            )]);
        }
        runs.push((build, output));
    }
    check(&runs, &repo.read(RECORDED).map_err(|e| vec![e])?)
}

/// What one run printed: a hash for each (operation, executor).
type Rows<'a> = BTreeMap<(&'a str, &'a str), &'a str>;

fn malformed(build: &Build, line: &str) -> Finding {
    Finding::new("determinism.malformed", STREAM, format!("{}: cannot read `{line}`", build.name), FIX)
}

/// The hashes one run printed, after checking that it really was the build it was meant to be.
fn parse<'a>(build: &Build, output: &'a str, findings: &mut Vec<Finding>) -> Rows<'a> {
    let mut lines = output.lines();
    let want = format!("build {} simd128={} threads={}", build.arch, build.simd128, build.threads);
    match lines.next() {
        Some(line) if line == want => {}
        Some(line) if line.starts_with("build ") => {
            let message = format!("{}: the probe says `{line}`, not `{want}`", build.name);
            findings.push(Finding::new("determinism.wrong-build", STREAM, message, FIX));
        }
        line => findings.push(malformed(build, line.unwrap_or("(nothing)"))),
    }
    let mut rows = Rows::new();
    for line in lines {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            [operation, executor, hash] if EXECUTORS.contains(executor) => {
                rows.insert((*operation, *executor), *hash);
            }
            _ => findings.push(malformed(build, line)),
        }
    }
    rows
}

/// A finding for every hash a build should have printed and did not.
fn missing(parsed: &[(&Build, Rows)], operations: &BTreeSet<&str>) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (build, rows) in parsed {
        for operation in operations {
            for executor in EXECUTORS.iter().filter(|executor| !rows.contains_key(&(*operation, **executor))) {
                let message = format!("{} printed no hash for {operation} {executor}", build.name);
                findings.push(Finding::new("determinism.missing", STREAM, message, FIX));
            }
        }
    }
    findings
}

/// One hash and who gave it, naming at most three sources.
fn describe(hash: &str, sources: &[String]) -> String {
    let shown: Vec<&str> = sources.iter().take(3).map(String::as_str).collect();
    let more = sources.len().saturating_sub(shown.len());
    let tail = if more == 0 { String::new() } else { format!(" and {more} more") };
    format!("{hash} from {}{tail}", shown.join(", "))
}

/// Each hash printed for `operation`, with who printed it.
fn hashes_of<'a>(parsed: &[(&Build, Rows<'a>)], operation: &str) -> BTreeMap<&'a str, Vec<String>> {
    let mut hashes: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (build, rows) in parsed {
        for executor in EXECUTORS {
            if let Some(hash) = rows.get(&(operation, executor)) {
                hashes.entry(*hash).or_default().push(format!("{} {executor}", build.name));
            }
        }
    }
    hashes
}

/// The recorded hash of each operation, from [`RECORDED`].
fn recorded<'a>(text: &'a str, findings: &mut Vec<Finding>) -> BTreeMap<&'a str, &'a str> {
    let mut hashes = BTreeMap::new();
    for line in text.lines().filter(|line| !line.trim().is_empty() && !line.starts_with('#')) {
        if let [operation, hash] = line.split_whitespace().collect::<Vec<_>>().as_slice() {
            hashes.insert(*operation, *hash);
        } else {
            let message = format!("cannot read `{line}`");
            findings.push(Finding::new("determinism.malformed", RECORDED, message, "write `<operation> <hash>`"));
        }
    }
    hashes
}

/// What is wrong with `operation`: more than one hash, or one that is not the recorded one.
fn judge(parsed: &[(&Build, Rows)], operation: &str, recorded: &BTreeMap<&str, &str>) -> Option<Finding> {
    let hashes = hashes_of(parsed, operation);
    if hashes.len() > 1 {
        let groups: Vec<String> = hashes.iter().map(|(hash, sources)| describe(hash, sources)).collect();
        let message = format!("{operation} differs: {}", groups.join("; "));
        return Some(Finding::new("determinism.mismatch", STREAM, message, FIX));
    }
    let hash = hashes.into_keys().next()?;
    match recorded.get(operation) {
        Some(want) if *want == hash => None,
        Some(want) => {
            let message = format!("every build gives {operation} {hash}, but {want} is recorded");
            Some(Finding::new("determinism.not-recorded", RECORDED, message, RECORD_FIX))
        }
        None => {
            let message = format!("{operation} has no recorded hash");
            Some(Finding::new("determinism.unrecorded", RECORDED, message, RECORD_FIX))
        }
    }
}

fn check(runs: &[(Build, String)], recorded_text: &str) -> Verdict {
    if runs.is_empty() {
        return Err(vec![Finding::new("determinism.no-builds", STREAM, "no build was compared", FIX)]);
    }
    let mut findings = Vec::new();
    let recorded = recorded(recorded_text, &mut findings);
    let parsed: Vec<(&Build, Rows)> =
        runs.iter().map(|(build, output)| (build, parse(build, output, &mut findings))).collect();
    let operations: BTreeSet<&str> =
        parsed.iter().flat_map(|(_, rows)| rows.keys().map(|(operation, _)| *operation)).collect();
    if operations.is_empty() {
        findings.push(Finding::new("determinism.no-hashes", STREAM, "no build printed a hash", FIX));
    }
    findings.extend(missing(&parsed, &operations));
    findings.extend(operations.iter().filter_map(|operation| judge(&parsed, operation, &recorded)));
    for operation in recorded.keys().filter(|operation| !operations.contains(*operation)) {
        let message = format!("{operation} is recorded but no build printed it");
        findings.push(Finding::new("determinism.missing", RECORDED, message, FIX));
    }
    let summary = format!(
        "{} builds agree with the record on {} operations, {} executors each",
        runs.len(),
        operations.len(),
        EXECUTORS.len()
    );
    verdict(summary, findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verdict::codes;

    const OPERATIONS: [&str; 2] = ["add", "sqrt"];
    const GOOD: &str = "# what the stream must hash to\n\nadd 00000000000000aa\nsqrt 00000000000000aa\n";

    fn output(build: &Build, hash: &str) -> String {
        let header = format!("build {} simd128={} threads={}", build.arch, build.simd128, build.threads);
        let rows = OPERATIONS
            .iter()
            .flat_map(|operation| EXECUTORS.iter().map(move |executor| format!("{operation} {executor} {hash}")));
        let lines: Vec<String> = std::iter::once(header).chain(rows).collect();
        lines.join("\n") + "\n"
    }

    fn agreeing() -> Vec<(Build, String)> {
        builds(4)
            .into_iter()
            .map(|build| {
                let text = output(&build, "00000000000000aa");
                (build, text)
            })
            .collect()
    }

    #[test]
    fn hashes_that_agree_pass() {
        assert_eq!(
            check(&agreeing(), GOOD),
            Ok("4 builds agree with the record on 2 operations, 4 executors each".to_string())
        );
    }

    #[test]
    fn one_flipped_hash_fails_naming_the_operation_and_the_builds() {
        let mut runs = agreeing();
        runs[2].1 = runs[2].1.replace("sqrt lanes4 00000000000000aa", "sqrt lanes4 00000000000000bb");
        let found = check(&runs, GOOD).unwrap_err();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].code, "determinism.mismatch");
        assert_eq!(found[0].file, STREAM);
        assert!(found[0].message.contains("sqrt"), "{found:?}");
        let message = &found[0].message;
        assert!(message.starts_with("sqrt differs: 00000000000000aa from x86-64, 1 thread(s) scalar"), "{found:?}");
        assert!(message.contains("and 12 more; "), "{found:?}");
        assert!(message.ends_with("00000000000000bb from wasm32, baseline lanes4"), "{found:?}");
    }

    #[test]
    fn scalar_and_lanes_must_agree_within_one_build() {
        let mut runs = agreeing();
        for (_, text) in &mut runs {
            *text = text.replace("add lanes2 00000000000000aa", "add lanes2 00000000000000cc");
        }
        assert_eq!(codes(&check(&runs, GOOD)), ["determinism.mismatch"]);
    }

    #[test]
    fn thread_counts_must_agree() {
        let mut runs = agreeing();
        runs[1].1 = runs[1].1.replace("add scalar 00000000000000aa", "add scalar 00000000000000dd");
        let found = check(&runs, GOOD).unwrap_err();
        assert_eq!(found[0].code, "determinism.mismatch");
        assert!(found[0].message.contains("00000000000000dd from x86-64, 4 thread(s) scalar"), "{found:?}");
    }

    #[test]
    fn every_build_agreeing_on_a_hash_that_is_not_recorded_fails() {
        let found = check(&agreeing(), "add 00000000000000ff\nsqrt 00000000000000aa\n").unwrap_err();
        assert_eq!(codes(&Err(found.clone())), ["determinism.not-recorded"]);
        assert_eq!(found[0].file, RECORDED);
        assert_eq!(found[0].message, "every build gives add 00000000000000aa, but 00000000000000ff is recorded");
    }

    #[test]
    fn an_operation_with_no_recorded_hash_fails() {
        let found = check(&agreeing(), "add 00000000000000aa\n").unwrap_err();
        assert_eq!(codes(&Err(found.clone())), ["determinism.unrecorded"]);
        assert_eq!(found[0].message, "sqrt has no recorded hash");
    }

    #[test]
    fn a_recorded_operation_no_build_printed_fails() {
        let found =
            check(&agreeing(), "add 00000000000000aa\nsqrt 00000000000000aa\ncos 00000000000000aa\n").unwrap_err();
        assert_eq!(codes(&Err(found.clone())), ["determinism.missing"]);
        assert_eq!(found[0].file, RECORDED);
        assert!(found[0].message.starts_with("cos "), "{found:?}");
    }

    #[test]
    fn a_recorded_line_that_is_not_two_words_is_malformed() {
        let found = check(&agreeing(), "add 00000000000000aa\nsqrt\n").unwrap_err();
        let which: Vec<(&str, &str)> = found.iter().map(|f| (f.code, f.file.as_str())).collect();
        assert!(which.contains(&("determinism.malformed", RECORDED)), "{found:?}");
    }

    #[test]
    fn no_builds_is_a_failure() {
        assert_eq!(codes(&check(&[], GOOD)), ["determinism.no-builds"]);
    }

    #[test]
    fn a_build_with_no_hashes_is_a_failure() {
        let mut runs = agreeing();
        for (build, text) in &mut runs {
            *text = format!("build {} simd128={} threads={}\n", build.arch, build.simd128, build.threads);
        }
        assert!(codes(&check(&runs, GOOD)).contains(&"determinism.no-hashes"));
    }

    #[test]
    fn a_missing_hash_is_a_failure_even_when_the_rest_agree() {
        let mut runs = agreeing();
        runs[3].1 = runs[3].1.replace("add lanes8 00000000000000aa\n", "");
        let found = check(&runs, GOOD).unwrap_err();
        assert_eq!(codes(&Err(found.clone())), ["determinism.missing"]);
        assert!(found[0].message.contains("wasm32, simd128") && found[0].message.contains("add lanes8"), "{found:?}");
    }

    #[test]
    fn every_build_needs_every_executor_even_if_all_lack_the_same_one() {
        let mut runs = agreeing();
        for (_, text) in &mut runs {
            *text = text.lines().filter(|line| !line.contains("lanes4")).collect::<Vec<_>>().join("\n");
        }
        let found = codes(&check(&runs, GOOD));
        assert!(!found.is_empty() && found.iter().all(|code| *code == "determinism.missing"), "{found:?}");
    }

    #[test]
    fn a_build_reporting_the_wrong_arch_is_a_failure() {
        let mut runs = agreeing();
        runs[2].1 = runs[2].1.replace("build wasm32", "build aarch64");
        assert_eq!(codes(&check(&runs, GOOD)), ["determinism.wrong-build"]);
    }

    #[test]
    fn a_simd128_build_without_simd128_is_a_failure() {
        let mut runs = agreeing();
        runs[3].1 = runs[3].1.replace("simd128=true", "simd128=false");
        let found = check(&runs, GOOD).unwrap_err();
        assert_eq!(found[0].code, "determinism.wrong-build");
        assert!(found[0].message.contains("wasm32, simd128"), "{found:?}");
    }

    #[test]
    fn a_build_on_the_wrong_thread_count_is_a_failure() {
        let mut runs = agreeing();
        runs[1].1 = runs[1].1.replace("threads=4", "threads=1");
        assert_eq!(codes(&check(&runs, GOOD)), ["determinism.wrong-build"]);
    }

    #[test]
    fn output_without_a_build_line_is_malformed() {
        let mut runs = agreeing();
        runs[0].1 = "add scalar 00000000000000aa\n".to_string();
        assert!(codes(&check(&runs, GOOD)).contains(&"determinism.malformed"));
    }

    #[test]
    fn a_line_that_is_not_three_words_is_malformed() {
        let mut runs = agreeing();
        runs[0].1.push_str("sqrt scalar\n");
        assert!(codes(&check(&runs, GOOD)).contains(&"determinism.malformed"));
    }

    #[test]
    fn an_unknown_executor_is_malformed() {
        let mut runs = agreeing();
        runs[0].1.push_str("sqrt lanes16 00000000000000aa\n");
        assert!(codes(&check(&runs, GOOD)).contains(&"determinism.malformed"));
    }

    #[test]
    fn the_builds_are_native_at_one_and_many_threads_and_wasm_with_and_without_simd128() {
        let found = builds(6);
        let [one, many, baseline, simd] = &found;
        assert_eq!((one.threads, one.triple, one.arch, one.simd128), (1, None, "x86_64", false));
        assert_eq!((many.threads, many.triple, many.arch, many.simd128), (6, None, "x86_64", false));
        assert_eq!(
            (baseline.threads, baseline.triple, baseline.arch, baseline.simd128),
            (1, Some(WASM), "wasm32", false)
        );
        assert_eq!((simd.threads, simd.triple, simd.arch, simd.simd128), (1, Some(WASM), "wasm32", true));
        assert_eq!(many.name, "x86-64, 6 thread(s)");
    }

    #[test]
    fn only_the_simd128_build_has_its_own_target_directory_and_flag() {
        let [native, _, baseline, simd] = builds(2);
        let joined = |build: &Build| cargo_args(build, "SIMD-DIR", "3").join(" ");
        assert_eq!(joined(&native), "run --quiet --release -p occam-math --example determinism -- --threads 3");
        assert_eq!(
            joined(&baseline),
            "run --quiet --release -p occam-math --example determinism --target wasm32-wasip2 -- --threads 3"
        );
        let want = format!(
            "run --quiet --release -p occam-math --example determinism --target wasm32-wasip2 --target-dir SIMD-DIR --config {SIMD128} -- --threads 3"
        );
        assert_eq!(joined(&simd), want);
    }
}
