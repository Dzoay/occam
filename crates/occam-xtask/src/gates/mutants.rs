//! `gates mutants`: every change is tested (L13, L14). Adapted from phasekit's gate of the same name. It runs the
//! unmutated suite first, then cargo-mutants over the Rust changes since the merge base with `origin/main`. A mutant no
//! test catches fails the gate unless `.cargo/mutants.toml` excludes it with a reason.
//!
//! Nothing is mutated or built in the checkout. Its files are copied to a directory under `TMPDIR` and the tests are
//! built and run there once, unmutated: the gate times that run and passes the timeout, since cargo-mutants has no
//! baseline of its own to derive one from. Each pass then copies the built tree once per job, file times kept, so that
//! no copy builds from nothing, and runs `cargo mutants --in-place` in every copy at once, each on its own shard
//! (`docs/decisions/2026-11-14-mutants-keep-the-whole-workspace.md`). The copies are removed however the gate ends.
//!
//! Two outcomes are not taken at a copy's word (L13). A mutant that does not build is unviable and passes, so a copy
//! that lists one must build the unmutated tests once more when it is done: if it cannot, the copy broke, not the
//! mutants, and the gate fails. A mutant whose tests ran out of time beside the other copies' is tested again, alone
//! and one at a time, as the suite was when it was timed, and what that says stands: only one that runs out of time
//! alone too counts as caught.
//!
//! It runs in two passes. No crate depends on `occam-xtask`, so only its own tests can catch its mutants and they run
//! those alone; every other mutant runs the whole workspace's tests. `--shard k/n` runs one of n slices of both, and
//! the copies split that slice: cargo-mutants deals mutant `i` to shard `i % n`, so copy `j` of `m` takes shard
//! `k + n·j` of `n·m`. The gate lists the slice first and fails unless the copies tested each of its mutants once.
//! `--iterate` skips the mutants an earlier run in this checkout caught, to try a fix on what survived or was not
//! reached; the gate is a run without it.
//!
//! First, whatever the diff, it checks that every exclusion in `.cargo/mutants.toml` still excludes a mutant
//! (the `exclusions` module); `--exclusions` runs that check alone, as `just preflight` does.

mod exclusions;
mod pass;

use std::collections::BTreeMap;
use std::time::{Duration, Instant};

use crate::repo::{MutantsRun, Repo};
use crate::verdict::{Finding, Verdict};

/// Each pass: its name and the cargo-mutants arguments that select it. Together they cover every mutant once.
const PASSES: [(&str, &[&str]); 2] = [
    ("workspace", &["--exclude", "crates/occam-xtask/**"]),
    ("xtask", &["--package", "occam-xtask", "--test-workspace", "false"]),
];

/// The least a mutant's tests are given, in seconds: cargo-mutants' own default where it has no baseline to go by.
const TIMEOUT_FLOOR: u64 = 300;

/// How many times the unmutated suite's time a mutant's tests are given: cargo-mutants' own default multiple of its
/// baseline.
const TIMEOUT_MULTIPLE: u64 = 5;

/// The unmutated suite, as it is run to be timed; with `--no-run` it is the build the copies must find done.
const SUITE: [&str; 7] = ["nextest", "run", "--workspace", "--cargo-profile", "mutants", "--profile", "ci"];

/// The lists cargo-mutants writes, one mutant per line: caught, missed, timeout and unviable.
type Lists = [String; 4];

/// What a run was asked for.
#[derive(Debug, Default, PartialEq, Eq)]
struct Options {
    /// One of n slices of each pass: slice k of n.
    shard: Option<(u32, u32)>,
    /// Skip what an earlier run caught.
    iterate: bool,
    /// Only check the exclusions.
    exclusions: bool,
}

/// What cargo-mutants wrote: one mutant per line.
struct Outcomes<'a> {
    caught: &'a str,
    missed: &'a str,
    timeout: &'a str,
}

fn no_diff() -> String {
    "no Rust code changes since origin/main".to_string()
}

fn no_mutants() -> String {
    "no mutants to test in the changes since origin/main".to_string()
}

/// Refuses to run where there is no Unix (`unix` is whether there is): what the gate starts in the copies runs in a
/// process group of its own, so that it can be stopped and the copies removed if the gate is killed, and the copies
/// are made by `cp` and removed by `sh`.
fn supported(unix: bool) -> Result<(), Vec<Finding>> {
    if unix {
        return Ok(());
    }
    let message =
        "the mutants gate needs Unix process groups, `sh` and `cp`, to stop what it starts and remove its copies";
    Err(vec![Finding::new("mutants.unix-only", "", message, "run `just mutants` on Linux or macOS, or leave it to CI")])
}

pub fn run(repo: &Repo, args: &[String]) -> Verdict {
    let options = options(args)?;
    let exclusions = exclusions::run(repo)?;
    if options.exclusions {
        return Ok(exclusions);
    }
    supported(cfg!(unix))?;
    let (setting, threads) = Repo::mutants_jobs();
    let jobs = jobs(setting.as_deref(), threads)?;
    let base = repo.base().map_err(|e| vec![e])?;
    let mut diff = code_hunks(&repo.git(&["diff", "-U0", &base, "--", "*.rs"]).map_err(|e| vec![e])?);
    for path in repo.git(&["ls-files", "--others", "--exclude-standard", "--", "*.rs"]).map_err(|e| vec![e])?.lines() {
        diff.push_str(&new_file_diff(path, &repo.read(path).map_err(|e| vec![e])?));
    }
    if diff.trim().is_empty() {
        return Ok(format!("{exclusions}; {}", no_diff()));
    }
    // Listed before anything is built: a slice with no mutant in it, as most of CI's are on a small PR, ends here.
    let slice = slice(options.shard);
    let mut listed = Vec::new();
    for (pass, selection) in PASSES {
        let args = listing(selection, options.iterate, &slice);
        listed.push(repo.mutants_list(pass, &diff, &args).map_err(|e| vec![e])?);
    }
    if listed.iter().all(|list| lines(list).next().is_none()) {
        return Ok(format!("{exclusions}; {}", no_mutants()));
    }
    let warm = repo.warm(leftover).map_err(|e| vec![e])?;
    // Built first, so that the run that is timed is the tests alone.
    let build: Vec<&str> = SUITE.into_iter().chain(["--no-run"]).collect();
    warm.checked("mutants.unmutated-failed", &build).map_err(|e| vec![e])?;
    let started = Instant::now();
    warm.checked("mutants.unmutated-failed", &SUITE).map_err(|e| vec![e])?;
    let timeout = timeout(started.elapsed()).to_string();
    let gate = pass::Gate { repo, warm: &warm, options: &options, jobs, timeout: &timeout, build: &build };
    let mut passes = Vec::new();
    for ((pass, selection), listed) in PASSES.into_iter().zip(&listed) {
        passes.push(pass::run(&gate, pass, selection, listed)?);
    }
    Ok(format!("{exclusions}; mutants of the changes since origin/main: {}", passes.join("; ")))
}

/// One pass's line of the summary, or what fails it: a mutant no test caught, then a mutant the copies did not test
/// exactly once.
fn summary(success: bool, lists: &Lists, mut untested: Vec<Finding>) -> Result<String, Vec<Finding>> {
    let [caught, missed, timeout, unviable] = lists.each_ref().map(String::as_str);
    let (caught, hung) = check(success, &Outcomes { caught, missed, timeout }).map_err(|mut found| {
        found.append(&mut untested);
        found
    })?;
    if !untested.is_empty() {
        return Err(untested);
    }
    Ok(format!("{caught} caught ({} by timeout), {} unviable", hung.len(), lines(unviable).count()))
}

/// Reads the gate's arguments: `--shard <k>/<n>`, `--iterate` and `--exclusions`, in any order.
fn options(args: &[String]) -> Result<Options, Vec<Finding>> {
    let remedy =
        "pass --shard <k>/<n> with k < n, --iterate, both or neither; or --exclusions to check only the exclusions";
    let usage = |detail: String| vec![Finding::new("mutants.usage", "", detail, remedy)];
    let mut options = Options::default();
    let mut rest = args.iter();
    while let Some(flag) = rest.next() {
        let value = if flag == "--shard" { rest.next() } else { None };
        match (flag.as_str(), value.map(|slice| shard(slice))) {
            ("--iterate", _) => options.iterate = true,
            ("--exclusions", _) => options.exclusions = true,
            ("--shard", Some(Some(slice))) => options.shard = Some(slice),
            _ => return Err(usage(format!("unexpected: {flag} {}", value.map_or("", String::as_str)))),
        }
    }
    Ok(options)
}

/// `slice` as k and n, if it is `k/n` with k < n.
fn shard(slice: &str) -> Option<(u32, u32)> {
    let (k, n) = slice.split_once('/')?;
    Some((k.parse().ok()?, n.parse().ok()?)).filter(|(k, n)| k < n)
}

/// How many copies test mutants at once: `setting`, which is `OCCAM_MUTANTS_JOBS`, or by default a quarter of the
/// hardware threads, 1 to 4, as each copy runs its own parallel build and test binaries.
fn jobs(setting: Option<&str>, threads: usize) -> Result<usize, Vec<Finding>> {
    let Some(setting) = setting else {
        return Ok((threads / 4).clamp(1, 4));
    };
    setting.parse().ok().filter(|jobs| *jobs > 0).ok_or_else(|| {
        let message = format!("OCCAM_MUTANTS_JOBS is {setting:?}, not a number of jobs");
        vec![Finding::new("mutants.usage", "", message, "set it to a whole number, 1 or more, or unset it")]
    })
}

/// The run's slice as cargo-mutants takes it: the one asked for, or the one slice there is.
fn slice(shard: Option<(u32, u32)>) -> String {
    let (k, n) = shard.unwrap_or((0, 1));
    format!("{k}/{n}")
}

/// The shard each copy takes, for `mutants` mutants in the run's slice: one copy per job and never more than there
/// are mutants. cargo-mutants deals mutant `i` to shard `i % n` (`--sharding round-robin`), so slice k of n holds
/// `k`, `k + n`, `k + 2n`, …, and copy j of m takes every m-th of those from the j-th: shard `k + n·j` of `n·m`.
fn shards(shard: Option<(u32, u32)>, jobs: usize, mutants: usize) -> Vec<String> {
    let (k, n) = shard.map_or((0, 1), |(k, n)| (u64::from(k), u64::from(n)));
    let copies = u64::try_from(jobs.min(mutants)).unwrap_or(u64::MAX);
    let of = n.saturating_mul(copies);
    (0..copies).map(|j| format!("{}/{of}", n.saturating_mul(j).saturating_add(k))).collect()
}

/// What selects a pass's mutants in `shard`, for listing them and for testing them alike: its selection, the shard,
/// and `--iterate` where asked for, which leaves out what earlier runs settled before the shards are dealt.
fn selected<'a>(selection: &[&'a str], iterate: bool, shard: &'a str) -> impl Iterator<Item = &'a str> {
    let dealt = ["--sharding", "round-robin", "--shard", shard];
    selection.iter().copied().chain(dealt).chain(iterate.then_some("--iterate"))
}

/// The cargo-mutants arguments that list a pass's mutants in the run's `slice`.
fn listing<'a>(selection: &[&'a str], iterate: bool, slice: &'a str) -> Vec<&'a str> {
    ["--list"].into_iter().chain(selected(selection, iterate, slice)).collect()
}

/// The cargo-mutants arguments of one copy: it mutates the tree it runs in, which is the copy, tests its `shard` of
/// the pass, and gives a mutant's tests `timeout` seconds.
fn testing<'a>(selection: &[&'a str], iterate: bool, shard: &'a str, timeout: &'a str) -> Vec<&'a str> {
    ["--in-place", "--timeout", timeout].into_iter().chain(selected(selection, iterate, shard)).collect()
}

/// The copies' lists as one, each list in the copies' order, and what fails the gate because the copies did not test
/// every mutant `listed` exactly once: a copy that never started or stopped early, or shards that overlap.
fn merged(listed: &str, shards: impl Iterator<Item = Lists>) -> (Lists, Vec<Finding>) {
    let mut all = Lists::default();
    for lists in shards {
        for (list, mutants) in all.iter_mut().zip(&lists) {
            list.extend(lines(mutants).flat_map(|mutant| [mutant, "\n"]));
        }
    }
    let findings = once(listed, &all);
    (all, findings)
}

/// A finding for the mutants tested less often than `listed`, and one for those tested more often.
fn once(listed: &str, tested: &Lists) -> Vec<Finding> {
    // For each mutant, how often it was listed and how often tested.
    let mut times: BTreeMap<&str, (usize, usize)> = BTreeMap::new();
    for mutant in lines(listed) {
        let (listed, _) = times.entry(mutant).or_default();
        *listed = listed.saturating_add(1);
    }
    for mutant in tested.iter().flat_map(|list| lines(list)) {
        let (_, tested) = times.entry(mutant).or_default();
        *tested = tested.saturating_add(1);
    }
    let finding = |code, mutants: &[&str], message: String, remedy: &str| {
        let first = mutants.first().copied().unwrap_or_default();
        let file = first.split(':').next().unwrap_or_default();
        (!mutants.is_empty()).then(|| Finding::new(code, file, format!("{message}, the first: {first}"), remedy))
    };
    let untested: Vec<&str> = times.iter().filter(|(_, (listed, tested))| tested < listed).map(|(m, _)| *m).collect();
    let twice: Vec<&str> = times.iter().filter(|(_, (listed, tested))| tested > listed).map(|(m, _)| *m).collect();
    let message = format!("{} of {} mutants were not tested", untested.len(), lines(listed).count());
    let remedy = "read the output of the copy that stopped, in mutants.out/<pass>/shard-<j> or retest-<j>, and run \
                  `just mutants` again";
    let stopped = finding("mutants.not-tested", &untested, message, remedy);
    let were = if twice.len() == 1 { "mutant was" } else { "mutants were" };
    let message = format!("{} {were} tested more often than listed", twice.len());
    let remedy =
        "check that `cargo mutants --shard` still deals mutant i to shard i % n, which the copies' shards rely on";
    stopped.into_iter().chain(finding("mutants.tested-twice", &twice, message, remedy)).collect()
}

/// `line` without the styling cargo gives the word it starts with where colour is forced, as CI forces it: each
/// `ESC [ … m`. A sequence cut short is dropped.
fn plain(line: &str) -> String {
    let (mut out, mut rest) = (String::new(), line);
    while let Some((before, styled)) = rest.split_once("\u{1b}[") {
        out.push_str(before);
        rest = styled.split_once('m').map_or("", |(_, after)| after);
    }
    out + rest
}

/// Refuses a copy in which the build of the unmutated tests, whose `output` this is, compiled anything: the copy was
/// to find them built, and a copy that does not would build everything, each mutant's time with it. The finding
/// carries cargo's reason for the first crate, which that build is asked for (`--cargo-verbose`).
fn fresh(output: &str) -> Result<(), Finding> {
    let said: Vec<String> = output.lines().map(|line| plain(line).trim().to_string()).collect();
    let compiled: Vec<&str> = said.iter().filter_map(|line| line.strip_prefix("Compiling ")).collect();
    let Some(first) = compiled.first() else {
        return Ok(());
    };
    let why = said.iter().find_map(|line| line.strip_prefix("Dirty "));
    let why = why.map_or_else(String::new, |reason| format!("; cargo's reason: {reason}"));
    let message = format!("a copy of the built tree rebuilt {} crates, the first: {first}{why}", compiled.len());
    let remedy = "make what cargo's reason names the same in a copy as in the tree it was copied from: a build script \
                  names its paths from its crate's directory, and `cp -a` keeps file times where TMPDIR is";
    Err(Finding::new("mutants.copy-cold", "", message, remedy))
}

/// Whether a copy must build the unmutated tests again once its run is over, going by its `lists`: it found a mutant
/// unviable, which is a mutant that does not build, or a copy that no longer builds anything.
fn recheck(lists: &Lists) -> bool {
    let [_, _, _, unviable] = lists;
    lines(unviable).next().is_some()
}

/// Fails for each copy that could not build the unmutated tests after its run: its unviable mutants are the
/// environment's doing, a full disk or a copy removed, and say nothing about the code.
fn broke(runs: &[MutantsRun]) -> Result<(), Vec<Finding>> {
    let remedy = "free space where TMPDIR is, check that nothing removes its occam-mutants-warm directory while the \
                  gate runs, and run `just mutants` again";
    let found: Vec<Finding> = runs
        .iter()
        .enumerate()
        .filter_map(|(copy, run)| {
            let said = run.broken.as_deref()?;
            let [_, _, _, unviable] = &run.outcomes;
            let (count, first) = (lines(unviable).count(), lines(unviable).next().unwrap_or_default());
            let are = if count == 1 { "mutant is" } else { "mutants are" };
            let message = format!(
                "copy {copy} no longer builds the unmutated tests after its run, so the environment broke and its \
                 {count} unviable {are} not trusted, the first: {first}\n{said}"
            );
            Some(Finding::new(
                "mutants.environment-broke",
                first.split(':').next().unwrap_or_default(),
                message,
                remedy,
            ))
        })
        .collect();
    if found.is_empty() { Ok(()) } else { Err(found) }
}

/// The shard of each mutant to test again, alone: those of `listed`, the mutants of the run's slice `shard`, whose
/// tests ran out of time in `lists`, in the order listed. None where the pass fails as it is, by `findings` or a
/// survivor. Dealt one mutant to a shard, the mutant listed at `p` of `c` is alone in the shard copy `p` of `c` takes.
fn retests(shard: Option<(u32, u32)>, listed: &str, lists: &Lists, findings: &[Finding]) -> Vec<String> {
    let [_, missed, hung, _] = lists;
    if !findings.is_empty() || lines(missed).next().is_some() {
        return Vec::new();
    }
    let mut hung: Vec<&str> = lines(hung).collect();
    let count = lines(listed).count();
    let alone = shards(shard, count, count);
    let again = lines(listed).zip(alone).filter_map(|(mutant, shard)| {
        let at = hung.iter().position(|hung| *hung == mutant)?;
        hung.swap_remove(at);
        Some(shard)
    });
    again.collect()
}

/// The pass's lists once the mutants that ran out of time in `first` were tested again: what the retest says of each
/// stands, in place of the first timeout.
fn retried(first: &Lists, retested: &Lists) -> Lists {
    let [caught, missed, _, unviable] = first;
    let [caught_alone, missed_alone, hung_alone, unviable_alone] = retested;
    [
        format!("{caught}{caught_alone}"),
        format!("{missed}{missed_alone}"),
        hung_alone.clone(),
        format!("{unviable}{unviable_alone}"),
    ]
}

/// What a pass's line of the summary says of its `retested` mutants.
fn note(retested: usize) -> String {
    if retested == 0 { String::new() } else { format!(", {retested} tested again alone after running out of time") }
}

/// Whether `name`, found beside this run's directory, is a run's directory: each is named by its process's number.
/// Nothing else there is ever removed, and such a directory only once its run is gone.
fn leftover(name: &str) -> bool {
    !name.is_empty() && name.bytes().all(|byte| byte.is_ascii_digit())
}

/// How long a mutant's tests may run, in seconds, given how long the unmutated suite took: [`TIMEOUT_MULTIPLE`] times
/// it, a part of a second counted as a whole one, and never less than [`TIMEOUT_FLOOR`]. A mutant no test catches runs
/// the whole suite, beside the other jobs' builds and tests, where the suite was timed alone: one that runs out of time
/// there is given the same time again, alone ([`retests`]), and only then counts as caught (L13).
fn timeout(suite: Duration) -> u64 {
    let seconds = suite.as_secs().saturating_add(u64::from(suite.subsec_nanos() > 0));
    seconds.saturating_mul(TIMEOUT_MULTIPLE).max(TIMEOUT_FLOOR)
}

/// Checks one pass (`success` is whether every cargo-mutants run succeeded, which none does after a timeout). Returns how many
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

    fn owned(list: &[&str]) -> Vec<String> {
        list.iter().map(ToString::to_string).collect()
    }

    #[test]
    fn a_shard_selects_one_slice_of_the_mutants() {
        assert_eq!(options(&[]), Ok(Options::default()));
        for (good, slice) in [("0/4", (0, 4)), ("3/4", (3, 4))] {
            let want = Options { shard: Some(slice), ..Options::default() };
            assert_eq!(options(&owned(&["--shard", good])), Ok(want));
        }
        for bad in [
            &["--shard", "4/4"][..],
            &["--shard", "1/0"],
            &["--shard", "a/4"],
            &["--shard", "1/b"],
            &["--shard", "1"],
            &["--shard"],
            &["--jobs", "2"],
            &["--base", "abc123"],
            &["0/4"],
        ] {
            assert_eq!(codes(&options(&owned(bad)).map(|_| String::new())), ["mutants.usage"], "{bad:?}");
        }
    }

    #[test]
    fn the_options_combine_in_either_order() {
        let both = Options { shard: Some((1, 16)), iterate: true, ..Options::default() };
        assert_eq!(options(&owned(&["--iterate", "--shard", "1/16"])), Ok(both));
        let both = Options { shard: Some((1, 16)), iterate: true, ..Options::default() };
        assert_eq!(options(&owned(&["--shard", "1/16", "--iterate"])), Ok(both));
        assert_eq!(options(&owned(&["--iterate"])), Ok(Options { iterate: true, ..Options::default() }));
    }

    #[test]
    fn exclusions_alone_are_checked_when_asked() {
        assert_eq!(options(&owned(&["--exclusions"])), Ok(Options { exclusions: true, ..Options::default() }));
        let found = options(&owned(&["--exclusion"])).unwrap_err();
        assert!(found[0].remedy.contains("--exclusions"), "{found:?}");
    }

    #[test]
    fn the_jobs_are_the_setting_or_a_quarter_of_the_threads_from_one_to_four() {
        assert_eq!(
            [0, 1, 3, 4, 7, 8, 12, 16, 17, 64].map(|threads| jobs(None, threads)).map(Result::ok),
            [Some(1), Some(1), Some(1), Some(1), Some(1), Some(2), Some(3), Some(4), Some(4), Some(4)]
        );
        assert_eq!(jobs(Some("1"), 64), Ok(1));
        assert_eq!(jobs(Some("6"), 1), Ok(6));
        for bad in ["0", "", "two", "-1", "1.5"] {
            let found = jobs(Some(bad), 8).unwrap_err();
            assert_eq!(found.len(), 1, "{bad}");
            assert_eq!(found[0].code, "mutants.usage");
            assert!(found[0].message.contains("OCCAM_MUTANTS_JOBS") && found[0].remedy.contains("whole number"));
        }
    }

    #[test]
    fn the_copies_split_the_runs_slice_and_together_are_exactly_it() {
        assert_eq!(slice(None), "0/1");
        assert_eq!(slice(Some((3, 16))), "3/16");
        // The whole of a pass, among three copies.
        assert_eq!(shards(None, 3, 58), ["0/3", "1/3", "2/3"]);
        // Slice 3 of 16 holds the mutants numbered 3, 19, 35, 51, …: two copies take every other one of those.
        assert_eq!(shards(Some((3, 16)), 2, 40), ["3/32", "19/32"]);
        assert_eq!(shards(Some((0, 2)), 3, 10), ["0/6", "2/6", "4/6"]);
        // Never more copies than mutants, and none where there is nothing to test.
        assert_eq!(shards(None, 4, 2), ["0/2", "1/2"]);
        assert_eq!(shards(Some((15, 16)), 2, 1), ["15/16"]);
        assert_eq!(shards(Some((1, 4)), 3, 0), Vec::<String>::new());
        assert_eq!(shards(None, 1, 7), ["0/1"]);
        // Every mutant of a slice falls to one copy: i is in slice k of n, and in exactly one of the copies' shards.
        for (k, n, copies) in [(0, 1, 3), (3, 16, 2), (1, 2, 4), (4, 5, 3)] {
            let within: Vec<(u32, u32)> = shards(Some((k, n)), copies, 100).iter().filter_map(|s| shard(s)).collect();
            assert_eq!(within.len(), copies);
            for i in 0..200_u32 {
                let takers = within.iter().filter(|(shard, of)| i % of == *shard).count();
                assert_eq!(takers, usize::from(i % n == k), "mutant {i} of slice {k}/{n}");
            }
        }
        assert_eq!(
            shards(Some((u32::MAX - 1, u32::MAX)), 2, 9),
            [
                format!("{}/{}", u32::MAX - 1, 2 * u64::from(u32::MAX)),
                format!("{}/{}", 2 * u64::from(u32::MAX) - 1, 2 * u64::from(u32::MAX))
            ]
        );
    }

    #[test]
    fn listing_and_testing_select_the_same_mutants_and_only_testing_mutates() {
        let [(_, workspace), (_, xtask)] = PASSES;
        let dealt = ["--sharding", "round-robin", "--shard"];
        let listed = [&["--list", "--exclude", "crates/occam-xtask/**"][..], &dealt, &["0/1"]].concat();
        assert_eq!(listing(workspace, false, "0/1"), listed);
        let tested = [&["--in-place", "--timeout", "300", "--exclude", "crates/occam-xtask/**"][..], &dealt].concat();
        assert_eq!(testing(workspace, false, "1/3", "300"), [&tested[..], &["1/3"]].concat());
        assert_eq!(testing(workspace, true, "1/3", "300"), [&tested[..], &["1/3", "--iterate"]].concat());
        let own = ["--package", "occam-xtask", "--test-workspace", "false"];
        let sliced = [&["--list"][..], &own, &dealt, &["2/16", "--iterate"]].concat();
        assert_eq!(listing(xtask, true, "2/16"), sliced);
        let copy = [&["--in-place", "--timeout", "450"][..], &own, &dealt, &["18/32", "--iterate"]].concat();
        assert_eq!(testing(xtask, true, "18/32", "450"), copy);
    }

    fn lists(caught: &str, missed: &str, timeout: &str, unviable: &str) -> Lists {
        [caught, missed, timeout, unviable].map(str::to_string)
    }

    const A: &str = "src/a.rs:1:5: replace f -> bool with true";
    const B: &str = "src/a.rs:2:9: replace < with <= in f";
    const C: &str = "src/b.rs:7:1: replace g with ()";
    const D: &str = "src/b.rs:9:3: delete ! in g";

    #[test]
    fn the_copies_lists_merge_into_one_in_the_copies_order() {
        let listed = format!("{A}\n{B}\n{C}\n{D}\n");
        let copies = [lists(&format!("{A}\n"), "", &format!("{C}\n"), ""), lists("", &format!("{B}\n"), "", D)];
        let (merged_lists, untested) = merged(&listed, copies.into_iter());
        assert_eq!(merged_lists, lists(&format!("{A}\n"), &format!("{B}\n"), &format!("{C}\n"), &format!("{D}\n")));
        assert_eq!(untested, []);
        let both = [lists(&format!("{C}\n\n {A} \n"), "", "", ""), lists(&format!("{B}\n{D}\n"), "", "", "")];
        let (merged_lists, untested) = merged(&listed, both.into_iter());
        assert_eq!(merged_lists, lists(&format!("{C}\n{A}\n{B}\n{D}\n"), "", "", ""));
        assert_eq!(untested, []);
        assert_eq!(merged("", std::iter::empty()), (Lists::default(), vec![]));
    }

    #[test]
    fn a_mutant_no_copy_tested_fails_the_gate() {
        let listed = format!("{A}\n{B}\n{C}\n{D}\n");
        // The second copy never started.
        let (merged_lists, untested) = merged(&listed, [lists(&format!("{A}\n{C}\n"), "", "", "")].into_iter());
        assert_eq!(merged_lists, lists(&format!("{A}\n{C}\n"), "", "", ""));
        assert_eq!(untested.len(), 1);
        assert_eq!(untested[0].code, "mutants.not-tested");
        assert_eq!(untested[0].file, "src/a.rs");
        assert!(untested[0].message.starts_with("2 of 4 mutants were not tested"), "{untested:?}");
        assert!(untested[0].message.ends_with(B), "{untested:?}");
        let (_, none) = merged(&listed, std::iter::empty());
        assert!(none[0].message.starts_with("4 of 4 mutants were not tested") && none[0].message.ends_with(A));
    }

    #[test]
    fn a_mutant_tested_twice_or_never_listed_fails_the_gate() {
        let listed = format!("{A}\n{B}\n");
        let copies = [lists(&format!("{A}\n{B}\n"), "", "", ""), lists("", "", "", &format!("{B}\n"))];
        let (merged_lists, twice) = merged(&listed, copies.into_iter());
        assert_eq!(merged_lists, lists(&format!("{A}\n{B}\n"), "", "", &format!("{B}\n")));
        assert_eq!(twice.len(), 1);
        assert_eq!(twice[0].code, "mutants.tested-twice");
        assert_eq!(twice[0].file, "src/a.rs");
        assert!(twice[0].message.starts_with("1 mutant was tested more often than listed"), "{twice:?}");
        assert!(twice[0].message.ends_with(B), "{twice:?}");
        let (_, unlisted) = merged(&listed, [lists(&format!("{A}\n{B}\n{C}\n{D}\n"), "", "", "")].into_iter());
        assert_eq!(codes(&Err(unlisted.clone())), ["mutants.tested-twice"]);
        assert!(unlisted[0].message.starts_with("2 mutants were tested") && unlisted[0].message.ends_with(C));
        // Both at once: one finding each.
        let (_, both) = merged(&listed, [lists(&format!("{A}\n{A}\n"), "", "", "")].into_iter());
        assert_eq!(codes(&Err(both)), ["mutants.not-tested", "mutants.tested-twice"]);
        // A mutant listed twice is tested twice.
        let (_, same) = merged(&format!("{A}\n{A}\n"), [lists(&format!("{A}\n{A}\n"), "", "", "")].into_iter());
        assert_eq!(same, []);
    }

    #[test]
    fn a_pass_is_summed_up_or_fails_on_survivors_first_and_then_on_what_was_not_tested_once() {
        let clean = lists(&format!("{A}\n{B}\n"), "", &format!("{C}\n"), &format!("{D}\n"));
        assert_eq!(summary(false, &clean, vec![]), Ok("3 caught (1 by timeout), 1 unviable".to_string()));
        assert_eq!(summary(true, &lists(A, "", "", ""), vec![]), Ok("1 caught (0 by timeout), 0 unviable".to_string()));
        let untested = Finding::new("mutants.not-tested", "", "1 of 4", "run it again");
        assert_eq!(summary(false, &clean, vec![untested.clone()]), Err(vec![untested.clone()]));
        let survivor = lists(A, B, "", "");
        let found = summary(false, &survivor, vec![untested]);
        assert_eq!(codes(&found), ["mutants.missed", "mutants.not-tested"]);
        assert_eq!(codes(&summary(false, &survivor, vec![])), ["mutants.missed"]);
        assert_eq!(codes(&summary(false, &lists(A, "", "", ""), vec![])), ["mutants.run-failed"]);
    }

    fn ran(unviable: &str, broken: Option<&str>) -> MutantsRun {
        MutantsRun { success: true, outcomes: lists(A, "", "", unviable), broken: broken.map(str::to_string) }
    }

    #[test]
    fn a_copy_that_found_a_mutant_unviable_builds_the_unmutated_tests_again() {
        assert!(recheck(&lists("", "", "", &format!("{D}\n"))));
        assert!(recheck(&lists(A, B, C, D)));
        // Nothing unviable, nothing to doubt: no build.
        assert!(!recheck(&lists(A, B, C, "")));
        assert!(!recheck(&lists(A, "", "", " \n\n")));
        assert!(!recheck(&Lists::default()));
    }

    #[test]
    fn a_copy_that_no_longer_builds_fails_the_gate_and_does_not_pass_for_unviable_mutants() {
        // A mutant that does not build is unviable, and the copy builds clean afterwards: accepted, as before.
        assert_eq!(broke(&[ran(&format!("{C}\n{D}\n"), None), ran("", None)]), Ok(()));
        assert_eq!(broke(&[]), Ok(()));
        // The disk filled: every mutant after it "does not build", and neither do the unmutated tests.
        let full = "error: failed to write `target/x.rmeta`: No space left on device (os error 28)";
        let found = broke(&[ran("", None), ran(&format!("{C}\n{D}\n"), Some(full)), ran(D, Some("gone"))]).unwrap_err();
        assert_eq!(found.iter().map(|f| f.code).collect::<Vec<_>>(), ["mutants.environment-broke"; 2]);
        assert_eq!(found[0].file, "src/b.rs");
        assert!(found[0].message.starts_with("copy 1 no longer builds the unmutated tests"), "{found:?}");
        assert!(found[0].message.contains("the environment broke and its 2 unviable mutants are not trusted"));
        assert!(found[0].message.contains(&format!("the first: {C}")) && found[0].message.ends_with(full));
        assert!(found[1].message.starts_with("copy 2 ") && found[1].message.contains("its 1 unviable mutant is not"));
        // The remedy, followed: with room on the disk and the copies left alone, the next run's copies build.
        assert!(found[0].remedy.contains("TMPDIR") && found[0].remedy.contains("run `just mutants` again"));
    }

    #[test]
    fn a_mutant_that_ran_out_of_time_is_tested_again_alone_in_a_shard_of_its_own() {
        let listed = format!("{A}\n{B}\n{C}\n{D}\n");
        let hung = |timeout: &str| lists("", "", timeout, "");
        // The whole pass: the mutant listed at p of 4 is the only one of shard p/4.
        assert_eq!(retests(None, &listed, &hung(&format!("{C}\n")), &[]), ["2/4"]);
        // In the order listed, whatever order the copies reported them in.
        assert_eq!(retests(None, &listed, &hung(&format!("{D}\n{A}\n")), &[]), ["0/4", "3/4"]);
        // Slice 3 of 16 holds the mutants numbered 3, 19, 35 and 51: the third of them is 35, alone in 35/64.
        assert_eq!(retests(Some((3, 16)), &listed, &hung(&format!("{C}\n")), &[]), ["35/64"]);
        // Each shard holds its mutant alone: of the slice's mutants, numbered k + n·p, only the one at p falls in it.
        for (p, again) in retests(Some((3, 16)), &listed, &hung(&listed), &[]).iter().enumerate() {
            let (shard, of) = shard(again).unwrap();
            let within: Vec<u32> = (0..4_u32).filter(|q| (3 + 16 * q) % of == shard).collect();
            assert_eq!(within, [u32::try_from(p).unwrap()], "{again}");
        }
        // A mutant listed twice and hung once is retested once; hung twice, twice.
        let twice = format!("{A}\n{B}\n{A}\n");
        assert_eq!(retests(None, &twice, &hung(A), &[]), ["0/3"]);
        assert_eq!(retests(None, &twice, &hung(&format!("{A}\n{A}\n")), &[]), ["0/3", "2/3"]);
        // No timeout, no retest.
        assert_eq!(retests(None, &listed, &lists(&listed, "", "", ""), &[]), Vec::<String>::new());
    }

    #[test]
    fn a_pass_that_fails_as_it_is_spends_no_time_on_retests() {
        let listed = format!("{A}\n{B}\n{C}\n");
        let untested = Finding::new("mutants.not-tested", "", "1 of 3", "run it again");
        assert_eq!(retests(None, &listed, &lists(A, "", C, ""), &[untested]), Vec::<String>::new());
        assert_eq!(retests(None, &listed, &lists(A, B, C, ""), &[]), Vec::<String>::new());
        assert_eq!(retests(None, &listed, &lists(A, " \n", C, ""), &[]), ["2/3"]);
    }

    #[test]
    fn what_the_retest_says_of_a_mutant_stands_in_place_of_its_first_timeout() {
        let first = lists(&format!("{A}\n"), "", &format!("{B}\n{C}\n"), &format!("{D}\n"));
        // Alone, one is caught and the other runs out of time again: that one counts as caught, by timeout.
        let retested = lists(&format!("{B}\n"), "", &format!("{C}\n"), "");
        let after = retried(&first, &retested);
        assert_eq!(after, lists(&format!("{A}\n{B}\n"), "", &format!("{C}\n"), &format!("{D}\n")));
        assert_eq!(summary(false, &after, vec![]), Ok("3 caught (1 by timeout), 1 unviable".to_string()));
        // Alone, one survives: it had only been slow beside the other copies, and the gate fails on it.
        let survived = retried(&first, &lists(&format!("{C}\n"), &format!("{B}\n"), "", ""));
        assert_eq!(survived, lists(&format!("{A}\n{C}\n"), &format!("{B}\n"), "", &format!("{D}\n")));
        let found = summary(false, &survived, vec![]).unwrap_err();
        assert_eq!(found.iter().map(|f| f.code).collect::<Vec<_>>(), ["mutants.missed"]);
        assert!(found[0].message.ends_with(B), "{found:?}");
        // Both caught alone: the retests' cargo-mutants succeeded, and nothing is left by timeout.
        let caught = retried(&first, &lists(&format!("{B}\n{C}\n"), "", "", ""));
        assert_eq!(summary(true, &caught, vec![]), Ok("3 caught (0 by timeout), 1 unviable".to_string()));
        // Every mutant is still in one list, once.
        let listed = format!("{A}\n{B}\n{C}\n{D}\n");
        assert_eq!([once(&listed, &after), once(&listed, &survived), once(&listed, &caught)], [vec![], vec![], vec![]]);
    }

    #[test]
    fn a_pass_says_how_many_mutants_it_tested_again() {
        assert_eq!(note(0), "");
        assert_eq!(note(1), ", 1 tested again alone after running out of time");
        assert_eq!(note(12), ", 12 tested again alone after running out of time");
    }

    #[test]
    fn a_run_claiming_its_directory_is_never_swept_as_one_whose_run_is_gone() {
        use crate::repo::{stake, sweep};
        let all = std::env::temp_dir().join(format!("occam-mutants-claim-{}", std::process::id()));
        drop(std::fs::remove_dir_all(&all));
        std::fs::create_dir_all(all.join("7")).unwrap();
        // A run whose process is 41 has made its directory. Another run starts and sweeps before 41 settles.
        let claim = stake(&all, "41").unwrap();
        let names = |all: &std::path::Path| {
            let mut names: Vec<String> =
                std::fs::read_dir(all).unwrap().map(|e| e.unwrap().file_name().into_string().unwrap()).collect();
            names.sort();
            names
        };
        // Until then, nothing a sweep takes for a run's directory names it.
        assert_eq!(names(&all).iter().filter(|name| leftover(name)).collect::<Vec<_>>(), ["7"]);
        sweep(&all, leftover);
        // The sweep removed the run that is gone, 7, and not the one that is starting.
        assert_eq!(names(&all).len(), 1);
        let (home, held) = claim.settle().unwrap();
        assert_eq!((home.clone(), names(&all)), (all.join("41"), vec!["41".to_string()]));
        // Settled, it is locked: a later sweep leaves it, until its run is gone.
        std::fs::write(home.join("staging"), "").unwrap();
        sweep(&all, leftover);
        assert!(home.join("staging").is_file());
        // A second run of the same number, which a container can give, is refused while this one lives.
        assert!(stake(&all, "41").and_then(crate::repo::Claim::settle).is_err());
        assert!(home.join("staging").is_file());
        drop(held);
        sweep(&all, leftover);
        assert_eq!(names(&all), Vec::<String>::new());
        std::fs::remove_dir_all(&all).unwrap();
    }

    #[test]
    fn a_copy_that_compiles_anything_was_not_warm() {
        let built =
            "    Finished `mutants` profile [unoptimized] target(s) in 0.31s\n  Executable unittests src/lib.rs\n";
        assert_eq!(fresh(built), Ok(()));
        assert_eq!(fresh(""), Ok(()));
        let cold =
            format!("   Compiling occam-math v0.0.0 (/x/crates/occam-math)\n   Compiling occam v0.0.0 (/x)\n{built}");
        let found = fresh(&cold).unwrap_err();
        assert_eq!(found.code, "mutants.copy-cold");
        assert!(
            found.message.contains("2 crates") && found.message.ends_with("occam-math v0.0.0 (/x/crates/occam-math)")
        );
        assert!(found.remedy.contains("file times") && found.remedy.contains("build script"), "{found:?}");
        // Asked to say why, cargo does, before the crate: the finding carries the first reason.
        let why = "occam-cli v0.0.0 (/x/crates/occam-cli): the rerun-if-changed instructions changed";
        let told = format!(
            "       Fresh occam-math v0.0.0 (/x/crates/occam-math)\n       Dirty {why}\n   Compiling occam-cli v0.0.0 \
             (/x/crates/occam-cli)\n     Running `rustc --crate-name occam`\n       Dirty occam v0.0.0 (/x): later\n{built}"
        );
        let found = fresh(&told).unwrap_err();
        assert!(found.message.contains("1 crates, the first: occam-cli v0.0.0 (/x/crates/occam-cli)"), "{found:?}");
        assert!(found.message.ends_with(&format!("; cargo's reason: {why}")), "{found:?}");
        // Where colour is forced, as CI forces it, each word cargo starts a line with is styled: still a build.
        let styled = "\u{1b}[1m\u{1b}[92m   Compiling\u{1b}[0m occam v0.0.0 (/x)\n\u{1b}[1m\u{1b}[92m    Finished\u{1b}[0m `mutants`\n";
        let found = fresh(styled).unwrap_err();
        assert!(found.message.ends_with("rebuilt 1 crates, the first: occam v0.0.0 (/x)"), "{found:?}");
        assert_eq!(
            fresh(
                "\u{1b}[1m\u{1b}[92m       Fresh\u{1b}[0m occam v0.0.0 (/x)\n\u{1b}[1m\u{1b}[92m    Finished\u{1b}[0m\n"
            ),
            Ok(())
        );
        // A sequence cut short styles nothing and hides nothing before it.
        assert!(fresh("Compiling occam v0.0.0 (/x) \u{1b}[1").unwrap_err().message.ends_with("occam v0.0.0 (/x)"));
        // Only cargo's own line: a test or a path that says the word is not a build.
        assert_eq!(fresh("  Executable tests/Compiling.rs\nwarning: not Compiling anything\n"), Ok(()));
        assert!(fresh("Compiling occam v0.0.0 (/x)").is_err());
    }

    #[test]
    fn only_a_directory_named_by_a_process_is_a_runs_leftover() {
        assert!(leftover("1") && leftover("4194304"));
        for other in ["", "staging", "12a", "-12", "1.lock", " 12", "12/"] {
            assert!(!leftover(other), "{other:?}");
        }
    }

    #[test]
    fn off_unix_the_gate_says_where_to_run_it_and_there_it_runs() {
        let found = supported(false).unwrap_err();
        assert_eq!(found.len(), 1);
        assert_eq!(found[0].code, "mutants.unix-only");
        assert!(found[0].message.contains("process groups"), "{found:?}");
        // The remedy, followed: on Linux or macOS, which are Unix, the gate goes on.
        assert!(found[0].remedy.contains("Linux or macOS"), "{found:?}");
        assert_eq!(supported(true), Ok(()));
    }

    #[test]
    fn with_no_mutants_in_the_slice_there_is_nothing_to_test() {
        assert_eq!(no_mutants(), "no mutants to test in the changes since origin/main");
    }

    #[test]
    fn a_mutants_tests_get_five_times_the_unmutated_suite_and_never_less_than_the_floor() {
        let of = |millis| timeout(Duration::from_millis(millis));
        // A quick suite keeps the floor, up to the suite that five times over is the floor.
        assert_eq!([of(0), of(23_000), of(59_000), of(59_999)], [300; 4]);
        // Past it, five times the suite's seconds, a part of a second counted as a whole one.
        assert_eq!([of(60_000), of(60_001), of(66_400), of(136_600)], [300, 305, 335, 685]);
        assert_eq!(timeout(Duration::MAX), u64::MAX);
    }

    fn header(path: &str) -> String {
        format!("diff --git a/{path} b/{path}\nindex 1..2 100644\n--- a/{path}\n+++ b/{path}\n")
    }

    #[test]
    fn comment_only_hunks_leave_the_diff() {
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
