//! `gates mutants`: every change is tested (L13, L14). Adapted from phasekit's gate of the same name. It runs the
//! unmutated suite first, then `cargo mutants --in-diff` over the Rust changes since the merge base with `origin/main`.
//! A mutant no test catches fails the gate unless `.cargo/mutants.toml` excludes it with a reason.
//!
//! It runs in two passes. No crate depends on `occam-xtask`, so only its own tests can catch its mutants and they run
//! those alone; every other mutant runs the whole workspace's tests. `--shard k/n` runs one of n slices of both.

use crate::repo::Repo;
use crate::verdict::{Finding, Verdict};

/// Each pass: its name and the cargo-mutants arguments that select it. Together they cover every mutant once.
const PASSES: [(&str, &[&str]); 2] = [
    ("workspace", &["--exclude", "crates/occam-xtask/**"]),
    ("xtask", &["--package", "occam-xtask", "--test-workspace", "false"]),
];

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
    let shard = shard(args)?;
    let base = repo.base().map_err(|e| vec![e])?;
    let mut diff = code_hunks(&repo.git(&["diff", "-U0", &base, "--", "*.rs"]).map_err(|e| vec![e])?);
    for path in repo.git(&["ls-files", "--others", "--exclude-standard", "--", "*.rs"]).map_err(|e| vec![e])?.lines() {
        diff.push_str(&new_file_diff(path, &repo.read(path).map_err(|e| vec![e])?));
    }
    if diff.trim().is_empty() {
        return Ok(no_diff());
    }
    repo.checked(
        "mutants.unmutated-failed",
        env!("CARGO"),
        &["nextest", "run", "--workspace", "--cargo-profile", "mutants", "--profile", "ci"],
    )
    .map_err(|e| vec![e])?;
    let mut passes = Vec::new();
    for (pass, selection) in PASSES {
        let args: Vec<&str> = selection.iter().copied().chain(shard.iter().map(String::as_str)).collect();
        let result = repo.cargo_mutants(pass, &diff, &args).map_err(|e| vec![e])?;
        let [caught, missed, timeout, unviable] = result.outcomes.each_ref().map(String::as_str);
        let (caught, hung) = check(result.success, &Outcomes { caught, missed, timeout })?;
        passes.push(format!(
            "{pass}: {caught} caught ({} by timeout), {} unviable",
            hung.len(),
            lines(unviable).count()
        ));
    }
    Ok(format!("mutants of the changes since origin/main: {}", passes.join("; ")))
}

/// The cargo-mutants arguments that select this run's shard: none without arguments, `--shard k/n` for one of n.
fn shard(args: &[String]) -> Result<Vec<String>, Vec<Finding>> {
    let usage =
        |detail: String| vec![Finding::new("mutants.usage", "", detail, "pass --shard <k>/<n> with k < n, or nothing")];
    let [flag, slice] = args else {
        return if args.is_empty() {
            Ok(Vec::new())
        } else {
            Err(usage(format!("unexpected arguments: {}", args.join(" "))))
        };
    };
    let parsed = slice.split_once('/').and_then(|(k, n)| Some((k.parse::<u32>().ok()?, n.parse::<u32>().ok()?)));
    match parsed {
        Some((k, n)) if flag == "--shard" && k < n => Ok(vec![flag.clone(), slice.clone()]),
        _ => Err(usage(format!("expected --shard <k>/<n> with k < n, not: {flag} {slice}"))),
    }
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

    #[test]
    fn a_shard_selects_one_slice_of_the_mutants() {
        let args = |list: &[&str]| list.iter().map(ToString::to_string).collect::<Vec<_>>();
        assert_eq!(shard(&[]), Ok(vec![]));
        assert_eq!(shard(&args(&["--shard", "0/4"])), Ok(args(&["--shard", "0/4"])));
        assert_eq!(shard(&args(&["--shard", "3/4"])), Ok(args(&["--shard", "3/4"])));
        for bad in [
            &["--shard", "4/4"][..],
            &["--shard", "1/0"],
            &["--shard", "a/4"],
            &["--shard", "1"],
            &["--shard"],
            &["--jobs", "2"],
        ] {
            assert!(shard(&args(bad)).is_err(), "{bad:?}");
        }
    }

    #[test]
    fn comment_only_hunks_leave_the_diff() {
        let header =
            |path: &str| format!("diff --git a/{path} b/{path}\nindex 1..2 100644\n--- a/{path}\n+++ b/{path}\n");
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
