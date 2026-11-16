//! Source: original
//! P9's parts drawn and built: their counts, two of them recorded by digest, and each meshed whole.

use core::f64::consts::FRAC_PI_4;

use occam_mesh::{Limits, mesh};

use super::parts::Frame;
use super::*;

#[test]
fn p9s_targets_draw_p9s_parts() {
    let counted = |target| parts(target, P9_SEED).iter().map(|part| part.shape.p9_faces()).sum::<usize>();
    assert_eq!(P9_TARGETS.map(counted), [1002, 10_001]);
}

/// The mesh of `model` at P9's angle and `deflection`, which must be whole: every face meshed and nothing open.
fn whole(model: &Snapshot, deflection: f64) -> occam_mesh::Mesh {
    let limits = Limits::new(Length::from_millimetres(deflection), Angle::from_radians(0.35)).unwrap();
    let meshed = mesh(&Exec::serial(), model, limits);
    assert!(meshed.refused.is_empty(), "{:?}", meshed.refused);
    assert!(meshed.open.is_empty(), "{:?}", meshed.open);
    meshed
}

#[test]
fn the_first_of_p9s_parts_are_a_plate_a_panel_and_another_plate_as_recorded() {
    let drawn = parts(20, P9_SEED);
    let kinds: Vec<bool> = drawn.iter().map(|part| matches!(part.shape, Shape::Panel { .. })).collect();
    assert_eq!(kinds, [false, true, false]);
    let model = synthetic(&Exec::serial(), 20, P9_SEED).unwrap();
    // Each plate's faces, and the panel's eight: a top, a bottom, four walls and its hole's wall in two halves.
    assert_eq!(face_count(&model), 28);
    assert_eq!(format!("{:016x}", model.digest()), "3b83f6d69d93df11");
    let t = std::time::Instant::now();
    whole(&model, 0.2);
    eprintln!("DIAG meshed {:?}", t.elapsed());
}

#[test]
fn a_rational_quilt_is_a_closed_solid_of_its_patches() {
    // A quilt of rational quadratic patches, the smallest across that closes; P9's cubic quilts take the same path but
    // for their weights. The commit's checks of spline faces make a larger one slow in a debug build.
    let frame = Frame { origin: [10.0, 20.0, 0.0], turn: FRAC_PI_4 };
    let quilt = Part { frame, shape: Shape::Quilt { across: (3, 3), radius: 2.0, degree: 2, seed: 11 } };
    let model = written(&Exec::serial(), &Snapshot::empty(tolerance().unwrap()), &[&quilt]).unwrap();
    assert_eq!(face_count(&model), 9);
    assert_eq!(format!("{:016x}", model.digest()), "2feaa467409deb9d");
    whole(&model, 0.2);
}
