//! `gates docs`: links resolve, step and decision ids are unique, and evidence is frozen once merged (L41, L45).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use super::no_args;
use crate::plan::{PLAN, steps};
use crate::repo::Repo;
use crate::verdict::{Finding, Verdict, verdict};

const EVIDENCE: &str = "docs/evidence/";
const DECISIONS: &str = "docs/decisions/";

/// Everything the gate reads.
struct Input<'a> {
    /// Every tracked Markdown file.
    markdown: &'a [(String, String)],
    /// Every tracked path.
    paths: &'a BTreeSet<String>,
    /// `git diff --name-status` against the merge base.
    name_status: &'a str,
}

pub fn run(repo: &Repo, args: &[String]) -> Verdict {
    no_args(args)?;
    let markdown = repo
        .texts(|path| Path::new(path).extension().is_some_and(|extension| extension == "md"))
        .map_err(|e| vec![e])?;
    let paths = repo.paths().map_err(|e| vec![e])?.into_iter().collect();
    let base = repo.base().map_err(|e| vec![e])?;
    let name_status = repo.git(&["diff", "--name-status", &base, "--", EVIDENCE]).map_err(|e| vec![e])?;
    check(&Input { markdown: &markdown, paths: &paths, name_status: &name_status })
}

/// The numbered lines of `text` outside code fences.
fn prose(text: &str) -> Vec<(usize, &str)> {
    let mut fenced = false;
    let mut lines = Vec::new();
    for (line, n) in text.lines().zip(1..) {
        if line.trim_start().starts_with("```") {
            fenced = !fenced;
        } else if !fenced {
            lines.push((n, line));
        }
    }
    lines
}

fn is_external(target: &str) -> bool {
    target.split_once(':').is_some_and(|(scheme, _)| scheme.chars().all(|c| c.is_ascii_alphabetic()))
}

/// Every relative link target in `text`, with its line.
fn links(text: &str) -> Vec<(usize, String)> {
    let mut found = Vec::new();
    for (n, line) in prose(text) {
        let visible: String = line.split('`').step_by(2).collect();
        let mut rest = visible.as_str();
        while let Some((_, after)) = rest.split_once("](") {
            let Some((target, tail)) = after.split_once(')') else { break };
            rest = tail;
            let target = target.split_whitespace().next().unwrap_or_default();
            if !is_external(target) {
                found.push((n, target.to_string()));
            }
        }
    }
    found
}

/// The anchors GitHub gives the headings of `text`.
fn anchors(text: &str) -> BTreeSet<String> {
    let mut seen: BTreeMap<String, usize> = BTreeMap::new();
    let mut found = BTreeSet::new();
    for (_, line) in prose(text) {
        let rest = line.trim_start_matches('#');
        if !line.starts_with('#') || !rest.starts_with(' ') {
            continue;
        }
        let kept = rest.trim().to_lowercase();
        let slug: String = kept
            .chars()
            .filter(|c| c.is_alphanumeric() || matches!(c, ' ' | '-' | '_'))
            .map(|c| if c == ' ' { '-' } else { c })
            .collect();
        let count = seen.entry(slug.clone()).or_insert(0);
        found.insert(if *count == 0 { slug } else { format!("{slug}-{count}") });
        *count = count.saturating_add(1);
    }
    found
}

/// `target` relative to the file `from`, or `None` if it leaves the repository.
fn resolve(from: &str, target: &str) -> Option<String> {
    let mut parts: Vec<&str> = from.split('/').collect();
    parts.pop();
    for part in target.split('/') {
        match part {
            "" | "." => {}
            ".." => {
                parts.pop()?;
            }
            other => parts.push(other),
        }
    }
    Some(parts.join("/"))
}

fn exists(paths: &BTreeSet<String>, path: &str) -> bool {
    paths.contains(path) || paths.iter().any(|known| known.strip_prefix(path).is_some_and(|rest| rest.starts_with('/')))
}

/// What is wrong with one link.
fn check_link(
    from: &str,
    target: &str,
    paths: &BTreeSet<String>,
    known: &BTreeMap<&str, BTreeSet<String>>,
) -> Option<Finding> {
    let (path, fragment) = target.split_once('#').map_or((target, None), |(path, fragment)| (path, Some(fragment)));
    let resolved = if path.is_empty() { Some(from.to_string()) } else { resolve(from, path) };
    let Some(resolved) = resolved.filter(|resolved| exists(paths, resolved)) else {
        return Some(Finding::new(
            "docs.link-missing",
            from,
            format!("`{target}` does not resolve"),
            "correct the link, or add its target",
        ));
    };
    let fragment = fragment?;
    let found = known.get(resolved.as_str())?;
    (!found.contains(fragment)).then(|| {
        Finding::new(
            "docs.anchor-missing",
            from,
            format!("`{target}`: no heading gives the anchor {fragment}"),
            "correct the anchor",
        )
    })
}

fn duplicate_steps(markdown: &[(String, String)]) -> Vec<Finding> {
    let mut seen = BTreeSet::new();
    let mut findings = Vec::new();
    for (path, text) in markdown.iter().filter(|(path, _)| path == PLAN || path.starts_with("docs/plan/")) {
        for (line, id) in steps(text) {
            if !seen.insert(id.clone()) {
                findings.push(
                    Finding::new(
                        "docs.step-duplicate",
                        path,
                        format!("step `{id}` already exists"),
                        "give it its own id",
                    )
                    .at(line),
                );
            }
        }
    }
    findings
}

/// The slug of a decision file's name, if it has the form `<date>-<slug>.md`.
pub fn decision_slug(name: &str) -> Option<&str> {
    let stem = name.strip_suffix(".md")?;
    let (date, dash, slug) = (stem.get(..10)?, stem.get(10..11)?, stem.get(11..)?);
    let dated = date.char_indices().all(|(i, c)| if i == 4 || i == 7 { c == '-' } else { c.is_ascii_digit() });
    let named = !slug.is_empty() && slug.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    (dated && dash == "-" && named).then_some(slug)
}

/// The slugs of every decision among `paths`.
pub fn decision_slugs(paths: &[String]) -> BTreeSet<String> {
    let names = paths.iter().filter_map(|path| path.strip_prefix(DECISIONS));
    names.filter_map(decision_slug).map(str::to_string).collect()
}

fn decisions(paths: &BTreeSet<String>) -> Vec<Finding> {
    let mut seen = BTreeSet::new();
    let mut findings = Vec::new();
    for path in paths.iter().filter(|path| path.starts_with(DECISIONS)) {
        match path.strip_prefix(DECISIONS).and_then(decision_slug) {
            None => {
                findings.push(Finding::new("docs.decision-name", path, "not named `<date>-<slug>.md`", "rename it"));
            }
            Some(slug) if !seen.insert(slug) => {
                findings.push(Finding::new(
                    "docs.decision-duplicate",
                    path,
                    format!("decision `{slug}` already exists"),
                    "give it its own slug",
                ));
            }
            Some(_) => {}
        }
    }
    findings
}

fn evidence_changes(name_status: &str) -> Vec<Finding> {
    let entry = |line: &str| {
        let mut fields = line.split('\t');
        Some((fields.next()?.to_string(), fields.next()?.to_string()))
    };
    let changed =
        name_status.lines().filter_map(entry).filter(|(status, path)| path.starts_with(EVIDENCE) && status != "A");
    changed
        .map(|(status, path)| {
            Finding::new(
                "docs.evidence-changed",
                &path,
                format!("merged evidence is frozen, but this change is {status}"),
                "add a new dated file instead (L45)",
            )
        })
        .collect()
}

fn check(input: &Input) -> Verdict {
    if input.markdown.is_empty() {
        return Err(vec![Finding::new(
            "docs.no-files",
            "docs",
            "no Markdown files to check",
            "run from a checkout of the repository",
        )]);
    }
    let known: BTreeMap<&str, BTreeSet<String>> =
        input.markdown.iter().map(|(path, text)| (path.as_str(), anchors(text))).collect();
    let mut findings = Vec::new();
    let mut count = 0_usize;
    // Evidence is frozen, so a link that later breaks there cannot be repaired.
    for (path, text) in input.markdown.iter().filter(|(path, _)| !path.starts_with(EVIDENCE)) {
        for (line, target) in links(text) {
            count = count.saturating_add(1);
            findings.extend(check_link(path, &target, input.paths, &known).map(|finding| finding.at(line)));
        }
    }
    findings.extend(duplicate_steps(input.markdown));
    findings.extend(decisions(input.paths));
    findings.extend(evidence_changes(input.name_status));
    verdict(format!("{} Markdown files, {count} links", input.markdown.len()), findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verdict::codes;

    fn run_check(files: &[(&str, &str)], name_status: &str) -> Verdict {
        let markdown: Vec<(String, String)> = files.iter().map(|(p, t)| (p.to_string(), t.to_string())).collect();
        let paths: BTreeSet<String> = files.iter().map(|(p, _)| p.to_string()).collect();
        check(&Input { markdown: &markdown, paths: &paths, name_status })
    }

    fn with(text: &str) -> Vec<(&'static str, String)> {
        vec![
            ("docs/a.md", text.to_string()),
            ("docs/b.md", "# Title here\n\n## `Code` and (more): yes\n\n## Again\n\n## Again\n".to_string()),
        ]
    }

    fn check_text(text: &str) -> Verdict {
        let files = with(text);
        let refs: Vec<(&str, &str)> = files.iter().map(|(p, t)| (*p, t.as_str())).collect();
        run_check(&refs, "")
    }

    #[test]
    fn a_resolving_link_passes() {
        assert_eq!(check_text("[b](b.md) and [up](../docs/b.md)"), Ok("2 Markdown files, 2 links".to_string()));
    }

    #[test]
    fn a_missing_target_fails_with_its_line() {
        let found = check_text("first\n\n[b](nope.md)").unwrap_err();
        assert_eq!(found[0].code, "docs.link-missing");
        assert_eq!(found[0].file, "docs/a.md");
        assert_eq!(found[0].line, Some(3));
        assert!(found[0].message.contains("nope.md"), "{found:?}");
    }

    #[test]
    fn a_link_cannot_leave_the_repository() {
        assert_eq!(codes(&check_text("[x](../../../outside.md)")), ["docs.link-missing"]);
    }

    #[test]
    fn a_directory_resolves() {
        assert!(check_text("[d](./) [e](../docs/)").is_ok());
        assert_eq!(codes(&check_text("[d](nodir/)")), ["docs.link-missing"]);
    }

    #[test]
    fn external_links_and_code_are_not_checked() {
        assert_eq!(
            check_text("[x](https://example.com/a) [m](mailto:a@b)"),
            Ok("2 Markdown files, 0 links".to_string())
        );
        assert_eq!(
            check_text("`[x](nope.md)`\n\n```text\n[x](nope.md)\n```\n"),
            Ok("2 Markdown files, 0 links".to_string())
        );
    }

    #[test]
    fn code_inside_link_text_is_still_a_link() {
        assert_eq!(codes(&check_text("[`x`](nope.md)")), ["docs.link-missing"]);
    }

    #[test]
    fn a_link_title_is_not_part_of_the_target() {
        assert!(check_text("[b](b.md \"Title\")").is_ok());
    }

    #[test]
    fn several_links_on_a_line_are_all_checked() {
        assert_eq!(codes(&check_text("[a](b.md) [b](x.md) [c](y.md)")), ["docs.link-missing", "docs.link-missing"]);
    }

    #[test]
    fn an_unclosed_link_is_not_a_link() {
        assert_eq!(check_text("[a](b.md"), Ok("2 Markdown files, 0 links".to_string()));
    }

    #[test]
    fn anchors_follow_github_slugs() {
        assert!(check_text("[b](b.md#title-here)").is_ok());
        assert!(check_text("[b](b.md#code-and-more-yes)").is_ok());
        assert!(check_text("[b](b.md#again) [c](b.md#again-1)").is_ok());
        let found = check_text("[b](b.md#again-2)").unwrap_err();
        assert_eq!(found[0].code, "docs.anchor-missing");
        assert!(found[0].message.contains("again-2"), "{found:?}");
    }

    #[test]
    fn an_anchor_in_the_same_file_resolves() {
        assert!(check_text("# Heading one\n\n[a](#heading-one)").is_ok());
        assert_eq!(codes(&check_text("# Heading one\n\n[a](#heading-two)")), ["docs.anchor-missing"]);
    }

    #[test]
    fn a_heading_needs_its_hashes_and_a_space() {
        assert_eq!(codes(&check_text("#nospace\n\n[a](#nospace)")), ["docs.anchor-missing"]);
        assert_eq!(codes(&check_text(" indented\n\n[a](#indented)")), ["docs.anchor-missing"]);
    }

    #[test]
    fn a_heading_in_a_code_fence_is_no_anchor() {
        assert_eq!(codes(&check_text("```\n# Fenced\n```\n[a](#fenced)")), ["docs.anchor-missing"]);
    }

    #[test]
    fn evidence_is_not_link_checked_because_it_is_frozen() {
        assert!(run_check(&[("docs/evidence/a.md", "[x](nope.md)")], "").is_ok());
    }

    #[test]
    fn a_repeated_step_id_fails() {
        let files =
            [("docs/PLAN.md", ""), ("docs/plan/a.md", "### `a.one`\n"), ("docs/plan/b.md", "\n| `a.one` | x |\n")];
        let found = run_check(&files, "").unwrap_err();
        assert_eq!(found[0].code, "docs.step-duplicate");
        assert_eq!(found[0].file, "docs/plan/b.md");
        assert_eq!(found[0].line, Some(2));
    }

    #[test]
    fn steps_only_count_in_the_plan_files() {
        assert!(run_check(&[("docs/plan/a.md", "### `a.one`\n"), ("docs/notes.md", "### `a.one`\n")], "").is_ok());
    }

    #[test]
    fn a_decision_needs_a_dated_name() {
        for bad in [
            "docs/decisions/no-date.md",
            "docs/decisions/2026-1-08-x.md",
            "docs/decisions/2026-11-08-.md",
            "docs/decisions/2026-11-08-Upper.md",
            "docs/decisions/2026-11-08_x.md",
            "docs/decisions/2026x11-08-x.md",
        ] {
            assert_eq!(codes(&run_check(&[(bad, "")], "")), ["docs.decision-name"], "{bad}");
        }
        assert!(run_check(&[("docs/decisions/2026-11-08-a-b2.md", "")], "").is_ok());
    }

    #[test]
    fn a_repeated_decision_slug_fails() {
        let files = [("docs/decisions/2026-11-08-same.md", ""), ("docs/decisions/2026-12-01-same.md", "")];
        let found = run_check(&files, "").unwrap_err();
        assert_eq!(found[0].code, "docs.decision-duplicate");
        assert_eq!(found[0].file, "docs/decisions/2026-12-01-same.md");
    }

    #[test]
    fn slugs_are_taken_from_dated_names() {
        assert_eq!(decision_slug("2026-11-08-a-b.md"), Some("a-b"));
        assert_eq!(decision_slug("2026-11-08-a-b.txt"), None);
    }

    #[test]
    fn the_decisions_are_read_from_their_directory() {
        let paths = [
            "docs/decisions/2026-11-08-a.md".to_string(),
            "docs/plan/2026-11-08-b.md".to_string(),
            "docs/decisions/nope.md".to_string(),
        ];
        assert_eq!(decision_slugs(&paths).into_iter().collect::<Vec<_>>(), ["a"]);
    }

    #[test]
    fn changing_merged_evidence_fails() {
        for status in
            ["M\tdocs/evidence/a.md\n", "D\tdocs/evidence/a.md\n", "R100\tdocs/evidence/a.md\tdocs/evidence/b.md\n"]
        {
            let found = run_check(&[("docs/x.md", "")], status).unwrap_err();
            assert_eq!(found[0].code, "docs.evidence-changed", "{status}");
            assert_eq!(found[0].file, "docs/evidence/a.md", "{status}");
        }
    }

    #[test]
    fn adding_evidence_passes() {
        assert!(run_check(&[("docs/x.md", "")], "A\tdocs/evidence/new.md\n").is_ok());
    }

    #[test]
    fn nothing_to_check_fails_closed() {
        assert_eq!(codes(&run_check(&[], "")), ["docs.no-files"]);
    }
}
