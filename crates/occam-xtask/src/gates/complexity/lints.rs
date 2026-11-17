//! The part of the workspace manifest's rule on lints that the sources show: every `allow` and `expect` gives its
//! reason, and an `allow` stands only at a crate's root, where the rule puts a relaxation. Anywhere else it is an
//! `expect`, which stops compiling once nothing under it needs it. `cfg_attr` is read through.

use syn::punctuated::Punctuated;
use syn::visit::Visit;
use syn::{Attribute, File, Item, Meta, Token};

use crate::verdict::Finding;

/// The relaxations counted in the files measured.
#[derive(Clone, Copy, Debug, Default, PartialEq, Eq)]
pub(super) struct Count {
    /// Every `allow` and `expect`.
    pub all: usize,
    /// Those away from a crate's root: on an item, or in a module.
    pub off_root: usize,
}

/// One `allow` or `expect`.
struct Relaxation {
    line: usize,
    allow: bool,
    reason: bool,
    at_root: bool,
}

/// Whether `path` is the root of a crate: a library, a binary, a build script, or a test, example or bench target.
fn is_crate_root(path: &str) -> bool {
    let parts: Vec<&str> = path.split('/').skip(2).collect();
    matches!(
        parts.as_slice(),
        ["build.rs"] | ["src", "lib.rs" | "main.rs"] | ["src", "bin", _] | ["tests" | "examples" | "benches", _]
    )
}

/// Every `allow` and `expect` in `file`. At the root are a crate root's own inner attributes and those on the modules
/// it declares in other files.
fn relaxations(file: &File, path: &str) -> Vec<Relaxation> {
    let root = is_crate_root(path);
    let mut walk = Walk { at_root: root, found: Vec::new() };
    for attribute in &file.attrs {
        walk.visit_attribute(attribute);
    }
    for item in &file.items {
        match item {
            Item::Mod(module) if root && module.content.is_none() => {
                module.attrs.iter().for_each(|attribute| walk.visit_attribute(attribute));
            }
            _ => {
                walk.at_root = false;
                walk.visit_item(item);
                walk.at_root = root;
            }
        }
    }
    walk.found
}

/// Collects the relaxations of the attributes it is shown, each at the root or not as it is told.
struct Walk {
    at_root: bool,
    found: Vec<Relaxation>,
}

impl Walk {
    /// The `allow`s and `expect`s of one attribute's content, through any `cfg_attr`.
    fn read(&mut self, meta: &Meta, line: usize) {
        let Meta::List(list) = meta else { return };
        let (allow, expect) = (list.path.is_ident("allow"), list.path.is_ident("expect"));
        if !(allow || expect || list.path.is_ident("cfg_attr")) {
            return;
        }
        let inner = list.parse_args_with(Punctuated::<Meta, Token![,]>::parse_terminated).unwrap_or_default();
        if allow || expect {
            let reason = inner.iter().any(|meta| matches!(meta, Meta::NameValue(pair) if pair.path.is_ident("reason")));
            self.found.push(Relaxation { line, allow, reason, at_root: self.at_root });
        } else {
            inner.iter().skip(1).for_each(|meta| self.read(meta, line));
        }
    }
}

impl<'ast> Visit<'ast> for Walk {
    fn visit_attribute(&mut self, attribute: &'ast Attribute) {
        self.read(&attribute.meta, attribute.pound_token.span.start().line);
    }
}

/// Adds `file`'s relaxations to `count`, and a finding for each that breaks the rule.
pub(super) fn check(file: &File, path: &str, count: &mut Count, findings: &mut Vec<Finding>) {
    for found in relaxations(file, path) {
        count.all = count.all.saturating_add(1);
        count.off_root = count.off_root.saturating_add(usize::from(!found.at_root));
        if !found.reason {
            let remedy = "add `reason = \"...\"`, saying why the lint does not apply here";
            findings.push(
                Finding::new("complexity.relaxation-unexplained", path, "a lint is relaxed without a reason", remedy)
                    .at(found.line),
            );
        }
        if found.allow && !found.at_root {
            let message = "`allow` away from the crate's root, where it stays after what it allowed is gone";
            let remedy = "make it `expect`, which fails once nothing needs it, or relax the lint at the crate's root";
            findings.push(Finding::new("complexity.allow-off-root", path, message, remedy).at(found.line));
        }
    }
}

#[cfg(test)]
mod tests {
    use super::super::check;
    use crate::verdict::codes;

    fn at(path: &str, text: &str) -> Vec<(String, String)> {
        vec![(path.to_string(), text.to_string())]
    }

    const LIB: &str = "crates/a/src/lib.rs";

    #[test]
    fn a_relaxation_without_a_reason_fails_at_its_line() {
        for text in ["//! Doc.\n#![allow(dead_code)]\n", "\n#[expect(dead_code)]\nfn f() {}\n"] {
            let found = check(&at(LIB, text), "", "").unwrap_err();
            assert_eq!(codes(&Err(found.clone())), ["complexity.relaxation-unexplained"], "{text}");
            assert_eq!((found[0].file.as_str(), found[0].line), (LIB, Some(2)), "{text}");
        }
        let both = "#[allow(clippy::unwrap_used, dead_code)]\nfn f() {}\n";
        assert_eq!(
            codes(&check(&at(LIB, both), "", "")),
            ["complexity.relaxation-unexplained", "complexity.allow-off-root"]
        );
    }

    #[test]
    fn an_allow_away_from_the_crate_root_fails_and_an_expect_there_passes() {
        let off_root = [
            "\n\n#[allow(dead_code, reason = \"r\")]\nfn f() {}\n",
            "mod m {\n\n    #![allow(dead_code, reason = \"r\")]\n}\n",
            "fn f() {\n\n    #[allow(unused, reason = \"r\")]\n    let a = 1;\n}\n",
            "struct S;\nimpl S {\n    #[allow(dead_code, reason = \"r\")]\n    fn f() {}\n}\n",
            "\n\n#[allow(dead_code, reason = \"r\")]\nmod m {}\n",
        ];
        for text in off_root {
            let found = check(&at(LIB, text), "", "").unwrap_err();
            assert_eq!(codes(&Err(found.clone())), ["complexity.allow-off-root"], "{text}");
            assert_eq!(found[0].line, Some(3), "{text}");
            assert!(found[0].remedy.contains("expect"), "{found:?}");
            let expected = text.replace("allow", "expect");
            assert_eq!(
                check(&at(LIB, &expected), "", "")
                    .map(|summary| summary.ends_with("1 lint relaxations, 1 off a crate root")),
                Ok(true),
                "{expected}"
            );
        }
    }

    #[test]
    fn an_allow_stands_at_a_crate_root_and_on_a_module_it_declares() {
        let inner = "#![allow(dead_code, reason = \"r\")]\nfn f() {}\n";
        for root in
            ["src/lib.rs", "src/main.rs", "build.rs", "src/bin/b.rs", "tests/t.rs", "examples/e.rs", "benches/b.rs"]
        {
            let summary = check(&at(&format!("crates/a/{root}"), inner), "", "").unwrap();
            assert!(summary.ends_with("1 lint relaxations, 0 off a crate root"), "{root}: {summary}");
        }
        for other in ["src/m.rs", "src/m/lib.rs", "tests/common/mod.rs", "src/bin/b/main.rs"] {
            assert_eq!(
                codes(&check(&at(&format!("crates/a/{other}"), inner), "", "")),
                ["complexity.allow-off-root"],
                "{other}"
            );
        }
        let declared = "fn f() {}\n#[cfg(feature = \"f\")]\n#[allow(dead_code, reason = \"r\")]\nmod m;\n";
        assert!(check(&at(LIB, declared), "", "").is_ok());
        assert_eq!(codes(&check(&at("crates/a/src/m.rs", declared), "", "")), ["complexity.allow-off-root"]);
    }

    #[test]
    fn cfg_attr_is_read_through() {
        let unexplained = "#![cfg_attr(not(test), allow(dead_code))]\n";
        assert_eq!(codes(&check(&at(LIB, unexplained), "", "")), ["complexity.relaxation-unexplained"]);
        let off_root = "#[cfg_attr(test, cfg_attr(unix, allow(dead_code, reason = \"r\")))]\nfn f() {}\n";
        assert_eq!(codes(&check(&at(LIB, off_root), "", "")), ["complexity.allow-off-root"]);
        let fine = "mod m {\n    #![cfg_attr(not(test), expect(dead_code, reason = \"r\"))]\n}\n";
        assert!(check(&at(LIB, fine), "", "").unwrap().ends_with("1 lint relaxations, 1 off a crate root"));
    }

    #[test]
    fn raising_a_lint_and_other_attributes_are_not_relaxations() {
        let text = "#![deny(dead_code)]\n#[warn(unused)]\n#[derive(Clone)]\n#[cfg(test)]\n#[must_use]\nstruct S;\n";
        assert!(check(&at(LIB, text), "", "").unwrap().ends_with("0 lint relaxations, 0 off a crate root"));
    }
}
