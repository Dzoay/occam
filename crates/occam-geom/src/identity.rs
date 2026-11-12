//! Source: original
//! What a curve or surface is, apart from where it is: its family's name, what can share a sorted run of calls with it,
//! the canonical bits that define it, and the form a description shows (S4, S6).

use occam_math::canonical_bits;
use occam_report::NamedValue;
use occam_units::Vec3;

/// A family's stable name. The canonical order of a pair of surfaces depends on it (S4).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct FamilyName(pub &'static str);

/// What can share a lane loop and a sorted run of calls (L29, as O23 narrows it): one family, at one degree.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub struct BatchKey {
    /// The family.
    pub family: FamilyName,
    /// The degree in each parameter, or zero for a family that has none.
    pub degree: (u16, u16),
}

/// The canonical bits of everything that defines a curve or surface, its family's name first. The order of a pair and
/// a later cache key compare these words, never a pointer (P8). Nothing is rounded, and every NaN is the same word.
#[derive(Clone, Debug, Default, PartialEq, Eq, PartialOrd, Ord)]
pub struct Content(Vec<u64>);

impl Content {
    /// Starts again with `family`'s name: its length, then a word a byte, so that no two families' contents are equal.
    pub fn start(&mut self, family: FamilyName) {
        self.0.clear();
        self.0.push(u64::try_from(family.0.len()).unwrap_or(u64::MAX));
        self.0.extend(family.0.bytes().map(u64::from));
    }

    /// Adds `family`'s name as [`Content::start`] writes it, for a family that holds another, as a swept surface holds
    /// its curve.
    pub fn push_family(&mut self, family: FamilyName) {
        self.0.push(u64::try_from(family.0.len()).unwrap_or(u64::MAX));
        self.0.extend(family.0.bytes().map(u64::from));
    }

    /// Adds a defining number.
    pub fn push(&mut self, x: f64) {
        self.0.push(canonical_bits(x));
    }

    /// Adds three defining numbers, `x` first.
    pub fn push_vec3(&mut self, v: Vec3) {
        for x in [v.x, v.y, v.z] {
            self.push(x);
        }
    }

    /// The words, in order.
    #[must_use]
    pub fn words(&self) -> &[u64] {
        &self.0
    }
}

/// What a curve or surface is in the caller's terms: a form's name and its parameters (S6, O26). The form is text, so a
/// new family edits no list in `occam-report` (B:4).
#[derive(Clone, Debug, PartialEq)]
pub struct Shape {
    /// The form's name, such as `plane`.
    pub form: &'static str,
    /// Its parameters, named.
    pub values: Vec<NamedValue>,
}

#[cfg(test)]
mod tests {
    use occam_math::CANONICAL_NAN;

    use super::*;

    #[test]
    fn content_starts_with_the_family_name_and_holds_canonical_bits() {
        let mut content = Content::default();
        content.push(9.0);
        content.start(FamilyName("ab"));
        content.push(-0.0);
        content.push_vec3(Vec3::new(1.5, f64::NAN, -f64::NAN));
        let words = [2, 97, 98, (-0.0f64).to_bits(), 1.5f64.to_bits(), CANONICAL_NAN, CANONICAL_NAN];
        assert_eq!(content.words(), words);
    }

    #[test]
    fn families_with_a_common_prefix_differ() {
        let started = |name| {
            let mut content = Content::default();
            content.start(FamilyName(name));
            content
        };
        let (short, long) = (started("plane"), started("planes"));
        assert_ne!(short, long);
        assert!(short < long, "the length comes first");
        let mut held = Content::default();
        held.start(FamilyName("ab"));
        held.push_family(FamilyName("c"));
        assert_eq!(held.words(), [2, 97, 98, 1, 99]);
    }
}
