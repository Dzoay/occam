//! `gates provenance`: nothing is named after a kernel occam may not read, every kernel file says where it came from,
//! and no licence outside the allow-list enters the repository (L34, L36, the provenance and no-copyleft decisions).

use std::collections::{BTreeMap, BTreeSet};
use std::path::Path;

use proc_macro2::{TokenStream, TokenTree};

use super::no_args;
use crate::glob;
use crate::plan::{self, PLAN, cells};
use crate::repo::Repo;
use crate::verdict::{Finding, Verdict, verdict};

const FORBIDDEN: &str = "ci/forbidden-identifiers.txt";
const SOURCES: &str = "docs/SOURCES.md";
const REUSE: &str = "REUSE.toml";
const DENY: &str = "deny.toml";

/// Everything the gate reads.
struct Input<'a> {
    plan: &'a str,
    forbidden: &'a str,
    sources: &'a str,
    reuse: &'a str,
    deny: &'a str,
    /// Every Rust file under `crates/` and `web/`.
    rust: &'a [(String, String)],
    /// Every tracked text file, for `SPDX-License-Identifier` tags.
    tracked: &'a [(String, String)],
    /// The names in `LICENSES/`.
    licences: &'a [String],
}

fn is_rust(path: &str) -> bool {
    (path.starts_with("crates/") || path.starts_with("web/"))
        && Path::new(path).extension().is_some_and(|extension| extension == "rs")
}

pub fn run(repo: &Repo, args: &[String]) -> Verdict {
    no_args(args)?;
    let read = |path: &str| repo.read(path).map_err(|e| vec![e]);
    let (plan, forbidden, sources, reuse, deny) =
        (read(PLAN)?, read(FORBIDDEN)?, read(SOURCES)?, read(REUSE)?, read(DENY)?);
    let rust = repo.texts(is_rust).map_err(|e| vec![e])?;
    let tracked = repo.texts(|path| !path.starts_with("LICENSE")).map_err(|e| vec![e])?;
    let paths = repo.paths().map_err(|e| vec![e])?;
    let licences: Vec<String> = paths.iter().filter_map(|p| p.strip_prefix("LICENSES/")).map(str::to_string).collect();
    check(&Input {
        plan: &plan,
        forbidden: &forbidden,
        sources: &sources,
        reuse: &reuse,
        deny: &deny,
        rust: &rust,
        tracked: &tracked,
        licences: &licences,
    })
}

fn idents(stream: TokenStream, found: &mut Vec<(usize, String)>) {
    for tree in stream {
        match tree {
            TokenTree::Ident(ident) => found.push((ident.span().start().line, ident.to_string())),
            TokenTree::Group(group) => idents(group.stream(), found),
            _ => {}
        }
    }
}

/// The licence ids of an SPDX expression.
fn spdx_ids(expression: &str) -> Vec<String> {
    let words = expression.replace(['(', ')'], " ");
    words.split_whitespace().filter(|word| !matches!(*word, "AND" | "OR" | "WITH")).map(str::to_string).collect()
}

/// The ids `deny.toml` allows, the one home of the licence allow-list.
fn allowed(deny: &str) -> Option<BTreeSet<String>> {
    let table: toml::Table = deny.parse().ok()?;
    let list = table.get("licenses")?.get("allow")?.as_array()?;
    let ids: BTreeSet<String> = list.iter().filter_map(toml::Value::as_str).flat_map(spdx_ids).collect();
    (!ids.is_empty()).then_some(ids)
}

/// A row of `docs/SOURCES.md`.
struct Source<'a> {
    code: bool,
    licence: &'a str,
    line: usize,
}

fn sources(text: &str) -> BTreeMap<&str, Source<'_>> {
    let mut found = BTreeMap::new();
    for (row, line) in text.lines().zip(1..) {
        let Some(cells) = cells(row) else { continue };
        let (Some(key), Some(kind), Some(licence)) = (cells.first(), cells.get(1), cells.get(2)) else { continue };
        if let Some(key) = key.strip_prefix('`').and_then(|key| key.strip_suffix('`')) {
            found.insert(key, Source { code: *kind == "code", licence, line });
        }
    }
    found
}

/// One `[[annotations]]` entry of `REUSE.toml`: its paths and its licence expression.
fn annotations(text: &str) -> Option<Vec<(Vec<String>, String)>> {
    let table: toml::Table = text.parse().ok()?;
    let entries = table.get("annotations")?.as_array()?;
    let one = |entry: &toml::Value| {
        let path = entry.get("path")?;
        let paths = match path.as_array() {
            Some(list) => list.iter().filter_map(toml::Value::as_str).map(str::to_string).collect(),
            None => vec![path.as_str()?.to_string()],
        };
        Some((paths, entry.get("SPDX-License-Identifier")?.as_str()?.to_string()))
    };
    entries.iter().map(one).collect()
}

/// A licence tag on a comment line. A quotation in code is not a tag.
fn tag(line: &str) -> Option<&str> {
    let trimmed = line.trim_start();
    if !["//", "#", "/*", "*", "<!--"].iter().any(|prefix| trimmed.starts_with(prefix)) {
        return None;
    }
    let (_, rest) = trimmed.split_once("SPDX-License-Identifier:")?;
    Some(rest.trim().trim_end_matches("-->").trim_end_matches("*/").trim())
}

/// A finding for every id in `expression` outside the allow-list.
fn licence_findings(allow: &BTreeSet<String>, file: &str, line: Option<usize>, expression: &str) -> Vec<Finding> {
    let bad = spdx_ids(expression).into_iter().filter(|id| !allow.contains(id));
    bad.map(|id| {
        let remedy = "remove it: no copyleft enters this repository (docs/decisions/2026-11-08-no-copyleft.md)";
        let finding = Finding::new(
            "provenance.licence-not-allowed",
            file,
            format!("{id} is not on the allow-list in deny.toml"),
            remedy,
        );
        line.map_or(finding.clone(), |n| finding.at(n))
    })
    .collect()
}

/// The licences declared anywhere in the repository.
fn licences(input: &Input, allow: &BTreeSet<String>, reuse: &[(Vec<String>, String)]) -> Vec<Finding> {
    let mut findings: Vec<Finding> =
        reuse.iter().flat_map(|(_, expression)| licence_findings(allow, REUSE, None, expression)).collect();
    for source in sources(input.sources).values().filter(|source| source.code) {
        findings.extend(licence_findings(allow, SOURCES, Some(source.line), source.licence));
    }
    for name in input.licences {
        let id = name.strip_suffix(".txt").unwrap_or(name);
        findings.extend(licence_findings(allow, &format!("LICENSES/{name}"), None, id));
    }
    for (path, text) in input.tracked {
        for (line, n) in text.lines().zip(1..) {
            findings.extend(
                tag(line).into_iter().flat_map(|expression| licence_findings(allow, path, Some(n), expression)),
            );
        }
    }
    findings
}

/// What is wrong with a kernel file's `Source:` line.
fn source_finding(input: &Input, reuse: &[(Vec<String>, String)], path: &str, text: &str) -> Option<Finding> {
    let Some(key) = text.lines().find_map(|line| line.strip_prefix("//! Source: ")).map(str::trim) else {
        let remedy = format!("start the file with `//! Source: original`, or a key from {SOURCES}");
        return Some(Finding::new("provenance.no-source", path, "no `//! Source:` line", remedy));
    };
    if key == "original" {
        return None;
    }
    let table = sources(input.sources);
    let Some(source) = table.get(key) else {
        return Some(Finding::new(
            "provenance.unknown-source",
            path,
            format!("`{key}` is not in {SOURCES}"),
            "add its row, or correct the key",
        ));
    };
    let specific = |paths: &Vec<String>| paths.iter().any(|pattern| pattern != "**" && glob::matches(pattern, path));
    if source.code && !reuse.iter().any(|(paths, _)| specific(paths)) {
        return Some(Finding::new(
            "provenance.undeclared-derived",
            path,
            format!("ported from `{key}` but not declared in {REUSE}"),
            "add an annotation naming this file and its original's copyright",
        ));
    }
    None
}

fn is_kernel(path: &str, kernel: &BTreeSet<String>) -> bool {
    path.strip_prefix("crates/").and_then(|rest| rest.split('/').next()).is_some_and(|name| kernel.contains(name))
}

fn check(input: &Input) -> Verdict {
    let patterns: Vec<&str> =
        input.forbidden.lines().map(str::trim).filter(|line| !line.is_empty() && !line.starts_with('#')).collect();
    if patterns.is_empty() {
        return Err(vec![Finding::new(
            "provenance.no-forbidden-list",
            FORBIDDEN,
            "the list has no names",
            "restore the list",
        )]);
    }
    let Some(allow) = allowed(input.deny) else {
        return Err(vec![Finding::new(
            "provenance.no-allow-list",
            DENY,
            "no licences under [licenses] allow",
            "restore the allow-list",
        )]);
    };
    let Some(reuse) = annotations(input.reuse) else {
        return Err(vec![Finding::new("provenance.unparsable", REUSE, "not readable as REUSE.toml", "fix the file")]);
    };
    let kernel = plan::kernel(&plan::tiers(input.plan));
    let mut findings = licences(input, &allow, &reuse);
    let mut with_source = 0_usize;
    for (path, text) in input.rust {
        let stream: TokenStream = match text.parse() {
            Ok(stream) => stream,
            Err(error) => {
                findings.push(Finding::new("provenance.unparsable", path, format!("{error}"), "fix the syntax"));
                continue;
            }
        };
        let mut found = Vec::new();
        idents(stream, &mut found);
        for (line, ident) in
            found.iter().filter(|(_, ident)| patterns.iter().any(|pattern| glob::matches(pattern, ident)))
        {
            let remedy = "rename it: nothing is named after a kernel occam may not read (docs/decisions/2026-11-08-licence-and-provenance.md)";
            findings.push(
                Finding::new("provenance.forbidden-identifier", path, format!("`{ident}` matches {FORBIDDEN}"), remedy)
                    .at(*line),
            );
        }
        if is_kernel(path, &kernel) {
            with_source = with_source.saturating_add(1);
            findings.extend(source_finding(input, &reuse, path, text));
        }
    }
    verdict(format!("{} Rust files, {with_source} kernel files with a source", input.rust.len()), findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verdict::codes;

    const PLAN_TEXT: &str = "## 4. Layers\n\n```text\nocc-skip  x\noccam-math        Real\noccam             composition\noccam-cli         facade\n```\n";
    const GOOD: &str = "//! Source: original\nfn f() {}\n";

    struct Fixture {
        plan: String,
        forbidden: String,
        sources: String,
        reuse: String,
        deny: String,
        rust: Vec<(String, String)>,
        tracked: Vec<(String, String)>,
        licences: Vec<String>,
    }

    impl Fixture {
        fn good() -> Self {
            Self {
                plan: PLAN_TEXT.to_string(),
                forbidden: "# names\n\nTopoDS*\ngp_*\nPave\n".to_string(),
                sources: "| Key | Kind | Licence | Citation |\n|---|---|---|---|\n| `paper-a` | paper | n/a | A paper |\n| `port-b` | code | MIT | A port |\n".to_string(),
                reuse: "version = 1\n[[annotations]]\npath = \"**\"\nSPDX-License-Identifier = \"MIT OR Apache-2.0\"\n".to_string(),
                deny: "[licenses]\nallow = [\"MIT\", \"Apache-2.0\", \"Apache-2.0 WITH LLVM-exception\"]\n".to_string(),
                rust: vec![("crates/occam-math/src/lib.rs".to_string(), GOOD.to_string())],
                tracked: Vec::new(),
                licences: vec!["MIT.txt".to_string(), "Apache-2.0.txt".to_string()],
            }
        }

        fn check(&self) -> Verdict {
            check(&Input {
                plan: &self.plan,
                forbidden: &self.forbidden,
                sources: &self.sources,
                reuse: &self.reuse,
                deny: &self.deny,
                rust: &self.rust,
                tracked: &self.tracked,
                licences: &self.licences,
            })
        }

        fn with_rust(mut self, path: &str, text: &str) -> Self {
            self.rust = vec![(path.to_string(), text.to_string())];
            self
        }
    }

    const KERNEL: &str = "crates/occam-math/src/lib.rs";

    #[test]
    fn a_clean_tree_passes() {
        assert_eq!(Fixture::good().check(), Ok("1 Rust files, 1 kernel files with a source".to_string()));
    }

    #[test]
    fn a_forbidden_identifier_fails_with_its_line() {
        let found = Fixture::good().with_rust(KERNEL, "//! Source: original\n\nstruct gp_Pnt;\n").check().unwrap_err();
        assert_eq!(found[0].code, "provenance.forbidden-identifier");
        assert_eq!(found[0].file, KERNEL);
        assert_eq!(found[0].line, Some(3));
        assert!(found[0].message.contains("gp_Pnt"), "{found:?}");
    }

    #[test]
    fn an_identifier_inside_a_macro_or_block_is_still_found() {
        let verdict = Fixture::good()
            .with_rust(KERNEL, "//! Source: original\nfn f() { println!(\"{}\", g(TopoDS_Shape)); }\n")
            .check();
        assert_eq!(codes(&verdict), ["provenance.forbidden-identifier"]);
    }

    #[test]
    fn a_name_in_a_string_or_comment_is_not_an_identifier() {
        let text = "//! Source: original\n// TopoDS_Shape\nconst A: &str = \"gp_Pnt\";\n";
        assert!(Fixture::good().with_rust(KERNEL, text).check().is_ok());
    }

    #[test]
    fn an_exact_pattern_matches_only_the_whole_name() {
        assert!(Fixture::good().with_rust(KERNEL, "//! Source: original\nstruct Paved;\n").check().is_ok());
        assert_eq!(
            codes(&Fixture::good().with_rust(KERNEL, "//! Source: original\nstruct Pave;\n").check()),
            ["provenance.forbidden-identifier"]
        );
    }

    #[test]
    fn a_facade_needs_no_source_but_is_still_scanned() {
        assert!(Fixture::good().with_rust("crates/occam-cli/src/main.rs", "fn f() {}\n").check().is_ok());
        assert_eq!(
            codes(&Fixture::good().with_rust("crates/occam-cli/src/main.rs", "struct gp_Vec;\n").check()),
            ["provenance.forbidden-identifier"]
        );
        assert_eq!(
            codes(&Fixture::good().with_rust("web/a.rs", "struct gp_Vec;\n").check()),
            ["provenance.forbidden-identifier"]
        );
    }

    #[test]
    fn a_kernel_file_without_a_source_fails() {
        let found = Fixture::good().with_rust(KERNEL, "fn f() {}\n").check().unwrap_err();
        assert_eq!(found[0].code, "provenance.no-source");
        assert_eq!(found[0].file, KERNEL);
    }

    #[test]
    fn a_source_not_in_the_list_fails() {
        assert_eq!(
            codes(&Fixture::good().with_rust(KERNEL, "//! Source: made-up\n").check()),
            ["provenance.unknown-source"]
        );
    }

    #[test]
    fn a_paper_source_needs_no_declaration() {
        assert!(Fixture::good().with_rust(KERNEL, "//! Source: paper-a\n").check().is_ok());
    }

    #[test]
    fn a_ported_file_must_be_declared_in_reuse() {
        assert_eq!(
            codes(&Fixture::good().with_rust(KERNEL, "//! Source: port-b\n").check()),
            ["provenance.undeclared-derived"]
        );
        let mut fixture = Fixture::good().with_rust(KERNEL, "//! Source: port-b\n");
        fixture
            .reuse
            .push_str("[[annotations]]\npath = [\"crates/occam-math/src/*.rs\"]\nSPDX-License-Identifier = \"MIT\"\n");
        assert!(fixture.check().is_ok());
        let mut other = Fixture::good().with_rust(KERNEL, "//! Source: port-b\n");
        other.reuse.push_str("[[annotations]]\npath = \"crates/other/**\"\nSPDX-License-Identifier = \"MIT\"\n");
        assert_eq!(codes(&other.check()), ["provenance.undeclared-derived"]);
    }

    #[test]
    fn a_copyleft_annotation_in_reuse_fails() {
        let mut fixture = Fixture::good();
        fixture.reuse.push_str("[[annotations]]\npath = \"a\"\nSPDX-License-Identifier = \"GPL-3.0-only\"\n");
        let found = fixture.check().unwrap_err();
        assert_eq!(found[0].code, "provenance.licence-not-allowed");
        assert_eq!(found[0].file, REUSE);
        assert!(found[0].message.contains("GPL-3.0-only"), "{found:?}");
    }

    #[test]
    fn every_id_in_an_expression_must_be_allowed() {
        let mut fixture = Fixture::good();
        fixture.reuse.push_str("[[annotations]]\npath = \"a\"\nSPDX-License-Identifier = \"(MIT OR Apache-2.0 WITH LLVM-exception) AND LGPL-2.1-only\"\n");
        let found = fixture.check().unwrap_err();
        assert_eq!(found.len(), 1, "{found:?}");
        assert!(found[0].message.contains("LGPL-2.1-only"), "{found:?}");
    }

    #[test]
    fn a_copyleft_source_row_fails() {
        let mut fixture = Fixture::good();
        fixture.sources.push_str("| `port-c` | code | AGPL-3.0-only | A port |\n");
        let found = fixture.check().unwrap_err();
        assert_eq!(found[0].code, "provenance.licence-not-allowed");
        assert_eq!(found[0].file, SOURCES);
    }

    #[test]
    fn a_paper_row_has_no_licence_to_check() {
        let mut fixture = Fixture::good();
        fixture.sources.push_str("| `paper-c` | paper | GPL-3.0-only | Text only |\n");
        assert!(fixture.check().is_ok());
    }

    #[test]
    fn a_copyleft_licence_text_fails() {
        let mut fixture = Fixture::good();
        fixture.licences.push("GPL-3.0-only.txt".to_string());
        let found = fixture.check().unwrap_err();
        assert_eq!(found[0].code, "provenance.licence-not-allowed");
        assert_eq!(found[0].file, "LICENSES/GPL-3.0-only.txt");
    }

    #[test]
    fn a_copyleft_tag_in_any_file_fails() {
        for line in [
            "// SPDX-License-Identifier: GPL-2.0-or-later",
            "# SPDX-License-Identifier: MPL-2.0",
            "/* SPDX-License-Identifier: EPL-2.0 */",
            "<!-- SPDX-License-Identifier: GPL-3.0-only -->",
        ] {
            let mut fixture = Fixture::good();
            fixture.tracked = vec![("web/a.js".to_string(), format!("first\n{line}\n"))];
            let found = fixture.check().unwrap_err();
            assert_eq!(found[0].code, "provenance.licence-not-allowed", "{line}");
            assert_eq!(found[0].line, Some(2), "{line}");
        }
    }

    #[test]
    fn a_permitted_tag_passes_and_a_quotation_is_not_a_tag() {
        let mut fixture = Fixture::good();
        fixture.tracked = vec![("a.rs".to_string(), "// SPDX-License-Identifier: MIT OR Apache-2.0\nconst A: &str = \"SPDX-License-Identifier: GPL-3.0-only\";\n".to_string())];
        assert!(fixture.check().is_ok());
    }

    #[test]
    fn missing_lists_fail_closed() {
        let mut no_names = Fixture::good();
        no_names.forbidden = "# nothing\n".to_string();
        assert_eq!(codes(&no_names.check()), ["provenance.no-forbidden-list"]);
        let mut no_allow = Fixture::good();
        no_allow.deny = "[licenses]\n".to_string();
        assert_eq!(codes(&no_allow.check()), ["provenance.no-allow-list"]);
    }

    #[test]
    fn broken_inputs_fail() {
        assert_eq!(
            codes(&Fixture::good().with_rust(KERNEL, "//! Source: original\nfn (\"").check()),
            ["provenance.unparsable"]
        );
        let mut reuse = Fixture::good();
        reuse.reuse = "[".to_string();
        assert_eq!(codes(&reuse.check()), ["provenance.unparsable"]);
        let mut deny = Fixture::good();
        deny.deny = "[".to_string();
        assert_eq!(codes(&deny.check()), ["provenance.no-allow-list"]);
    }

    #[test]
    fn only_rust_under_crates_and_web_is_scanned() {
        assert!(is_rust("crates/a/src/lib.rs"));
        assert!(is_rust("web/a.rs"));
        assert!(!is_rust("tools/oracle/a.rs"));
        assert!(!is_rust("crates/a/Cargo.toml"));
    }
}
