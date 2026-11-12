//! Source: original
//! Sweeps built from requests, as the runner builds them, each step committed and the whole model checked again. The
//! plate extruded and a rectangle turned a whole turn off its axis are solids of genus 1; a half disc turned about its
//! diameter is a sphere with its poles as vertices; a wire turned is a sheet. Every face lies on an analytic surface
//! where one fits. An extrusion along the face's plane, and a profile that leaves the plane of its axis or crosses it,
//! are refused before any model is changed.

use std::sync::Arc;

use occam_geom::{Line, Plane, UvCurve};
use occam_math::Exec;
use occam_report::{Code, EntityKind, EntityRef, Input, Report, StepRole};
use occam_topo::{
    Body, BodyKind, CurveRecord, Derivation, Edge, EdgeRecord, EdgeUseRecord, Face, FaceRecord, Id, Loop, LoopRecord,
    ParamCurveRecord, Snapshot, SurfaceRecord, Vertex, Wire,
};
use occam_units::{Direction, Point, UvPoint, Vec3};
use serde_json::{Value, json};

use crate::construct::space::{Bent, Flatland, unit};

use crate::construct::tests::{built, number, plate, refused, request};
use crate::{Labels, Request, default_tolerance, execute};

/// The counts of vertices, edges, faces and loops of `model`.
fn counts(model: &Snapshot) -> [usize; 4] {
    [model.ids::<Vertex>().len(), model.ids::<Edge>().len(), model.ids::<Face>().len(), model.ids::<Loop>().len()]
}

/// The genus of the one closed shell of `model`, from its counts: `V − E + F − (L − F) = 2 − 2g`.
fn genus(model: &Snapshot) -> f64 {
    let [v, e, f, l] = counts(model).map(|n| f64::from(u32::try_from(n).unwrap()));
    (2.0 - (v - e + f - (l - f))) / 2.0
}

/// The families of the surfaces of `model`'s faces, in face order.
fn families(model: &Snapshot) -> Vec<&'static str> {
    model.ids::<Face>().into_iter().map(|f| model.view(f).unwrap().surface().unwrap().family().0).collect()
}

/// The families of the surfaces of `model`'s faces, sorted.
fn sorted(model: &Snapshot) -> Vec<&'static str> {
    let mut found = families(model);
    found.sort_unstable();
    found
}

/// That every face's outward normal points out of the solid `inside` says which points are in: a little way back from
/// its loop's witness is inside, and a little way on is not.
fn outward(model: &Snapshot, inside: impl Fn(Vec3) -> bool) {
    for face in model.ids::<Face>() {
        let view = model.view(face).unwrap();
        let witness = model.view(view.outer()).unwrap().witness();
        let jet = view.surface().unwrap().first(witness);
        let normal = jet.du.rate().cross(jet.dv.rate());
        let normal = normal * (if view.reversed() { -1e-3 } else { 1e-3 } / normal.norm());
        let at = jet.point.to_vec3();
        assert!(inside(at - normal) && !inside(at + normal), "{face:?} at {at:?} faces {normal:?}");
    }
}

/// The radius from the `z` axis and the height along it of `p`.
fn around_z(p: Vec3) -> (f64, f64) {
    ((p.x * p.x + p.y * p.y).sqrt(), p.z)
}

/// The kind of `model`'s one body.
fn kind(model: &Snapshot) -> BodyKind {
    let bodies = model.ids::<Body>();
    assert_eq!(bodies.len(), 1, "one body");
    model.view(bodies[0]).unwrap().kind()
}

/// The plate's steps, its face labelled `plate`, then `more`.
fn on_plate(more: &[Value]) -> Value {
    let mut steps = plate().as_array().unwrap().clone();
    steps.extend(more.iter().cloned());
    Value::Array(steps)
}

/// The rectangle from `(10, 0, 0)` to `(20, 0, 5)` in the plane `y = 0`, as a face labelled `rect`, then `step`.
#[expect(clippy::needless_pass_by_value, reason = "the tests write their steps in place with `json!`")]
fn with_rectangle(step: Value) -> Value {
    json!([
        {"op": "wire", "label": "outline", "points": [[10, 0, 0], [20, 0, 0], [20, 0, 5], [10, 0, 5]], "closed": true},
        {"op": "face", "label": "rect", "outer": "outline"},
        step
    ])
}

/// A revolution of `profile` about the `z` axis through `angle` degrees.
fn about_z(profile: &str, angle: f64) -> Value {
    json!({"op": "revolve", "label": "turned", "profile": profile, "origin": [0, 0, 0], "axis": [0, 0, 1], "angle": angle})
}

#[test]
fn the_extruded_rectangle_with_a_hole_is_a_solid_of_genus_one_that_passes_the_checker() {
    let (model, made) =
        built(on_plate(&[json!({"op": "extrude", "label": "slab", "face": "plate", "vector": [0, 0, 5]})]));
    assert_eq!(made[3].map(|e| e.kind), Some(EntityKind::Body));
    assert_eq!(kind(&model), BodyKind::Solid);
    // Four lines and a circle cut in two: twelve points, six edges at each end and six along, two ends and six sides.
    assert_eq!(counts(&model), [12, 18, 8, 10]);
    assert_eq!(genus(&model), 1.0);
    assert_eq!(sorted(&model), ["cylinder", "cylinder", "plane", "plane", "plane", "plane", "plane", "plane"]);
    let profile = model.resolve::<Face>(made[2].unwrap()).map_err(|r| r.code);
    assert_eq!(profile, Err(Code::StaleId), "the sweep took its face");
    outward(&model, |p| plate_at(p, 0.0));
}

/// Whether `p` is inside the plate extruded from height `low` up 5.
fn plate_at(p: Vec3, low: f64) -> bool {
    let hole = (p.x - 20.0) * (p.x - 20.0) + (p.y - 10.0) * (p.y - 10.0) < 9.0;
    p.x > 0.0 && p.x < 40.0 && p.y > 0.0 && p.y < 20.0 && p.z > low && p.z < low + 5.0 && !hole
}

#[test]
fn a_full_revolution_of_a_rectangle_off_its_axis_is_a_solid_of_genus_one() {
    let (model, _) = built(with_rectangle(about_z("rect", 360.0)));
    assert_eq!(kind(&model), BodyKind::Solid);
    // Two patches of half a turn: each corner at two angles, each side at two, and each corner's two arcs.
    assert_eq!(counts(&model), [8, 16, 8, 8]);
    assert_eq!(genus(&model), 1.0);
    assert_eq!(sorted(&model), ["cylinder", "cylinder", "cylinder", "cylinder", "plane", "plane", "plane", "plane"]);
    let ring = |p: Vec3| {
        let (r, z) = around_z(p);
        r > 10.0 && r < 20.0 && z > 0.0 && z < 5.0
    };
    outward(&model, ring);
    // The same rectangle written the other way round faces the other way, and is turned so its loops run with the turn.
    let steps = json!([
        {"op": "wire", "label": "outline", "points": [[10, 0, 5], [20, 0, 5], [20, 0, 0], [10, 0, 0]], "closed": true},
        {"op": "face", "label": "rect", "outer": "outline"},
        about_z("rect", 360.0)
    ]);
    let (other, _) = built(steps);
    outward(&other, ring);
}

#[test]
fn an_extrusion_along_a_vector_in_the_faces_plane_is_refused() {
    let report = refused(on_plate(&[json!({"op": "extrude", "face": "plate", "vector": [3, 4, 0]})]));
    assert_eq!(report.code, Code::VectorInPlane);
    assert_eq!(report.input, Some(Input::at("steps[3].vector").in_step(3, Some("extrude"))));
    assert!(number(&report, "height").abs() < 1e-12);
}

#[test]
fn an_extrusion_against_the_faces_normal_or_slanting_is_a_solid_too() {
    let (down, _) = built(on_plate(&[json!({"op": "extrude", "face": "plate", "vector": [0, 0, -5]})]));
    assert_eq!((counts(&down), genus(&down)), ([12, 18, 8, 10], 1.0));
    outward(&down, |p| plate_at(p, -5.0));
    // Slanting, the hole's sides are no cylinder: a surface of linear extrusion of each half circle.
    let (slant, _) = built(on_plate(&[json!({"op": "extrude", "face": "plate", "vector": [0, 3, 5]})]));
    assert_eq!((counts(&slant), genus(&slant)), ([12, 18, 8, 10], 1.0));
    assert_eq!(families(&slant).iter().filter(|f| **f == "extrusion").count(), 2);
}

#[test]
fn a_half_disc_turned_about_its_diameter_is_a_sphere_with_its_poles_as_vertices() {
    let steps = json!([
        {"op": "wire", "label": "half", "edges": [{"arc": [[0, 0, -4], [4, 0, 0], [0, 0, 4]]}, {"line": [[0, 0, 4], [0, 0, -4]]}]},
        {"op": "face", "label": "disc", "outer": "half"},
        about_z("disc", 360.0)
    ]);
    let (model, _) = built(steps);
    assert_eq!((kind(&model), counts(&model), genus(&model)), (BodyKind::Solid, [2, 2, 2, 2], 0.0));
    assert_eq!(families(&model), ["sphere", "sphere"]);
    outward(&model, |p| p.norm() < 4.0);
}

#[test]
fn a_triangle_turned_part_of_a_turn_is_closed_by_its_two_ends() {
    let steps = json!([
        {"op": "wire", "label": "t", "points": [[0, 0, 2], [4, 0, 2], [0, 0, 12]], "closed": true},
        {"op": "face", "label": "tri", "outer": "t"},
        about_z("tri", 90.0)
    ]);
    let (model, _) = built(steps);
    // The corners on the axis stay put and the other turns once; the side on the axis is one edge both ends share.
    assert_eq!((kind(&model), counts(&model), genus(&model)), (BodyKind::Solid, [4, 6, 4, 4], 0.0));
    assert_eq!(sorted(&model), ["cone", "plane", "plane", "plane"]);
    outward(&model, |p| {
        let (r, z) = around_z(p);
        p.x > 0.0 && p.y > 0.0 && z > 2.0 && r / 4.0 + (z - 2.0) / 10.0 < 1.0
    });
}

#[test]
fn a_disc_off_the_axis_turned_a_whole_turn_is_a_torus() {
    let steps = json!([
        {"op": "wire", "label": "c", "edges": [{"circle": {"centre": [10, 0, 0], "radius": 3, "normal": [0, 1, 0]}}]},
        {"op": "face", "label": "disc", "outer": "c"},
        about_z("disc", 360.0)
    ]);
    let (model, _) = built(steps);
    assert_eq!((kind(&model), genus(&model)), (BodyKind::Solid, 1.0));
    assert!(families(&model).iter().all(|f| *f == "torus"), "{:?}", families(&model));
}

#[test]
fn an_arc_across_a_torus_s_outer_equator_is_cut_there_so_no_face_crosses_its_seam() {
    // Two arcs round the disc, the first through (13, 0, 0), where the torus's `v` comes round.
    let steps = json!([
        {"op": "wire", "label": "c", "edges": [{"arc": [[10, 0, 3], [13, 0, 0], [10, 0, -3]]}, {"arc": [[10, 0, -3], [7, 0, 0], [10, 0, 3]]}]},
        {"op": "face", "label": "disc", "outer": "c"},
        about_z("disc", 360.0)
    ]);
    let (model, _) = built(steps);
    // Three pieces at two angles, three corners' two arcs, three pieces' two patches.
    assert_eq!((kind(&model), counts(&model), genus(&model)), (BodyKind::Solid, [6, 12, 6, 6], 1.0));
    assert!(families(&model).iter().all(|f| *f == "torus"), "{:?}", families(&model));
}

#[test]
fn a_rectangle_turned_more_than_half_a_turn_is_cut_into_two_patches() {
    // About an axis along z through (3, 0, 7), which the rectangle's plane holds.
    let turn = json!({"op": "revolve", "profile": "rect", "origin": [3, 0, 7], "axis": [0, 0, 1], "angle": 270});
    let (model, _) = built(with_rectangle(turn));
    // Three columns of four corners; four sides at three angles and four corners' two arcs; eight patches, two ends.
    assert_eq!((counts(&model), genus(&model)), ([12, 20, 10, 10], 0.0));
    outward(&model, |p| {
        let (r, z) = around_z(p - Vec3::new(3.0, 0.0, 0.0));
        r > 7.0 && r < 17.0 && z > 0.0 && z < 5.0 && !(p.x > 3.0 && p.y < 0.0)
    });
}

#[test]
fn a_quarter_disc_turned_a_whole_turn_is_a_hemisphere() {
    let steps = json!([
        {"op": "wire", "label": "q", "edges": [
            {"line": [[0, 0, 1], [4, 0, 1]]},
            {"arc": [[4, 0, 1], [2.4, 0, 4.2], [0, 0, 5]]},
            {"line": [[0, 0, 5], [0, 0, 1]]}
        ]},
        {"op": "face", "label": "quarter", "outer": "q"},
        about_z("quarter", 360.0)
    ]);
    let (model, _) = built(steps);
    // Two poles and the rim's two points; the arc and the radius at two angles and the rim's two arcs.
    assert_eq!((kind(&model), counts(&model), genus(&model)), (BodyKind::Solid, [4, 6, 4, 4], 0.0));
    assert_eq!(sorted(&model), ["plane", "plane", "sphere", "sphere"]);
    outward(&model, |p| (p - Vec3::new(0.0, 0.0, 1.0)).norm() < 4.0 && p.z > 1.0);
}

#[test]
fn a_wire_turned_is_a_sheet() {
    // Falling as it leaves the axis, so the cone opens against the axis's direction.
    let steps = json!([
        {"op": "wire", "label": "w", "points": [[10, 0, 5], [12, 0, 0]]},
        about_z("w", 360.0)
    ]);
    let (model, made) = built(steps);
    assert_eq!(kind(&model), BodyKind::Sheet);
    assert_eq!(made[1].map(|e| e.kind), Some(EntityKind::Body));
    assert_eq!(counts(&model), [4, 6, 2, 2]);
    assert_eq!(families(&model), ["cone", "cone"]);
}

#[test]
fn an_arc_about_a_point_beyond_the_axis_turns_on_a_surface_of_revolution() {
    // A lens: an arc about (-2, 0, 0) of radius 5 from the axis and back, which no torus fits.
    let r = 21.0_f64.sqrt();
    let steps = json!([
        {"op": "wire", "label": "lens", "edges": [{"arc": [[0, 0, -r], [3, 0, 0], [0, 0, r]]}, {"line": [[0, 0, r], [0, 0, -r]]}]},
        {"op": "face", "label": "f", "outer": "lens"},
        about_z("f", 360.0)
    ]);
    let (model, _) = built(steps);
    assert_eq!((counts(&model), genus(&model)), ([2, 2, 2, 2], 0.0));
    assert_eq!(families(&model), ["revolution", "revolution"]);
}

#[test]
fn a_profile_off_the_plane_of_its_axis_or_across_it_is_refused() {
    let turn = |origin: Value, axis: Value| json!({"op": "revolve", "profile": "rect", "origin": origin, "axis": axis, "angle": 90});
    let off = refused(with_rectangle(turn(json!([0, 3, 0]), json!([1, 0, 0]))));
    assert_eq!(off.code, Code::AxisOffPlane);
    assert_eq!(off.input, Some(Input::at("steps[2].axis").in_step(2, Some("revolve"))));
    assert!((number(&off, "distance") - 15.0 / 34.0_f64.sqrt()).abs() < 1e-9, "{off:?}");
    let across = refused(with_rectangle(turn(json!([15, 0, 0]), json!([0, 0, 1]))));
    assert_eq!(across.code, Code::ProfileCrossesAxis);
    assert!((number(&across, "distance") - 5.0).abs() < 1e-9, "{across:?}");
    // Of the corners as far beyond the axis, the first the profile reaches is named.
    let steps = json!([
        {"op": "wire", "label": "outline", "points": [[20, 0, 0], [20, 0, 5], [10, 0, 5], [10, 0, 0]], "closed": true},
        {"op": "face", "label": "rect", "outer": "outline"},
        turn(json!([14, 0, 0]), json!([0, 0, 1]))
    ]);
    let first = refused(steps);
    assert_eq!(first.value("point").map(|v| v.value), Some(occam_report::Value::Point([10.0, 0.0, 5.0])));
}

#[test]
fn a_solid_or_a_wire_is_not_a_face_to_extrude() {
    let mut steps = on_plate(&[json!({"op": "extrude", "label": "slab", "face": "plate", "vector": [0, 0, 5]})]);
    steps.as_array_mut().unwrap().push(json!({"op": "extrude", "face": "slab", "vector": [0, 0, 5]}));
    let report = refused(steps);
    assert_eq!(report.code, Code::NotAProfile);
    assert_eq!(report.input.as_ref().and_then(|i| i.given.as_deref()), Some("a solid"));
    let wire = refused(json!([
        {"op": "wire", "label": "w", "points": [[0, 0, 0], [1, 0, 0]]},
        {"op": "extrude", "face": "w", "vector": [0, 0, 1]}
    ]));
    let given = wire.input.as_ref().and_then(|i| i.given.as_deref());
    assert_eq!((wire.code, given), (Code::NotAProfile, Some("a wire body")));
}

#[test]
fn a_sweep_step_with_bad_fields_is_refused_with_each_one_in_place() {
    let reports = Request::parse(
        &request(json!([
            {"op": "extrude", "face": 3, "vector": [0, 0], "depth": 1},
            {"op": "revolve", "profile": "p", "origin": [0, 0, 0], "axis": [0, 0, 0], "angle": 400},
            {"op": "revolve", "profile": "p", "origin": [0, 0, 0], "axis": [0, 0, 1], "angle": 0}
        ]))
        .to_string(),
    )
    .unwrap_err();
    let found: Vec<(Code, String)> =
        reports.iter().map(|r| (r.code, r.input.as_ref().map_or(String::new(), |i| i.parameter.clone()))).collect();
    let want = [
        (Code::InputUnknownField, "steps[0].depth"),
        (Code::InputWrongType, "steps[0].face"),
        (Code::InputWrongType, "steps[0].vector"),
        (Code::DegenerateDirection, "steps[1].axis"),
        (Code::OutOfRange, "steps[1].angle"),
        (Code::OutOfRange, "steps[2].angle"),
    ];
    assert_eq!(found, want.map(|(code, at)| (code, at.to_string())));
}

#[test]
fn everything_a_sweep_makes_is_its_own_and_names_what_of_the_profile_it_was_swept_from() {
    let request = Request::parse(&request(with_rectangle(about_z("rect", 90.0))).to_string()).unwrap();
    let mut model = Snapshot::empty(default_tolerance().unwrap());
    let mut labels = Labels::default();
    let mut last = None;
    for step in &request.steps {
        let mut edit = model.edit();
        let made = execute(&mut edit, step, &labels).unwrap();
        let (after, changes) = edit.commit(&Exec::serial()).unwrap();
        if let (Some(label), Some(entity)) = (&step.label, made) {
            labels.insert(label, entity);
        }
        last = Some(changes);
        model = after;
    }
    let changes = last.unwrap();
    assert!(changes.lineage.iter().all(|l| l.how == Derivation::Swept && changes.deleted.contains(&l.old)));
    for kind in [EntityKind::Vertex, EntityKind::Edge, EntityKind::Face, EntityKind::Body] {
        let created = changes.created.iter().filter(|e| e.kind == kind).count();
        let named = changes.lineage.iter().filter(|l| l.new.kind == kind).count();
        assert_eq!(named, created, "every new {kind:?} names the profile's entity it was swept from");
    }
    assert_eq!(changes.deleted.iter().filter(|e| e.kind == EntityKind::Face).count(), 1, "the profile went");
    for face in model.ids::<Face>() {
        assert_eq!(model.view(face).unwrap().born().role, StepRole::Profile);
    }
}

#[test]
fn a_wire_named_by_its_id_is_a_profile_to_revolve_but_not_a_face_to_extrude() {
    let wire = json!({"op": "wire", "points": [[10, 0, 0], [12, 0, 5]]});
    let (model, _) = built(json!([wire, about_z("wire:0/1", 360.0)]));
    assert_eq!((kind(&model), counts(&model)), (BodyKind::Sheet, [4, 6, 2, 2]));
    let report = refused(json!([wire, {"op": "extrude", "face": "wire:0/1", "vector": [0, 0, 1]}]));
    let given = report.input.as_ref().and_then(|i| i.given.as_deref());
    assert_eq!((report.code, given), (Code::NotAProfile, Some("wire:0/1")));
}

#[test]
fn a_face_of_a_solid_or_of_a_sheet_of_several_faces_is_not_a_profile() {
    let given = |report: &occam_report::Report| report.input.as_ref().and_then(|i| i.given.clone());
    let solid = refused(json!([
        {"op": "box", "size": [1, 2, 3]},
        {"op": "extrude", "face": "face:0/1", "vector": [0, 0, 1]}
    ]));
    assert_eq!((solid.code, given(&solid)), (Code::NotAProfile, Some("a face of a solid".to_string())));
    let sheet = refused(json!([
        {"op": "wire", "label": "w", "points": [[10, 0, 0], [12, 0, 5]]},
        about_z("w", 360.0),
        {"op": "extrude", "face": "turned", "vector": [0, 0, 1]}
    ]));
    assert_eq!((sheet.code, given(&sheet)), (Code::NotAProfile, Some("a face of a sheet with others".to_string())));
}

#[test]
fn a_wire_of_a_body_with_other_wires_is_not_a_profile() {
    let (model, _) = built(json!([
        {"op": "wire", "points": [[10, 0, 0], [12, 0, 5]]},
        {"op": "wire", "points": [[10, 0, 6], [12, 0, 9]]}
    ]));
    let wires = model.ids::<Wire>();
    let (first, second) = (model.view(wires[0]).unwrap(), model.view(wires[1]).unwrap());
    let (keep, gone) = (first.body(), second.body());
    let mut edit = model.edit();
    edit.set(wires[0], occam_topo::WireRecord { next: Some(wires[1]), ..edit.record(wires[0]).unwrap() }).unwrap();
    edit.set(wires[1], occam_topo::WireRecord { body: keep, ..edit.record(wires[1]).unwrap() }).unwrap();
    edit.remove(gone).unwrap();
    let (joined, _) = edit.commit(&Exec::serial()).unwrap();
    let request = Request::parse(&request(json!([about_z("wire:0/1", 360.0)])).to_string()).unwrap();
    let report = execute(&mut joined.edit(), &request.steps[0], &Labels::default()).unwrap_err();
    let given = report.input.as_ref().and_then(|i| i.given.as_deref());
    assert_eq!((report.code, given), (Code::NotAProfile, Some("a wire of a body with others")));
}

#[test]
fn a_profile_whose_arc_bulges_across_the_axis_is_refused_at_the_arc_s_furthest_point() {
    // A line at x = 2 closed by an arc about (2, 0, 0) of radius 3 through (-1, 0, 0), 1 beyond the z axis.
    let report = refused(json!([
        {"op": "wire", "label": "d", "edges": [{"line": [[2, 0, -3], [2, 0, 3]]}, {"arc": [[2, 0, 3], [-1, 0, 0], [2, 0, -3]]}]},
        {"op": "face", "label": "f", "outer": "d"},
        about_z("f", 90.0)
    ]));
    assert_eq!(report.code, Code::ProfileCrossesAxis);
    assert!((number(&report, "distance") - 1.0).abs() < 1e-9, "{report:?}");
    let Some(occam_report::Value::Point(p)) = report.value("point").map(|v| v.value) else { panic!("{report:?}") };
    assert!((Vec3::new(p[0], p[1], p[2]) - Vec3::new(-1.0, 0.0, 0.0)).norm() < 1e-9, "{p:?}");
}

/// A rectangle reaching 0.5 from the `z` axis, as a face labelled `small`, turned through `angle` degrees.
fn small(angle: f64) -> Value {
    json!([
        {"op": "wire", "label": "s", "points": [[0.25, 0, 0], [0.5, 0, 0], [0.5, 0, 0.25], [0.25, 0, 0.25]], "closed": true},
        {"op": "face", "label": "small", "outer": "s"},
        about_z("small", angle)
    ])
}

/// `radians` in degrees.
fn degrees(radians: f64) -> f64 {
    radians * 180.0 / core::f64::consts::PI
}

#[test]
fn a_turn_that_moves_the_profile_no_further_than_the_band_is_refused() {
    // The band is 1e-7: turned 1e-7 radians, a point 0.5 from the axis moves half the band.
    let report = refused(small(degrees(1e-7)));
    assert_eq!(report.code, Code::NoLength);
    assert_eq!(report.input, Some(Input::at("steps[2].angle").in_step(2, Some("revolve"))));
    assert!((number(&report, "length") - 0.5e-7).abs() < 1e-18, "{report:?}");
}

#[test]
fn a_turn_whose_ends_meet_within_the_band_is_a_whole_turn_and_half_a_turn_is_one_patch() {
    // 1e-7 radians short of a whole turn, the farthest corner ends half the band from where it began.
    let (nearly, _) = built(small(360.0 - degrees(1e-7)));
    assert_eq!((counts(&nearly), genus(&nearly)), ([8, 16, 8, 8], 1.0));
    // Each corner at both ends and once round; each side at both ends; four patches and the two ends.
    let (half, _) = built(with_rectangle(about_z("rect", 180.0)));
    assert_eq!((counts(&half), genus(&half)), ([8, 12, 6, 6], 0.0));
}

/// The model `steps` build, then `step`, with every straight edge's line begun `by` before its start, so that its range
/// begins at `by`, and each use's curve in its face's parameters with it.
fn with_lines_begun_before(steps: &Value, by: f64, step: Value) -> Result<Snapshot, Vec<Report>> {
    let (model, made) = built(steps.clone());
    let mut edit = model.edit();
    for e in model.ids::<Edge>() {
        let view = model.view(e).unwrap();
        let curve = view.curve().unwrap();
        let Some(Bent::Line { origin, along }) = Bent::of(curve) else { continue };
        let line = Line::new(Point::from_vec3(origin - along * by), Direction::normalised(along));
        let record = edit.record(e).unwrap();
        let (lo, hi) = record.range;
        let begun = EdgeRecord { curve: edit.add(CurveRecord(Arc::new(line))), range: (lo + by, hi + by), ..record };
        edit.set(e, begun).unwrap();
        for u in view.uses() {
            let Some(old) = model.view(u).unwrap().pcurve() else { continue };
            let (a, b) = (old.uv(0.0), old.uv(1.0));
            let (du, dv) = (b.u() - a.u(), b.v() - a.v());
            let start = Point::from_vec3(Vec3::new(a.u() - du * by, a.v() - dv * by, 0.0));
            let image = UvCurve::new(Line::new(start, Direction::normalised(Vec3::new(du, dv, 0.0)))).unwrap();
            let pcurve = edit.add(ParamCurveRecord(Arc::new(image)));
            edit.set(u, EdgeUseRecord { pcurve: Some(pcurve), ..edit.record(u).unwrap() }).unwrap();
        }
    }
    let (begun, _) = edit.commit(&Exec::serial()).unwrap();
    then(&begun, &made, steps, step)
}

/// `step` run on `model`, which the labelled steps of `steps` made `made`.
#[expect(clippy::needless_pass_by_value, reason = "the tests write their steps in place with `json!`")]
fn then(model: &Snapshot, made: &[Option<EntityRef>], steps: &Value, step: Value) -> Result<Snapshot, Vec<Report>> {
    let mut labels = Labels::default();
    for (given, entity) in steps.as_array().unwrap().iter().zip(made) {
        if let (Some(label), Some(entity)) = (given["label"].as_str(), entity) {
            labels.insert(label, *entity);
        }
    }
    let parsed = Request::parse(&request(json!([step])).to_string())?;
    let mut edit = model.edit();
    execute(&mut edit, &parsed.steps[0], &labels).map_err(|report| vec![report])?;
    let (after, _) = edit.commit(&Exec::serial())?;
    assert_eq!(after.check_all(&Exec::serial()), [], "the whole model passes the checker");
    Ok(after)
}

#[test]
fn a_profile_whose_lines_do_not_begin_at_their_start_sweeps_the_same() {
    let triangle = json!([
        {"op": "wire", "label": "t", "points": [[0, 0, 2], [4, 0, 2], [0, 0, 12]], "closed": true},
        {"op": "face", "label": "tri", "outer": "t"}
    ]);
    let model = with_lines_begun_before(&triangle, 15.0, about_z("tri", 90.0)).unwrap();
    assert_eq!((counts(&model), genus(&model)), ([4, 6, 4, 4], 0.0));
    assert_eq!(sorted(&model), ["cone", "plane", "plane", "plane"]);
    let plate = with_lines_begun_before(&plate(), 15.0, json!({"op": "extrude", "face": "plate", "vector": [0, 0, 5]}));
    let plate = plate.unwrap();
    assert_eq!((counts(&plate), genus(&plate)), ([12, 18, 8, 10], 1.0));
    outward(&plate, |p| plate_at(p, 0.0));
}

#[test]
fn a_face_whose_normal_is_its_surface_s_turned_over_sweeps_the_same() {
    // The plate on a plane facing down, the face turned over, so its outward normal still points up.
    let (model, made) = built(plate());
    let face = model.resolve::<Face>(made[2].unwrap()).unwrap();
    let view = model.view(face).unwrap();
    let surface = view.surface().unwrap();
    let witness = model.view(view.outer()).unwrap().witness();
    let jet = surface.first(witness);
    let down = Flatland::new(surface.point(witness).to_vec3(), unit(jet.du.rate().cross(jet.dv.rate())) * -1.0);
    let mut edit = model.edit();
    let plane = edit.add(SurfaceRecord(Arc::new(Plane::placed(&down.placement))));
    let record = edit.record(face).unwrap();
    edit.set(face, FaceRecord { surface: plane, reversed: !record.reversed, ..record }).unwrap();
    for l in view.loops() {
        let ring = edit.record(l).unwrap();
        let ([u, v], _) = down.uv(surface.point(ring.witness).to_vec3());
        edit.set(l, LoopRecord { witness: UvPoint::new(u, v), ..ring }).unwrap();
        for used in model.view(l).unwrap().uses() {
            let link = edit.record(used).unwrap();
            let bent = Bent::of(model.view(link.edge).unwrap().curve().unwrap()).unwrap();
            let pcurve = edit.add(down.pcurve(bent).unwrap());
            edit.set(used, EdgeUseRecord { pcurve: Some(pcurve), ..link }).unwrap();
        }
    }
    let (turned, _) = edit.commit(&Exec::serial()).unwrap();
    let id: Id<Face> = face;
    assert!(turned.view(id).unwrap().reversed());
    let step = json!({"op": "extrude", "face": "plate", "vector": [0, 0, 5]});
    let solid = then(&turned, &made, &plate(), step).unwrap();
    assert_eq!((counts(&solid), genus(&solid)), ([12, 18, 8, 10], 1.0));
    outward(&solid, |p| plate_at(p, 0.0));
}
