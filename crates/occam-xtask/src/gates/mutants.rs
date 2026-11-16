//! `gates mutants`: every change is tested (L13, L14). Adapted from phasekit's gate of the same name. It runs the
//! unmutated suite first, then `cargo mutants --in-diff` over the Rust changes since the merge base with `origin/main`.
//! A mutant no test catches fails the gate unless `.cargo/mutants.toml` excludes it with a reason.
//!
//! cargo-mutants has no baseline of its own to derive a timeout from, since the gate runs the suite itself, so the
//! gate times that run and passes the timeout.
//!
//! It runs in two passes. No crate depends on `occam-xtask`, so only its own tests can catch its mutants and they run
//! those alone; every other mutant runs the whole workspace's tests, which
//! `docs/decisions/2026-11-14-mutants-keep-the-whole-workspace.md` measured against narrower scopes. `--shard k/n` runs
//! one of n slices of both. `--iterate` skips the mutants an earlier run in this checkout caught, to try a fix on what
//! survived or was not reached; the gate is a run without it.
//!
//! First, whatever the diff, it checks that every exclusion in `.cargo/mutants.toml` still excludes a mutant
//! (the `exclusions` module); `--exclusions` runs that check alone, as `just preflight` does.

mod exclusions;

use std::time::{Duration, Instant};

use crate::repo::Repo;
use crate::verdict::{Finding, Verdict};

/// Each pass: its name and the cargo-mutants arguments that select it. Together they cover every mutant once.
const PASSES: [(&str, &[&str]); 2] = [
    ("workspace", &["--exclude", "crates/occam-xtask/**"]),
    ("xtask", &["--package", "occam-xtask", "--test-workspace", "false"]),
];

/// The least a mutant's tests are given, in seconds: cargo-mutants' own default where it has no baseline to go by.
const TIMEOUT_FLOOR: u64 = 300;

/// How many times the unmutated suite's time a mutant's tests are given: cargo-mutants' own default multiple of its
/// baseline.
const TIMEOUT_MULTIPLE: u64 = 5;

/// What a run was asked for.
#[derive(Debug, Default, PartialEq, Eq)]
struct Options {
    /// One of n slices of each pass, as `k/n`.
    shard: Option<String>,
    /// Skip what an earlier run caught.
    iterate: bool,
    /// Only check the exclusions.
    exclusions: bool,
}

/// What cargo-mutants wrote: one mutant per line.
struct Outcomes<'a> {
    caught: &'a str,
    missed: &'a str,
    timeout: &'a str,
}

fn no_diff() -> String {
    "no Rust code changes since origin/main".to_string()
}

pub fn run(repo: &Repo, args: &[String]) -> Verdict {
    let options = options(args)?;
    let exclusions = exclusions::run(repo)?;
    if options.exclusions {
        return Ok(exclusions);
    }
    let base = repo.base().map_err(|e| vec![e])?;
    let mut diff = code_hunks(&repo.git(&["diff", "-U0", &base, "--", "*.rs"]).map_err(|e| vec![e])?);
    for path in repo.git(&["ls-files", "--others", "--exclude-standard", "--", "*.rs"]).map_err(|e| vec![e])?.lines() {
        diff.push_str(&new_file_diff(path, &repo.read(path).map_err(|e| vec![e])?));
    }
    if diff.trim().is_empty() {
        return Ok(format!("{exclusions}; {}", no_diff()));
    }
    // Built first, so that the run that is timed is the tests alone.
    let suite = ["nextest", "run", "--workspace", "--cargo-profile", "mutants", "--profile", "ci"];
    let build: Vec<&str> = suite.into_iter().chain(["--no-run"]).collect();
    repo.checked("mutants.unmutated-failed", env!("CARGO"), &build).map_err(|e| vec![e])?;
    let started = Instant::now();
    repo.checked("mutants.unmutated-failed", env!("CARGO"), &suite).map_err(|e| vec![e])?;
    let timeout = timeout(started.elapsed()).to_string();
    let mut passes = Vec::new();
    for (pass, selection) in PASSES {
        let args = arguments(selection, &options, &timeout);
        let result = repo.cargo_mutants(pass, &diff, &args).map_err(|e| vec![e])?;
        let [caught, missed, timeout, unviable] = result.outcomes.each_ref().map(String::as_str);
        let (caught, hung) = check(result.success, &Outcomes { caught, missed, timeout })?;
        passes.push(format!(
            "{pass}: {caught} caught ({} by timeout), {} unviable",
            hung.len(),
            lines(unviable).count()
        ));
    }
    Ok(format!("{exclusions}; mutants of the changes since origin/main: {}", passes.join("; ")))
}

/// Reads the gate's arguments: `--shard <k>/<n>`, `--iterate` and `--exclusions`, in any order.
fn options(args: &[String]) -> Result<Options, Vec<Finding>> {
    let remedy =
        "pass --shard <k>/<n> with k < n, --iterate, both or neither; or --exclusions to check only the exclusions";
    let usage = |detail: String| vec![Finding::new("mutants.usage", "", detail, remedy)];
    let mut options = Options::default();
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        let value = if flag == "--shard" { rest.next() } else { None };
        match (flag.as_str(), value) {
            ("--iterate", _) => options.iterate = true,
            ("--exclusions", _) => options.exclusions = true,
            ("--shard", Some(slice)) if is_shard(slice) => options.shard = Some(slice.clone()),
            (_, value) => return Err(usage(format!("unexpected: {flag} {}", value.map_or("", String::as_str)))),
        }
    }
    Ok(options)
}

/// Whether `slice` is `k/n` with k < n.
fn is_shard(slice: &str) -> bool {
    let parsed = slice.split_once('/').and_then(|(k, n)| Some((k.parse::<u32>().ok()?, n.parse::<u32>().ok()?)));
    parsed.is_some_and(|(k, n)| k < n)
}

/// How long a mutant's tests may run, in seconds, given how long the unmutated suite took: [`TIMEOUT_MULTIPLE`] times
/// it, a part of a second counted as a whole one, and never less than [`TIMEOUT_FLOOR`]. A mutant no test catches runs
/// the whole suite, beside the other jobs' builds and tests, and one that ran out of time would count as caught (L13).
fn timeout(suite: Duration) -> u64 {
    let seconds = suite.as_secs().saturating_add(u64::from(suite.subsec_nanos() > 0));
    seconds.saturating_mul(TIMEOUT_MULTIPLE).max(TIMEOUT_FLOOR)
}

/// The cargo-mutants arguments of one pass: its selection and the timeout, then the run's shard and `--iterate` where
/// asked for.
fn arguments<'a>(selection: &[&'a str], options: &'a Options, timeout: &'a str) -> Vec<&'a str> {
    let shard = options.shard.iter().flat_map(|slice| ["--shard", slice.as_str()]);
    let selected = selection.iter().copied().chain(["--timeout", timeout]);
    selected.chain(shard).chain(options.iterate.then_some("--iterate")).collect()
}

/// Checks one cargo-mutants run (`success` is its exit status, which is also false after a timeout). Returns how many
/// mutants were caught, timeouts included, and the timeouts.
fn check<'a>(success: bool, outcomes: &Outcomes<'a>) -> Result<(usize, Vec<&'a str>), Vec<Finding>> {
    let missed: Vec<Finding> = lines(outcomes.missed)
        .map(|mutant| {
            let file = mutant.split(':').next().unwrap_or_default();
            let remedy = "add a test that catches it, or exclude it in .cargo/mutants.toml with a reason";
            Finding::new("mutants.missed", file, format!("no test fails without this code: {mutant}"), remedy)
        })
        .collect();
    if !missed.is_empty() {
        return Err(missed);
    }
    let hung: Vec<&str> = lines(outcomes.timeout).collect();
    if !success && hung.is_empty() {
        let message = "cargo mutants failed without naming a mutant";
        return Err(vec![Finding::new(
            "mutants.run-failed",
            ".cargo/mutants.toml",
            message,
            "run `just mutants` and read its output",
        )]);
    }
    Ok((lines(outcomes.caught).count().saturating_add(hung.len()), hung))
}

/// The diff without its hunks that only add or remove comments and blank lines (`git diff -U0`, so each hunk is one
/// change). A comment changes no mutant, yet cargo-mutants tests every whole-function mutant of a function a changed
/// line falls in.
fn code_hunks(diff: &str) -> String {
    let (mut out, mut header, mut hunk, mut code) = (String::new(), String::new(), String::new(), false);
    let mut finish = |header: &mut String, hunk: &mut String, code: bool| {
        if code && !hunk.is_empty() {
            out.push_str(&std::mem::take(header));
            out.push_str(hunk);
        }
        hunk.clear();
    };
    for line in diff.lines() {
        if line.starts_with("diff --git ") {
            finish(&mut header, &mut hunk, code);
            header = format!("{line}\n");
        } else if line.starts_with("@@") {
            finish(&mut header, &mut hunk, code);
            hunk = format!("{line}\n");
            code = false;
        } else if hunk.is_empty() {
            header.push_str(line);
            header.push('\n');
        } else {
            hunk.push_str(line);
            hunk.push('\n');
            let changed = line.strip_prefix(['+', '-']).map(str::trim);
            code |= changed.is_some_and(|text| !text.is_empty() && !text.starts_with("//"));
        }
    }
    finish(&mut header, &mut hunk, code);
    out
}

/// A unified diff that adds `path` with `text`, as `git diff` would show a new file.
fn new_file_diff(path: &str, text: &str) -> String {
    let count = text.lines().count();
    if count == 0 {
        return String::new();
    }
    let mut diff = format!(
        "diff --git a/{path} b/{path}\nnew file mode 100644\n--- /dev/null\n+++ b/{path}\n@@ -0,0 +1,{count} @@\n"
    );
    for line in text.lines() {
        diff.push('+');
        diff.push_str(line);
        diff.push('\n');
    }
    diff
}

fn lines(text: &str) -> impl Iterator<Item = &str> {
    text.lines().map(str::trim).filter(|line| !line.is_empty())
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verdict::codes;

    const CAUGHT: &str = "crates/occam-xtask/src/gates/layers.rs:61:5: replace f -> bool with true\n";
    const MISSED: &str = "crates/occam-xtask/src/gates/layers.rs:70:9: replace < with <= in check\n";

    fn outcomes<'a>(missed: &'a str, timeout: &'a str) -> Outcomes<'a> {
        Outcomes { caught: CAUGHT, missed, timeout }
    }

    #[test]
    fn a_clean_run_counts_what_was_caught() {
        assert_eq!(check(true, &outcomes("", "")), Ok((1, vec![])));
    }

    #[test]
    fn a_surviving_mutant_fails_the_gate() {
        let found = check(false, &outcomes(MISSED, "")).unwrap_err();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].code, "mutants.missed");
        assert_eq!(found[0].file, "crates/occam-xtask/src/gates/layers.rs");
        assert!(found[0].message.contains("70:9"), "{found:?}");
        assert!(check(true, &outcomes(MISSED, "")).is_err());
    }

    #[test]
    fn a_mutant_that_hangs_the_tests_counts_as_caught_and_is_listed() {
        assert_eq!(check(false, &outcomes("", MISSED)), Ok((2, vec![MISSED.trim()])));
    }

    #[test]
    fn a_failed_run_that_names_no_mutant_fails_closed() {
        assert_eq!(codes(&check(false, &outcomes("", "")).map(|_| String::new())), ["mutants.run-failed"]);
    }

    fn owned(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn a_shard_selects_one_slice_of_the_mutants() {
        assert_eq!(options(&[]), Ok(Options::default()));
        for good in ["0/4", "3/4"] {
            let want = Options { shard: Some(good.to_string()), ..Options::default() };
            assert_eq!(options(&owned(&["--shard", good])), Ok(want));
        }
        for bad in [
            &["--shard", "4/4"][..],
            &["--shard", "1/0"],
            &["--shard", "a/4"],
            &["--shard", "1/b"],
            &["--shard", "1"],
            &["--shard"],
            &["--jobs", "2"],
            &["--base", "abc123"],
            &["0/4"],
        ] {
            assert_eq!(codes(&options(&owned(bad)).map(|_| String::new())), ["mutants.usage"], "{bad:?}");
        }
    }

    #[test]
    fn the_options_combine_in_either_order() {
        let both = Options { shard: Some("1/16".to_string()), iterate: true, ..Options::default() };
        assert_eq!(options(&owned(&["--iterate", "--shard", "1/16"])), Ok(both));
        let both = Options { shard: Some("1/16".to_string()), iterate: true, ..Options::default() };
        assert_eq!(options(&owned(&["--shard", "1/16", "--iterate"])), Ok(both));
        assert_eq!(options(&owned(&["--iterate"])), Ok(Options { iterate: true, ..Options::default() }));
    }

    #[test]
    fn exclusions_alone_are_checked_when_asked() {
        assert_eq!(options(&owned(&["--exclusions"])), Ok(Options { exclusions: true, ..Options::default() }));
        let found = options(&owned(&["--exclusion"])).unwrap_err();
        assert!(found[0].remedy.contains("--exclusions"), "{found:?}");
    }

    #[test]
    fn a_shard_and_iterate_reach_every_pass_and_the_timeout_always_does() {
        let [(_, workspace), (_, xtask)] = PASSES;
        let plain = ["--exclude", "crates/occam-xtask/**", "--timeout", "300"];
        assert_eq!(arguments(workspace, &Options::default(), "300"), plain);
        let options = Options { shard: Some("2/16".to_string()), iterate: true, ..Options::default() };
        let sliced = ["--package", "occam-xtask", "--test-workspace", "false", "--timeout", "450"];
        assert_eq!(arguments(xtask, &options, "450"), [&sliced[..], &["--shard", "2/16", "--iterate"]].concat());
        let again = Options { iterate: true, ..Options::default() };
        assert_eq!(arguments(workspace, &again, "300"), [&plain[..], &["--iterate"]].concat());
    }

    #[test]
    fn a_mutants_tests_get_five_times_the_unmutated_suite_and_never_less_than_the_floor() {
        let of = |millis| timeout(Duration::from_millis(millis));
        // A quick suite keeps the floor, up to the suite that five times over is the floor.
        assert_eq!([of(0), of(23_000), of(59_000), of(59_999)], [300; 4]);
        // Past it, five times the suite's seconds, a part of a second counted as a whole one.
        assert_eq!([of(60_000), of(60_001), of(66_400), of(136_600)], [300, 305, 335, 685]);
        assert_eq!(timeout(Duration::MAX), u64::MAX);
    }

    fn header(path: &str) -> String {
        format!("diff --git a/{path} b/{path}\nindex 1..2 100644\n--- a/{path}\n+++ b/{path}\n")
    }

    #[test]
    fn comment_only_hunks_leave_the_diff() {
        let comments = "@@ -3,0 +4,2 @@ fn f() {\n+    // Why.\n+    // More.\n";
        let blank = "@@ -9,1 +11,1 @@ fn g() {\n-    // old\n+\n";
        let code = "@@ -20,1 +22,2 @@ fn h() {\n-    a < b\n+    // Why.\n+    a <= b\n";
        let diff = format!("{}{comments}{blank}{code}{}{comments}", header("a.rs"), header("b.rs"));
        assert_eq!(code_hunks(&diff), format!("{}{code}", header("a.rs")));
        assert_eq!(code_hunks(&format!("{}{comments}", header("b.rs"))), "");
        assert_eq!(code_hunks(""), "");
    }

    #[test]
    fn untracked_files_join_the_diff() {
        let want = "diff --git a/src/new.rs b/src/new.rs\nnew file mode 100644\n--- /dev/null\n+++ b/src/new.rs\n@@ -0,0 +1,2 @@\n+fn a() {}\n+fn b() {}\n";
        assert_eq!(new_file_diff("src/new.rs", "fn a() {}\nfn b() {}\n"), want);
        assert_eq!(new_file_diff("src/empty.rs", ""), "");
    }

    #[test]
    fn blank_lines_are_not_mutants() {
        assert_eq!(lines("a\n\n  \n b \n").collect::<Vec<_>>(), ["a", "b"]);
    }

    #[test]
    fn with_no_changes_there_is_nothing_to_mutate() {
        assert_eq!(no_diff(), "no Rust code changes since origin/main");
    }
}
