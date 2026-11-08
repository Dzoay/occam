//! `gates complexity`: function length, file length and duplication, with a shrink-only exemption list (L20). The
//! function walk is adapted from phasekit's gate of the same name.
//!
//! An entry in the list holds only while its measure is exactly its ceiling, and it may be neither added nor raised
//! against the merge base, so the list can only shrink. Clippy's `too_many_lines` is the hard bar above this one.

use std::collections::BTreeMap;
use std::path::Path;

use syn::visit::{self, Visit};
use syn::{File, Ident, ImplItemFn, ItemFn, ItemImpl, ItemMod, ItemTrait, TraitItemFn, Type};

use super::no_args;
use crate::repo::Repo;
use crate::verdict::{Finding, Verdict, verdict};

/// A function over this many lines of code fails.
const FUNCTION: usize = 60;
/// A file with more lines of non-test code than this fails.
const FILE: usize = 500;
/// This many consecutive lines of non-test code, found twice, are duplication.
const WINDOW: usize = 10;

const EXEMPTIONS: &str = "ci/complexity-exemptions.txt";

fn measured(path: &str) -> bool {
    path.starts_with("crates/")
        && Path::new(path).extension().is_some_and(|extension| extension == "rs")
        && !path.contains("/probes/")
}

pub fn run(repo: &Repo, args: &[String]) -> Verdict {
    no_args(args)?;
    let files = repo.texts(measured).map_err(|e| vec![e])?;
    let exemptions = repo.read(EXEMPTIONS).map_err(|e| vec![e])?;
    let base = repo.base().map_err(|e| vec![e])?;
    check(&files, &exemptions, &repo.at(&base, EXEMPTIONS).unwrap_or_default())
}

/// Something over a bar, by the key an exemption names it with.
struct Over {
    key: String,
    file: String,
    line: usize,
    value: usize,
    what: String,
}

fn is_code(line: &str) -> bool {
    let line = line.trim();
    !line.is_empty() && !line.starts_with("//")
}

/// The numbered, trimmed code lines of `text` before its test module.
fn code_lines(text: &str) -> Vec<(usize, &str)> {
    let lines: Vec<&str> = text.lines().collect();
    let end = lines.windows(2).position(|pair| pair == ["#[cfg(test)]", "mod tests {"]).unwrap_or(lines.len());
    let numbered = lines.into_iter().take(end).zip(1..);
    numbered.filter(|(line, _)| is_code(line)).map(|(line, n)| (n, line.trim())).collect()
}

/// One measured function.
struct Function {
    name: String,
    line: usize,
    loc: usize,
}

/// The functions of one file: modules and the types of `impl` and `trait` blocks name its scope.
struct Collector<'a> {
    lines: &'a [&'a str],
    scope: Vec<String>,
    found: Vec<Function>,
}

impl Collector<'_> {
    fn record(&mut self, name: &Ident, first: usize, last: usize) {
        let name = name.to_string();
        let path: Vec<&str> = self.scope.iter().map(String::as_str).chain([name.as_str()]).collect();
        let body = self.lines.iter().take(last).skip(first.saturating_sub(1));
        self.found.push(Function {
            name: path.join("::"),
            line: first,
            loc: body.filter(|line| is_code(line)).count(),
        });
    }

    fn within(&mut self, name: String, f: impl FnOnce(&mut Self)) {
        self.scope.push(name);
        f(self);
        self.scope.pop();
    }
}

impl<'ast> Visit<'ast> for Collector<'_> {
    fn visit_item_mod(&mut self, item: &'ast ItemMod) {
        self.within(item.ident.to_string(), |c| visit::visit_item_mod(c, item));
    }

    fn visit_item_impl(&mut self, item: &'ast ItemImpl) {
        let name = match &*item.self_ty {
            Type::Path(path) => path.path.segments.last().map_or_else(|| "_".into(), |s| s.ident.to_string()),
            _ => "_".into(),
        };
        self.within(name, |c| visit::visit_item_impl(c, item));
    }

    fn visit_item_trait(&mut self, item: &'ast ItemTrait) {
        self.within(item.ident.to_string(), |c| visit::visit_item_trait(c, item));
    }

    fn visit_item_fn(&mut self, item: &'ast ItemFn) {
        let first = item.sig.fn_token.span.start().line;
        self.record(&item.sig.ident, first, item.block.brace_token.span.close().end().line);
    }

    fn visit_impl_item_fn(&mut self, item: &'ast ImplItemFn) {
        let first = item.sig.fn_token.span.start().line;
        self.record(&item.sig.ident, first, item.block.brace_token.span.close().end().line);
    }

    fn visit_trait_item_fn(&mut self, item: &'ast TraitItemFn) {
        if let Some(block) = &item.default {
            let first = item.sig.fn_token.span.start().line;
            self.record(&item.sig.ident, first, block.brace_token.span.close().end().line);
        }
    }
}

fn functions(file: &File, text: &str) -> Vec<Function> {
    let lines: Vec<&str> = text.lines().collect();
    let mut collector = Collector { lines: &lines, scope: Vec::new(), found: Vec::new() };
    collector.visit_file(file);
    collector.found
}

/// Every window of [`WINDOW`] lines seen before, grouped by the pair of files sharing it.
fn duplicates(files: &[(&str, Vec<(usize, &str)>)]) -> Vec<Over> {
    let mut seen: BTreeMap<String, (&str, usize)> = BTreeMap::new();
    let mut pairs: BTreeMap<String, Over> = BTreeMap::new();
    for (file, lines) in files {
        for (index, window) in lines.windows(WINDOW).enumerate() {
            let key = window.iter().map(|(_, line)| *line).collect::<Vec<_>>().join("\n");
            match seen.get(&key) {
                Some(&(first, at)) if first != *file || index.abs_diff(at) >= WINDOW => {
                    let line = window.first().map_or(1, |(n, _)| *n);
                    let key = format!("dup {first} {file}");
                    let entry = pairs.entry(key.clone()).or_insert_with(|| Over {
                        key,
                        file: (*file).to_string(),
                        line,
                        value: 0,
                        what: format!("shares at least {WINDOW} lines with {first}"),
                    });
                    entry.value = entry.value.saturating_add(1);
                }
                Some(_) => {}
                None => {
                    seen.insert(key, (file, index));
                }
            }
        }
    }
    pairs.into_values().collect()
}

/// One exemption line as its key and ceiling, or `None` if malformed or without a reason.
fn exemption(line: &str) -> Option<(String, usize)> {
    let mut words = line.split_whitespace();
    let kind = words.next()?;
    let (names, measure) = match kind {
        "fn" => (2, "lines="),
        "file" => (1, "lines="),
        "dup" => (2, "windows="),
        _ => return None,
    };
    let mut key = vec![kind];
    for _ in 0..names {
        key.push(words.next()?);
    }
    let ceiling = words.next()?.strip_prefix(measure)?.parse().ok()?;
    words.next()?;
    Some((key.join(" "), ceiling))
}

/// The entries of a list by key; a malformed line becomes a finding.
fn list(text: &str, findings: &mut Vec<Finding>) -> BTreeMap<String, usize> {
    let mut entries = BTreeMap::new();
    for (line, n) in text.lines().zip(1..).filter(|(line, _)| !line.trim().is_empty() && !line.starts_with('#')) {
        if let Some((key, ceiling)) = exemption(line) {
            entries.insert(key, ceiling);
        } else {
            let message = "not `fn <file> <name> lines=<n> <reason>`, `file <file> lines=<n> <reason>` or `dup <a> <b> windows=<n> <reason>`";
            findings
                .push(Finding::new("complexity.exemption-malformed", EXEMPTIONS, message, "correct the line").at(n));
        }
    }
    entries
}

/// Measures every file; what is over a bar is returned with the number of functions measured.
fn measure(files: &[(String, String)], findings: &mut Vec<Finding>) -> (Vec<Over>, usize) {
    let (mut over, mut measured, mut code) = (Vec::new(), 0_usize, Vec::new());
    for (path, text) in files {
        match syn::parse_file(text) {
            Ok(file) => {
                let found = functions(&file, text);
                measured = measured.saturating_add(found.len());
                let long = found.into_iter().filter(|function| function.loc > FUNCTION);
                over.extend(long.map(|function| Over {
                    key: format!("fn {path} {}", function.name),
                    file: path.clone(),
                    line: function.line,
                    value: function.loc,
                    what: format!("{} has {} lines of code, over {FUNCTION}", function.name, function.loc),
                }));
            }
            Err(error) => {
                findings.push(Finding::new("complexity.unparsable", path, error.to_string(), "fix the syntax"));
            }
        }
        let lines = code_lines(text);
        if lines.len() > FILE {
            let what = format!("{} lines of non-test code, over {FILE}", lines.len());
            over.push(Over { key: format!("file {path}"), file: path.clone(), line: 1, value: lines.len(), what });
        }
        code.push((path.as_str(), lines));
    }
    over.extend(duplicates(&code));
    (over, measured)
}

fn check(files: &[(String, String)], exemptions: &str, base: &str) -> Verdict {
    let mut findings = Vec::new();
    let (over, measured) = measure(files, &mut findings);
    let exempt = list(exemptions, &mut findings);
    let base = list(base, &mut Vec::new());
    for item in over.iter().filter(|item| !exempt.contains_key(&item.key)) {
        let remedy = "split it up; a new exemption is not available, because the list only shrinks (L20)";
        findings.push(Finding::new("complexity.over-bar", &item.file, item.what.clone(), remedy).at(item.line));
    }
    let values: BTreeMap<&str, usize> = over.iter().map(|item| (item.key.as_str(), item.value)).collect();
    for (key, ceiling) in &exempt {
        if values.get(key.as_str()) != Some(ceiling) {
            let message = format!("`{key}` no longer measures {ceiling}");
            findings.push(Finding::new(
                "complexity.exemption-stale",
                EXEMPTIONS,
                message,
                "lower its ceiling to the measure, or remove it",
            ));
        }
        if base.get(key).is_none_or(|old| ceiling > old) {
            let message = format!("`{key}` was added or raised");
            findings.push(Finding::new(
                "complexity.exemption-grown",
                EXEMPTIONS,
                message,
                "the list only shrinks: bring the code under the bar",
            ));
        }
    }
    verdict(format!("{measured} functions in {} files, {} exempted", files.len(), exempt.len()), findings)
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::verdict::codes;

    const PATH: &str = "crates/a/src/lib.rs";

    fn long_fn(name: &str, loc: usize) -> String {
        let body = (0..loc.saturating_sub(2)).map(|i| format!("    let _ = {i};\n")).collect::<Vec<_>>().concat();
        format!("fn {name}() {{\n{body}}}\n")
    }

    fn file(text: String) -> Vec<(String, String)> {
        vec![(PATH.to_string(), text)]
    }

    #[test]
    fn a_function_at_the_bar_passes_and_one_over_fails() {
        assert_eq!(check(&file(long_fn("f", 60)), "", ""), Ok("1 functions in 1 files, 0 exempted".to_string()));
        let found = check(&file(long_fn("f", 61)), "", "").unwrap_err();
        assert_eq!(found[0].code, "complexity.over-bar");
        assert_eq!(found[0].file, PATH);
        assert_eq!(found[0].line, Some(1));
        assert!(found[0].message.contains("61") && found[0].message.contains('f'), "{found:?}");
    }

    #[test]
    fn comments_and_blank_lines_do_not_count() {
        let body = format!("{}\n    // a comment\n\n", long_fn("f", 60).trim_end_matches("}\n"));
        assert!(check(&file(format!("{body}}}\n")), "", "").is_ok());
    }

    #[test]
    fn functions_are_named_by_module_and_type() {
        let text =
            format!("mod m {{ struct S; impl S {{ {} }} trait T {{ {} }} }}\n", long_fn("a", 61), long_fn("b", 61));
        let found = check(&file(text), "", "").unwrap_err();
        let messages: Vec<&str> = found.iter().map(|f| f.message.as_str()).collect();
        assert!(messages.iter().any(|m| m.contains("m::S::a")), "{messages:?}");
        assert!(messages.iter().any(|m| m.contains("m::T::b")), "{messages:?}");
    }

    #[test]
    fn a_nested_function_is_inside_its_outer_function() {
        let text = format!("fn outer() {{ {} }}\n", long_fn("inner", 40));
        assert_eq!(check(&file(text), "", ""), Ok("1 functions in 1 files, 0 exempted".to_string()));
    }

    #[test]
    fn a_trait_method_without_a_body_is_not_measured() {
        assert_eq!(
            check(&file("trait T { fn f(); }\n".into()), "", ""),
            Ok("0 functions in 1 files, 0 exempted".to_string())
        );
    }

    #[test]
    fn an_impl_of_an_unnamed_type_still_measures() {
        let text = format!("impl T for [u8; 1] {{ {} }}\n", long_fn("f", 61));
        assert!(check(&file(text), "", "").unwrap_err()[0].message.contains("_::f"));
    }

    fn lines(n: usize) -> String {
        (0..n).map(|i| format!("const C{i}: u8 = 0;\n")).collect::<Vec<_>>().concat()
    }

    #[test]
    fn a_file_at_the_bar_passes_and_one_over_fails() {
        assert!(check(&file(lines(500)), "", "").is_ok());
        let found = check(&file(lines(501)), "", "").unwrap_err();
        assert_eq!(found[0].code, "complexity.over-bar");
        assert!(found[0].message.contains("501"), "{found:?}");
    }

    #[test]
    fn test_code_is_outside_the_file_bar() {
        let text = format!("{}#[cfg(test)]\nmod tests {{\n{}}}\n", lines(500), lines(10).replace("const C", "const T"));
        assert!(check(&file(text), "", "").is_ok());
    }

    fn pair(shared: &str) -> Vec<(String, String)> {
        vec![
            ("crates/a.rs".to_string(), format!("fn a() {{\n{shared}}}\n")),
            ("crates/b.rs".to_string(), format!("fn b() {{\n{shared}}}\n")),
        ]
    }

    fn shared(n: usize) -> String {
        (0..n).map(|i| format!("    let _ = {i};\n")).collect::<Vec<_>>().concat()
    }

    #[test]
    fn nine_shared_lines_pass_and_ten_fail() {
        assert!(check(&pair(&shared(8)), "", "").is_ok(), "eight shared lines and the closing brace make nine");
        let found = check(&pair(&shared(9)), "", "").unwrap_err();
        assert_eq!(found[0].code, "complexity.over-bar");
        assert_eq!(found[0].file, "crates/b.rs");
        assert_eq!(found[0].line, Some(2));
        assert!(found[0].message.contains("crates/a.rs"), "{found:?}");
    }

    #[test]
    fn repetition_inside_one_file_counts_only_when_it_does_not_overlap() {
        let period =
            |p: usize| -> String { (0..20).map(|i| format!("    let _ = {};\n", i % p)).collect::<Vec<_>>().concat() };
        assert!(check(&file(format!("fn f() {{\n{}}}\n", period(9))), "", "").is_ok());
        assert_eq!(codes(&check(&file(format!("fn f() {{\n{}}}\n", period(10))), "", "")), ["complexity.over-bar"]);
    }

    #[test]
    fn test_code_is_outside_duplication() {
        let text = format!("#[cfg(test)]\nmod tests {{\nfn a() {{\n{0}}}\nfn b() {{\n{0}}}\n}}\n", shared(12));
        assert!(check(&file(text), "", "").is_ok());
    }

    #[test]
    fn a_kept_exemption_passes_and_counts() {
        let list = format!("fn {PATH} f lines=61 waiting for the split\n");
        assert_eq!(check(&file(long_fn("f", 61)), &list, &list), Ok("1 functions in 1 files, 1 exempted".to_string()));
    }

    #[test]
    fn a_file_and_a_duplicate_can_be_exempted() {
        let list = format!("file {PATH} lines=501 why\n");
        assert!(check(&file(lines(501)), &list, &list).is_ok());
        let list = "# comment\n\ndup crates/a.rs crates/b.rs windows=2 why\n";
        assert!(check(&pair(&shared(10)), list, list).is_ok());
    }

    #[test]
    fn an_exemption_is_exact_so_it_must_shrink_with_the_code() {
        let list = format!("fn {PATH} f lines=62 why\n");
        assert_eq!(codes(&check(&file(long_fn("f", 61)), &list, &list)), ["complexity.exemption-stale"]);
    }

    #[test]
    fn an_exemption_for_code_under_the_bar_is_stale() {
        let list = format!("fn {PATH} f lines=61 why\n");
        let found = check(&file(long_fn("f", 60)), &list, &list).unwrap_err();
        assert_eq!(found[0].code, "complexity.exemption-stale");
        assert_eq!(found[0].file, EXEMPTIONS);
        assert!(found[0].message.contains(PATH), "{found:?}");
    }

    #[test]
    fn the_list_cannot_gain_an_entry() {
        let list = format!("fn {PATH} f lines=61 why\n");
        assert_eq!(codes(&check(&file(long_fn("f", 61)), &list, "")), ["complexity.exemption-grown"]);
    }

    #[test]
    fn the_list_cannot_raise_a_ceiling() {
        let head = format!("fn {PATH} f lines=62 why\n");
        let base = format!("fn {PATH} f lines=61 why\n");
        assert_eq!(codes(&check(&file(long_fn("f", 62)), &head, &base)), ["complexity.exemption-grown"]);
        assert!(check(&file(long_fn("f", 62)), &head, &head).is_ok(), "an unchanged ceiling is not growth");
        let higher = format!("fn {PATH} f lines=63 why\n");
        assert!(check(&file(long_fn("f", 62)), &head, &higher).is_ok(), "a lowered ceiling is shrinkage");
    }

    #[test]
    fn a_malformed_exemption_fails() {
        for bad in [
            "fn a.rs f lines=61",
            "fn a.rs f windows=61 why",
            "fn a.rs f lines=x why",
            "file a.rs windows=1 why",
            "dup a.rs b.rs lines=1 why",
            "other a.rs lines=1 why",
            "fn a.rs",
        ] {
            assert_eq!(codes(&check(&file(String::new()), bad, bad)), ["complexity.exemption-malformed"], "{bad}");
        }
    }

    #[test]
    fn an_unparsable_file_fails() {
        assert_eq!(codes(&check(&file("fn (".into()), "", "")), ["complexity.unparsable"]);
    }

    #[test]
    fn only_crate_sources_are_measured() {
        assert!(measured("crates/a/src/lib.rs"));
        assert!(!measured("crates/occam-xtask/probes/bans/src/lib.rs"));
        assert!(!measured("crates/a/Cargo.toml"));
        assert!(!measured("tools/a.rs"));
    }
}
