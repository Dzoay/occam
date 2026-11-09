//! `gates errors`: every suggested fix is a claim with a test that follows it (L2). Every code and remedy pair in the
//! catalogue is registered by a `followed_remedy!` inside a `#[test]`, the catalogue is well formed, and a match on a code or
//! remedy has no catch-all, so a new one cannot be forgotten (L4).

use std::collections::BTreeSet;

use serde::Deserialize;
use syn::punctuated::Punctuated;
use syn::visit::{self, Visit};
use syn::{Expr, ExprMatch, ItemFn, Macro, Pat, Token};

use super::complexity::measured;
use super::no_args;
use crate::repo::Repo;
use crate::verdict::{Finding, Verdict, verdict};

/// Where the catalogue is written.
const CATALOGUE: &str = "crates/occam-report/src/code.rs";

pub fn run(repo: &Repo, args: &[String]) -> Verdict {
    no_args(args)?;
    let (ok, json, errors) = repo
        .capture(env!("CARGO"), &["run", "--quiet", "-p", "occam-report", "--example", "catalogue"])
        .map_err(|e| vec![e])?;
    if !ok {
        return Err(vec![Finding::new("errors.catalogue-unreadable", CATALOGUE, errors, "fix the build")]);
    }
    check(&json, &repo.texts(measured).map_err(|e| vec![e])?)
}

#[derive(Deserialize)]
struct CodeRow {
    name: String,
    code: String,
    remedies: Vec<String>,
}

#[derive(Deserialize)]
struct RemedyRow {
    name: String,
    kind: String,
}

#[derive(Deserialize)]
struct Catalogue {
    codes: Vec<CodeRow>,
    remedies: Vec<RemedyRow>,
}

/// Where in the source a `followed_remedy!` was found.
#[derive(Clone, Copy)]
enum Place {
    Outside,
    Test,
    /// A `#[should_panic]` test expects the remedy to fail, so it proves nothing.
    ShouldPanic,
}

/// A `followed_remedy!` inside a test.
struct Registration<'a> {
    path: &'a str,
    line: usize,
    code: String,
    kind: String,
}

/// What one file says.
struct Scan<'a> {
    path: &'a str,
    place: Place,
    registered: Vec<Registration<'a>>,
    findings: Vec<Finding>,
}

/// The variant named by a path such as `Code::Alpha`, if its owner is `owner`.
fn variant_of(expr: &Expr, owner: &str) -> Option<String> {
    let Expr::Path(path) = expr else { return None };
    let mut segments = path.path.segments.iter().rev();
    let variant = segments.next()?;
    (segments.next()?.ident == owner).then(|| variant.ident.to_string())
}

fn named_pair(item: &Macro) -> Option<(String, String)> {
    let args = item.parse_body_with(Punctuated::<Expr, Token![,]>::parse_terminated).ok()?;
    Some((variant_of(args.iter().nth(1)?, "Code")?, variant_of(args.iter().nth(2)?, "RemedyKind")?))
}

fn is_catalogued_path(path: &syn::Path) -> bool {
    let mut segments = path.segments.iter().rev().skip(1);
    segments.next().is_some_and(|owner| owner.ident == "Code" || owner.ident == "RemedyKind")
}

fn names_catalogued(pattern: &Pat) -> bool {
    match pattern {
        Pat::Path(path) => is_catalogued_path(&path.path),
        Pat::Or(or) => or.cases.iter().any(names_catalogued),
        Pat::Ident(ident) => ident.subpat.as_ref().is_some_and(|(_, inner)| names_catalogued(inner)),
        _ => false,
    }
}

fn is_catch_all(pattern: &Pat) -> bool {
    match pattern {
        Pat::Wild(_) => true,
        Pat::Ident(ident) => ident.subpat.is_none(),
        _ => false,
    }
}

impl Scan<'_> {
    fn registration(&mut self, item: &Macro, line: usize) {
        let Some((code, kind)) = named_pair(item) else {
            let message = "expected `followed_remedy!(report, Code::X, RemedyKind::Y, retry)`";
            self.findings.push(
                Finding::new("errors.malformed-registration", self.path, message, "name the pair by its paths")
                    .at(line),
            );
            return;
        };
        match self.place {
            Place::Test => self.registered.push(Registration { path: self.path, line, code, kind }),
            Place::Outside => {
                let message = format!("`followed_remedy!` for {code} and {kind} is outside a test");
                self.findings
                    .push(Finding::new("errors.not-in-a-test", self.path, message, "move it into a #[test]").at(line));
            }
            Place::ShouldPanic => {}
        }
    }
}

impl<'ast> Visit<'ast> for Scan<'_> {
    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        let outer = self.place;
        let has = |name: &str| item.attrs.iter().any(|attribute| attribute.path().is_ident(name));
        if has("should_panic") {
            self.place = Place::ShouldPanic;
        } else if has("test") {
            self.place = Place::Test;
        }
        visit::visit_item_fn(self, item);
        self.place = outer;
    }

    fn visit_macro(&mut self, item: &'ast Macro) {
        if let Some(segment) = item.path.segments.last().filter(|segment| segment.ident == "followed_remedy") {
            self.registration(item, segment.ident.span().start().line);
        }
        visit::visit_macro(self, item);
    }

    fn visit_expr_match(&mut self, item: &'ast ExprMatch) {
        let names = item.arms.iter().any(|arm| names_catalogued(&arm.pat));
        if names && item.arms.iter().any(|arm| is_catch_all(&arm.pat)) {
            let message = "a catch-all arm on a code or remedy would hide a new one (L4)";
            let finding = Finding::new("errors.non-exhaustive-match", self.path, message, "name every variant");
            self.findings.push(finding.at(item.match_token.span.start().line));
        }
        visit::visit_expr_match(self, item);
    }
}

/// Whether a code has the form `<area>.<name>`.
fn code_shape(code: &str) -> bool {
    let valid =
        |part: &str| !part.is_empty() && part.chars().all(|c| c.is_ascii_lowercase() || c.is_ascii_digit() || c == '-');
    code.split_once('.').is_some_and(|(area, name)| valid(area) && valid(name))
}

fn well_formed(catalogue: &Catalogue, findings: &mut Vec<Finding>) {
    let mut seen = BTreeSet::new();
    let codes = catalogue.codes.iter().map(|row| row.code.as_str());
    for text in codes.chain(catalogue.remedies.iter().map(|row| row.kind.as_str())) {
        if !seen.insert(text) {
            findings.push(Finding::new(
                "errors.duplicate",
                CATALOGUE,
                format!("`{text}` is listed twice"),
                "give each its own text",
            ));
        }
    }
    for row in catalogue.codes.iter().filter(|row| !code_shape(&row.code)) {
        findings.push(Finding::new(
            "errors.malformed-code",
            CATALOGUE,
            format!("`{}` is not `<area>.<name>`", row.code),
            "rename it",
        ));
    }
}

/// Every (code, remedy) pair the catalogue makes, by Rust name.
fn pairs(catalogue: &Catalogue, findings: &mut Vec<Finding>) -> BTreeSet<(String, String)> {
    let mut found = BTreeSet::new();
    for row in &catalogue.codes {
        for kind in &row.remedies {
            match catalogue.remedies.iter().find(|remedy| remedy.kind == *kind) {
                Some(remedy) => {
                    found.insert((row.name.clone(), remedy.name.clone()));
                }
                None => findings.push(Finding::new(
                    "errors.unknown-remedy",
                    CATALOGUE,
                    format!("{} names the unlisted remedy {kind}", row.code),
                    "add the remedy",
                )),
            }
        }
    }
    found
}

/// Reads every source file, returning what is registered in tests.
fn scan<'a>(sources: &'a [(String, String)], findings: &mut Vec<Finding>) -> Vec<Registration<'a>> {
    let mut registered = Vec::new();
    for (path, text) in sources {
        match syn::parse_file(text) {
            Ok(file) => {
                let mut scan = Scan { path, place: Place::Outside, registered: Vec::new(), findings: Vec::new() };
                scan.visit_file(&file);
                registered.append(&mut scan.registered);
                findings.append(&mut scan.findings);
            }
            Err(error) => findings.push(Finding::new("errors.unparsable", path, error.to_string(), "fix the syntax")),
        }
    }
    registered
}

fn check(catalogue: &str, sources: &[(String, String)]) -> Verdict {
    let catalogue: Catalogue = serde_json::from_str(catalogue).map_err(|error| {
        vec![Finding::new("errors.catalogue-unreadable", CATALOGUE, error.to_string(), "fix the catalogue listing")]
    })?;
    if catalogue.codes.is_empty() {
        return Err(vec![Finding::new(
            "errors.no-codes",
            CATALOGUE,
            "the catalogue lists no codes",
            "restore the catalogue",
        )]);
    }
    let mut findings = Vec::new();
    well_formed(&catalogue, &mut findings);
    let expected = pairs(&catalogue, &mut findings);
    let registered = scan(sources, &mut findings);
    let followed: BTreeSet<(String, String)> = registered.iter().map(|r| (r.code.clone(), r.kind.clone())).collect();
    for item in registered.iter().filter(|r| !expected.contains(&(r.code.clone(), r.kind.clone()))) {
        let message = format!("{} and {} are not paired in the catalogue", item.code, item.kind);
        findings.push(
            Finding::new("errors.unknown-pair", item.path, message, "pair them in the catalogue, or correct the test")
                .at(item.line),
        );
    }
    for (code, kind) in expected.iter().filter(|pair| !followed.contains(*pair)) {
        let message = format!("{code} suggests {kind}, but no test follows it (L2)");
        let remedy = format!("add a #[test] with followed_remedy!(report, Code::{code}, RemedyKind::{kind}, retry)");
        findings.push(Finding::new("errors.unregistered-pair", CATALOGUE, message, remedy));
    }
    verdict(
        format!("{} codes, {} code and remedy pairs, all followed", catalogue.codes.len(), expected.len()),
        findings,
    )
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verdict::codes;

    const GOOD: &str = r#"{"schema":1,"codes":[{"name":"Alpha","code":"area.alpha","remedies":["do-it"]},{"name":"Beta","code":"area.beta","remedies":[]}],"remedies":[{"name":"DoIt","kind":"do-it"}]}"#;
    const FOLLOWED: &str =
        "#[test]\nfn t() {\n    followed_remedy!(report, Code::Alpha, RemedyKind::DoIt, |r| retry(r));\n}\n";

    fn source(text: &str) -> Vec<(String, String)> {
        vec![("crates/a/src/lib.rs".to_string(), text.to_string())]
    }

    #[test]
    fn a_followed_pair_passes() {
        assert_eq!(check(GOOD, &source(FOLLOWED)), Ok("2 codes, 1 code and remedy pairs, all followed".to_string()));
    }

    #[test]
    fn a_catalogued_pair_with_no_test_fails() {
        let found = check(GOOD, &source("fn nothing() {}\n")).unwrap_err();
        assert_eq!(found[0].code, "errors.unregistered-pair");
        assert_eq!(found[0].file, CATALOGUE);
        assert!(found[0].message.contains("Alpha") && found[0].message.contains("DoIt"), "{found:?}");
        assert!(found[0].remedy.contains("followed_remedy!"), "{found:?}");
    }

    #[test]
    fn a_registration_outside_a_test_does_not_count() {
        let text = "fn t() {\n    followed_remedy!(report, Code::Alpha, RemedyKind::DoIt, |r| retry(r));\n}\n";
        let found = check(GOOD, &source(text)).unwrap_err();
        assert_eq!(
            found.iter().map(|f| f.code).collect::<Vec<_>>(),
            ["errors.not-in-a-test", "errors.unregistered-pair"]
        );
        assert_eq!(found[0].file, "crates/a/src/lib.rs");
        assert_eq!(found[0].line, Some(2));
    }

    #[test]
    fn a_function_after_a_test_is_not_a_test() {
        let text = format!(
            "{FOLLOWED}fn later() {{\n    followed_remedy!(report, Code::Alpha, RemedyKind::DoIt, |r| retry(r));\n}}\n"
        );
        assert_eq!(codes(&check(GOOD, &source(&text))), ["errors.not-in-a-test"]);
    }

    #[test]
    fn a_closure_inside_a_test_counts() {
        let text = "#[test]\nfn t() {\n    let f = || { followed_remedy!(report, Code::Alpha, RemedyKind::DoIt, |r| retry(r)); };\n    f();\n}\n";
        assert!(check(GOOD, &source(text)).is_ok());
    }

    #[test]
    fn a_registration_for_a_pair_not_catalogued_fails() {
        let text = format!(
            "{FOLLOWED}#[test]\nfn u() {{\n    followed_remedy!(report, Code::Beta, RemedyKind::DoIt, |r| retry(r));\n}}\n"
        );
        let found = check(GOOD, &source(&text)).unwrap_err();
        assert_eq!(found[0].code, "errors.unknown-pair");
        assert_eq!(found[0].line, Some(7));
        assert!(found[0].message.contains("Beta"), "{found:?}");
    }

    #[test]
    fn a_registration_that_does_not_name_a_pair_fails() {
        for text in [
            "#[test]\nfn t() { followed_remedy!(report, Code::Alpha); }\n",
            "#[test]\nfn t() { followed_remedy!(report, alpha(), RemedyKind::DoIt, |r| retry(r)); }\n",
            "#[test]\nfn t() { followed_remedy!(report, Code::Alpha, kind(), |r| retry(r)); }\n",
            "#[test]\nfn t() { followed_remedy!(fn fn fn); }\n",
        ] {
            assert_eq!(
                codes(&check(GOOD, &source(text))),
                ["errors.malformed-registration", "errors.unregistered-pair"],
                "{text}"
            );
        }
    }

    #[test]
    fn a_should_panic_test_proves_nothing() {
        let text = "#[test]\n#[should_panic]\nfn t() {\n    followed_remedy!(report, Code::Alpha, RemedyKind::DoIt, |r| retry(r));\n}\n";
        assert_eq!(codes(&check(GOOD, &source(text))), ["errors.unregistered-pair"]);
    }

    #[test]
    fn other_macros_are_ignored() {
        let text = format!("{FOLLOWED}#[test]\nfn u() {{ assert!(true); }}\n");
        assert!(check(GOOD, &source(&text)).is_ok());
    }

    fn with_match(arms: &str) -> Verdict {
        check(GOOD, &source(&format!("{FOLLOWED}fn m(c: Code) -> u8 {{ match c {{ {arms} }} }}\n")))
    }

    #[test]
    fn a_catch_all_on_a_code_fails() {
        let found = with_match("Code::Alpha => 1, _ => 2").unwrap_err();
        assert_eq!(found[0].code, "errors.non-exhaustive-match");
        assert_eq!(found[0].line, Some(5));
        assert_eq!(codes(&with_match("Code::Alpha => 1, other => 2")), ["errors.non-exhaustive-match"]);
        assert_eq!(codes(&with_match("RemedyKind::DoIt => 1, _ => 2")), ["errors.non-exhaustive-match"]);
        assert_eq!(codes(&with_match("Code::Alpha | Code::Beta => 1, _ => 2")), ["errors.non-exhaustive-match"]);
        assert_eq!(codes(&with_match("crate::Code::Alpha => 1, _ => 2")), ["errors.non-exhaustive-match"]);
        assert_eq!(codes(&with_match("x @ Code::Alpha => 1, _ => 2")), ["errors.non-exhaustive-match"]);
    }

    #[test]
    fn an_exhaustive_match_passes() {
        assert!(with_match("Code::Alpha => 1, Code::Beta => 2").is_ok());
    }

    #[test]
    fn a_catch_all_on_something_else_passes() {
        assert!(with_match("Other::Alpha => 1, _ => 2").is_ok());
        assert!(with_match("1 => 1, _ => 2").is_ok());
        assert!(with_match("Alpha => 1, x @ 2 => 2").is_ok(), "a binding with a subpattern is not a catch-all");
    }

    #[test]
    fn a_subpattern_binding_is_not_a_catch_all() {
        assert!(with_match("Code::Alpha => 1, x @ Code::Beta => 2").is_ok());
    }

    #[test]
    fn duplicate_codes_and_kinds_fail() {
        let codes_twice = r#"{"schema":1,"codes":[{"name":"A","code":"a.b","remedies":[]},{"name":"B","code":"a.b","remedies":[]}],"remedies":[]}"#;
        let found = check(codes_twice, &source("")).unwrap_err();
        assert_eq!(found[0].code, "errors.duplicate");
        assert!(found[0].message.contains("a.b"), "{found:?}");
        let kinds_twice = r#"{"schema":1,"codes":[{"name":"A","code":"a.b","remedies":[]}],"remedies":[{"name":"X","kind":"k"},{"name":"Y","kind":"k"}]}"#;
        assert_eq!(codes(&check(kinds_twice, &source(""))), ["errors.duplicate"]);
    }

    #[test]
    fn a_code_must_be_area_dot_kebab() {
        for bad in ["nodot", ".b", "a.", "A.b", "a.B_c", "a.b.c"] {
            let json = format!(r#"{{"schema":1,"codes":[{{"name":"A","code":"{bad}","remedies":[]}}],"remedies":[]}}"#);
            assert_eq!(codes(&check(&json, &source(""))), ["errors.malformed-code"], "{bad}");
        }
        let json = r#"{"schema":1,"codes":[{"name":"A","code":"a-1.b-2","remedies":[]}],"remedies":[]}"#;
        assert!(check(json, &source("")).is_ok());
    }

    #[test]
    fn a_pair_naming_an_unlisted_remedy_fails() {
        let json = r#"{"schema":1,"codes":[{"name":"A","code":"a.b","remedies":["ghost"]}],"remedies":[]}"#;
        let found = check(json, &source("")).unwrap_err();
        assert_eq!(found[0].code, "errors.unknown-remedy");
        assert!(found[0].message.contains("ghost"), "{found:?}");
    }

    #[test]
    fn an_empty_or_unreadable_catalogue_fails_closed() {
        assert_eq!(codes(&check(r#"{"schema":1,"codes":[],"remedies":[]}"#, &source(""))), ["errors.no-codes"]);
        assert_eq!(codes(&check("not json", &source(""))), ["errors.catalogue-unreadable"]);
    }

    #[test]
    fn an_unparsable_source_fails() {
        let mut sources = source(FOLLOWED);
        sources.push(("crates/b.rs".to_string(), "fn (".to_string()));
        let found = check(GOOD, &sources).unwrap_err();
        assert_eq!(found[0].code, "errors.unparsable");
        assert_eq!(found[0].file, "crates/b.rs");
    }
}
