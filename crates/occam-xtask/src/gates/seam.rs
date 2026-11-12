//! `gates seam`: occam-geom's seam is frozen (L21; plan step `geometry.freeze`). `ci/seam.txt` records the signature of
//! every public item of the seam's modules: the family and object traits of surfaces, curves and parameter curves, and
//! every type their signatures name. Any difference from the source fails, naming each item changed, added or removed.
//! The record changes only with a decision: `--update <decision>` re-records the seam under a decision in
//! `docs/decisions/` newer than the last it names, and against the merge base a record that changed must name a newer
//! decision than the base's, so a hand edit is caught too.

mod signatures;

use std::collections::BTreeMap;

use crate::repo::Repo;
use crate::verdict::{Finding, Verdict, verdict};

const RECORD: &str = "ci/seam.txt";
const DECISIONS: &str = "docs/decisions/";

/// The seam's modules when the record is first made; afterwards the record's own list is what counts. The two layers of
/// surfaces, curves and parameter curves, and the modules of every type their signatures name.
const MODULES: [&str; 8] = [
    "crates/occam-geom/src/analytic.rs",
    "crates/occam-geom/src/batch.rs",
    "crates/occam-geom/src/bspline.rs",
    "crates/occam-geom/src/curve.rs",
    "crates/occam-geom/src/domain.rs",
    "crates/occam-geom/src/identity.rs",
    "crates/occam-geom/src/param_curve.rs",
    "crates/occam-geom/src/surface.rs",
];

const REMEDY: &str = "the seam is frozen (L21): write a decision in docs/decisions/ that allows the change, then run \
                      `just seam --update docs/decisions/<that decision>.md`";

const HEADER: &str =
    "# `gates seam`: occam-geom's frozen seam (L21). Each `item` line is a public item of a seam module, \
its signature with
# comments and whitespace normalised away; `gates seam` fails on any difference from the source. `just seam --update
# docs/decisions/<decision>.md` re-records it, only under a decision newer than the last named here.
";

/// The record: the decisions it was made under, oldest first; the seam's modules; and each item's signature, by module
/// and key.
#[derive(Debug, Default, PartialEq, Eq)]
struct Record {
    decisions: Vec<String>,
    modules: Vec<String>,
    items: BTreeMap<(String, String), String>,
}

impl Record {
    /// The seam itself, without the decisions: what a decision must allow to change.
    fn seam(&self) -> (&[String], &BTreeMap<(String, String), String>) {
        (&self.modules, &self.items)
    }
}

/// The record in `text`.
///
/// # Errors
///
/// A finding for each line that is neither a comment nor a `decision`, `module` or `item` line.
fn parse(text: &str) -> Result<Record, Vec<Finding>> {
    let (mut record, mut findings) = (Record::default(), Vec::new());
    for (line, n) in text.lines().zip(1..).filter(|(line, _)| !line.trim().is_empty() && !line.starts_with('#')) {
        let fields: Vec<&str> = line.split('\t').collect();
        match fields.as_slice() {
            ["decision", path] => record.decisions.push((*path).to_string()),
            ["module", path] => record.modules.push((*path).to_string()),
            ["item", module, key, signature] => {
                record.items.insert(((*module).to_string(), (*key).to_string()), (*signature).to_string());
            }
            _ => findings.push(
                Finding::new(
                    "seam.malformed",
                    RECORD,
                    format!("cannot read `{line}`"),
                    "write a decision, module or item line",
                )
                .at(n),
            ),
        }
    }
    if findings.is_empty() { Ok(record) } else { Err(findings) }
}

/// The record as text: its header, then its decisions, modules and items, one a line, fields apart by tabs.
fn render(record: &Record) -> String {
    let mut out = String::from(HEADER);
    out.extend(record.decisions.iter().map(|d| format!("decision\t{d}\n")));
    out.extend(record.modules.iter().map(|m| format!("module\t{m}\n")));
    out.extend(record.items.iter().map(|((module, key), signature)| format!("item\t{module}\t{key}\t{signature}\n")));
    out
}

/// What differs between the record's items for `module` and the source's.
fn compare(module: &str, recorded: &BTreeMap<String, String>, found: &BTreeMap<String, String>) -> Vec<Finding> {
    let mut findings = Vec::new();
    for (key, was) in recorded {
        match found.get(key) {
            Some(now) if now == was => {}
            Some(now) => findings.push(Finding::new(
                "seam.changed",
                module,
                format!("`{key}` was `{was}` and is `{now}`"),
                REMEDY,
            )),
            None => findings.push(Finding::new("seam.removed", module, format!("`{key}` (`{was}`) is gone"), REMEDY)),
        }
    }
    for (key, now) in found.iter().filter(|(key, _)| !recorded.contains_key(*key)) {
        findings.push(Finding::new("seam.added", module, format!("`{key}` (`{now}`) is new"), REMEDY));
    }
    findings
}

/// A file name in `docs/decisions/`, which starts with its date, so names sort in order of decision.
fn name(path: &str) -> &str {
    path.rsplit('/').next().unwrap_or(path)
}

/// Whether a record that differs from the merge base's names a decision newer than any the base's does: `None` if it
/// does, or if nothing of the seam changed, or if the base has no record.
fn undecided(base: Option<&Record>, now: &Record) -> Option<Finding> {
    let base = base?;
    if base.seam() == now.seam() {
        return None;
    }
    let extends = now.decisions.starts_with(&base.decisions) && now.decisions.len() > base.decisions.len();
    let newer = base
        .decisions
        .last()
        .is_none_or(|last| now.decisions.iter().skip(base.decisions.len()).all(|d| name(d) > name(last)));
    (!(extends && newer)).then(|| {
        Finding::new(
            "seam.undecided",
            RECORD,
            "the record differs from the merge base's without a newer decision",
            REMEDY,
        )
    })
}

/// The decisions a record re-made under `decision` names: those it named, and `decision` after them, which must be in
/// `docs/decisions/` and newer than the last. Before the freeze has landed, the last may be named again, so that the
/// record can be made afresh while the step that freezes the seam is still being rebased.
///
/// # Errors
///
/// A decision outside `docs/decisions/`, already named, or not newer than the last.
fn next_decisions(listed: &[String], decision: &str, landed: bool) -> Result<Vec<String>, Finding> {
    let refuse =
        |why: String| Err(Finding::new("seam.decision", RECORD, why, "name a new decision file in docs/decisions/"));
    let markdown = std::path::Path::new(decision).extension().is_some_and(|e| e.eq_ignore_ascii_case("md"));
    if !(decision.starts_with(DECISIONS) && markdown) {
        return refuse(format!("{decision} is not a decision in {DECISIONS}"));
    }
    match listed.last() {
        Some(last) if last == decision && !landed => Ok(listed.to_vec()),
        _ if listed.iter().any(|d| d == decision) => refuse(format!("{decision} already allowed the seam as it was")),
        Some(last) if name(decision) <= name(last) => refuse(format!("{decision} is not newer than {last}")),
        _ => Ok(listed.iter().cloned().chain(std::iter::once(decision.to_string())).collect()),
    }
}

/// The items of every module, by module and key, and a finding for each module that is missing or does not parse.
fn found(repo: &Repo, modules: &[String]) -> (BTreeMap<(String, String), String>, Vec<Finding>) {
    let (mut items, mut findings) = (BTreeMap::new(), Vec::new());
    for module in modules {
        match repo.read(module).map(|text| signatures::signatures(&text)) {
            Ok(Ok(found)) => items.extend(found.into_iter().map(|(key, sig)| ((module.clone(), key), sig))),
            Ok(Err(error)) => findings.push(Finding::new("seam.unparsable", module, error, "fix the syntax")),
            Err(missing) => findings.push(missing),
        }
    }
    (items, findings)
}

/// One module's items, by key.
fn of(items: &BTreeMap<(String, String), String>, module: &str) -> BTreeMap<String, String> {
    items.iter().filter(|((m, _), _)| m == module).map(|((_, key), sig)| (key.clone(), sig.clone())).collect()
}

/// The record's findings against the source and the merge base's record.
fn check(repo: &Repo, record: &Record, base: Option<&Record>) -> Verdict {
    let (items, mut findings) = found(repo, &record.modules);
    if record.decisions.is_empty() {
        findings.push(Finding::new("seam.decision", RECORD, "the record names no decision", REMEDY));
    }
    for decision in &record.decisions {
        findings.extend(repo.read(decision).err().map(|_| {
            Finding::new("seam.decision", RECORD, format!("{decision} does not exist"), "restore it or name another")
        }));
    }
    for ((module, key), _) in record.items.iter().filter(|((module, _), _)| !record.modules.contains(module)) {
        findings.push(Finding::new(
            "seam.malformed",
            RECORD,
            format!("`{key}` is in {module}, not a seam module"),
            REMEDY,
        ));
    }
    for module in &record.modules {
        findings.extend(compare(module, &of(&record.items, module), &of(&items, module)));
    }
    findings.extend(undecided(base, record));
    let last = record.decisions.last().map_or("no decision", |d| name(d));
    verdict(
        format!("{} items in {} modules as recorded, frozen under {last}", items.len(), record.modules.len()),
        findings,
    )
}

/// The record re-made from the source under `decision`.
fn update(repo: &Repo, decision: &str, base: Option<&Record>) -> Verdict {
    repo.read(decision).map_err(|missing| vec![missing])?;
    let current = match repo.read(RECORD) {
        Ok(text) => parse(&text)?,
        Err(_) => Record { modules: MODULES.iter().map(ToString::to_string).collect(), ..Record::default() },
    };
    let decisions = next_decisions(&current.decisions, decision, base.is_some()).map_err(|refused| vec![refused])?;
    let (items, findings) = found(repo, &current.modules);
    if !findings.is_empty() {
        return Err(findings);
    }
    let record = Record { decisions, modules: current.modules, items };
    repo.write(RECORD, &render(&record)).map_err(|e| vec![e])?;
    Ok(format!(
        "{RECORD} recorded under {}: {} items in {} modules",
        name(decision),
        record.items.len(),
        record.modules.len()
    ))
}

pub fn run(repo: &Repo, args: &[String]) -> Verdict {
    let base_record = repo.base().ok().and_then(|base| repo.at(&base, RECORD)).map(|text| parse(&text)).transpose()?;
    match args {
        [] => {
            let text = repo.read(RECORD).map_err(|_| {
                vec![Finding::new(
                    "seam.no-record",
                    RECORD,
                    "the seam has no record",
                    "run `just seam --update <the decision that freezes it>`",
                )]
            })?;
            check(repo, &parse(&text)?, base_record.as_ref())
        }
        [flag, decision] if flag == "--update" => update(repo, decision, base_record.as_ref()),
        _ => Err(vec![Finding::new(
            "seam.usage",
            "",
            format!("unexpected arguments: {}", args.join(" ")),
            "pass nothing, or --update docs/decisions/<decision>.md",
        )]),
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    fn record(decisions: &[&str], items: &[(&str, &str, &str)]) -> Record {
        Record {
            decisions: decisions.iter().map(ToString::to_string).collect(),
            modules: vec!["m.rs".to_string()],
            items: items.iter().map(|(m, k, s)| (((*m).to_string(), (*k).to_string()), (*s).to_string())).collect(),
        }
    }

    fn keyed(items: &[(&str, &str)]) -> BTreeMap<String, String> {
        items.iter().map(|(k, s)| ((*k).to_string(), (*s).to_string())).collect()
    }

    const FREEZE: &str = "docs/decisions/2026-11-08-the-plan-and-its-first-answers.md";

    #[test]
    fn a_record_renders_and_parses_back() {
        let r = record(&[FREEZE], &[("m.rs", "trait T", "trait T:Send"), ("m.rs", "T::f", "fn f(&self)->u8")]);
        assert_eq!(parse(&render(&r)), Ok(r));
        let bad = parse("decision\ta\nnonsense here\n").unwrap_err();
        assert_eq!((bad[0].code, bad[0].line), ("seam.malformed", Some(2)));
    }

    #[test]
    fn a_changed_added_or_removed_signature_fails_by_name() {
        let recorded =
            keyed(&[("T::f", "fn f(&self)->u8"), ("T::g", "fn g(&self)"), ("struct S", "struct S{pub x:u8}")]);
        let found = keyed(&[("T::f", "fn f(&self)->u16"), ("struct S", "struct S{pub x:u8}"), ("T::h", "fn h(&self)")]);
        let findings = compare("m.rs", &recorded, &found);
        let codes: Vec<(&str, bool)> = findings.iter().map(|f| (f.code, f.remedy == REMEDY)).collect();
        assert_eq!(codes, [("seam.changed", true), ("seam.removed", true), ("seam.added", true)]);
        assert!(findings[0].message.contains("fn f(&self)->u8") && findings[0].message.contains("fn f(&self)->u16"));
        assert_eq!(compare("m.rs", &recorded, &recorded), Vec::<Finding>::new());
    }

    #[test]
    fn a_decision_must_be_new_and_newer() {
        let later = "docs/decisions/2026-12-01-a-change.md";
        assert_eq!(next_decisions(&[], FREEZE, false), Ok(vec![FREEZE.to_string()]));
        assert_eq!(next_decisions(&[FREEZE.to_string()], later, true), Ok(vec![FREEZE.to_string(), later.to_string()]));
        assert_eq!(
            next_decisions(&[FREEZE.to_string()], FREEZE, false),
            Ok(vec![FREEZE.to_string()]),
            "re-made before it lands"
        );
        let refused = |listed: &[&str], decision: &str| {
            let listed: Vec<String> = listed.iter().map(ToString::to_string).collect();
            next_decisions(&listed, decision, true).unwrap_err().code
        };
        assert_eq!(refused(&[FREEZE], FREEZE), "seam.decision", "named once it has landed");
        assert_eq!(refused(&[later], FREEZE), "seam.decision", "older");
        assert_eq!(refused(&[FREEZE, later], FREEZE), "seam.decision", "named before");
        let again = next_decisions(&[FREEZE.to_string(), later.to_string()], FREEZE, true).unwrap_err();
        assert!(again.message.contains("already allowed"), "named before, not merely older: {}", again.message);
        assert_eq!(refused(&[], "notes/2026-12-01-x.md"), "seam.decision", "not a decision");
    }

    #[test]
    fn a_seam_changed_against_the_base_needs_a_newer_decision() {
        let base = record(&[FREEZE], &[("m.rs", "T::f", "fn f(&self)->u8")]);
        let edited = record(&[FREEZE], &[("m.rs", "T::f", "fn f(&self)->u16")]);
        assert_eq!(undecided(Some(&base), &edited).map(|f| f.code), Some("seam.undecided"), "a hand edit");
        let decided =
            record(&[FREEZE, "docs/decisions/2026-12-01-a-change.md"], &[("m.rs", "T::f", "fn f(&self)->u16")]);
        assert_eq!(undecided(Some(&base), &decided), None);
        let backdated = record(&[FREEZE, "docs/decisions/2026-01-01-old.md"], &[("m.rs", "T::f", "fn f(&self)->u16")]);
        assert!(undecided(Some(&base), &backdated).is_some(), "a decision older than the base's last");
        let repeated = record(&[FREEZE, FREEZE], &[("m.rs", "T::f", "fn f(&self)->u16")]);
        assert!(undecided(Some(&base), &repeated).is_some(), "the base's last decision named again is no newer");
        assert_eq!(undecided(Some(&base), &base), None, "unchanged");
        assert_eq!(undecided(None, &edited), None, "before the freeze lands");
    }

    #[test]
    fn a_module_s_items_are_its_own_by_key() {
        let r = record(&[FREEZE], &[("m.rs", "T::f", "fn f()"), ("n.rs", "T::g", "fn g()"), ("m.rs", "S", "struct S")]);
        assert_eq!(of(&r.items, "m.rs"), keyed(&[("S", "struct S"), ("T::f", "fn f()")]));
        assert_eq!(of(&r.items, "o.rs"), keyed(&[]));
    }

    /// A repository in a fresh scratch directory holding `files`, by path and text.
    fn scratch(name: &str, files: &[(&str, &str)]) -> (Repo, std::path::PathBuf) {
        let root = std::env::temp_dir().join(format!("occam-seam-{name}-{}", std::process::id()));
        for (path, text) in files {
            let path = root.join(path);
            std::fs::create_dir_all(path.parent().unwrap()).unwrap();
            std::fs::write(path, text).unwrap();
        }
        (Repo::rooted(root.clone()), root)
    }

    /// The record of `modules` as they read in `repo`, under `FREEZE`.
    fn recorded(repo: &Repo, modules: &[&str]) -> Record {
        let modules: Vec<String> = modules.iter().map(ToString::to_string).collect();
        let (items, findings) = found(repo, &modules);
        assert!(findings.is_empty(), "{findings:?}");
        Record { decisions: vec![FREEZE.to_string()], modules, items }
    }

    #[test]
    fn the_items_found_are_each_module_s_public_signatures_and_a_missing_or_broken_module_is_named() {
        let files = [("m.rs", "pub trait T { fn f(&self) -> u8; }"), ("n.rs", "pub fn broken(")];
        let (repo, root) = scratch("found", &files);
        let modules: Vec<String> = ["m.rs", "n.rs", "o.rs"].iter().map(ToString::to_string).collect();
        let (items, findings) = found(&repo, &modules);
        let want: BTreeMap<(String, String), String> = [("trait T", "trait T"), ("T::f", "fn f(&self)->u8")]
            .iter()
            .map(|(k, sig)| (("m.rs".to_string(), (*k).to_string()), (*sig).to_string()))
            .collect();
        assert_eq!(items, want);
        let named: Vec<(&str, &str)> = findings.iter().map(|f| (f.code, f.file.as_str())).collect();
        assert_eq!(named, [("seam.unparsable", "n.rs"), ("repo.io", "o.rs")]);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn a_record_is_checked_against_its_modules_and_may_hold_no_item_outside_them() {
        let (repo, root) = scratch("check", &[("m.rs", "pub trait T { fn f(&self) -> u8; }"), (FREEZE, "")]);
        let mut made = recorded(&repo, &["m.rs"]);
        assert!(check(&repo, &made, None).is_ok(), "{:?}", check(&repo, &made, None));
        made.items.insert(("n.rs".to_string(), "T::g".to_string()), "fn g(&self)".to_string());
        let findings = check(&repo, &made, None).unwrap_err();
        assert_eq!(findings.iter().map(|f| f.code).collect::<Vec<_>>(), ["seam.malformed"]);
        std::fs::remove_dir_all(root).unwrap();
    }

    #[test]
    fn the_first_record_lists_the_seam_s_modules_under_its_decision() {
        let mut files: Vec<(&str, &str)> = MODULES.iter().map(|m| (*m, "pub struct S;")).collect();
        files.extend([(FREEZE, ""), ("ci/.keep", "")]);
        let (repo, root) = scratch("update", &files);
        assert!(update(&repo, FREEZE, None).is_ok());
        let made = parse(&repo.read(RECORD).unwrap()).unwrap();
        assert_eq!(
            (made.decisions, made.modules),
            (vec![FREEZE.to_string()], MODULES.map(ToString::to_string).to_vec())
        );
        std::fs::remove_dir_all(root).unwrap();
    }
}
