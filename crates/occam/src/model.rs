//! Source: original
//! A model. For now it is only the tolerance it is built to.

use occam_units::Tolerance;

/// What an operation works on.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Model {
    tolerance: Tolerance,
}

impl Model {
    /// A model built to `tolerance`.
    #[must_use]
    pub const fn new(tolerance: Tolerance) -> Self {
        Self { tolerance }
    }

    /// The tolerance it is built to. No operation on it can be asked for anything finer.
    #[must_use]
    pub const fn tolerance(self) -> Tolerance {
        self.tolerance
    }
}
