//! Source: original
//! What a family that holds a surface shares, as an offset and a trimmed surface do: the held surface's family and
//! numbers inside its content, and its shape's values inside its own.

use occam_report::NamedValue;

use crate::identity::Content;
use crate::surface::Parametric;

/// Adds what defines `surface` to a holding family's content: its family's name, then its numbers.
pub(crate) fn push_surface<S: Parametric>(out: &mut Content, surface: &S) {
    out.push_family(surface.family());
    surface.content(out);
}

/// `surface`'s shape's values, each named with `base-` before its own name, after the holding family's own `values`.
pub(crate) fn with_surface<S: Parametric>(mut values: Vec<NamedValue>, surface: &S) -> Vec<NamedValue> {
    values.extend(surface.shape().values.into_iter().map(|v| NamedValue { name: format!("base-{}", v.name), ..v }));
    values
}

#[cfg(test)]
mod tests {
    use occam_units::Placement;

    use super::*;
    use crate::{FamilyName, Plane};

    #[test]
    fn a_held_surface_adds_its_name_numbers_and_values() {
        let plane = Plane::placed(&Placement::identity());
        let (mut out, mut alone) = (Content::default(), Content::default());
        out.start(FamilyName("holding"));
        push_surface(&mut out, &plane);
        alone.push_family(FamilyName("plane"));
        plane.content(&mut alone);
        assert!(out.words().ends_with(alone.words()) && out.words().len() == 8 + alone.words().len());
        let values = with_surface(vec![NamedValue::length("a", 1.0)], &plane);
        assert_eq!((values[0].name.as_str(), values[1].name.as_str(), values.len()), ("a", "base-origin", 4));
    }
}
