//! `occam-intersect`: P5's answer through S4's ordered method list. What was found is kept beside what stopped it (L5),
//! because marching reported none of its 13 wrong answers (P5).

use std::sync::Arc;

use occam_report::Report;
use occam_units::{Length, Point, Tolerance, UvPoint, World};

use occam_geom::{Curve, ParamCurve, Surface};
use occam_math::{Exec, Grain, Tally};

/// A section curve, with its image on both surfaces (S1 item 7), its fit (S1 item 10) carried by the curves themselves.
#[derive(Clone)]
pub struct SectionCurve {
    pub curve: Arc<dyn Curve>,
    pub on_a: Arc<dyn ParamCurve>,
    pub on_b: Arc<dyn ParamCurve>,
}

#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Contact {
    Point {
        at: Point<World>,
        radius: Length,
        on_a: UvPoint,
        on_b: UvPoint,
    },
    /// A contact, or a loop smaller than `bound`, that this tolerance cannot tell apart (O24). Not a tangency.
    BelowResolution {
        at: Point<World>,
        bound: Length,
    },
}

/// Where the faces meet on one surface (O25). Found from the analytic presentation where there is one.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coincidence {
    Coplanar,
    Coaxial,
}

/// How much work a method may spend before it reports `budget-spent` with what it found.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Budget {
    pub steps: u32,
}

#[derive(Clone)]
pub struct Section {
    pub method: &'static str,
    pub curves: Vec<SectionCurve>,
    pub contacts: Vec<Contact>,
    pub coincident: Option<Coincidence>,
    /// A stalled trace, a spent budget, an unresolved leaf or an undecided predicate. What was found stays above.
    pub failures: Vec<Report>,
    pub tally: Tally,
}

/// One way of intersecting a pair; `None` means it does not apply. The list's order is the only ordered choice (S4).
pub trait PairMethod: Send + Sync {
    fn name(&self) -> &'static str;
    fn attempt(&self, a: &dyn Surface, b: &dyn Surface, tol: Tolerance, budget: Budget) -> Option<Section>;
}

pub struct Methods {
    list: Vec<Box<dyn PairMethod>>,
}

impl Methods {
    /// Closed forms through `analytic()` (P5: 58 of 58 within 1e-12), then the hybrid through evaluation and `bounds`:
    /// subdivision for start points, scalar marching, traces in rounds merged by index (P5: 104 of 110, 2 silent). The
    /// hybrid needs no B-spline form, so offsets and mesh-backed surfaces reach it too.
    #[must_use]
    pub fn standard() -> Self {
        unimplemented!()
    }

    /// One pair, put first in canonical order by family, batch key and content, so argument order cannot change the
    /// answer (S4).
    #[must_use]
    pub fn section(&self, a: &dyn Surface, b: &dyn Surface, tol: Tolerance, budget: Budget) -> Section {
        unimplemented!()
    }
}

/// Many pairs, one result each in pair order (L6). Threads across pairs, one pair a task (P5).
#[must_use]
pub fn sections(
    exec: &Exec,
    methods: Arc<Methods>,
    pairs: Arc<[(Arc<dyn Surface>, Arc<dyn Surface>)]>,
    tol: Tolerance,
    budget: Budget,
) -> Vec<Section> {
    let count = pairs.len();
    exec.map(count, Grain::Each, move |index| match pairs.get(index) {
        Some((a, b)) => methods.section(a.as_ref(), b.as_ref(), tol, budget),
        None => unimplemented!(),
    })
}

/// An edge against a surface, with the edge's conservative box tested first (P8: 66 to 86% of scans removed).
pub struct Hits {
    pub hits: Vec<(f64, UvPoint)>,
    pub failures: Vec<Report>,
}

#[must_use]
pub fn hits(curve: &dyn Curve, range: (f64, f64), surface: &dyn Surface, tol: Tolerance) -> Hits {
    unimplemented!()
}
