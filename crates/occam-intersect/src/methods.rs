//! Source: original
//! The ordered list of methods (S4): the pair put in canonical order, each method asked in turn until one applies, its
//! work bounded by a budget, and many pairs run one a task.

use std::sync::Arc;

use occam_geom::{BatchKey, Content, FamilyName, Surface};
use occam_math::{Exec, Grain, Tally};
use occam_report::{Code, Fault, NamedValue, Reason, Report};
use occam_units::Tolerance;

use crate::section::Section;

/// Two surfaces to intersect, shared, so that many pairs can run as tasks.
pub type SurfacePair = (Arc<dyn Surface>, Arc<dyn Surface>);

/// How much work a method may do on one pair, in the steps it counts: a step of a trace, a leaf of a subdivision, an
/// iteration. Each method asked gets the whole of it.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    /// The steps allowed.
    pub steps: u32,
}

/// That a method ran out of its budget. Only [`Work::spend`] gives one, so a method cannot claim it without having
/// spent its budget.
#[derive(Debug)]
pub struct Spent(());

/// A method's budget as it spends it.
#[derive(Debug)]
pub struct Work {
    left: u32,
}

impl Work {
    /// Spends `steps` of the budget, or, when fewer are left, spends nothing and gives [`Spent`].
    ///
    /// # Errors
    ///
    /// [`Spent`] when fewer than `steps` are left. A method passes it on, and the section keeps what it found.
    pub fn spend(&mut self, steps: u32) -> Result<(), Spent> {
        self.left = self.left.checked_sub(steps).ok_or(Spent(()))?;
        Ok(())
    }
}

/// What a method did with a pair.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Answer {
    /// It does not apply to the pair, and the next method is asked.
    Declined,
    /// It applies, and the section holds what it found.
    Answered,
}

/// One way of intersecting two surfaces, such as the closed forms or the trace.
///
/// [`Methods`] calls [`Method::attempt`] with the pair in canonical order, so a method never sees the same pair both
/// ways round. It reads the surfaces through the frozen seam only: `analytic`, `bounds`, `domain`, `degeneracies`,
/// `closest` and evaluation. It decides through occam-math's predicates, merges their [`Tally`] into the section's, and
/// reports an undecided one in the section's failures rather than guessing.
pub trait Method: Send + Sync {
    /// The method's name, which the section records when it answers.
    fn name(&self) -> &'static str;

    /// Intersects `a` with `b`, or declines.
    ///
    /// A method that does not apply returns [`Answer::Declined`]: its section is dropped, all but its tally, since the
    /// decisions that found it does not apply were made. One that applies adds what it finds to `section` as it finds
    /// it, curves with their images on `a` and `b`, contacts, any coincidence and failures, and returns
    /// [`Answer::Answered`]. It leaves `section.method` alone. It spends `work` as it goes, and when [`Work::spend`]
    /// gives [`Spent`] it returns that: the section keeps what it holds and records the spent budget.
    ///
    /// # Errors
    ///
    /// [`Spent`], from [`Work::spend`], when the method ran out of its budget.
    fn attempt(
        &self,
        a: &dyn Surface,
        b: &dyn Surface,
        tol: Tolerance,
        work: &mut Work,
        section: &mut Section,
    ) -> Result<Answer, Spent>;
}

/// The methods, in the order they are asked. The order is the only ordered choice there is (S4).
pub struct Methods {
    list: Vec<Box<dyn Method>>,
}

/// What a surface is put in order by: its family, batch key and content (S4). Two surfaces with the same key are the
/// same surface.
fn key(surface: &dyn Surface) -> (FamilyName, BatchKey, Content) {
    let mut content = Content::default();
    surface.content(&mut content);
    (surface.family(), surface.batch_key(), content)
}

/// A report of a section that stopped with `found` held, for `reason`.
fn incomplete(fault: Fault, reason: Reason, found: &Section) -> Report {
    let count = |name: &str, n: usize| NamedValue::count(name, u32::try_from(n).unwrap_or(u32::MAX));
    Report::new(Code::Incomplete, fault)
        .with_reason(reason)
        .with_value(count("curves", found.curves.len()))
        .with_value(count("contacts", found.contacts.len()))
}

/// The report of a method that ran out of `budget`. It offers no remedy: an agent cannot set a budget, and `booleans`
/// adds one it can follow when it carries the report to agents.
fn spent(budget: Budget, found: &Section) -> Report {
    incomplete(Fault::NumericallySensitive, Reason::BudgetSpent, found)
        .with_value(NamedValue::count("budget", budget.steps))
}

impl Methods {
    /// The methods `list`, asked in its order.
    #[must_use]
    pub fn new(list: Vec<Box<dyn Method>>) -> Self {
        Self { list }
    }

    /// The section of `a` and `b`. The pair is put in canonical order first, a pair with equal keys as given, and the
    /// section is mapped back, so `section(b, a)` is `section(a, b)` with its images swapped (S4). A pair no method
    /// applies to gives `intersect.incomplete` for `pair-not-supported`, with the decisions made in finding that out, and
    /// a method that runs out of `budget` gives it for `budget-spent`, beside what the method found.
    #[must_use]
    pub fn section(&self, a: &dyn Surface, b: &dyn Surface, tol: Tolerance, budget: Budget) -> Section {
        if key(a) > key(b) { self.ordered(b, a, tol, budget).swapped() } else { self.ordered(a, b, tol, budget) }
    }

    /// The section of a pair already in canonical order: the first method that applies answers, each with the whole
    /// budget.
    fn ordered(&self, a: &dyn Surface, b: &dyn Surface, tol: Tolerance, budget: Budget) -> Section {
        let mut tally = Tally::default();
        for method in &self.list {
            let (mut work, mut found) = (Work { left: budget.steps }, Section::default());
            let answer = method.attempt(a, b, tol, &mut work, &mut found);
            tally = tally.merged(found.tally);
            let failure = match answer {
                Ok(Answer::Declined) => continue,
                Ok(Answer::Answered) => None,
                Err(Spent(())) => Some(spent(budget, &found)),
            };
            found.failures.extend(failure);
            return Section { method: Some(method.name()), tally, ..found };
        }
        let none = Section::default();
        let failure = incomplete(Fault::NotImplemented, Reason::PairNotSupported, &none);
        Section { failures: vec![failure], tally, ..none }
    }
}

/// The section of every pair, one pair a task through `exec`, one result each in pair order (P5, L6). The same pairs
/// give the same bits on any number of threads.
#[must_use]
pub fn sections(
    exec: &Exec,
    methods: Arc<Methods>,
    pairs: Arc<[SurfacePair]>,
    tol: Tolerance,
    budget: Budget,
) -> Vec<Section> {
    let count = pairs.len();
    let found = exec.map(count, Grain::Each, move |index| {
        pairs.get(index).map(|(a, b)| methods.section(a.as_ref(), b.as_ref(), tol, budget))
    });
    found.into_iter().flatten().collect()
}
