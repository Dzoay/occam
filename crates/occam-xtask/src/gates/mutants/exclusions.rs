//! The mutants gate's first check: every `exclude_re` entry in `.cargo/mutants.toml` still excludes a mutant of the
//! workspace, occam-xtask's included. An entry is written for a mutant, often by its line, and code added above that line
//! moves the mutant from under it, in a file the PR may not otherwise touch: the entry then excludes nothing, and the
//! mutant it was for fails a later PR that does touch it. So an entry that excludes nothing is refused, whatever the
//! diff, with the remedy of re-pointing it to where its mutant is now, or dropping it.
//!
//! cargo-mutants itself decides what an entry excludes, as in a run, since no other reading of the regex is exactly its
//! own: it does not apply them to a "delete field" mutant, for one. One listing of the workspace per entry would cost
//! minutes, so the entries are listed in groups. Each entry is given a code: its own set of half the groups, as many
//! groups as it takes for every entry to have one. The workspace is listed once with no exclusion and once with each
//! group's entries as the only exclusions. A mutant excluded by exactly the groups of one entry's code is excluded by
//! that entry alone, since an entry that excludes it has its whole code among those groups, and the codes are distinct
//! and each holds as many groups: it proves that entry excludes something. An entry no mutant proves, because another entry also
//! excludes every mutant it does, or because it excludes none, is listed alone.

use std::collections::{BTreeMap, BTreeSet};
use std::iter;

use super::lines;
use crate::repo::Repo;
use crate::verdict::{Finding, Verdict, verdict};

/// cargo-mutants' configuration.
const CONFIG: &str = ".cargo/mutants.toml";

/// The key of its regexes of mutants to exclude.
const KEY: &str = "exclude_re";

/// Lists the workspace's mutants with no exclusion and with each group's, then alone each entry no mutant proved, and
/// refuses every entry that excludes nothing.
pub fn run(repo: &Repo) -> Verdict {
    let (config, entries) = entries(&repo.read(CONFIG).map_err(|e| vec![e])?).map_err(|e| vec![e])?;
    let codes = codes(entries.len());
    let groups = iter::once(Vec::new()).chain(groups(&entries, &codes));
    let listings = repo.mutants_listings(&groups.map(|group| with_exclusions(&config, &group)).collect::<Vec<_>>());
    let listings = listings.map_err(|e| vec![e])?;
    let Some((all, kept)) = listings.split_first() else {
        let message = "cargo mutants listed nothing";
        return Err(vec![Finding::new("mutants.listing-failed", CONFIG, message, "run `cargo mutants --list`")]);
    };
    let unproven = unproven(all, kept, &codes);
    let alone = unproven.iter().filter_map(|&entry| entries.get(entry));
    let alone: Vec<String> = alone.map(|entry| with_exclusions(&config, &[entry.as_str()])).collect();
    let alone = repo.mutants_listings(&alone).map_err(|e| vec![e])?;
    let count = listings.len().saturating_add(alone.len());
    let summary = format!("each of the {} exclude_re entries excludes a mutant, in {count} listings", entries.len());
    verdict(summary, stale(&entries, &unproven, &alone, all))
}

/// The configuration `text` holds, and its `exclude_re` entries in order.
fn entries(text: &str) -> Result<(toml::Table, Vec<String>), Finding> {
    let invalid = |detail: String| {
        let remedy = "make it TOML whose exclude_re is a list of strings, as cargo-mutants reads it";
        Finding::new("mutants.config", CONFIG, detail, remedy)
    };
    let config: toml::Table = text.parse().map_err(|e: toml::de::Error| invalid(e.to_string()))?;
    let Some(listed) = config.get(KEY) else { return Ok((config, Vec::new())) };
    let strings = listed.as_array().and_then(|list| list.iter().map(|v| v.as_str().map(str::to_string)).collect());
    let entries = strings.ok_or_else(|| invalid(format!("{KEY} is not a list of strings")))?;
    Ok((config, entries))
}

/// `config` as a configuration's text, with `exclude` as its only `exclude_re` entries.
fn with_exclusions(config: &toml::Table, exclude: &[&str]) -> String {
    let mut config = config.clone();
    let listed = exclude.iter().map(|entry| toml::Value::String((*entry).to_string())).collect();
    config.insert(KEY.to_string(), toml::Value::Array(listed));
    config.to_string()
}

/// Each of `count` entries' code, its groups as bits: the first `count` numbers that have half of `n` bits set, a half
/// rounded up, for the least `n` that has as many.
fn codes(count: usize) -> Vec<u32> {
    let of = |groups: u32| {
        let half = groups.div_ceil(2);
        let codes: Vec<u32> =
            (0..1_u32.checked_shl(groups)?).filter(|code| code.count_ones() == half).take(count).collect();
        (codes.len() == count).then_some(codes)
    };
    (1..u32::BITS).find_map(of).unwrap_or_default()
}

/// How many groups `codes` use.
fn width(codes: &[u32]) -> u32 {
    u32::BITS.saturating_sub(codes.iter().fold(0, |all, code| all | code).leading_zeros())
}

/// Each group's entries, in order: those whose code holds it.
fn groups<'a>(entries: &'a [String], codes: &[u32]) -> Vec<Vec<&'a str>> {
    let holds = |code: u32, group: u32| code.checked_shr(group).is_some_and(|rest| rest & 1 == 1);
    let group = |g| entries.iter().zip(codes).filter(|&(_, &code)| holds(code, g)).map(|(e, _)| e.as_str()).collect();
    (0..width(codes)).map(group).collect()
}

/// The entries no mutant of `all` proves to exclude something, given `kept`, each group's listing: a mutant proves the
/// entry whose code is exactly the groups whose listings leave it out.
fn unproven(all: &str, kept: &[String], codes: &[u32]) -> Vec<usize> {
    let kept: Vec<BTreeSet<&str>> = kept.iter().map(|listing| lines(listing).collect()).collect();
    let owner: BTreeMap<u32, usize> = codes.iter().enumerate().map(|(entry, &code)| (code, entry)).collect();
    let mut proven = vec![false; codes.len()];
    for mutant in lines(all) {
        let left_out = (0_u32..).zip(&kept).filter(|(_, listing)| !listing.contains(mutant));
        let code: u32 = left_out.map(|(group, _)| 1_u32.checked_shl(group).unwrap_or(0)).sum();
        if let Some(entry) = owner.get(&code).and_then(|&entry| proven.get_mut(entry)) {
            *entry = true;
        }
    }
    proven.iter().enumerate().filter(|&(_, &proven)| !proven).map(|(entry, _)| entry).collect()
}

/// A finding for each entry of `unproven` whose listing alone, in `alone`, keeps every mutant of `all`.
fn stale(entries: &[String], unproven: &[usize], alone: &[String], all: &str) -> Vec<Finding> {
    let every = lines(all).count();
    let none = unproven.iter().zip(alone).filter(|(_, listing)| lines(listing).count() >= every);
    let remedy = "find where the mutant it was written for is now (`cargo mutants --list --file <its file>`), check the \
        code there is still equivalent and re-point the entry to its line, or drop the entry if the code is gone";
    let finding = |entry: &String| {
        let message = format!("the exclude_re entry \"{entry}\" excludes no mutant of the workspace");
        Finding::new("mutants.stale-exclusion", CONFIG, message, remedy)
    };
    none.filter_map(|(&entry, _)| entries.get(entry)).map(finding).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    fn owned(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    const CONFIGURED: &str = r#"
gitignore = true
test_workspace = true
exclude_globs = ["crates/a/src/glue.rs"]
exclude_re = [
    # Why.
    "a\\.rs:12:5: replace < with <= in f$",
    "in run$",
]
"#;

    #[test]
    fn the_entries_are_read_in_order() {
        let (config, listed) = entries(CONFIGURED).unwrap();
        assert_eq!(listed, [r"a\.rs:12:5: replace < with <= in f$", "in run$"]);
        assert_eq!(config.get("test_workspace").and_then(toml::Value::as_bool), Some(true));
        assert_eq!(entries("gitignore = true\n").unwrap().1, Vec::<String>::new());
    }

    #[test]
    fn a_configuration_that_cannot_be_read_is_refused() {
        for bad in ["exclude_re = [", "exclude_re = \"in run$\"", "exclude_re = [\"in run$\", 3]"] {
            let found = entries(bad).unwrap_err();
            assert_eq!((found.code, found.file.as_str()), ("mutants.config", CONFIG), "{bad}");
        }
    }

    #[test]
    fn a_configuration_is_written_with_only_the_exclusions_given() {
        let (config, _) = entries(CONFIGURED).unwrap();
        let (written, kept) = entries(&with_exclusions(&config, &["in f$", r"b\.rs"])).unwrap();
        assert_eq!(kept, [r"in f$", r"b\.rs"]);
        assert_eq!(written.get("exclude_globs"), config.get("exclude_globs"));
        assert_eq!(written.get("gitignore"), config.get("gitignore"));
        assert_eq!(written.len(), config.len());
        assert_eq!(entries(&with_exclusions(&config, &[])).unwrap().1, Vec::<String>::new());
    }

    #[test]
    fn each_entry_has_a_code_of_its_own_of_half_the_groups() {
        assert_eq!(codes(0), Vec::<u32>::new());
        assert_eq!(codes(1), [0b1]);
        assert_eq!(codes(2), [0b01, 0b10]);
        assert_eq!(codes(3), [0b011, 0b101, 0b110]);
        assert_eq!(codes(4), [0b0011, 0b0101, 0b0110, 0b1001]);
        // C(10, 5) is 252, the most codes ten groups give, and 237 entries need all ten; 253 need eleven.
        for (count, groups) in [(237, 10_u32), (252, 10), (253, 11)] {
            let given = codes(count);
            let distinct: BTreeSet<u32> = given.iter().copied().collect();
            assert_eq!(distinct.len(), count);
            assert!(given.iter().all(|code| code.count_ones() == groups.div_ceil(2)), "{count}");
            assert_eq!(width(&given), groups, "{count}");
        }
    }

    #[test]
    fn a_group_holds_the_entries_whose_codes_hold_it() {
        let entries = owned(&["a", "b", "c", "d"]);
        let want: [&[&str]; 4] = [&["a", "b", "d"], &["a", "c"], &["b", "c"], &["d"]];
        assert_eq!(groups(&entries, &codes(4)), want);
        assert_eq!(groups(&[], &[]), Vec::<Vec<&str>>::new());
    }

    /// Lists `all` as cargo-mutants would with each group's entries excluded, given which mutants each entry excludes.
    fn kept_by_groups(all: &[&str], excludes: &[&[&str]], codes: &[u32]) -> Vec<String> {
        let names: Vec<String> = (0..excludes.len()).map(|e| e.to_string()).collect();
        let gone = |group: &[&str], mutant: &str| {
            group.iter().filter_map(|e| excludes.get(e.parse::<usize>().ok()?)).any(|own| own.contains(&mutant))
        };
        let kept = |group: Vec<&str>| all.iter().filter(|m| !gone(&group, m)).flat_map(|m| [*m, "\n"]).collect();
        groups(&names, codes).into_iter().map(kept).collect()
    }

    /// The entries that exclude no mutant no other entry excludes too, found by trying each.
    fn shared_or_none(all: &[&str], excludes: &[&[&str]]) -> Vec<usize> {
        let alone = |e: usize, m: &str| excludes.iter().enumerate().all(|(o, own)| o == e || !own.contains(&m));
        let proven = |e: usize| all.iter().any(|m| excludes[e].contains(m) && alone(e, m));
        (0..excludes.len()).filter(|&e| !proven(e)).collect()
    }

    #[test]
    fn a_mutant_only_one_entry_excludes_proves_it() {
        // a excludes m1 alone and b m2 alone; both exclude m3; c, written for m3, excludes only what a does; d nothing.
        let all = ["m1", "m2", "m3", "m4"];
        let excludes: [&[&str]; 4] = [&["m1", "m3"], &["m2", "m3"], &["m3"], &[]];
        let codes = codes(4);
        let kept = kept_by_groups(&all, &excludes, &codes);
        assert_eq!(unproven(&all.map(|m| format!("{m}\n")).concat(), &kept, &codes), [2, 3]);
        assert_eq!(shared_or_none(&all, &excludes), [2, 3]);
    }

    #[test]
    fn exactly_the_entries_that_exclude_a_mutant_alone_are_proven() {
        // Pseudo-random relations: each entry excludes a mutant of its own and, now and then, a few of the others'.
        let mut seed = 0x2545_f491_u32;
        let mut next = move || {
            seed ^= seed.wrapping_shl(13);
            seed ^= seed.wrapping_shr(17);
            seed ^= seed.wrapping_shl(5);
            seed
        };
        for count in [1_usize, 2, 5, 30, 70, 300] {
            let names: Vec<String> = (0..count.saturating_add(3)).map(|m| format!("m{m}")).collect();
            let all: Vec<&str> = names.iter().map(String::as_str).collect();
            // Each mutant is another's about half the time, so that some entries are proven and some are not.
            let rarely = u32::try_from(count.saturating_mul(2)).unwrap();
            let shared: Vec<Vec<&str>> = (0..count)
                .map(|e| {
                    let mut own = vec![all[e]];
                    own.extend(all.iter().filter(|_| next().is_multiple_of(rarely)));
                    own
                })
                .collect();
            let excludes: Vec<&[&str]> = shared.iter().map(Vec::as_slice).collect();
            let codes = codes(count);
            let kept = kept_by_groups(&all, &excludes, &codes);
            let want = shared_or_none(&all, &excludes);
            assert!(count < 30 || (!want.is_empty() && want.len() < count), "{count}: {} unproven", want.len());
            assert_eq!(unproven(&format!("{}\n", all.join("\n")), &kept, &codes), want, "{count}");
        }
    }

    #[test]
    fn an_entry_listed_alone_that_keeps_every_mutant_is_stale() {
        let entries =
            owned(&[r"a\.rs:3:9: replace < with <= in f$", "in g$", r"b\.rs:7:1: replace h -> bool with true$"]);
        let all = "m1\nm2\nm3\n";
        let alone = owned(&["m1\nm2\nm3\n", "m1\nm3\n"]);
        let found = stale(&entries, &[0, 2], &alone, all);
        assert_eq!(found.len(), 1);
        assert_eq!((found[0].code, found[0].file.as_str()), ("mutants.stale-exclusion", CONFIG));
        assert!(found[0].message.contains(r"a\.rs:3:9: replace < with <= in f$"), "{found:?}");
        assert!(found[0].remedy.contains("re-point") && found[0].remedy.contains("drop"), "{found:?}");
        assert_eq!(stale(&entries, &[1], &owned(&["m1\nm3\n"]), all), []);
        assert_eq!(stale(&entries, &[], &[], all), []);
    }
}
