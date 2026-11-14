//! Source: original
//! A revolution's profile held to its axis exactly, piece by piece: where a line or an arc reaches furthest along a
//! direction, at an end or where an arc turns, whatever part of its circle it covers; a disc across a tilted axis by a
//! tenth, and one beside the plane of its axis, refused where they reach furthest, which no fixed points along a circle
//! need find; and a wire along the axis, which moves nothing.

use occam_report::{Code, EntityKind, EntityRef, Input, Report};
use occam_units::Vec3;
use serde_json::json;

use super::about_z;
use crate::construct::space::Bent;
use crate::construct::tests::{number, refused};
use crate::sweep::profile::Piece;
use crate::sweep::revolve::furthest_on;

/// The value `point` of `report`.
fn point(report: &Report) -> Vec3 {
    let Some(occam_report::Value::Point(p)) = report.value("point").map(|v| v.value) else { panic!("{report:?}") };
    Vec3::new(p[0], p[1], p[2])
}

#[test]
fn an_arc_reaches_furthest_where_it_turns_or_at_an_end_whatever_part_of_its_circle_it_covers() {
    let x = Vec3::new(1.0, 0.0, 0.0);
    let circle = Bent::Circle { centre: Vec3::new(0.0, 0.0, 2.0), x, y: Vec3::new(0.0, 1.0, 0.0), radius: 3.0 };
    let from = EntityRef::new(EntityKind::Edge, 0, 1);
    let on = |range: (f64, f64), along: Vec3| {
        furthest_on(&Piece { bent: circle, range, back: false, ends: (0, 1), from }, Vec3::new(0.0, 0.0, 0.0), along)
    };
    // Along y the circle turns at a quarter turn and every turn from it: once inside each of these ranges.
    for range in [(1.0, 3.0), (7.0, 9.0), (-5.0, -4.0), (0.0, core::f64::consts::TAU)] {
        let (p, reach) = on(range, Vec3::new(0.0, 1.0, 0.0));
        assert!((p - Vec3::new(0.0, 3.0, 2.0)).norm() < 1e-12 && (reach - 3.0).abs() < 1e-12, "{range:?}: {p:?}");
    }
    // Where it does not turn inside the range, the end nearer where it would is furthest.
    for (range, end) in [((2.0, 3.0), 2.0), ((1.0, 1.2), 1.2)] {
        let (p, reach) = on(range, Vec3::new(0.0, 1.0, 0.0));
        assert!((p - circle.point(end)).norm() < 1e-12, "{range:?}: {p:?}");
        assert!((reach - 3.0 * occam_math::sin(end)).abs() < 1e-12, "{range:?}: {reach}");
    }
    let (p, _) = on((8.0, 8.5), x * -1.0);
    assert!((p - circle.point(8.5)).norm() < 1e-12, "{p:?}");
}

#[test]
fn a_disc_across_a_tilted_axis_by_a_tenth_is_refused_where_it_reaches_furthest_beyond_it() {
    // A disc of radius 10 whose centre is 9.9 from the axis, tilted in their plane: the point 0.1 beyond the axis is
    // where neither the circle's one vertex nor any of a fixed number of points along it need fall.
    for tilt in [11.25_f64, 33.75] {
        for side in [1.0, -1.0] {
            let (sin, cos) = occam_math::sin_cos(tilt * core::f64::consts::PI / 180.0);
            let (axis, across) = (Vec3::new(sin, 0.0, cos), Vec3::new(cos, 0.0, -sin));
            let centre = across * 9.9;
            let report = refused(json!([
                {"op": "wire", "label": "c", "edges": [{"circle": {"centre": [centre.x, 0, centre.z], "radius": 10, "normal": [0, side, 0]}}]},
                {"op": "face", "label": "disc", "outer": "c"},
                {"op": "revolve", "profile": "disc", "origin": [0, 0, 0], "axis": [axis.x, 0, axis.z], "angle": 90}
            ]));
            assert_eq!(report.code, Code::ProfileCrossesAxis, "{tilt} {side}");
            assert!((number(&report, "distance") - 0.1).abs() < 1e-9, "{tilt} {side}: {report:?}");
            assert!((point(&report) - across * -0.1).norm() < 1e-9, "{tilt} {side}: {report:?}");
        }
    }
}

#[test]
fn a_profile_off_the_plane_of_its_axis_is_refused_where_an_arc_leaves_it_furthest() {
    // A disc in the plane y = 0.5 beside the z axis, as two arcs from 20 and 200 degrees round it: neither reaches its
    // farthest from the axis, or from the plane through the axis and that point, at an end, a middle or a sixteenth.
    let round = |degrees: f64| {
        let (sin, cos) = occam_math::sin_cos(degrees * core::f64::consts::PI / 180.0);
        json!([20.0 + 5.0 * cos, 0.5, 5.0 * sin])
    };
    let report = refused(json!([
        {"op": "wire", "label": "c", "edges": [{"arc": [round(20.0), round(110.0), round(200.0)]}, {"arc": [round(200.0), round(290.0), round(20.0)]}]},
        {"op": "face", "label": "disc", "outer": "c"},
        about_z("disc", 90.0)
    ]));
    assert_eq!(report.code, Code::AxisOffPlane);
    // The plane holds the axis and the farthest point, (25, 0.5, 0); the point furthest from it is (15, 0.5, 0).
    assert!((number(&report, "distance") - 5.0 / 625.25_f64.sqrt()).abs() < 1e-9, "{report:?}");
    assert!((point(&report) - Vec3::new(15.0, 0.5, 0.0)).norm() < 1e-9, "{report:?}");
}

#[test]
fn a_wire_along_the_axis_moves_nothing_and_is_refused() {
    let report = refused(json!([{"op": "wire", "label": "w", "points": [[0, 0, 1], [0, 0, 4]]}, about_z("w", 360.0)]));
    assert_eq!(report.code, Code::NoLength);
    assert_eq!(report.input, Some(Input::at("steps[1].angle").in_step(1, Some("revolve"))));
    assert!(number(&report, "length").abs() < 1e-300, "{report:?}");
}
