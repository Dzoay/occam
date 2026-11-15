//! Source: original
//! A frame's `x` as it is read: only its part across `z` counts, as a unit direction, and `x` lies along `z` where
//! that part is no longer than the angle tolerance, the sine of the angle between them.

use occam_report::{Code, Input};
use occam_units::Vec3;
use serde_json::{Value, json};

use super::Pose;
use crate::Args;
use crate::request::Request;

/// The frame of a sphere's step at `frame` as it is read, or the codes and places of the reports refusing it.
fn read(frame: &Value) -> Result<Pose, Vec<(Code, Option<Input>)>> {
    let text = json!({"schema": 1, "units": {"length": "mm", "angle": "rad"}, "steps": [
        {"op": "sphere", "radius": 1, "frame": frame}
    ]});
    match Request::parse(&text.to_string()) {
        Ok(request) => match &request.steps[0].args {
            Args::Primitive(args) => Ok(args.pose),
            other => panic!("not a primitive's: {other:?}"),
        },
        Err(reports) => Err(reports.into_iter().map(|r| (r.code, r.input.clone())).collect()),
    }
}

#[test]
fn a_frame_s_x_is_read_as_the_unit_direction_of_its_part_across_its_z() {
    // (3, 0, 4) is (0.6, 0, 0.8) as a unit, and its part across (0, 0, 1) is (0.6, 0, 0).
    let pose = read(&json!({"z": [0, 0, 2], "x": [3, 0, 4]})).unwrap();
    assert!((pose.x - Vec3::new(1.0, 0.0, 0.0)).norm() < 1e-15, "{pose:?}");
    // (1, 0, 0)'s part across (1, 1, 1) / sqrt 3 is (2, -1, -1) / 3, sqrt 6 / 3 long.
    let pose = read(&json!({"z": [1, 1, 1], "x": [5, 0, 0]})).unwrap();
    let across = Vec3::new(2.0, -1.0, -1.0) * (1.0 / 6.0_f64.sqrt());
    assert!((pose.x - across).norm() < 1e-15, "{pose:?}");
}

#[test]
fn an_x_whose_sine_with_z_is_at_most_the_angle_tolerance_lies_along_it() {
    // (1e-9, 0, 1) is a unit direction to the last bit, and its part across (0, 0, 1) is exactly 1e-9 long, the
    // tolerance; twice as far from `z`, it is across it.
    let along = Input::at("steps[0].frame.x").in_step(0, Some("sphere"));
    assert_eq!(read(&json!({"z": [0, 0, 1], "x": [1e-9, 0, 1]})), Err(vec![(Code::InputAxesParallel, Some(along))]));
    let pose = read(&json!({"z": [0, 0, 1], "x": [2e-9, 0, 1]})).unwrap();
    assert!((pose.x - Vec3::new(1.0, 0.0, 0.0)).norm() < 1e-15, "{pose:?}");
}
