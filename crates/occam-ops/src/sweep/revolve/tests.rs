//! Source: original
//! Where a revolution finds its profile's farthest point and holds the rest to the plane through it and the axis: a
//! profile beside that plane is refused on whichever side of it it lies, and one whose lines begin before their start
//! is held by its own points, not by points on those lines beyond their ends.

use occam_report::{Code, Input, Value};
use serde_json::json;

use crate::construct::tests::{number, refused};
use crate::sweep::tests::{about_z, counts, genus, sorted, with_lines_begun_before};

#[test]
fn a_profile_beside_the_plane_of_its_axis_is_refused_on_either_side_of_it() {
    // Out at x = 5, its farthest from the z axis at (5, 0, 0), in along y = 0, then 2 off that plane, either way.
    for side in [-2.0, 2.0] {
        let wire = json!({"op": "wire", "label": "w", "points": [[5, 0, 0], [5, 0, 2], [1, 0, 2], [1, side, 2]]});
        let report = refused(json!([wire, about_z("w", 90.0)]));
        assert_eq!(report.code, Code::AxisOffPlane, "{side}: {report:?}");
        assert_eq!(report.input, Some(Input::at("steps[1].axis").in_step(1, Some("revolve"))));
        assert_eq!(report.value("point").map(|v| v.value), Some(Value::Point([1.0, side, 2.0])), "{side}");
        assert!((number(&report, "distance") - 2.0).abs() < 1e-12, "{side}: {report:?}");
    }
}

#[test]
fn a_profile_whose_lines_begin_before_their_start_is_held_to_its_axis_by_its_own_points() {
    // A triangle with a side on the axis, its last side running in to the axis along x. Each line begins 15 before its
    // start: that last line, carried on as far again beyond its end, is 30 across the axis, farther than the profile.
    let triangle = json!([
        {"op": "wire", "label": "t", "points": [[0, 0, 2], [0, 0, 12], [4, 0, 2]], "closed": true},
        {"op": "face", "label": "tri", "outer": "t"}
    ]);
    let model = with_lines_begun_before(&triangle, 15.0, about_z("tri", 90.0)).unwrap();
    assert_eq!((counts(&model), genus(&model)), ([4, 6, 4, 4], 0.0));
    assert_eq!(sorted(&model), ["cone", "plane", "plane", "plane"]);
}
