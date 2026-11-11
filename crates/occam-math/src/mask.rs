//! Source: original
//! [`Mask`]: which lanes a comparison held in.

use std::fmt::Debug;

/// Which lanes a comparison held in: a `bool` for `f64`, and a [`LaneMask`] for [`Lanes`](crate::Lanes).
pub trait Mask: Copy + Debug + Send + Sync + 'static {
    /// Whether it held in at least one lane.
    fn any(self) -> bool;

    /// Whether it held in every lane.
    fn all(self) -> bool;
}

impl Mask for bool {
    #[inline]
    fn any(self) -> bool {
        self
    }

    #[inline]
    fn all(self) -> bool {
        self
    }
}

/// One `bool` a lane, lane 0 first.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct LaneMask<const W: usize>([bool; W]);

impl<const W: usize> LaneMask<W> {
    /// The lanes, lane 0 first.
    #[must_use]
    pub const fn new(lanes: [bool; W]) -> Self {
        Self(lanes)
    }

    /// The lanes as an array, lane 0 first.
    #[must_use]
    pub const fn to_array(self) -> [bool; W] {
        self.0
    }
}

impl<const W: usize> Mask for LaneMask<W> {
    #[inline]
    fn any(self) -> bool {
        self.0.iter().any(|&lane| lane)
    }

    #[inline]
    fn all(self) -> bool {
        self.0.iter().all(|&lane| lane)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn a_bool_is_a_mask_of_one_lane() {
        assert!(true.any() && true.all());
        assert!(!false.any() && !false.all());
    }

    #[test]
    fn any_needs_one_lane_and_all_needs_every_lane() {
        let some = LaneMask::new([false, true, false]);
        assert!(some.any() && !some.all());
        let every = LaneMask::new([true; 3]);
        assert!(every.any() && every.all());
        let none = LaneMask::new([false; 3]);
        assert!(!none.any() && !none.all());
        assert_eq!(some.to_array(), [false, true, false]);
    }
}
