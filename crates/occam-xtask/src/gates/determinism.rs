//! `gates determinism`: the same inputs give identical bits on every build the contract covers (L23,
//! `docs/DETERMINISM.md`). Each probe is built and run as each of them, the maths natively at every instruction level
//! the machine has, and every hash is compared with the other builds' and with the probe's record.

use std::collections::{BTreeMap, BTreeSet};
use std::thread;

use super::no_args;
use crate::repo::Repo;
use crate::verdict::{Finding, Verdict, verdict};

const STREAM: &str = "crates/occam-math/src/stream.rs";
const RECORDED: &str = "ci/determinism-hashes.txt";
const FIX: &str = "see docs/DETERMINISM.md; run `just gate determinism` to reproduce";
const RECORD_FIX: &str = "a result changed, which docs/DETERMINISM.md does not allow; if the probe's definition changed on purpose, re-derive the hashes as its record's header says and say so in the PR";
const WASM: &str = "wasm32-wasip2";
const SIMD128: &str = r#"target.wasm32-wasip2.rustflags=["-Ctarget-feature=+simd128"]"#;

/// Every way one operation of the maths stream is run in one build.
const EXECUTORS: [&str; 4] = ["scalar", "lanes2", "lanes4", "lanes8"];

/// A program the gate builds and runs as every build, the `determinism` example of a package, and what holds it.
struct Probe {
    /// What the summary calls it.
    name: &'static str,
    /// The package whose example it is.
    package: &'static str,
    /// The cargo features the example needs.
    features: Option<&'static str>,
    /// The file its findings point at.
    source: &'static str,
    /// The record it must equal.
    record: &'static str,
    /// Every executor each of its operations is run as in one build.
    executors: &'static [&'static str],
    /// Whether it runs at each narrower instruction level too: only code that dispatches on one can differ there.
    levels: bool,
}

/// The maths stream, recorded from a separate implementation (`tools/determinism/stream.py`).
const MATHS: Probe = Probe {
    name: "maths",
    package: "occam-math",
    features: None,
    source: STREAM,
    record: RECORDED,
    executors: &EXECUTORS,
    levels: true,
};

/// The topology's digests and whole-model checks, recorded from the x86-64 build on one thread. Topology passes no
/// instruction level to anything, so it is not run at the narrower ones.
const TOPOLOGY: Probe = Probe {
    name: "topology",
    package: "occam-topo",
    features: Some("determinism"),
    source: "crates/occam-topo/src/probe.rs",
    record: "ci/determinism-topology.txt",
    executors: &["scalar"],
    levels: false,
};

/// Every probe, in the order the summary gives them.
const PROBES: [&Probe; 2] = [&MATHS, &TOPOLOGY];

/// One way a probe is built and run.
#[derive(Clone)]
struct Build {
    name: String,
    /// The target triple, or the host's when `None`.
    triple: Option<&'static str>,
    arch: &'static str,
    simd128: bool,
    threads: usize,
    /// The instruction level it runs at. A native build is told it; a wasm build has one, fixed when it is built.
    level: String,
    /// A native build at a level narrower than the widest, which only a probe that dispatches is run as.
    narrower: bool,
}

/// Every build the contract covers: natively at the widest level on one thread and on many, on wasm32 with and without
/// simd128, then natively at each narrower level the machine has. They are release builds, as what ships is; the test
/// suite covers the debug ones. `levels` is the machine's, widest first.
fn builds(threads: usize, levels: &[String]) -> Vec<Build> {
    let native = |threads: usize, level: &str, narrower: bool| Build {
        name: format!("x86-64, {threads} thread(s), {level}"),
        triple: None,
        arch: "x86_64",
        simd128: false,
        threads,
        level: level.to_string(),
        narrower,
    };
    let wasm = |name: &str, simd128: bool, level: &str| Build {
        name: name.to_string(),
        triple: Some(WASM),
        arch: "wasm32",
        simd128,
        threads: 1,
        level: level.to_string(),
        narrower: false,
    };
    let widest = levels.first().map_or("sse2", String::as_str);
    let mut found = vec![
        native(1, widest, false),
        native(threads, widest, false),
        wasm("wasm32, baseline", false, "fallback"),
        wasm("wasm32, simd128", true, "simd128"),
    ];
    found.extend(levels.iter().skip(1).map(|level| native(1, level, true)));
    found
}

/// The builds `probe` is run as.
fn builds_of<'a>(probe: &Probe, builds: &'a [Build]) -> Vec<&'a Build> {
    builds.iter().filter(|build| probe.levels || !build.narrower).collect()
}

/// Cargo's arguments for running `probe` as `build`. The simd128 build has a target directory of its own, because its
/// flags differ.
fn cargo_args<'a>(build: &'a Build, probe: &'a Probe, simd_dir: &'a str, threads: &'a str) -> Vec<&'a str> {
    let mut args = vec!["run", "--quiet", "--release", "-p", probe.package, "--example", "determinism"];
    if let Some(features) = probe.features {
        args.extend(["--features", features]);
    }
    if let Some(triple) = build.triple {
        args.extend(["--target", triple]);
    }
    if build.simd128 {
        args.extend(["--target-dir", simd_dir, "--config", SIMD128]);
    }
    args.extend(["--", "--threads", threads]);
    if build.triple.is_none() && probe.levels {
        args.extend(["--level", &build.level]);
    }
    args
}

/// A finding for a probe that did not run.
fn run_failed(source: &str, what: &str, errors: &str) -> Vec<Finding> {
    let message = format!("{what} did not run: {errors}");
    vec![Finding::new("determinism.run-failed", source, message, "run `just toolchain`, then fix what it reports")]
}

/// The levels the probe printed, one a line, or why there are none.
fn parsed_levels(ok: bool, output: &str, errors: &str) -> Result<Vec<String>, Vec<Finding>> {
    let levels: Vec<String> = output.lines().filter(|line| !line.is_empty()).map(str::to_string).collect();
    if !ok || levels.is_empty() {
        return Err(run_failed(STREAM, "the level query", errors));
    }
    Ok(levels)
}

/// The instruction levels this machine has, widest first, as the maths probe names them.
fn levels(repo: &Repo) -> Result<Vec<String>, Vec<Finding>> {
    let args = ["run", "--quiet", "--release", "-p", MATHS.package, "--example", "determinism", "--", "--levels"];
    let (ok, output, errors) = repo.capture(env!("CARGO"), &args).map_err(|e| vec![e])?;
    parsed_levels(ok, &output, &errors)
}

/// Every run of `probe`, each build and what it printed, or why one did not run.
fn runs_of(repo: &Repo, probe: &Probe, builds: &[Build]) -> Result<Vec<(Build, String)>, Vec<Finding>> {
    let simd_dir = repo.nested_target("simd");
    let mut runs = Vec::new();
    for build in builds_of(probe, builds) {
        let count = build.threads.to_string();
        let args = cargo_args(build, probe, &simd_dir, &count);
        let (ok, output, errors) = repo.capture(env!("CARGO"), &args).map_err(|e| vec![e])?;
        if !ok {
            return Err(run_failed(probe.source, &format!("{} as {}", probe.name, build.name), &errors));
        }
        runs.push((build.clone(), output));
    }
    Ok(runs)
}

pub fn run(repo: &Repo, args: &[String]) -> Verdict {
    no_args(args)?;
    let threads = thread::available_parallelism().map_or(2, |count| count.get().max(2));
    let builds = builds(threads, &levels(repo)?);
    let mut summaries = Vec::new();
    let mut findings = Vec::new();
    for probe in PROBES {
        let runs = runs_of(repo, probe, &builds)?;
        match check(probe, &runs, &repo.read(probe.record).map_err(|e| vec![e])?) {
            Ok(summary) => summaries.push(format!("{}: {summary}", probe.name)),
            Err(found) => findings.extend(found),
        }
    }
    if findings.is_empty() { Ok(summaries.join("; ")) } else { Err(findings) }
}

/// What one run printed: a hash for each (operation, executor).
type Rows<'a> = BTreeMap<(&'a str, &'a str), &'a str>;

fn malformed(probe: &Probe, build: &Build, line: &str) -> Finding {
    Finding::new("determinism.malformed", probe.source, format!("{}: cannot read `{line}`", build.name), FIX)
}

/// The line a run of `probe` as `build` must begin with: the level only for a probe that dispatches.
fn header(probe: &Probe, build: &Build) -> String {
    let base = format!("build {} simd128={} threads={}", build.arch, build.simd128, build.threads);
    if probe.levels { format!("{base} level={}", build.level) } else { base }
}

/// The hashes one run printed, after checking that it really was the build it was meant to be.
fn parse<'a>(probe: &Probe, build: &Build, output: &'a str, findings: &mut Vec<Finding>) -> Rows<'a> {
    let mut lines = output.lines();
    let want = header(probe, build);
    match lines.next() {
        Some(line) if line == want => {}
        Some(line) if line.starts_with("build ") => {
            let message = format!("{}: the probe says `{line}`, not `{want}`", build.name);
            findings.push(Finding::new("determinism.wrong-build", probe.source, message, FIX));
        }
        line => findings.push(malformed(probe, build, line.unwrap_or("(nothing)"))),
    }
    let mut rows = Rows::new();
    for line in lines {
        let words: Vec<&str> = line.split_whitespace().collect();
        match words.as_slice() {
            [operation, executor, hash] if probe.executors.contains(executor) => {
                rows.insert((*operation, *executor), *hash);
            }
            _ => findings.push(malformed(probe, build, line)),
        }
    }
    rows
}

/// A finding for every hash a build should have printed and did not.
fn missing(probe: &Probe, parsed: &[(&Build, Rows)], operations: &BTreeSet<&str>) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (build, rows) in parsed {
        for operation in operations {
            for executor in probe.executors.iter().filter(|executor| !rows.contains_key(&(*operation, **executor))) {
                let message = format!("{} printed no hash for {operation} {executor}", build.name);
                findings.push(Finding::new("determinism.missing", probe.source, message, FIX));
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
fn hashes_of<'a>(probe: &Probe, parsed: &[(&Build, Rows<'a>)], operation: &str) -> BTreeMap<&'a str, Vec<String>> {
    let mut hashes: BTreeMap<&str, Vec<String>> = BTreeMap::new();
    for (build, rows) in parsed {
        for executor in probe.executors {
            if let Some(hash) = rows.get(&(operation, *executor)) {
                hashes.entry(*hash).or_default().push(format!("{} {executor}", build.name));
            }
        }
    }
    hashes
}

/// The recorded hash of each operation, from `probe`'s record.
fn recorded<'a>(probe: &Probe, text: &'a str, findings: &mut Vec<Finding>) -> BTreeMap<&'a str, &'a str> {
    let mut hashes = BTreeMap::new();
    for line in text.lines().filter(|line| !line.trim().is_empty() && !line.starts_with('#')) {
        if let [operation, hash] = line.split_whitespace().collect::<Vec<_>>().as_slice() {
            hashes.insert(*operation, *hash);
        } else {
            let message = format!("cannot read `{line}`");
            findings.push(Finding::new("determinism.malformed", probe.record, message, "write `<operation> <hash>`"));
        }
    }
    hashes
}

/// What is wrong with `operation`: more than one hash, or one that is not the recorded one.
fn judge(
    probe: &Probe,
    parsed: &[(&Build, Rows)],
    operation: &str,
    recorded: &BTreeMap<&str, &str>,
) -> Option<Finding> {
    let hashes = hashes_of(probe, parsed, operation);
    if hashes.len() > 1 {
        let groups: Vec<String> = hashes.iter().map(|(hash, sources)| describe(hash, sources)).collect();
        let message = format!("{operation} differs: {}", groups.join("; "));
        return Some(Finding::new("determinism.mismatch", probe.source, message, FIX));
    }
    let hash = hashes.into_keys().next()?;
    match recorded.get(operation) {
        Some(want) if *want == hash => None,
        Some(want) => {
            let message = format!("every build gives {operation} {hash}, but {want} is recorded");
            Some(Finding::new("determinism.not-recorded", probe.record, message, RECORD_FIX))
        }
        None => {
            let message = format!("{operation} has no recorded hash");
            Some(Finding::new("determinism.unrecorded", probe.record, message, RECORD_FIX))
        }
    }
}

fn check(probe: &Probe, runs: &[(Build, String)], recorded_text: &str) -> Verdict {
    if runs.is_empty() {
        return Err(vec![Finding::new("determinism.no-builds", probe.source, "no build was compared", FIX)]);
    }
    let mut findings = Vec::new();
    let recorded = recorded(probe, recorded_text, &mut findings);
    let parsed: Vec<(&Build, Rows)> =
        runs.iter().map(|(build, output)| (build, parse(probe, build, output, &mut findings))).collect();
    let operations: BTreeSet<&str> =
        parsed.iter().flat_map(|(_, rows)| rows.keys().map(|(operation, _)| *operation)).collect();
    if operations.is_empty() {
        findings.push(Finding::new("determinism.no-hashes", probe.source, "no build printed a hash", FIX));
    }
    findings.extend(missing(probe, &parsed, &operations));
    findings.extend(operations.iter().filter_map(|operation| judge(probe, &parsed, operation, &recorded)));
    for operation in recorded.keys().filter(|operation| !operations.contains(*operation)) {
        let message = format!("{operation} is recorded but no build printed it");
        findings.push(Finding::new("determinism.missing", probe.record, message, FIX));
    }
    let executors = probe.executors.len();
    let summary = format!(
        "{} builds agree with the record on {} operations, {executors} {} each",
        runs.len(),
        operations.len(),
        if executors == 1 { "executor" } else { "executors" }
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
        let header = header(&MATHS, build);
        let rows = OPERATIONS
            .iter()
            .flat_map(|operation| EXECUTORS.iter().map(move |executor| format!("{operation} {executor} {hash}")));
        let lines: Vec<String> = std::iter::once(header).chain(rows).collect();
        lines.join("\n") + "\n"
    }

    fn levels(names: &[&str]) -> Vec<String> {
        names.iter().map(|name| (*name).to_string()).collect()
    }

    fn agreeing_at(names: &[&str]) -> Vec<(Build, String)> {
        builds(4, &levels(names))
            .into_iter()
            .map(|build| {
                let text = output(&build, "00000000000000aa");
                (build, text)
            })
            .collect()
    }

    fn agreeing() -> Vec<(Build, String)> {
        agreeing_at(&["avx2"])
    }

    #[test]
    fn hashes_that_agree_pass() {
        assert_eq!(
            check(&MATHS, &agreeing(), GOOD),
            Ok("4 builds agree with the record on 2 operations, 4 executors each".to_string())
        );
    }

    #[test]
    fn one_flipped_hash_fails_naming_the_operation_and_the_builds() {
        let mut runs = agreeing();
        runs[2].1 = runs[2].1.replace("sqrt lanes4 00000000000000aa", "sqrt lanes4 00000000000000bb");
        let found = check(&MATHS, &runs, GOOD).unwrap_err();
        assert_eq!(found.len(), 1, "{found:?}");
        assert_eq!(found[0].code, "determinism.mismatch");
        assert_eq!(found[0].file, STREAM);
        assert!(found[0].message.contains("sqrt"), "{found:?}");
        let message = &found[0].message;
        assert!(
            message.starts_with("sqrt differs: 00000000000000aa from x86-64, 1 thread(s), avx2 scalar"),
            "{found:?}"
        );
        assert!(message.contains("and 12 more; "), "{found:?}");
        assert!(message.ends_with("00000000000000bb from wasm32, baseline lanes4"), "{found:?}");
    }

    #[test]
    fn scalar_and_lanes_must_agree_within_one_build() {
        let mut runs = agreeing();
        for (_, text) in &mut runs {
            *text = text.replace("add lanes2 00000000000000aa", "add lanes2 00000000000000cc");
        }
        assert_eq!(codes(&check(&MATHS, &runs, GOOD)), ["determinism.mismatch"]);
    }

    #[test]
    fn thread_counts_must_agree() {
        let mut runs = agreeing();
        runs[1].1 = runs[1].1.replace("add scalar 00000000000000aa", "add scalar 00000000000000dd");
        let found = check(&MATHS, &runs, GOOD).unwrap_err();
        assert_eq!(found[0].code, "determinism.mismatch");
        assert!(found[0].message.contains("00000000000000dd from x86-64, 4 thread(s), avx2 scalar"), "{found:?}");
    }

    #[test]
    fn every_build_agreeing_on_a_hash_that_is_not_recorded_fails() {
        let found = check(&MATHS, &agreeing(), "add 00000000000000ff\nsqrt 00000000000000aa\n").unwrap_err();
        assert_eq!(codes(&Err(found.clone())), ["determinism.not-recorded"]);
        assert_eq!(found[0].file, RECORDED);
        assert_eq!(found[0].message, "every build gives add 00000000000000aa, but 00000000000000ff is recorded");
    }

    #[test]
    fn an_operation_with_no_recorded_hash_fails() {
        let found = check(&MATHS, &agreeing(), "add 00000000000000aa\n").unwrap_err();
        assert_eq!(codes(&Err(found.clone())), ["determinism.unrecorded"]);
        assert_eq!(found[0].message, "sqrt has no recorded hash");
    }

    #[test]
    fn a_recorded_operation_no_build_printed_fails() {
        let found = check(&MATHS, &agreeing(), "add 00000000000000aa\nsqrt 00000000000000aa\ncos 00000000000000aa\n")
            .unwrap_err();
        assert_eq!(codes(&Err(found.clone())), ["determinism.missing"]);
        assert_eq!(found[0].file, RECORDED);
        assert!(found[0].message.starts_with("cos "), "{found:?}");
    }

    #[test]
    fn a_recorded_line_that_is_not_two_words_is_malformed() {
        let found = check(&MATHS, &agreeing(), "add 00000000000000aa\nsqrt\n").unwrap_err();
        let which: Vec<(&str, &str)> = found.iter().map(|f| (f.code, f.file.as_str())).collect();
        assert!(which.contains(&("determinism.malformed", RECORDED)), "{found:?}");
    }

    #[test]
    fn no_builds_is_a_failure() {
        assert_eq!(codes(&check(&MATHS, &[], GOOD)), ["determinism.no-builds"]);
    }

    #[test]
    fn a_build_with_no_hashes_is_a_failure() {
        let mut runs = agreeing();
        for (build, text) in &mut runs {
            *text = format!("{}\n", header(&MATHS, build));
        }
        assert!(codes(&check(&MATHS, &runs, GOOD)).contains(&"determinism.no-hashes"));
    }

    #[test]
    fn a_missing_hash_is_a_failure_even_when_the_rest_agree() {
        let mut runs = agreeing();
        runs[3].1 = runs[3].1.replace("add lanes8 00000000000000aa\n", "");
        let found = check(&MATHS, &runs, GOOD).unwrap_err();
        assert_eq!(codes(&Err(found.clone())), ["determinism.missing"]);
        assert!(found[0].message.contains("wasm32, simd128") && found[0].message.contains("add lanes8"), "{found:?}");
    }

    #[test]
    fn every_build_needs_every_executor_even_if_all_lack_the_same_one() {
        let mut runs = agreeing();
        for (_, text) in &mut runs {
            *text = text.lines().filter(|line| !line.contains("lanes4")).collect::<Vec<_>>().join("\n");
        }
        let found = codes(&check(&MATHS, &runs, GOOD));
        assert!(!found.is_empty() && found.iter().all(|code| *code == "determinism.missing"), "{found:?}");
    }

    #[test]
    fn a_build_reporting_the_wrong_arch_is_a_failure() {
        let mut runs = agreeing();
        runs[2].1 = runs[2].1.replace("build wasm32", "build aarch64");
        assert_eq!(codes(&check(&MATHS, &runs, GOOD)), ["determinism.wrong-build"]);
    }

    #[test]
    fn a_simd128_build_without_simd128_is_a_failure() {
        let mut runs = agreeing();
        runs[3].1 = runs[3].1.replace("simd128=true", "simd128=false");
        let found = check(&MATHS, &runs, GOOD).unwrap_err();
        assert_eq!(found[0].code, "determinism.wrong-build");
        assert!(found[0].message.contains("wasm32, simd128"), "{found:?}");
    }

    #[test]
    fn a_build_on_the_wrong_thread_count_is_a_failure() {
        let mut runs = agreeing();
        runs[1].1 = runs[1].1.replace("threads=4", "threads=1");
        assert_eq!(codes(&check(&MATHS, &runs, GOOD)), ["determinism.wrong-build"]);
    }

    #[test]
    fn output_without_a_build_line_is_malformed() {
        let mut runs = agreeing();
        runs[0].1 = "add scalar 00000000000000aa\n".to_string();
        assert!(codes(&check(&MATHS, &runs, GOOD)).contains(&"determinism.malformed"));
    }

    #[test]
    fn a_line_that_is_not_three_words_is_malformed() {
        let mut runs = agreeing();
        runs[0].1.push_str("sqrt scalar\n");
        assert!(codes(&check(&MATHS, &runs, GOOD)).contains(&"determinism.malformed"));
    }

    #[test]
    fn an_unknown_executor_is_malformed() {
        let mut runs = agreeing();
        runs[0].1.push_str("sqrt lanes16 00000000000000aa\n");
        assert!(codes(&check(&MATHS, &runs, GOOD)).contains(&"determinism.malformed"));
    }

    #[test]
    fn a_level_that_differs_is_named() {
        let mut runs = agreeing_at(&["avx2", "sse4.2", "sse2"]);
        assert_eq!(runs.len(), 6);
        runs[5].1 = runs[5].1.replace("add lanes4 00000000000000aa", "add lanes4 00000000000000ee");
        let found = check(&MATHS, &runs, GOOD).unwrap_err();
        assert_eq!(codes(&Err(found.clone())), ["determinism.mismatch"]);
        assert!(found[0].message.ends_with("00000000000000ee from x86-64, 1 thread(s), sse2 lanes4"), "{found:?}");
    }

    #[test]
    fn a_native_build_at_the_wrong_level_is_a_failure() {
        let mut runs = agreeing_at(&["avx2", "sse2"]);
        runs[4].1 = runs[4].1.replace("level=sse2", "level=avx2");
        let found = check(&MATHS, &runs, GOOD).unwrap_err();
        assert_eq!(codes(&Err(found.clone())), ["determinism.wrong-build"]);
        assert!(found[0].message.contains("x86-64, 1 thread(s), sse2"), "{found:?}");
    }

    #[test]
    fn the_levels_are_the_probes_lines() {
        assert_eq!(parsed_levels(true, "avx2\n\nsse2\n", ""), Ok(levels(&["avx2", "sse2"])));
    }

    #[test]
    fn a_level_query_that_failed_or_printed_nothing_is_a_failure() {
        let failed = parsed_levels(false, "avx2\n", "no cargo").unwrap_err();
        assert_eq!(codes(&Err(failed.clone())), ["determinism.run-failed"]);
        assert_eq!(failed[0].message, "the level query did not run: no cargo");
        let nothing = parsed_levels(true, "\n", "").unwrap_err();
        assert_eq!(nothing.iter().map(|finding| finding.code).collect::<Vec<_>>(), ["determinism.run-failed"]);
    }

    #[test]
    fn every_narrower_level_runs_once_on_one_thread() {
        let found = builds(6, &levels(&["avx512", "avx2", "sse4.2", "sse2"]));
        let natives: Vec<(usize, &str)> =
            found.iter().filter(|build| build.triple.is_none()).map(|b| (b.threads, b.level.as_str())).collect();
        assert_eq!(natives, [(1, "avx512"), (6, "avx512"), (1, "avx2"), (1, "sse4.2"), (1, "sse2")]);
        let wasm: Vec<&str> = found.iter().filter(|build| build.triple.is_some()).map(|b| b.level.as_str()).collect();
        assert_eq!(wasm, ["fallback", "simd128"]);
    }

    #[test]
    fn the_builds_are_native_at_one_and_many_threads_and_wasm_with_and_without_simd128() {
        let found = builds(6, &levels(&["sse2"]));
        let [one, many, baseline, simd] = found.as_slice() else { panic!("{} builds", found.len()) };
        assert_eq!((one.threads, one.triple, one.arch, one.simd128), (1, None, "x86_64", false));
        assert_eq!((many.threads, many.triple, many.arch, many.simd128), (6, None, "x86_64", false));
        assert_eq!(
            (baseline.threads, baseline.triple, baseline.arch, baseline.simd128),
            (1, Some(WASM), "wasm32", false)
        );
        assert_eq!((simd.threads, simd.triple, simd.arch, simd.simd128), (1, Some(WASM), "wasm32", true));
        assert_eq!(many.name, "x86-64, 6 thread(s), sse2");
    }

    #[test]
    fn only_the_simd128_build_has_its_own_target_directory_and_flag() {
        let found = builds(2, &levels(&["avx2"]));
        let [native, _, baseline, simd] = found.as_slice() else { panic!("{} builds", found.len()) };
        let joined = |build: &Build| cargo_args(build, &MATHS, "SIMD-DIR", "3").join(" ");
        assert_eq!(
            joined(native),
            "run --quiet --release -p occam-math --example determinism -- --threads 3 --level avx2"
        );
        assert_eq!(
            joined(baseline),
            "run --quiet --release -p occam-math --example determinism --target wasm32-wasip2 -- --threads 3"
        );
        let want = format!(
            "run --quiet --release -p occam-math --example determinism --target wasm32-wasip2 --target-dir SIMD-DIR --config {SIMD128} -- --threads 3"
        );
        assert_eq!(joined(simd), want);
    }

    /// The topology probe's output as `build`, every operation hashed to `hash`.
    fn topology_output(build: &Build, hash: &str) -> String {
        let rows = OPERATIONS.iter().map(|operation| format!("{operation} scalar {hash}"));
        let lines: Vec<String> = std::iter::once(header(&TOPOLOGY, build)).chain(rows).collect();
        lines.join("\n") + "\n"
    }

    fn topology_agreeing() -> Vec<(Build, String)> {
        let all = builds(4, &levels(&["avx2", "sse2"]));
        builds_of(&TOPOLOGY, &all)
            .into_iter()
            .map(|build| (build.clone(), topology_output(build, "00000000000000aa")))
            .collect()
    }

    #[test]
    fn the_topology_runs_once_a_thread_count_and_a_target_with_no_level_and_one_executor() {
        let all = builds(4, &levels(&["avx2", "sse2"]));
        let names: Vec<&str> = builds_of(&TOPOLOGY, &all).iter().map(|build| build.name.as_str()).collect();
        assert_eq!(
            names,
            ["x86-64, 1 thread(s), avx2", "x86-64, 4 thread(s), avx2", "wasm32, baseline", "wasm32, simd128"]
        );
        assert_eq!(builds_of(&MATHS, &all).len(), 5, "the maths at sse2 too");
        assert_eq!(
            check(&TOPOLOGY, &topology_agreeing(), GOOD),
            Ok("4 builds agree with the record on 2 operations, 1 executor each".to_string())
        );
        let [native, many, _, simd] = builds_of(&TOPOLOGY, &all)[..] else { panic!() };
        assert_eq!(header(&TOPOLOGY, native), "build x86_64 simd128=false threads=1");
        assert_eq!(
            cargo_args(many, &TOPOLOGY, "SIMD-DIR", "4").join(" "),
            "run --quiet --release -p occam-topo --example determinism --features determinism -- --threads 4"
        );
        let want = format!(
            "run --quiet --release -p occam-topo --example determinism --features determinism --target wasm32-wasip2 --target-dir SIMD-DIR --config {SIMD128} -- --threads 1"
        );
        assert_eq!(cargo_args(simd, &TOPOLOGY, "SIMD-DIR", "1").join(" "), want);
    }

    #[test]
    fn a_topology_run_that_differs_or_names_a_level_or_a_lane_fails_against_its_own_files() {
        let mut runs = topology_agreeing();
        runs[1].1 = runs[1].1.replace("sqrt scalar 00000000000000aa", "sqrt scalar 00000000000000bb");
        let found = check(&TOPOLOGY, &runs, GOOD).unwrap_err();
        assert_eq!((found[0].code, found[0].file.as_str()), ("determinism.mismatch", TOPOLOGY.source));
        let mut levelled = topology_agreeing();
        levelled[0].1 = levelled[0].1.replacen("threads=1", "threads=1 level=avx2", 1);
        assert_eq!(codes(&check(&TOPOLOGY, &levelled, GOOD)), ["determinism.wrong-build"]);
        let mut laned = topology_agreeing();
        laned[2].1.push_str("add lanes4 00000000000000aa\n");
        assert_eq!(codes(&check(&TOPOLOGY, &laned, GOOD)), ["determinism.malformed"]);
        let unrecorded = check(&TOPOLOGY, &topology_agreeing(), "add 00000000000000aa\n").unwrap_err();
        assert_eq!((unrecorded[0].code, unrecorded[0].file.as_str()), ("determinism.unrecorded", TOPOLOGY.record));
    }

    #[test]
    fn every_probe_has_its_own_package_record_and_source() {
        assert_eq!(PROBES.map(|probe| probe.name), ["maths", "topology"]);
        assert_eq!(PROBES.map(|probe| probe.record), ["ci/determinism-hashes.txt", "ci/determinism-topology.txt"]);
        assert_eq!(PROBES.map(|probe| (probe.levels, probe.executors.len())), [(true, 4), (false, 1)]);
        assert_eq!(MATHS.features, None);
    }
}
