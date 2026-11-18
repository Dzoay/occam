//! One pass of `gates mutants`, through the copies: glue, as each gate's `run` is. What it decides is in the parent
//! module's pure functions.

use super::{Lists, Options, broke, fresh, lines, merged, note, recheck, retests, retried, shards, summary, testing};
use crate::repo::{Copies, Repo, Warm};
use crate::verdict::{Finding, Verdict};

/// What every pass of a run shares.
pub struct Gate<'a> {
    pub repo: &'a Repo,
    /// The built tree the copies are made from.
    pub warm: &'a Warm,
    pub options: &'a Options,
    /// How many copies test mutants at once.
    pub jobs: usize,
    /// How long a mutant's tests may run, in seconds.
    pub timeout: &'a str,
    /// The cargo arguments that built the tree's tests.
    pub build: &'a [&'a str],
}

/// Tests the mutants `listed` for the pass `name`, which `selection` selects: all copies at once, then each mutant
/// that ran out of time again, alone. Returns the pass's line of the summary.
pub fn run(gate: &Gate<'_>, name: &str, selection: &[&str], listed: &str) -> Verdict {
    let (slice, iterate) = (gate.options.shard, gate.options.iterate);
    let settled = if iterate { gate.repo.mutants_settled(name) } else { String::new() };
    // One stage: the copies on `shards`, which together are to test `listed`. Whether every cargo-mutants succeeded,
    // the lists as one, and what the copies did not test once.
    let stage = |listed: &str, shards: &[String], alone: bool| -> Result<(bool, Lists, Vec<Finding>), Vec<Finding>> {
        let args: Vec<Vec<&str>> =
            shards.iter().map(|shard| testing(selection, iterate, shard, gate.timeout)).collect();
        let copies = Copies { pass: name, settled: &settled, build: gate.build, fresh, recheck, alone, shards: &args };
        let runs = gate.warm.shards(gate.repo, &copies).map_err(|e| vec![e])?;
        // Before anything is recorded: `--iterate` would take a broken copy's unviable mutants for settled.
        broke(&runs)?;
        let success = runs.iter().all(|run| run.success);
        let (lists, untested) = merged(listed, runs.into_iter().map(|run| run.outcomes));
        Ok((success, lists, untested))
    };
    let first = stage(listed, &shards(slice, gate.jobs, lines(listed).count()), false)?;
    let again = retests(slice, listed, &first.1, &first.2);
    let (success, lists, untested) = if again.is_empty() {
        first
    } else {
        let [_, _, hung, _] = &first.1;
        let (success, retested, untested) = stage(hung, &again, true)?;
        (success, retried(&first.1, &retested), untested)
    };
    gate.repo.mutants_record(name, &settled, &lists).map_err(|e| vec![e])?;
    Ok(format!("{name}: {}{}", summary(success, &lists, untested)?, note(again.len())))
}
