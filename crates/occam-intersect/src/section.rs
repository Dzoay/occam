//! Source: original
//! What a section of two surfaces holds (O24, O25, L5): curves with their images on both, contacts, any coincidence,
//! the failures met on the way, and the tally of decisions.

use std::sync::Arc;

use occam_geom::{Curve, ParamCurve};
use occam_math::Tally;
use occam_report::Report;
use occam_units::{Length, Point, UvPoint, World};

/// A curve where the two surfaces meet, with its image in the parameters of each.
#[derive(Clone)]
pub struct SectionCurve {
    /// The curve in space.
    pub curve: Arc<dyn Curve>,
    /// Its image in the parameters of the first surface given.
    pub on_a: Arc<dyn ParamCurve>,
    /// Its image in the parameters of the second surface given.
    pub on_b: Arc<dyn ParamCurve>,
}

/// Where the two surfaces touch with no curve between them, or where branches of their section cross. Every method
/// gives one by one rule (O24): a point only where the decision certifies that nothing the band hides there reaches
/// beyond the band, and else below the resolution, with the bound of what may.
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Contact {
    /// A point where they touch, certified by the decision that found it: no tangency, as at a cone's apex, or one
    /// whose bound is decided within the band.
    Point {
        /// The point in space.
        at: Point<World>,
        /// Its parameters on the first surface given.
        on_a: UvPoint,
        /// Its parameters on the second surface given.
        on_b: UvPoint,
    },
    /// A contact, or a loop no larger than `bound`, that this tolerance cannot tell apart (O24). Not a tangency.
    BelowResolution {
        /// Where it is.
        at: Point<World>,
        /// How large it can be: every point of what hides there lies within this of `at`. Infinite where the surfaces
        /// are tangent along a curve through it, which no relative curvature bounds.
        bound: Length,
    },
}

/// How the two surfaces lie on each other, found only where both present an analytic form (O25).
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Coincidence {
    /// They lie in one plane.
    Coplanar,
    /// They are one surface about one axis, such as two cylinders of one radius on one axis.
    Coaxial,
}

/// What a pair of surfaces gives. A failure never takes the place of what was found: both are kept.
#[derive(Clone, Default)]
pub struct Section {
    /// The method that answered, or `None` when none applied.
    pub method: Option<&'static str>,
    /// Where the surfaces cross, each curve with its images.
    pub curves: Vec<SectionCurve>,
    /// Where they touch.
    pub contacts: Vec<Contact>,
    /// Whether they lie on each other.
    pub coincidence: Option<Coincidence>,
    /// A spent budget, a stalled trace, an unresolved leaf, an undecided predicate: what was found stays above.
    pub failures: Vec<Report>,
    /// The decisions made by every method asked, those that declined included.
    pub tally: Tally,
}

impl Section {
    /// The same section with its two surfaces' roles exchanged: every image on the first becomes the image on the
    /// second. Curves keep their direction, which canonical order alone decides.
    #[must_use]
    pub(crate) fn swapped(mut self) -> Self {
        for curve in &mut self.curves {
            std::mem::swap(&mut curve.on_a, &mut curve.on_b);
        }
        for contact in &mut self.contacts {
            if let Contact::Point { on_a, on_b, .. } = contact {
                std::mem::swap(on_a, on_b);
            }
        }
        self
    }
}
