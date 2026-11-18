//! Source: original
//! The transform's and the copy's tests: a copy is new from its body down, with the same counts and lineage to what it
//! copied; a transform keeps every id; a rigid move or a scale alike in every direction keeps every surface in its
//! family, and a scale that differs makes the round ones exact B-splines, every body still closed and its volume scaled
//! by the factors' product; and every bad input is refused where it is.
use core::f64::consts::TAU;
use std::collections::BTreeMap;
use std::sync::Arc;

use occam_geom::{OffsetSurface, Plane, SplineSurface, Surface, UvBox};
use occam_math::Exec;
use occam_report::{Code, EntityKind, EntityRef, Input, NamedValue, Quantity, Report};
use occam_topo::{Body, Changes, Derivation, Edge, Face, Shell, Snapshot, SurfaceGeom, Vertex};
use occam_units::{Affine, Length, Local, Placement, UvPoint, Vec3, World};
use serde_json::{Value, json};

use crate::construct::tests::run;
use crate::measure::{Measured, measure};
use crate::operand::Labels;
use crate::request::Request;

/// The request with these steps, in millimetres.
fn request(steps: &Value) -> String {
    json!({"schema": 1, "units": {"length": "mm", "angle": "rad"}, "steps": steps}).to_string()
}

/// Runs `steps` on a new model, as the runner does, each committed: the model, the labels, and what each step changed.
fn ran(steps: &Value) -> Result<(Snapshot, Labels, Vec<Changes>), Vec<Report>> {
    let request = Request::parse(&request(steps))?;
    let run = run(&request, &request.steps)?;
    Ok((run.model, run.labels, run.changes))
}

/// The model `steps` make.
fn made(steps: &Value) -> Snapshot {
    ran(steps).unwrap_or_else(|r| panic!("{steps}: {r:#?}")).0
}

/// A model's faces, edges and vertices.
fn counts(s: &Snapshot) -> (usize, usize, usize) {
    (s.ids::<Face>().len(), s.ids::<Edge>().len(), s.ids::<Vertex>().len())
}

/// Every vertex's point, by id.
fn points(s: &Snapshot) -> BTreeMap<EntityRef, Vec3> {
    s.ids::<Vertex>().into_iter().map(|v| (v.entity(), s.view(v).unwrap().point().to_vec3())).collect()
}

/// The families of a model's surfaces, in order of their ids.
fn families(s: &Snapshot) -> Vec<&'static str> {
    s.ids::<SurfaceGeom>().into_iter().map(|g| s.view(g).unwrap().get().family().0).collect()
}

/// One of each primitive, labelled `p`.
fn primitives() -> [Value; 6] {
    [
        json!({"op": "box", "size": [10, 20, 30], "label": "p"}),
        json!({"op": "wedge", "size": [10, 5, 4], "top": {"start": 2, "width": 3}, "label": "p"}),
        json!({"op": "cylinder", "radius": 5, "height": 10, "label": "p"}),
        json!({"op": "cone", "radius1": 5, "radius2": 2, "height": 10, "label": "p"}),
        json!({"op": "sphere", "radius": 5, "label": "p"}),
        json!({"op": "torus", "major": 10, "minor": 2, "label": "p"}),
    ]
}

/// The volume of what the step labelled `target` made, the last step of `steps`.
fn volume(steps: &Value, target: &str) -> f64 {
    let (model, labels, _) = ran(steps).unwrap_or_else(|r| panic!("{steps}: {r:#?}"));
    let asked = Request::parse(&request(&json!([{"op": "measure", "target": target, "what": ["volume"]}]))).unwrap();
    let Measured { values, .. } = measure(&model, &asked.steps[0], &labels).unwrap();
    values[0].number().unwrap()
}

#[test]
fn a_copied_box_moved_by_a_vector_has_new_ids_the_same_counts_and_lineage_to_the_original() {
    let steps = json!([
        {"op": "box", "size": [10, 20, 30], "label": "b"},
        {"op": "copy", "target": "b", "frame": {"origin": [100, 0, 0]}, "label": "c"}
    ]);
    let (s, labels, changes) = ran(&steps).unwrap();
    let (original, copy) = (labels.get("b").unwrap(), labels.get("c").unwrap());
    assert_ne!(original, copy);
    assert_eq!(counts(&s), (12, 24, 16), "the box twice");
    assert_eq!((s.ids::<Body>().len(), s.ids::<Shell>().len()), (2, 2));
    assert_eq!(s.check_all(&Exec::serial()), []);
    let copying = &changes[1];
    assert_eq!((copying.changed.len(), copying.deleted.len()), (0, 0), "the original is untouched");
    let made_by_box: Vec<EntityRef> = changes[0].created.clone();
    assert!(copying.created.iter().all(|e| !made_by_box.contains(e)), "every id of the copy is new");
    let from: BTreeMap<EntityRef, EntityRef> = copying.lineage.iter().map(|l| (l.new, l.old)).collect();
    assert!(copying.lineage.iter().all(|l| l.how == Derivation::Copied && l.new.kind == l.old.kind));
    assert_eq!(from.len(), copying.created.len(), "every new entity names what it was copied from");
    assert!(copying.created.iter().all(|e| made_by_box.contains(&from[e])), "each from the box");
    assert_eq!(from[&copy], original, "the copy's body names the box's");
    let at = points(&s);
    for v in copying.created.iter().filter(|e| e.kind == EntityKind::Vertex) {
        assert!((at[v] - (at[&from[v]] + Vec3::new(100.0, 0.0, 0.0))).norm() < 1e-12, "{v:?} moved by the vector");
    }
}

#[test]
fn a_transform_moves_a_body_in_place_keeping_every_id() {
    let frame = json!({"origin": [1, -2, 3], "z": [1, 1, 1], "x": [1, -1, 0]});
    let before = made(&json!([{"op": "box", "size": [10, 20, 30], "label": "b"}]));
    let steps = json!([
        {"op": "box", "size": [10, 20, 30], "label": "b"},
        {"op": "transform", "target": "b", "frame": frame}
    ]);
    let (after, labels, changes) = ran(&steps).unwrap();
    assert_eq!(after.check_all(&Exec::serial()), []);
    assert_eq!(after.ids::<Face>(), before.ids::<Face>());
    assert_eq!(after.ids::<Edge>(), before.ids::<Edge>());
    assert_eq!(after.ids::<Vertex>(), before.ids::<Vertex>());
    assert_eq!(after.ids::<SurfaceGeom>(), before.ids::<SurfaceGeom>(), "the geometry is replaced in place too");
    assert_eq!(labels.get("b").map(|b| b.kind), Some(EntityKind::Body));
    assert_eq!((changes[1].created.len(), changes[1].deleted.len()), (0, 0));
    let (z, x) = (Vec3::new(1.0, 1.0, 1.0) * (1.0 / 3f64.sqrt()), Vec3::new(1.0, -1.0, 0.0) * (1.0 / 2f64.sqrt()));
    let y = z.cross(x);
    let moved = |p: Vec3| Vec3::new(1.0, -2.0, 3.0) + x * p.x + y * p.y + z * p.z;
    let (was, is) = (points(&before), points(&after));
    for (v, p) in &was {
        assert!((moved(*p) - is[v]).norm() < 1e-12, "{v:?}: {p:?} went to {:?}", is[v]);
    }
    assert_eq!(families(&after), families(&before));
}

#[test]
fn a_uniform_scale_keeps_every_primitive_in_its_families_about_the_frame_given() {
    let centre = Vec3::new(1.0, 2.0, 3.0);
    for step in primitives() {
        let before = made(&json!([step]));
        let scaled = json!([step, {"op": "transform", "target": "p", "scale": 2.5, "about": {"origin": [1, 2, 3]}}]);
        let after = made(&scaled);
        assert_eq!(after.check_all(&Exec::serial()), [], "{step}");
        assert_eq!(families(&after), families(&before), "{step}");
        let (was, is) = (points(&before), points(&after));
        for (v, p) in &was {
            assert!((centre + (*p - centre) * 2.5 - is[v]).norm() < 1e-12, "{step}: {v:?}");
        }
        let (small, large) = (volume(&json!([step]), "p"), volume(&scaled, "p"));
        assert!((large / small - 2.5 * 2.5 * 2.5).abs() < 1e-6, "{step}: {small} to {large}");
    }
}

/// A frame turned off the world's axes, at (1, -2, 3).
fn tilted() -> Value {
    json!({"origin": [1, -2, 3], "z": [1, 2, 2], "x": [2, -1, 0]})
}

/// The primitive `step` made, and then stretched by 2, 1 and a half along [`tilted`]'s axes: what each model is, checked
/// whole, with the counts and surface families of each.
fn stretched(step: &Value) -> [Snapshot; 2] {
    let stretch = json!({"op": "transform", "target": "p", "scale": [2, 1, 0.5], "about": tilted()});
    let [before, after] = [made(&json!([step])), made(&json!([step, stretch]))];
    assert_eq!(after.check_all(&Exec::serial()), [], "{step}");
    assert_eq!(counts(&after), counts(&before), "{step}");
    let stayed = families(&before).into_iter().map(|f| if f == "plane" { f } else { "bspline" });
    assert_eq!(families(&after), stayed.collect::<Vec<_>>(), "{step}: planes stay planes, the rest are B-splines");
    [before, after]
}

#[test]
fn a_scale_that_differs_along_the_axes_keeps_a_box_and_a_wedge_planar_and_scales_their_volume() {
    let stretch = json!({"op": "transform", "target": "p", "scale": [2, 1, 0.25], "about": tilted()});
    for step in &primitives()[..2] {
        stretched(step);
        let (small, large) = (volume(&json!([step]), "p"), volume(&json!([step, stretch]), "p"));
        assert!((large / small - 0.5).abs() < 1e-9, "{step}: {small} to {large}, the factors' product a half");
    }
}

#[test]
fn a_cylinder_stretched_across_its_axis_has_exact_bspline_sides_and_the_volume_the_factors_give() {
    let step = &primitives()[2];
    stretched(step);
    let stretch = json!({"op": "transform", "target": "p", "scale": [2, 1, 0.5], "about": tilted()});
    let (small, stretched) = (volume(&json!([step]), "p"), volume(&json!([step, stretch]), "p"));
    assert!((stretched / small - 1.0).abs() < 1e-6, "the factors' product is 1, yet {small} to {stretched}");
}

#[test]
fn a_cone_stretched_stays_closed() {
    stretched(&primitives()[3]);
}

#[test]
fn a_sphere_stretched_stays_closed_through_its_poles() {
    stretched(&primitives()[4]);
}

#[test]
fn a_torus_stretched_stays_closed() {
    stretched(&primitives()[5]);
}

#[test]
fn a_copy_of_a_face_with_a_hole_and_of_a_wire_stretched_passes_the_checker() {
    let rectangle = json!({"points": [[0, 0, 0], [20, 0, 0], [20, 10, 0], [0, 10, 0]], "closed": true});
    let hole = json!({"edges": [{"circle": {"centre": [10, 5, 0], "normal": [0, 0, 1], "radius": 2}}]});
    let steps = json!([
        {"op": "face", "outer": rectangle, "holes": [hole], "label": "f"},
        {"op": "wire", "edges": [{"arc": [[0, 0, 5], [3, 4, 5], [5, 0, 5]]}], "label": "w"},
        {"op": "copy", "target": "f", "scale": [3, 1, 2], "about": tilted(), "label": "f2"},
        {"op": "copy", "target": "w", "scale": [3, 1, 2], "about": tilted(), "label": "w2"}
    ]);
    let (s, labels, _) = ran(&steps).unwrap_or_else(|r| panic!("{r:#?}"));
    assert_eq!(s.check_all(&Exec::serial()), []);
    assert_eq!(s.ids::<Body>().len(), 4);
    assert!(labels.get("f2").is_some() && labels.get("w2").is_some());
    let curves: Vec<&str> =
        s.ids::<Edge>().into_iter().map(|e| s.view(e).unwrap().curve().unwrap().family().0).collect();
    assert_eq!(curves.iter().filter(|f| **f == "bspline").count(), 2, "the stretched circle and arc: {curves:?}");
}

#[test]
fn a_swept_solid_moves_and_stretches_like_any_other() {
    let rectangle = json!({"points": [[2, 0, 0], [4, 0, 0], [4, 0, 3], [2, 0, 3]], "closed": true});
    let steps = json!([
        {"op": "face", "outer": rectangle, "label": "f"},
        {"op": "revolve", "profile": "f", "origin": [0, 0, 0], "axis": [0, 0, 1], "angle": TAU, "label": "r"},
        {"op": "copy", "target": "r", "frame": tilted(), "label": "moved"},
        {"op": "transform", "target": "r", "scale": [1, 2, 3]}
    ]);
    let s = made(&steps);
    assert_eq!(s.check_all(&Exec::serial()), []);
    assert_eq!(s.ids::<Body>().len(), 2);
}

/// The reports refusing `steps` when the request is read, as each one's code, place and values.
fn refused(steps: &Value) -> Vec<(Code, Option<Input>, Vec<NamedValue>)> {
    let reports = Request::parse(&request(steps)).map(|_| ()).unwrap_err();
    reports.into_iter().map(|r| (r.code, r.input.clone(), r.values.clone())).collect()
}

/// The place `path` in step 1, which asks for `op`.
fn place(path: &str, op: &str) -> Input {
    Input::at(path).in_step(1, Some(op))
}

#[test]
fn a_scale_that_is_not_positive_is_refused_with_its_factor_each_one() {
    let box_ = json!({"op": "box", "size": [1, 1, 1], "label": "b"});
    let not_positive = |path: &str, op: &str, value: f64| {
        (Code::InputScaleNotPositive, Some(place(path, op)), vec![NamedValue::new("value", Quantity::Ratio, value)])
    };
    let found = refused(&json!([box_, {"op": "transform", "target": "b", "scale": 0}]));
    assert_eq!(found, [not_positive("steps[1].scale", "transform", 0.0)]);
    let found = refused(&json!([box_, {"op": "copy", "target": "b", "scale": [2, -1, 0]}]));
    assert_eq!(
        found,
        [not_positive("steps[1].scale[1]", "copy", -1.0), not_positive("steps[1].scale[2]", "copy", 0.0)]
    );
    let found = refused(&json!([box_, {"op": "copy", "target": "b", "scale": "twice"}]));
    assert_eq!(found.iter().map(|(c, ..)| *c).collect::<Vec<_>>(), [Code::InputWrongType]);
    let found = refused(&json!([box_, {"op": "transform", "target": "b", "scale": [1, 2]}]));
    assert_eq!(found.iter().map(|(c, ..)| *c).collect::<Vec<_>>(), [Code::WrongCount]);
}

#[test]
fn every_bad_input_of_a_transform_is_reported_at_once() {
    let found = refused(&json!([
        {"op": "box", "size": [1, 1, 1], "label": "b"},
        {"op": "transform", "scale": -1, "about": {"z": [0, 0, 0]}, "frame": {"x": [0, 0, 1]}, "turn": 3}
    ]));
    let codes: Vec<(Code, Option<Input>)> = found.into_iter().map(|(c, i, _)| (c, i)).collect();
    assert_eq!(
        codes,
        [
            (
                Code::InputUnknownField,
                Some(place("steps[1].turn", "transform").given("turn").expected("target, scale, about, frame")),
            ),
            (Code::InputMissing, Some(place("steps[1].target", "transform"))),
            (Code::InputScaleNotPositive, Some(place("steps[1].scale", "transform"))),
            (Code::InputNoDirection, Some(place("steps[1].about.z", "transform"))),
            (Code::InputAxesParallel, Some(place("steps[1].frame.x", "transform"))),
        ]
    );
}

#[test]
fn a_transform_or_a_copy_of_anything_but_a_body_is_refused_naming_it() {
    let (model, ..) = ran(&json!([{"op": "box", "size": [1, 1, 1]}])).unwrap();
    let edge = model.ids::<Edge>()[0].entity().text();
    let asked = Request::parse(&request(&json!([{"op": "copy", "target": edge}]))).unwrap();
    let report = crate::execute(&mut model.edit(), &asked.steps[0], &Labels::default()).unwrap_err();
    assert_eq!(report.code, Code::NotABody);
    assert_eq!(report.input, Some(Input::at("steps[0].target").in_step(0, Some("copy")).given(&edge)));
    let asked = Request::parse(&request(&json!([{"op": "transform", "target": "nothing"}]))).unwrap();
    let report = crate::execute(&mut model.edit(), &asked.steps[0], &Labels::default()).unwrap_err();
    assert_eq!(report.code, Code::InputUnknownLabel);
}

#[test]
fn the_only_face_of_a_sheet_or_wire_of_a_wire_body_stands_for_its_body_and_a_face_of_a_solid_does_not() {
    let (model, ..) = ran(&json!([
        {"op": "box", "size": [1, 1, 1]},
        {"op": "wire", "points": [[5, 0, 0], [6, 0, 0], [6, 1, 0]]}
    ]))
    .unwrap();
    let wire = model.ids::<occam_topo::Wire>()[0].entity().text();
    let copy = |target: &str| {
        let asked = Request::parse(&request(&json!([{"op": "copy", "target": target, "scale": 2}]))).unwrap();
        let mut edit = model.edit();
        let made = crate::execute(&mut edit, &asked.steps[0], &Labels::default())?;
        edit.commit(&Exec::serial()).map_err(|r| r[0].clone())?;
        Ok::<_, Report>(made)
    };
    assert_eq!(copy(&wire).map(|b| b.map(|b| b.kind)), Ok(Some(EntityKind::Body)), "a wire body's wire");
    let face = model.ids::<Face>()[0].entity().text();
    assert_eq!(copy(&face).map_err(|r| r.code), Err(Code::NotABody), "a face of the box");
}

#[test]
fn a_request_in_inches_moves_by_millimetres_and_scales_by_plain_factors() {
    let text = json!({"schema": 1, "units": {"length": "in", "angle": "deg"}, "steps": [
        {"op": "box", "size": [1, 1, 1], "label": "b"},
        {"op": "copy", "target": "b", "frame": {"origin": [1, 0, 0]}, "scale": 2}
    ]});
    let request = Request::parse(&text.to_string()).unwrap();
    let crate::Args::Transform(args) = &request.steps[1].args else { panic!("{:?}", request.steps[1].args) };
    let p = args.map.point(occam_units::Point::from_vec3(Vec3::new(1.0, 0.0, 0.0))).to_vec3();
    assert_eq!(p, Vec3::new(2.0 + 25.4, 0.0, 0.0), "scaled by 2, then moved an inch");
}

/// The suite's placement, so that no axis is a world axis.
fn skew() -> Placement<Local, World> {
    let axis = occam_units::Direction::normalised(Vec3::new(1.0, 2.0, 3.0));
    Placement::<Local, Local>::rotation(axis, occam_units::Angle::from_radians(0.7)).then(&Placement::identity())
}

#[test]
fn an_offset_has_no_exact_form_under_a_stretch_and_is_not_approximated() {
    let patch = UvBox::new(UvPoint::new(0.0, 0.0), UvPoint::new(1.0, 1.0));
    let base = SplineSurface::new(&occam_fixtures::spline_workloads::spline()).unwrap();
    let offset: Arc<dyn Surface> = Arc::new(OffsetSurface::new(base, Length::from_millimetres(0.5)).unwrap());
    let stretch = Affine::stretch(&skew(), [2.0, 1.0, 0.5]).unwrap();
    let alike = Affine::stretch(&skew(), [2.0, 2.0, 2.0]).unwrap();
    assert!(super::moved::surface_moved(offset.clone(), patch, &stretch).is_none());
    assert!(super::moved::surface_moved(offset, patch, &alike).is_some(), "a similarity keeps an offset");
    let plane: Arc<dyn Surface> = Arc::new(Plane::placed(&skew()));
    let moved = super::moved::surface_moved(plane, patch, &stretch).unwrap();
    assert_eq!(moved.family().0, "plane", "a plane stays one");
}
