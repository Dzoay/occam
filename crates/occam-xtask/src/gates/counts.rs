//! `gates counts`: tests cannot silently disappear (L44). Each test binary has a minimum in `ci/test-counts.txt`, and
//! the file merges by construction through `scripts/merge-counts.sh`.

use std::collections::BTreeMap;

use crate::repo::Repo;
use crate::verdict::{Finding, Verdict, verdict};

pub const MANIFEST: &str = "ci/test-counts.txt";

pub fn run(repo: &Repo, args: &[String]) -> Verdict {
    let listing = repo.test_listing().map_err(|e| vec![e])?;
    let text = repo.read(MANIFEST).map_err(|e| vec![e])?;
    match args {
        [] => check(&text, &listing),
        [flag] if flag == "--update" => {
            repo.write(MANIFEST, &update(&text, &listing)).map_err(|e| vec![e])?;
            Ok(format!("{MANIFEST} updated"))
        }
        _ => Err(vec![Finding::new(
            "counts.usage",
            "",
            format!("unexpected arguments: {}", args.join(" ")),
            "pass --update or nothing",
        )]),
    }
}

fn is_entry(line: &str) -> bool {
    !line.trim().is_empty() && !line.starts_with('#')
}

fn entry(line: &str) -> Option<(String, usize)> {
    let mut words = line.split_whitespace();
    let (binary, minimum) = (words.next()?, words.next()?);
    if words.next().is_some() {
        return None;
    }
    Some((binary.to_string(), minimum.parse().ok()?))
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

fn check(manifest: &str, listing: &str) -> Verdict {
    let found = counted(listing);
    let mut findings = Vec::new();
    let mut minimums = BTreeMap::new();
    for (line, n) in manifest.lines().zip(1..).filter(|(line, _)| is_entry(line)) {
        match entry(line) {
            Some((binary, minimum)) => {
                minimums.insert(binary, minimum);
            }
            None => findings
                .push(Finding::new("counts.malformed", MANIFEST, "not `<binary> <minimum>`", "correct the line").at(n)),
        }
    }
    if found.is_empty() {
        findings.push(Finding::new("counts.no-tests", MANIFEST, "the listing has no tests", "fix the build"));
    }
    for (binary, minimum) in &minimums {
        match found.get(binary.as_str()) {
            None => findings.push(Finding::new(
                "counts.gone",
                MANIFEST,
                format!("{binary} no longer exists"),
                "restore its tests, or remove its line and say why in the PR",
            )),
            Some(count) if count < minimum => {
                findings.push(Finding::new(
                    "counts.fewer",
                    MANIFEST,
                    format!("{binary} has {count} tests, under its minimum of {minimum}"),
                    "restore the tests",
                ));
            }
            Some(_) => {}
        }
    }
    for binary in found.keys().filter(|binary| !minimums.contains_key(**binary)) {
        findings.push(Finding::new(
            "counts.unlisted",
            MANIFEST,
            format!("{binary} has no minimum"),
            "run `just counts`",
        ));
    }
    verdict(format!("{} tests in {} binaries", found.values().sum::<usize>(), found.len()), findings)
}

fn comments(text: &str) -> impl Iterator<Item = &str> {
    text.lines().filter(|line| line.starts_with('#'))
}

fn render<'a>(header: impl Iterator<Item = &'a str>, entries: &BTreeMap<String, usize>) -> String {
    let rows = entries.iter().map(|(binary, minimum)| format!("{binary} {minimum}"));
    let lines: Vec<String> = header.map(str::to_string).chain(rows).map(|line| line + "\n").collect();
    lines.concat()
}

/// Raises minimums and adds binaries; it never lowers one or removes one.
fn update(manifest: &str, listing: &str) -> String {
    let mut entries: BTreeMap<String, usize> = manifest.lines().filter_map(entry).collect();
    for (binary, count) in counted(listing) {
        let minimum = entries.entry(binary.to_string()).or_insert(0);
        *minimum = (*minimum).max(count);
    }
    render(comments(manifest), &entries)
}

fn strict(text: &str) -> Option<BTreeMap<String, usize>> {
    text.lines().filter(|line| is_entry(line)).map(entry).collect()
}

/// The three-way merge of the minimums, or `None` where it does not decide: the larger minimum wins, an addition stays,
/// and a removal stands only if the other side left the line alone.
pub fn merge(base: &str, ours: &str, theirs: &str) -> Option<String> {
    let (old, mine, other) = (strict(base)?, strict(ours)?, strict(theirs)?);
    let mut merged = BTreeMap::new();
    for key in mine.keys().chain(other.keys()) {
        match (mine.get(key), other.get(key)) {
            (Some(a), Some(b)) => {
                merged.insert(key.clone(), *a.max(b));
            }
            (a, b) => {
                let only = a.or(b)?;
                match old.get(key) {
                    None => {
                        merged.insert(key.clone(), *only);
                    }
                    Some(value) if value == only => {}
                    Some(_) => return None,
                }
            }
        }
    }
    Some(render(comments(ours), &merged))
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verdict::codes;

    const LISTING: &str = "a::lib t1\na::lib t2\nb::bin/b t1\n";

    #[test]
    fn minimums_met_pass() {
        assert_eq!(check("# c\n\na::lib 2\nb::bin/b 1\n", LISTING), Ok("3 tests in 2 binaries".to_string()));
    }

    #[test]
    fn more_tests_than_the_minimum_pass() {
        assert!(check("a::lib 1\nb::bin/b 1\n", LISTING).is_ok());
    }

    #[test]
    fn fewer_tests_than_the_minimum_fail() {
        let found = check("a::lib 3\nb::bin/b 1\n", LISTING).unwrap_err();
        assert_eq!(found[0].code, "counts.fewer");
        assert_eq!(found[0].file, MANIFEST);
        assert!(
            found[0].message.contains("a::lib") && found[0].message.contains('2') && found[0].message.contains('3'),
            "{found:?}"
        );
    }

    #[test]
    fn a_vanished_binary_fails() {
        assert_eq!(codes(&check("a::lib 2\nb::bin/b 1\nc::lib 1\n", LISTING)), ["counts.gone"]);
    }

    #[test]
    fn a_binary_with_no_minimum_fails() {
        let found = check("a::lib 2\n", LISTING).unwrap_err();
        assert_eq!(found[0].code, "counts.unlisted");
        assert!(found[0].message.contains("b::bin/b"), "{found:?}");
    }

    #[test]
    fn a_malformed_line_fails() {
        assert_eq!(codes(&check("a::lib 2\nb::bin/b 1\nbad\n", LISTING)), ["counts.malformed"]);
        assert_eq!(codes(&check("a::lib 2\nb::bin/b x\n", LISTING)), ["counts.malformed", "counts.unlisted"]);
    }

    #[test]
    fn no_tests_at_all_fails_closed() {
        assert_eq!(codes(&check("a::lib 2\n", "")), ["counts.no-tests", "counts.gone"]);
    }

    #[test]
    fn a_listing_line_without_a_test_name_is_not_a_test() {
        assert_eq!(check("a::lib 1\n", "a::lib t\njunk\n"), Ok("1 tests in 1 binaries".to_string()));
    }

    #[test]
    fn update_raises_and_adds_and_never_lowers() {
        let got = update("# header\na::lib 1\nb::bin/b 5\nkeep::lib 1\n", LISTING);
        assert_eq!(got, "# header\na::lib 2\nb::bin/b 5\nkeep::lib 1\n");
        assert_eq!(update("", "new::lib t\n"), "new::lib 1\n");
    }

    #[test]
    fn merge_takes_the_larger_minimum() {
        assert_eq!(merge("a 1\n", "a 2\n", "a 3\n").as_deref(), Some("a 3\n"));
        assert_eq!(merge("a 1\n", "a 3\n", "a 2\n").as_deref(), Some("a 3\n"));
    }

    #[test]
    fn merge_keeps_binaries_added_on_either_side() {
        assert_eq!(merge("", "# c\na 1\n", "b 2\n").as_deref(), Some("# c\na 1\nb 2\n"));
    }

    #[test]
    fn merge_keeps_a_removal_the_other_side_left_alone() {
        assert_eq!(merge("a 1\nb 1\n", "a 1\n", "a 1\nb 1\n").as_deref(), Some("a 1\n"));
        assert_eq!(merge("a 1\nb 1\n", "a 1\nb 1\n", "a 1\n").as_deref(), Some("a 1\n"));
    }

    #[test]
    fn merge_does_not_decide_a_removal_against_a_change() {
        assert_eq!(merge("a 1\nb 1\n", "a 1\n", "a 1\nb 2\n"), None);
        assert_eq!(merge("a 1\nb 1\n", "a 1\nb 2\n", "a 1\n"), None);
    }

    #[test]
    fn merge_does_not_decide_malformed_input() {
        assert_eq!(merge("bad\n", "a 1\n", "a 1\n"), None);
        assert_eq!(merge("a 1\n", "bad\n", "a 1\n"), None);
        assert_eq!(merge("a 1\n", "a 1\n", "bad\n"), None);
    }
}
