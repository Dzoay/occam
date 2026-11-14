//! `gates counts`: tests cannot silently disappear (L44). Given `--base <rev>`, it lists the tests of the merge base with
//! `<rev>` from a worktree of its own and fails when a test binary has fewer tests than there, or is gone, unless a
//! commit since the base names the binary in a `Tests-Removed:` footer with its reason
//! (`docs/decisions/2026-11-14-test-counts-measured-not-stored.md`). Listing the base is a second build, so CI passes the
//! base, and a run without one counts this tree's tests and says it compared nothing.

use std::collections::{BTreeMap, BTreeSet};

use crate::repo::Repo;
use crate::verdict::{Finding, Verdict, verdict};

/// The footer that names a deliberate removal: `Tests-Removed: <binary> <reason>`.
const FOOTER: &str = "Tests-Removed:";

pub fn run(repo: &Repo, args: &[String]) -> Verdict {
    let rev = base_arg(args)?;
    let head = repo.test_listing().map_err(|e| vec![e])?;
    let Some(rev) = rev else {
        return alone(&head, "no --base given, as CI gives (`just gate counts --base origin/main`)");
    };
    let base = repo.merge_base(rev).map_err(|e| vec![e])?;
    if base == repo.head().map_err(|e| vec![e])? {
        return alone(&head, &format!("HEAD is on {rev}, so there is no change to compare"));
    }
    let listing = repo.test_listing_at(&base).map_err(|e| vec![e])?;
    let log = repo.messages(&format!("{base}..HEAD")).map_err(|e| vec![e])?;
    compare(&listing, &head, &log)
}

/// The tests each binary has.
fn counted(listing: &str) -> BTreeMap<&str, usize> {
    let mut counts: BTreeMap<&str, usize> = BTreeMap::new();
    for binary in listing.lines().filter_map(|line| line.split_once(' ').map(|(binary, _)| binary)) {
        let count = counts.entry(binary).or_insert(0);
        *count = count.saturating_add(1);
    }
    counts
}

fn total(counts: &BTreeMap<&str, usize>) -> usize {
    counts.values().sum()
}

fn base_arg(args: &[String]) -> Result<Option<&String>, Vec<Finding>> {
    match args {
        [] => Ok(None),
        [flag, rev] if flag == "--base" => Ok(Some(rev)),
        _ => Err(vec![Finding::new(
            "counts.usage",
            "",
            format!("unexpected arguments: {}", args.join(" ")),
            "pass --base <rev>, such as origin/main, or nothing",
        )]),
    }
}

/// The binaries a log's footers name, each with its reason: `Tests-Removed: <binary> <reason>`, a line of its own. A
/// footer with no reason names none.
fn removals(log: &str) -> BTreeSet<&str> {
    log.lines()
        .filter_map(|line| line.trim().strip_prefix(FOOTER)?.trim().split_once(char::is_whitespace))
        .map(|(binary, _)| binary)
        .collect()
}

fn no_tests(whose: &str) -> Finding {
    Finding::new("counts.no-tests", "", format!("{whose} lists no tests"), "check that it builds and lists its tests")
}

/// This tree's tests alone, where there is no base to compare them with, and `why`.
fn alone(head: &str, why: &str) -> Verdict {
    let found = counted(head);
    let findings = if found.is_empty() { vec![no_tests("this tree")] } else { Vec::new() };
    verdict(format!("{} tests in {} binaries, not compared: {why}", total(&found), found.len()), findings)
}

/// A binary with `was` tests on the base and `now` here, `None` where it is gone.
fn lost(binary: &str, was: usize, now: Option<usize>) -> Finding {
    let remedy = format!(
        "restore the tests, or name the removal and its reason in a commit footer: `{FOOTER} {binary} <reason>`"
    );
    match now {
        Some(count) => {
            Finding::new("counts.fewer", "", format!("{binary} has {count} tests, {was} on the base"), remedy)
        }
        None => Finding::new("counts.gone", "", format!("{binary} is gone, with its {was} tests on the base"), remedy),
    }
}

/// The base's listing against this tree's: every binary keeps at least its tests, unless `log`, the messages of the
/// commits since the base, names it in a footer.
fn compare(base: &str, head: &str, log: &str) -> Verdict {
    let (before, after) = (counted(base), counted(head));
    let named = removals(log);
    let mut findings = Vec::new();
    if before.is_empty() {
        findings.push(no_tests("the base"));
    }
    if after.is_empty() {
        findings.push(no_tests("this tree"));
    }
    let mut excused = Vec::new();
    for (binary, &was) in &before {
        let now = after.get(binary).copied();
        if now.is_some_and(|count| count >= was) {
            continue;
        }
        if named.contains(binary) {
            excused.push(*binary);
        } else {
            findings.push(lost(binary, was, now));
        }
    }
    let note = if excused.is_empty() { String::new() } else { format!("; removals named for {}", excused.join(", ")) };
    verdict(
        format!("{} tests in {} binaries, {} on the base{note}", total(&after), after.len(), total(&before)),
        findings,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verdict::codes;

    const LISTING: &str = "a::lib t1\na::lib t2\nb::bin/b t1\n";

    fn owned(args: &[&str]) -> Vec<String> {
        args.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn the_same_tests_pass() {
        assert_eq!(compare(LISTING, LISTING, ""), Ok("3 tests in 2 binaries, 3 on the base".to_string()));
    }

    #[test]
    fn more_tests_and_new_binaries_pass() {
        let head = "a::lib t1\na::lib t2\na::lib t3\nb::bin/b t1\nc::lib t1\n";
        assert_eq!(compare(LISTING, head, ""), Ok("5 tests in 3 binaries, 3 on the base".to_string()));
    }

    #[test]
    fn fewer_tests_than_on_the_base_fail() {
        let found = compare(LISTING, "a::lib t1\nb::bin/b t1\n", "").unwrap_err();
        assert_eq!(found.iter().map(|f| f.code).collect::<Vec<_>>(), ["counts.fewer"]);
        assert_eq!(found[0].message, "a::lib has 1 tests, 2 on the base");
        assert!(found[0].remedy.contains("`Tests-Removed: a::lib <reason>`"), "{found:?}");
    }

    #[test]
    fn a_binary_gone_since_the_base_fails() {
        let found = compare(LISTING, "a::lib t1\na::lib t2\n", "").unwrap_err();
        assert_eq!(found.iter().map(|f| f.code).collect::<Vec<_>>(), ["counts.gone"]);
        assert_eq!(found[0].message, "b::bin/b is gone, with its 1 tests on the base");
        assert!(found[0].remedy.contains("`Tests-Removed: b::bin/b <reason>`"), "{found:?}");
    }

    #[test]
    fn a_footer_with_its_reason_allows_the_removal() {
        let log = "test(x): one table\n\nTests-Removed: a::lib two cases are one table test now\n\nfeat: y\n\n\
                   Tests-Removed: b::bin/b gone with the old command\n";
        assert_eq!(
            compare(LISTING, "a::lib t1\n", log),
            Ok("1 tests in 1 binaries, 3 on the base; removals named for a::lib, b::bin/b".to_string())
        );
        assert_eq!(
            compare(LISTING, "a::lib t1\nb::bin/b t1\n", log),
            Ok("2 tests in 2 binaries, 3 on the base; removals named for a::lib".to_string())
        );
    }

    #[test]
    fn a_footer_without_its_reason_or_for_another_binary_does_not() {
        let fewer = "a::lib t1\nb::bin/b t1\n";
        for log in ["Tests-Removed: a::lib\n", "Tests-Removed: a::lib   \n", "Tests-Removed: b::bin/b why\n"] {
            assert_eq!(codes(&compare(LISTING, fewer, log)), ["counts.fewer"], "{log:?}");
        }
        assert_eq!(codes(&compare(LISTING, fewer, "not Tests-Removed: a::lib why\n")), ["counts.fewer"]);
    }

    #[test]
    fn footers_name_each_binary_with_a_reason() {
        let log = "s\n\nTests-Removed: a::lib why\n  Tests-Removed:  b::bin/b  because  \nTests-Removed: c\nx Tests-Removed: d e\n";
        assert_eq!(removals(log), BTreeSet::from(["a::lib", "b::bin/b"]));
        assert_eq!(removals("Tests-Removed:a::lib why\n"), BTreeSet::from(["a::lib"]));
    }

    #[test]
    fn no_tests_on_either_side_fails_closed() {
        assert_eq!(codes(&compare("", LISTING, "")), ["counts.no-tests"]);
        assert_eq!(codes(&compare(LISTING, "", "")), ["counts.no-tests", "counts.gone", "counts.gone"]);
        let found = compare("", LISTING, "").unwrap_err();
        assert!(found[0].message.contains("the base"), "{found:?}");
        let found = compare(LISTING, "", "").unwrap_err();
        assert!(found[0].message.contains("this tree"), "{found:?}");
    }

    #[test]
    fn without_a_base_the_listing_alone_is_counted_and_says_so() {
        assert_eq!(alone(LISTING, "why"), Ok("3 tests in 2 binaries, not compared: why".to_string()));
        assert_eq!(codes(&alone("", "why")), ["counts.no-tests"]);
        assert_eq!(codes(&alone("junk\n", "why")), ["counts.no-tests"]);
    }

    #[test]
    fn a_listing_line_without_a_test_name_is_not_a_test() {
        assert_eq!(
            compare("a::lib t\n", "a::lib t\njunk\n", ""),
            Ok("1 tests in 1 binaries, 1 on the base".to_string())
        );
    }

    #[test]
    fn the_base_flag_is_parsed() {
        assert_eq!(base_arg(&[]), Ok(None));
        assert_eq!(base_arg(&owned(&["--base", "origin/main"])), Ok(Some(&"origin/main".to_string())));
        for bad in [&["--base"][..], &["--update"], &["--base", "a", "b"], &["--other", "a"]] {
            let found = base_arg(&owned(bad)).unwrap_err();
            assert_eq!(found.iter().map(|f| f.code).collect::<Vec<_>>(), ["counts.usage"], "{bad:?}");
        }
    }
}
