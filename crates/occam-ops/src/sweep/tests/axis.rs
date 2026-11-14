//! Source: original
//! A revolution's profile held to its axis exactly, piece by piece: where a line or an arc reaches furthest along a
//! direction, at an end or where an arc turns, whatever part of its circle it covers; a disc across a tilted axis by a
//! tenth, and one beside the plane of its axis, refused where they reach furthest, which no fixed points along a circle
//! need find; and a wire along the axis, which moves nothing. A face with holes turned a whole turn is a solid with a
//! void for each, a shell each; a profile touching the axis where what it makes would pinch is refused, at a vertex of
//! a wire or of a face turned a whole turn, or inside an arc; and a piece on the axis sweeps nothing and leaves nothing.

use std::collections::BTreeSet;

use occam_report::{Code, EntityKind, EntityRef, Input, Report};
use occam_topo::{BodyKind, Edge, Id, Shell, Snapshot, Vertex};
use occam_units::Vec3;
use serde_json::{Value, json};

use super::{about_z, around_z, kind, outward};
use crate::construct::space::Bent;
use crate::construct::tests::{built, number, refused};
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

/// Each shell of `model` with the genus its own counts give, `V − E + 2F − L = 2 − 2g`, in id order.
fn genera(model: &Snapshot) -> Vec<f64> {
    let whole = |n: usize| f64::from(u32::try_from(n).unwrap());
    let genus = |shell: Id<Shell>| {
        let faces = model.view(shell).unwrap().faces();
        let loops: usize = faces.iter().map(|&f| model.view(f).unwrap().loops().len()).sum();
        let edges: BTreeSet<Id<Edge>> = faces.iter().flat_map(|&f| model.view(f).unwrap().edges()).collect();
        let ends = edges.iter().flat_map(|&e| <[Id<Vertex>; 2]>::from(model.view(e).unwrap().ends()));
        let vertices: BTreeSet<Id<Vertex>> = ends.collect();
        (2.0 - (whole(vertices.len()) - whole(edges.len()) + 2.0 * whole(faces.len()) - whole(loops))) / 2.0
    };
    model.ids::<Shell>().into_iter().map(genus).collect()
}

/// The edges of `model` that bound no face, and its points that end no edge.
fn bare(model: &Snapshot) -> (Vec<Id<Edge>>, Vec<Id<Vertex>>) {
    let edges = model.ids::<Edge>().into_iter().filter(|&e| model.view(e).unwrap().faces().is_empty());
    let points = model.ids::<Vertex>().into_iter().filter(|&v| model.view(v).unwrap().edges().is_empty());
    (edges.collect(), points.collect())
}

/// The wire labelled `w` through `points`, closed or not.
fn wire(points: &Value, closed: bool) -> Value {
    json!({"op": "wire", "label": "w", "points": points, "closed": closed})
}

/// The face labelled `f` whose outer wire is `w` and whose holes are `holes`.
fn face(holes: &[&str]) -> Value {
    json!({"op": "face", "label": "f", "outer": "w", "holes": holes})
}

/// The face of the closed wire through `points`, turned through `angle` degrees about the `z` axis.
fn turned_face(points: &Value, angle: f64) -> Value {
    json!([wire(points, true), face(&[]), about_z("f", angle)])
}

/// The circle labelled `label` of radius 0.5 about `(3, 0, z)` in the plane y = 0.
fn hole(label: &str, z: f64) -> Value {
    json!({"op": "wire", "label": label, "edges": [{"circle": {"centre": [3, 0, z], "radius": 0.5, "normal": [0, 1, 0]}}]})
}

/// Whether the radius and height `(r, z)` are outside each hole about `(3, 0, z)` for `z` in `heights`.
fn clear_of(heights: &[f64], (r, z): (f64, f64)) -> bool {
    heights.iter().all(|h| (r - 3.0) * (r - 3.0) + (z - h) * (z - h) > 0.25)
}

#[test]
fn a_face_with_holes_turned_a_whole_turn_is_a_solid_with_a_void_for_each_hole() {
    // A 2 by 2 square from x = 2 to 4 with a hole of radius 0.5 at its middle: a ring round a ring of a void.
    let square = wire(&json!([[2, 0, 0], [4, 0, 0], [4, 0, 2], [2, 0, 2]]), true);
    let (model, _) = built(json!([square, hole("h", 1.0), face(&["h"]), about_z("f", 360.0)]));
    assert_eq!((kind(&model), genera(&model)), (BodyKind::Solid, vec![1.0, 1.0]));
    outward(&model, |p| {
        let (r, z) = around_z(p);
        r > 2.0 && r < 4.0 && z > 0.0 && z < 2.0 && clear_of(&[1.0], (r, z))
    });
    // From the axis out to x = 4 and 4 high, with two holes: a cylinder with two voids.
    let tall = wire(&json!([[0, 0, 0], [4, 0, 0], [4, 0, 4], [0, 0, 4]]), true);
    let (model, _) = built(json!([tall, hole("h", 1.0), hole("k", 3.0), face(&["h", "k"]), about_z("f", 360.0)]));
    assert_eq!((kind(&model), genera(&model)), (BodyKind::Solid, vec![0.0, 1.0, 1.0]));
    outward(&model, |p| {
        let (r, z) = around_z(p);
        r < 4.0 && z > 0.0 && z < 4.0 && clear_of(&[1.0, 3.0], (r, z))
    });
}

/// That the request `steps`, a revolution last, is refused for touching the axis at `at`.
fn pinched(steps: Value, at: Vec3) {
    let last = u32::try_from(steps.as_array().unwrap().len()).unwrap().saturating_sub(1);
    let report = refused(steps);
    assert_eq!(report.code, Code::ProfileTouchesAxis, "{report:?}");
    assert_eq!(report.input, Some(Input::at(&format!("steps[{last}].axis")).in_step(last, Some("revolve"))));
    assert!((point(&report) - at).norm() < 1e-9, "{report:?}");
}

#[test]
fn a_face_touching_the_axis_at_a_vertex_turns_part_of_a_turn_but_not_a_whole_one() {
    // A triangle with a corner on the axis: turned whole, its two cones would meet at that corner alone; turned less,
    // its ends close round it.
    let origin = Vec3::new(0.0, 0.0, 0.0);
    let triangle = json!([[0, 0, 0], [2, 0, 1], [2, 0, -1]]);
    let (model, _) = built(turned_face(&triangle, 90.0));
    assert_eq!((kind(&model), genera(&model)), (BodyKind::Solid, vec![0.0]));
    pinched(turned_face(&triangle, 360.0), origin);
    // A square on its corner; a disc as two arcs meeting on the axis; and a whole circle, cut halfway round, there.
    pinched(turned_face(&json!([[0, 0, 0], [1, 0, -1], [2, 0, 0], [1, 0, 1]]), 360.0), origin);
    let arcs = json!({"op": "wire", "label": "w", "edges": [{"arc": [[0, 0, 0], [1, 0, 1], [2, 0, 0]]}, {"arc": [[2, 0, 0], [1, 0, -1], [0, 0, 0]]}]});
    pinched(json!([arcs, face(&[]), about_z("f", 360.0)]), origin);
    let circle = json!({"op": "wire", "label": "w", "edges": [{"circle": {"centre": [1, 0, 0], "radius": 1, "normal": [0, 1, 0]}}]});
    let (model, _) = built(json!([circle, face(&[]), about_z("f", 90.0)]));
    assert_eq!(genera(&model), [0.0]);
    pinched(json!([circle, face(&[]), about_z("f", 360.0)]), origin);
}

#[test]
fn an_arc_touching_the_axis_between_its_ends_is_refused_and_split_there_is_turned() {
    // A disc about (1, 0, 0) of radius 1 as two arcs, the first through (0, 0, 0) on the axis, inside it.
    let touching = json!({"op": "wire", "label": "w", "edges": [{"arc": [[1, 0, 1], [0, 0, 0], [1, 0, -1]]}, {"arc": [[1, 0, -1], [2, 0, 0], [1, 0, 1]]}]});
    for angle in [90.0, 360.0] {
        pinched(json!([touching, face(&[]), about_z("f", angle)]), Vec3::new(0.0, 0.0, 0.0));
    }
    // Split there, as the report says, it is a vertex of a face turned less than a whole turn.
    let (a, b) = (1.0 - core::f64::consts::FRAC_1_SQRT_2, core::f64::consts::FRAC_1_SQRT_2);
    let split = json!({"op": "wire", "label": "w", "edges": [
        {"arc": [[1, 0, 1], [a, 0, b], [0, 0, 0]]},
        {"arc": [[0, 0, 0], [a, 0, -b], [1, 0, -1]]},
        {"arc": [[1, 0, -1], [2, 0, 0], [1, 0, 1]]}
    ]});
    let (model, _) = built(json!([split, face(&[]), about_z("f", 90.0)]));
    assert_eq!((kind(&model), genera(&model)), (BodyKind::Solid, vec![0.0]));
}

#[test]
fn a_wire_touching_the_axis_between_two_pieces_is_refused_but_one_ending_on_it_is_turned() {
    // A zigzag back to the axis at (0, 0, 2): the sheets either side would meet at that point alone.
    let zigzag = wire(&json!([[0, 0, 0], [1, 0, 1], [0, 0, 2], [1, 0, 3]]), false);
    for angle in [90.0, 360.0] {
        pinched(json!([zigzag, about_z("w", angle)]), Vec3::new(0.0, 0.0, 2.0));
    }
    // A closed wire with a corner on the axis likewise, whatever the angle.
    pinched(
        json!([wire(&json!([[0, 0, 0], [2, 0, 1], [2, 0, -1]]), true), about_z("w", 90.0)]),
        Vec3::new(0.0, 0.0, 0.0),
    );
    // A wire ending on the axis comes to a point there.
    for angle in [90.0, 360.0] {
        let (model, _) = built(json!([wire(&json!([[0, 0, 0], [1, 0, 1]]), false), about_z("w", angle)]));
        assert_eq!((kind(&model), model.ids::<Shell>().len(), bare(&model)), (BodyKind::Sheet, 1, (vec![], vec![])));
    }
}

#[test]
fn a_piece_on_the_axis_sweeps_nothing_and_leaves_no_bare_edge_or_point() {
    // A wire out from the axis, along it and out again: two discs apart, a shell each.
    let across = wire(&json!([[1, 0, 0], [0, 0, 0], [0, 0, 1], [1, 0, 1]]), false);
    // A can's outline closed along the axis, and a wire ending along it: one sheet each.
    let can = wire(&json!([[0, 0, 0], [1, 0, 0], [1, 0, 1], [0, 0, 1]]), true);
    let ending = wire(&json!([[1, 0, 0], [0, 0, 0], [0, 0, 1]]), false);
    for angle in [90.0, 360.0] {
        for (outline, shells) in [(&across, 2), (&can, 1), (&ending, 1)] {
            let (model, _) = built(json!([outline, about_z("w", angle)]));
            assert_eq!((model.ids::<Shell>().len(), bare(&model)), (shells, (vec![], vec![])), "{outline} {angle}");
        }
        // A face with two sides along the axis, which a whole turn keeps neither of, nor the corner between them.
        let (model, _) = built(turned_face(&json!([[0, 0, 0], [1, 0, 1], [0, 0, 2], [0, 0, 1]]), angle));
        assert_eq!((genera(&model), bare(&model)), (vec![0.0], (vec![], vec![])), "{angle}");
    }
}
