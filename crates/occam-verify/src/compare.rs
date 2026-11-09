//! Source: original
//! Comparison is geometric, with a tolerance per measure, and never by topology or bits (L12). Entity counts are not here at
//! all, so they cannot be compared by accident.

/// How close a number must be: within `rel` of the expected value's size, or within `abs`, whichever is larger.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Closeness {
    /// As a fraction of the expected value.
    pub rel: f64,
    /// As an amount.
    pub abs: f64,
}

/// A box, as lowest and highest corners, in millimetres.
pub type Bounds = ([f64; 3], [f64; 3]);

/// What is compared. `bbox` is `None` for an empty result.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Measured {
    /// The volume in cubic millimetres.
    pub volume: f64,
    /// The area in square millimetres.
    pub area: f64,
    /// The bounding box.
    pub bbox: Option<Bounds>,
    /// The Euler characteristic, `V - E + 2F - L`.
    pub euler: i64,
    /// Whether the checker accepts the shape.
    pub valid: bool,
    /// Whether the boundary has no free edge.
    pub closed: bool,
}

/// One thing that can be compared.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub enum Measure {
    /// The volume.
    Volume,
    /// The area.
    Area,
    /// The bounding box.
    Bbox,
    /// The Euler characteristic.
    Euler,
    /// The checker's verdict.
    Valid,
    /// Whether the boundary is closed.
    Closed,
}

/// A measure on which occam and the oracle differ by more than the tolerance.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub struct Disagreement {
    /// Which measure.
    pub measure: Measure,
}

/// The tolerance for each measure.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Tolerances {
    /// For the volume.
    pub volume: Closeness,
    /// For the area.
    pub area: Closeness,
    /// For each coordinate of the bounding box. The oracle's box is not always tight, so `tools/oracle/README.md` asks for
    /// at least 1e-6.
    pub bbox: f64,
}

impl Closeness {
    /// Whether `actual` is close enough to `expected`. Never, if either is not a number.
    #[must_use]
    pub fn accepts(self, actual: f64, expected: f64) -> bool {
        let difference = (actual - expected).abs();
        difference <= self.rel * expected.abs() || difference <= self.abs
    }
}

fn same_box(ours: Option<Bounds>, oracle: Option<Bounds>, tolerance: f64) -> bool {
    let close = Closeness { rel: 0.0, abs: tolerance };
    let corner = |a: [f64; 3], b: [f64; 3]| a.iter().zip(&b).all(|(x, y)| close.accepts(*x, *y));
    match (ours, oracle) {
        (None, None) => true,
        (Some((low, high)), Some((their_low, their_high))) => corner(low, their_low) && corner(high, their_high),
        _ => false,
    }
}

/// Every measure on which `ours` and `oracle` differ by more than `tolerances`, in a fixed order.
#[must_use]
pub fn compare(ours: &Measured, oracle: &Measured, tolerances: Tolerances) -> Vec<Disagreement> {
    let checks = [
        (Measure::Volume, tolerances.volume.accepts(ours.volume, oracle.volume)),
        (Measure::Area, tolerances.area.accepts(ours.area, oracle.area)),
        (Measure::Bbox, same_box(ours.bbox, oracle.bbox, tolerances.bbox)),
        (Measure::Euler, ours.euler == oracle.euler),
        (Measure::Valid, ours.valid == oracle.valid),
        (Measure::Closed, ours.closed == oracle.closed),
    ];
    checks.into_iter().filter(|(_, agrees)| !agrees).map(|(measure, _)| Disagreement { measure }).collect()
}

#[cfg(test)]
mod tests {
    use super::*;

    const RELATIVE: Closeness = Closeness { rel: 1e-6, abs: 0.0 };
    const TOLERANCES: Tolerances = Tolerances { volume: RELATIVE, area: RELATIVE, bbox: 1e-6 };

    fn shape() -> Measured {
        Measured {
            volume: 1000.0,
            area: 600.0,
            bbox: Some(([0.0, 0.0, 0.0], [10.0, 10.0, 10.0])),
            euler: 2,
            valid: true,
            closed: true,
        }
    }

    fn measures(ours: &Measured, oracle: &Measured) -> Vec<Measure> {
        compare(ours, oracle, TOLERANCES).iter().map(|found| found.measure).collect()
    }

    #[test]
    fn closeness_takes_the_larger_of_its_two_allowances() {
        let both = Closeness { rel: 0.01, abs: 2.0 };
        assert!(both.accepts(102.0, 100.0), "the amount allows it");
        assert!(!both.accepts(102.1, 100.0));
        let relative = Closeness { rel: 0.01, abs: 0.5 };
        assert!(relative.accepts(101.0, 100.0), "the fraction allows it");
        assert!(!relative.accepts(101.1, 100.0));
    }

    #[test]
    fn closeness_works_on_either_side_and_uses_the_size_of_the_expected_value() {
        let close = Closeness { rel: 0.01, abs: 0.0 };
        assert!(close.accepts(99.0, 100.0));
        assert!(!close.accepts(98.9, 100.0));
        assert!(close.accepts(-99.0, -100.0), "the size of a negative value");
        assert!(!close.accepts(-98.9, -100.0));
    }

    #[test]
    fn closeness_refuses_what_is_not_a_number() {
        let close = Closeness { rel: 1.0, abs: 1.0 };
        assert!(!close.accepts(f64::NAN, 1.0));
        assert!(!close.accepts(1.0, f64::NAN));
    }

    #[test]
    fn a_shape_agrees_with_itself() {
        assert_eq!(measures(&shape(), &shape()), []);
    }

    #[test]
    fn differences_within_tolerance_are_not_disagreements() {
        let near = Measured {
            volume: 1000.0005,
            area: 600.0003,
            bbox: Some(([5e-7, 0.0, 0.0], [10.0, 10.0, 10.0 - 5e-7])),
            ..shape()
        };
        assert_eq!(measures(&near, &shape()), []);
    }

    #[test]
    fn each_measure_is_compared() {
        let cases = [
            (Measured { volume: 1000.1, ..shape() }, Measure::Volume),
            (Measured { area: 600.1, ..shape() }, Measure::Area),
            (Measured { euler: 0, ..shape() }, Measure::Euler),
            (Measured { valid: false, ..shape() }, Measure::Valid),
            (Measured { closed: false, ..shape() }, Measure::Closed),
        ];
        for (ours, measure) in cases {
            assert_eq!(measures(&ours, &shape()), [measure]);
        }
    }

    #[test]
    fn the_volume_and_area_each_use_their_own_tolerance() {
        let loose = Closeness { rel: 1.0, abs: 0.0 };
        let off = Measured { volume: 1100.0, area: 660.0, ..shape() };
        let volume_only = Tolerances { volume: loose, ..TOLERANCES };
        let found: Vec<Measure> = compare(&off, &shape(), volume_only).iter().map(|d| d.measure).collect();
        assert_eq!(found, [Measure::Area]);
        let area_only = Tolerances { area: loose, ..TOLERANCES };
        let found: Vec<Measure> = compare(&off, &shape(), area_only).iter().map(|d| d.measure).collect();
        assert_eq!(found, [Measure::Volume]);
    }

    #[test]
    fn every_coordinate_of_the_box_counts() {
        let reference = ([0.0, 0.0, 0.0], [10.0, 10.0, 10.0]);
        for axis in 0..3 {
            let mut low = reference;
            *low.0.get_mut(axis).unwrap() = 1e-3;
            assert_eq!(measures(&Measured { bbox: Some(low), ..shape() }, &shape()), [Measure::Bbox], "low {axis}");
            let mut high = reference;
            *high.1.get_mut(axis).unwrap() = 10.001;
            assert_eq!(measures(&Measured { bbox: Some(high), ..shape() }, &shape()), [Measure::Bbox], "high {axis}");
        }
    }

    #[test]
    fn the_box_tolerance_is_the_one_given() {
        let off = Measured { bbox: Some(([0.0, 0.0, 0.0], [10.5, 10.0, 10.0])), ..shape() };
        assert_eq!(compare(&off, &shape(), Tolerances { bbox: 1.0, ..TOLERANCES }), []);
        assert_eq!(compare(&off, &shape(), Tolerances { bbox: 0.1, ..TOLERANCES }).len(), 1);
    }

    #[test]
    fn two_empty_results_agree_and_one_empty_result_does_not() {
        let empty = Measured { bbox: None, ..shape() };
        assert_eq!(measures(&empty, &empty), []);
        assert_eq!(measures(&empty, &shape()), [Measure::Bbox]);
        assert_eq!(measures(&shape(), &empty), [Measure::Bbox]);
    }

    #[test]
    fn every_disagreement_is_listed_in_a_fixed_order() {
        let ours = Measured { volume: 1.0, area: 1.0, bbox: None, euler: 0, valid: false, closed: false };
        let want = [Measure::Volume, Measure::Area, Measure::Bbox, Measure::Euler, Measure::Valid, Measure::Closed];
        assert_eq!(measures(&ours, &shape()), want);
    }
}
