//! Source: original
//! The describer's tests: every id a run's result names is described where it names it, in the request's units, with
//! the step that made it and its neighbours, from the model the run left, from where it was if it is gone, and from a
//! refused step's edit; an id that names nothing is described as such.

use std::collections::BTreeSet;

use occam_ops::{Args, Operation, Request, Step, Units};
use occam_report::{AngleUnit, Code, LengthUnit, Reason, StepRole, Value as Held};
use occam_topo::{Around, End, Snapshot, VertexRecord};
use occam_units::{Length, Point};
use serde_json::{Map, Value, json};

use super::*;
use crate::run::run_request;
use crate::{Ran, ran_json, run};

/// A request in `length` units and degrees of `steps`.
fn request(length: &str, steps: &Value) -> String {
    json!({"schema": 1, "units": {"length": length, "angle": "deg"}, "steps": steps}).to_string()
}

/// The plan's test face, in centimetres: a rectangle 4 by 2 with a round hole of radius 0.5, from two labelled wires.
fn plate() -> Value {
    json!([
        {"op": "wire", "label": "outline", "points": [[0, 0, 0], [4, 0, 0], [4, 2, 0], [0, 2, 0]], "closed": true},
        {"op": "wire", "label": "hole", "edges": [{"circle": {"centre": [2, 1, 0], "radius": 0.5, "normal": [0, 0, 1]}}]},
        {"op": "face", "label": "plate", "outer": "outline", "holes": ["hole"]}
    ])
}

/// Every id `value`, a run's JSON form or any part of it, names, other than inside its descriptions.
fn ids(value: &Value, out: &mut BTreeSet<String>) {
    match value {
        Value::String(text) if EntityRef::parse(text).is_some() => {
            out.insert(text.clone());
        }
        Value::Array(items) => items.iter().for_each(|item| ids(item, out)),
        Value::Object(fields) => fields.iter().filter(|(k, _)| *k != "described").for_each(|(_, v)| ids(v, out)),
        _ => {}
    }
}

/// The entities `value`'s descriptions describe, each once.
fn described_ids(value: &Value) -> BTreeSet<String> {
    let list = value.get("described").and_then(Value::as_array).cloned().unwrap_or_default();
    let found: Vec<String> = list.iter().filter_map(|d| d["entity"].as_str().map(str::to_string)).collect();
    assert_eq!(found.len(), found.iter().collect::<BTreeSet<_>>().len(), "each once: {found:?}");
    found.into_iter().collect()
}

/// Checks that each step and each report of `ran` describes exactly the ids it names, and gives the run's JSON form.
fn every_id_described(ran: &Ran) -> Value {
    let value: Value = serde_json::from_str(&ran_json(ran)).unwrap();
    let mut parts: Vec<&Value> = value["steps"].as_array().unwrap().iter().collect();
    parts.extend(value["failed"]["reports"].as_array().into_iter().flatten());
    parts.extend(value["diagnostics"].as_array().unwrap());
    for part in parts {
        let mut named = BTreeSet::new();
        ids(part, &mut named);
        assert_eq!(described_ids(part), named, "in {part}");
    }
    value
}

/// The description of `entity` in `part`.
fn description<'a>(part: &'a Value, entity: &str) -> &'a Value {
    let list = part["described"].as_array().unwrap();
    list.iter().find(|d| d["entity"] == entity).unwrap_or_else(|| panic!("{entity} in {part}"))
}

/// The value named `name` that `description` shows, in the caller's units.
fn shown<'a>(description: &'a Value, name: &str) -> &'a str {
    let values = description["values"].as_array().unwrap();
    let value = values.iter().find(|v| v["name"] == name).unwrap_or_else(|| panic!("{name} in {description}"));
    value["shown"].as_str().unwrap()
}

#[test]
fn every_id_a_run_names_is_described_where_it_names_it() {
    let ran = run(None, &request("cm", &plate()));
    assert_eq!(ran.failed, None);
    let value = every_id_described(&ran);
    let forms = |step: usize| -> BTreeSet<String> {
        let list = value["steps"][step]["described"].as_array().unwrap();
        list.iter().map(|d| d["form"].as_str().unwrap().to_string()).collect()
    };
    let want = ["edge-use", "line", "vertex", "wire"].map(str::to_string).into();
    assert_eq!(forms(0), want, "a wire body, a wire, and its uses, edges, vertices and lines");
    let want = ["circle", "edge-use", "outer-loop", "inner-loop", "plane", "sheet", "shell", "line", "wire"];
    let want: BTreeSet<String> = want.map(str::to_string).into();
    assert!(forms(2).is_superset(&want), "{:?}", forms(2));
    assert!(!forms(2).contains("none"), "the wires the face took, and their bodies, are described as they last were");
    let face = &value["steps"][2];
    for gone in face["deleted"].as_array().unwrap() {
        let gone = description(face, gone.as_str().unwrap());
        assert!(["wire", "edge-use"].contains(&gone["form"].as_str().unwrap()), "{gone}");
    }
}

#[test]
fn every_id_a_primitive_names_is_described_and_each_solid_gives_its_box() {
    let steps = json!([
        {"op": "box", "size": [10, 20, 30]},
        {"op": "wedge", "size": [10, 5, 4], "top": {"start": 2, "width": 3}},
        {"op": "cylinder", "radius": 5, "height": 10},
        {"op": "cone", "radius1": 5, "radius2": 2, "height": 10},
        {"op": "sphere", "radius": 5},
        {"op": "torus", "major": 10, "minor": 2}
    ]);
    let ran = run(None, &request("mm", &steps));
    assert_eq!(ran.failed, None);
    let value = every_id_described(&ran);
    let boxes = [
        ([0.0, 0.0, 0.0], [10.0, 20.0, 30.0]),
        ([0.0, 0.0, 0.0], [10.0, 5.0, 4.0]),
        ([-5.0, -5.0, 0.0], [5.0, 5.0, 10.0]),
        ([-5.0, -5.0, 0.0], [5.0, 5.0, 10.0]),
        ([-5.0, -5.0, -5.0], [5.0, 5.0, 5.0]),
        ([-12.0, -12.0, -2.0], [12.0, 12.0, 2.0]),
    ];
    let surfaces = ["plane", "plane", "cylinder", "cone", "sphere", "torus"];
    let model = ran.model.as_ref().unwrap();
    for (k, step) in value["steps"].as_array().unwrap().iter().enumerate() {
        let body = model.ids::<Body>()[k].entity().text();
        let solid = description(step, &body);
        assert_eq!((solid["form"].as_str(), shown(solid, "shells")), (Some("solid"), "1"), "{solid}");
        assert_eq!((&solid["origin"]["request-step"], &solid["origin"]["role"]), (&json!(k), &json!("made")));
        let (low, high) = corners(solid);
        assert!(near(low, boxes[k].0) && near(high, boxes[k].1), "the box of step {k}: {solid}");
        let forms: BTreeSet<&str> =
            step["described"].as_array().unwrap().iter().filter_map(|d| d["form"].as_str()).collect();
        assert!(forms.contains(surfaces[k]) && !forms.contains("none"), "step {k}: {forms:?}");
    }
}

#[test]
fn a_face_and_its_edges_are_described_in_the_requests_units_with_the_step_that_made_them() {
    let ran = run(None, &request("cm", &plate()));
    let value = every_id_described(&ran);
    let (wire, face) = (&value["steps"][0], &value["steps"][2]);
    let model = ran.model.as_ref().unwrap();
    let face_id = model.ids::<Face>()[0];
    assert_eq!(face["made"], face_id.entity().text());
    let body = model.view(model.view(face_id).unwrap().shell()).unwrap().body();
    let sheet = description(face, &body.entity().text());
    assert_eq!((sheet["form"].clone(), shown(sheet, "faces")), (json!("sheet"), "1"));
    let plane = description(face, &face_id.entity().text());
    assert_eq!((plane["form"].clone(), shown(plane, "loops")), (json!("plane"), "2"));
    let origin = json!({"step": 3, "request-step": 2, "operation": "face", "role": "made"});
    assert_eq!(plane["origin"], origin);
    let (low, high) = corners(plane);
    assert!(near(low, [0.0, 0.0, 0.0]) && near(high, [40.0, 20.0, 0.0]), "{plane}");
    assert!(low.iter().all(|&x| x <= 0.0) && high[0] >= 40.0 && high[1] >= 20.0, "the box holds the face: {plane}");
    let adjacent: Vec<&str> = plane["adjacent"].as_array().unwrap().iter().map(|a| a.as_str().unwrap()).collect();
    let view = model.view(face_id).unwrap();
    let mut want = vec![view.shell().entity().text(), view.surface_id().entity().text()];
    want.extend(view.loops().into_iter().map(|l| l.entity().text()));
    want.extend(view.edges().into_iter().map(|e| e.entity().text()));
    assert_eq!(adjacent, want, "its shell, its plane, its loops and its edges");
    let loops: Vec<&str> =
        view.loops().iter().map(|&l| description(face, &l.entity().text())["form"].as_str().unwrap()).collect();
    assert_eq!(loops, ["outer-loop", "inner-loop"]);
    // The rectangle's first edge, made by the first step and kept by the face, as the run left it.
    let first = model.view(view.edges()[0]).unwrap();
    let edge = description(wire, &first.id().entity().text());
    assert_eq!(edge["form"], "line");
    assert_eq!((shown(edge, "start"), shown(edge, "finish")), ("(0, 0, 0) cm", "(4, 0, 0) cm"));
    assert_eq!(edge["origin"], json!({"step": 1, "request-step": 0, "operation": "wire", "role": "made"}));
    let edge_adjacent = edge["adjacent"].as_array().unwrap();
    assert_eq!(edge_adjacent[0], face_id.entity().text(), "the face round it first: {edge}");
    let (start, _) = first.ends();
    let corner = description(wire, &start.entity().text());
    assert_eq!((corner["form"].clone(), shown(corner, "point")), (json!("vertex"), "(0, 0, 0) cm"));
    assert_eq!(corner["adjacent"].as_array().map(Vec::len), Some(2), "the two edges that meet there: {corner}");
    assert_eq!(corner.get("origin"), None, "a vertex keeps no step");
    let circle =
        model.ids::<Edge>().into_iter().find(|&e| model.view(e).unwrap().ends().0 == model.view(e).unwrap().ends().1);
    let circle = description(&value["steps"][1], &circle.unwrap().entity().text());
    assert_eq!((circle["form"].clone(), shown(circle, "radius")), (json!("circle"), "0.5 cm"));
    let vertices = circle["adjacent"].as_array().unwrap().iter().filter(|a| a.as_str().unwrap().starts_with("vertex"));
    assert_eq!(vertices.count(), 1, "a closed edge's one vertex, once: {circle}");
    let (low, high) = corners(circle);
    assert!(near(low, [15.0, 5.0, 0.0]) && near(high, [25.0, 15.0, 0.0]), "{circle}");
}

#[test]
fn a_faces_loops_shell_and_body_say_what_they_are_made_of_and_hold_it() {
    let ran = run(None, &request("cm", &plate()));
    let value = every_id_described(&ran);
    let (face, model) = (&value["steps"][2], ran.model.as_ref().unwrap());
    let view = model.view(model.ids::<Face>()[0]).unwrap();
    let body = model.view(view.shell()).unwrap().body();
    let counted = |id: String, name: &str| {
        let d = description(face, &id);
        (d["form"].as_str().unwrap().to_string(), shown(d, name).to_string(), corners(d))
    };
    let [outer, hole] = [view.loops()[0], view.loops()[1]].map(|l| counted(l.entity().text(), "edges"));
    assert_eq!(
        (outer.0.as_str(), outer.1.as_str(), hole.0.as_str(), hole.1.as_str()),
        ("outer-loop", "4", "inner-loop", "1")
    );
    assert!(near(outer.2.0, [0.0; 3]) && near(outer.2.1, [40.0, 20.0, 0.0]) && near(hole.2.1, [25.0, 15.0, 0.0]));
    let shell = counted(view.shell().entity().text(), "faces");
    assert_eq!((shell.1.as_str(), near(shell.2.1, [40.0, 20.0, 0.0])), ("1", true));
    let sheet = counted(body.entity().text(), "shells");
    assert_eq!((sheet.1.as_str(), near(sheet.2.0, [0.0; 3]), near(sheet.2.1, [40.0, 20.0, 0.0])), ("1", true, true));
    assert_eq!(shown(description(face, &body.entity().text()), "wires"), "0");
}

/// The corners of the box `description` gives, in millimetres.
fn corners(description: &Value) -> ([f64; 3], [f64; 3]) {
    let corner = |name: &str| -> [f64; 3] {
        let values = description["values"].as_array().unwrap();
        let held = values.iter().find(|v| v["name"] == name).unwrap_or_else(|| panic!("{name} in {description}"));
        serde_json::from_value(held["value"].clone()).unwrap()
    };
    (corner("low"), corner("high"))
}

/// Whether two points are the same to within rounding.
fn near(got: [f64; 3], want: [f64; 3]) -> bool {
    got.iter().zip(want).all(|(g, w)| (g - w).abs() < 1e-9)
}

#[test]
fn a_wire_alone_and_its_parts_are_described_with_their_boxes_and_counts() {
    let steps = json!([{"op": "wire", "points": [[0, 0, 0], [40, 0, 0], [40, 20, 0], [0, 20, 0]], "closed": true}]);
    let ran = run(None, &request("mm", &steps));
    let value = every_id_described(&ran);
    let (step, model) = (&value["steps"][0], ran.model.as_ref().unwrap());
    let body = model.ids::<Body>()[0];
    let wire = model.view(body).unwrap().wires()[0];
    let edges: Vec<String> = model.ids::<Edge>().iter().map(|e| e.entity().text()).collect();
    let b = description(step, &body.entity().text());
    let counts = ["shells", "faces", "wires"].map(|name| shown(b, name));
    assert_eq!((b["form"].as_str(), counts), (Some("wire"), ["0", "0", "1"]));
    assert!(near(corners(b).0, [0.0; 3]) && near(corners(b).1, [40.0, 20.0, 0.0]), "{b}");
    assert_eq!(b["origin"]["role"], "made");
    let w = description(step, &wire.entity().text());
    assert_eq!((w["form"].as_str(), shown(w, "edges")), (Some("wire"), "4"));
    assert!(near(corners(w).1, [40.0, 20.0, 0.0]), "{w}");
    let mut want = vec![body.entity().text()];
    want.extend(edges.iter().cloned());
    assert_eq!(w["adjacent"], json!(want), "its body, then its edges in order");
    let first = description(step, &edges[0]);
    assert!(near(corners(first).0, [0.0; 3]) && near(corners(first).1, [40.0, 0.0, 0.0]), "{first}");
    assert_eq!(first["adjacent"][0], wire.entity().text(), "the wire that uses it: {first}");
    let u = model.view(wire).unwrap().uses()[0];
    let used = description(step, &u.entity().text());
    assert_eq!((shown(used, "start"), shown(used, "finish")), ("(0, 0, 0) mm", "(40, 0, 0) mm"));
    assert!(near(corners(used).1, [40.0, 0.0, 0.0]), "{used}");
    assert_eq!(used["adjacent"], json!([wire.entity().text(), edges[0]]), "its wire and its edge: {used}");
}

#[test]
fn what_a_refused_step_names_is_described_from_the_model_or_from_nothing() {
    let first = run(None, &request("mm", &plate()));
    let model = first.model.unwrap();
    let face = model.ids::<Face>()[0].entity().text();
    let ran = run(Some(&model), &request("mm", &json!([{"op": "face", "outer": face}])));
    let value = every_id_described(&ran);
    let report = &value["failed"]["reports"][0];
    assert_eq!(report["code"], "construct.not-a-wire");
    assert_eq!(description(report, &face)["form"], "plane", "the face given is described: {report}");
    let gone = run(Some(&model), &request("mm", &json!([{"op": "face", "outer": "wire:40/1"}])));
    let value = every_id_described(&gone);
    let report = &value["failed"]["reports"][0];
    assert_eq!(report["code"], "topo.stale-id");
    assert_eq!(
        description(report, "wire:40/1"),
        &json!({"entity": "wire:40/1", "form": "none", "values": [], "adjacent": []})
    );
}

/// A request of `n` steps that ask for nothing the runner reads, in millimetres.
fn steps(n: u32) -> Request {
    let step = |index| Step { index, label: None, operation: Operation::Box, parameters: Map::new(), args: Args::None };
    let units = Units { length: LengthUnit::Millimetre, angle: AngleUnit::Radian };
    Request { units, tolerance: None, steps: (0..n).map(step).collect() }
}

fn at_x(x: f64) -> VertexRecord {
    let mm = Length::from_millimetres;
    VertexRecord { point: Point::new(mm(x), mm(0.0), mm(0.0)), radius: mm(0.0), star: None }
}

#[test]
fn what_a_refused_commit_names_is_described_from_the_edit_as_it_stood() {
    // The second step adds a vertex whose star names an edge that does not exist: the checker names both.
    let execute = |edit: &mut Edit, step: &Step, _: &occam_ops::Labels| {
        if step.index == 0 {
            return Ok(Some(edit.add(at_x(1.0)).entity()));
        }
        let nowhere = Id::<Edge>::from_entity(EntityRef::new(EntityKind::Edge, 5, 1)).unwrap();
        let star = Some(Around { edge: nowhere, end: End::Start });
        Ok(Some(edit.add(VertexRecord { star, radius: Length::from_millimetres(1e-3), ..at_x(2.0) }).entity()))
    };
    let ran = run_request(None, &steps(2), execute);
    let failed = ran.failed.as_ref().unwrap();
    let report = &failed.reports[0];
    assert_eq!((report.code, report.reason), (Code::InvariantFailed, Some(Reason::DeadLink)));
    let forms: Vec<(String, Option<Held>)> =
        report.described.iter().map(|d| (d.form.clone(), d.value("point").map(|p| p.value))).collect();
    assert_eq!(forms, [("vertex".to_string(), Some(Held::Point([2.0, 0.0, 0.0]))), ("none".to_string(), None)]);
    let radius = report.described[0].value("radius").and_then(NamedValue::number);
    assert_eq!(radius, Some(1e-3), "a computed vertex says how sure its point is");
    let kept = describe(ran.model.as_ref().unwrap(), &[EntityRef::new(EntityKind::Vertex, 0, 1)]);
    assert_eq!(kept[0].values.iter().map(|v| v.name.as_str()).collect::<Vec<_>>(), ["point"], "a given one does not");
    assert_eq!(ran.model.map(|m| m.ids::<Vertex>().len()), Some(1), "the refused vertex never reached a model");
    every_id_described(&run_request(None, &steps(2), execute));
}

#[test]
fn describing_any_ids_gives_one_description_each_in_the_order_asked() {
    let model = run(None, &request("mm", &plate())).model.unwrap();
    let (face, edge) = (model.ids::<Face>()[0].entity(), model.ids::<Edge>()[0].entity());
    let stale = EntityRef::new(EntityKind::Face, face.slot, 2);
    let scratch = EntityRef::new(EntityKind::Section, 0, 0);
    let whole = EntityRef::new(EntityKind::Model, 0, 1);
    let asked = [edge, face, stale, edge, scratch, whole];
    let got = describe(&model, &asked);
    let entities: Vec<EntityRef> = got.iter().map(|d| d.entity).collect();
    assert_eq!(entities, asked);
    let forms: Vec<&str> = got.iter().map(|d| d.form.as_str()).collect();
    assert_eq!(forms, ["line", "plane", "none", "line", "none", "none"]);
    assert_eq!(
        got[1].origin.as_ref().map(|o| (o.step, o.request_step, o.role)),
        Some((3, None, StepRole::Made)),
        "no request here"
    );
    let surface = model.view(model.ids::<Face>()[0]).unwrap().surface_id().entity();
    let curve = model.view(model.ids::<Edge>()[0]).unwrap().curve_id().entity();
    let geometry = describe(&model, &[surface, curve]);
    assert_eq!(geometry.iter().map(|d| d.form.as_str()).collect::<Vec<_>>(), ["plane", "line"]);
    assert!(geometry.iter().all(|d| d.adjacent.is_empty() && d.origin.is_none()), "geometry names no neighbours");
}

#[test]
fn a_box_round_nothing_and_a_union_of_none_are_no_box() {
    assert_eq!(union(Vec::new()), None);
    let mm = Length::from_millimetres;
    let one = Bounds::of([Point::<World>::new(mm(1.0), mm(0.0), mm(0.0))]);
    assert_eq!(union([one, None]), None);
    assert_eq!(union([one, one]), one);
}

#[test]
fn an_edge_made_by_an_extrusion_is_described_as_a_line_made_by_that_step_as_a_profile() {
    let steps = json!([
        {"op": "wire", "label": "square", "points": [[0, 0, 0], [1, 0, 0], [1, 1, 0], [0, 1, 0]], "closed": true},
        {"op": "face", "label": "base", "outer": "square"},
        {"op": "extrude", "face": "base", "vector": [0, 0, 2]}
    ]);
    let ran = run(None, &request("in", &steps));
    assert_eq!(ran.failed, None);
    let value = every_id_described(&ran);
    let extrude = &value["steps"][2];
    let ends = |d: &Value| BTreeSet::from([shown(d, "start").to_string(), shown(d, "finish").to_string()]);
    let rising = BTreeSet::from(["(0, 0, 0) in".to_string(), "(0, 0, 2) in".to_string()]);
    let edges =
        extrude["described"].as_array().unwrap().iter().filter(|d| d["entity"].as_str().unwrap().starts_with("edge:"));
    let up = edges.clone().find(|d| ends(d) == rising);
    let up = up.unwrap_or_else(|| panic!("the edge up from the origin in {extrude}"));
    assert_eq!(up["form"], "line");
    assert_eq!((&up["origin"]["request-step"], &up["origin"]["operation"]), (&json!(2), &json!("extrude")));
    assert_eq!(up["origin"]["role"], "profile", "made by that step from its profile, not carried from the wire");
    let created: BTreeSet<&str> = extrude["created"].as_array().unwrap().iter().filter_map(Value::as_str).collect();
    let made: Vec<&Value> = edges.clone().filter(|d| created.contains(d["entity"].as_str().unwrap())).collect();
    assert_eq!(made.len(), 12, "a cube's edges");
    assert!(made.iter().all(|d| d["origin"]["role"] == "profile"), "every edge it made: {extrude}");
    let gone = edges.clone().find(|d| !created.contains(d["entity"].as_str().unwrap())).unwrap();
    assert_eq!(gone["origin"]["operation"], "wire", "the profile's edges it took, as they last were: {gone}");
    let solid = description(extrude, extrude["made"].as_str().unwrap());
    assert_eq!((solid["form"].as_str(), shown(solid, "faces")), (Some("solid"), "6"), "{solid}");
}

#[test]
fn a_snapshot_is_its_own_source_and_nothing_else_is_read() {
    let model = run(None, &request("mm", &plate())).model.unwrap();
    let edit = model.edit();
    let face = model.ids::<Face>()[0].entity();
    let from_edit = said(Source::Edit(&edit), face).map(|s| s.form);
    assert_eq!(from_edit, Some("plane".to_string()));
    assert!(said(Source::Model(&Snapshot::empty(model.tolerance())), face).is_none());
}

/// Solids every sweep and a turned copy make, in millimetres: c8's hexagon plate, c2's angle bar, c3's stepped shaft, a
/// pin and its copy lying along x, the plate's copy tilted out of every axis, and a sphere and a torus turned.
fn swept_and_turned() -> Value {
    let (r, s) = (30.0, 25.980_762_114);
    json!([
        {"op": "wire", "label": "outline", "closed": true,
         "points": [[r, 0, 0], [15, s, 0], [-15, s, 0], [-r, 0, 0], [-15, -s, 0], [15, -s, 0]]},
        {"op": "face", "label": "blank", "outer": "outline"},
        {"op": "extrude", "label": "plate", "face": "blank", "vector": [0, 0, 5]},
        {"op": "face", "label": "section",
         "outer": {"points": [[0, 0, 0], [50, 0, 0], [50, 0, 5], [5, 0, 5], [5, 0, 40], [0, 0, 40]], "closed": true}},
        {"op": "extrude", "face": "section", "vector": [0, 200, 0]},
        {"op": "face", "label": "profile",
         "outer": {"points": [[0, 0, 0], [20, 0, 0], [20, 0, 30], [12, 0, 30], [12, 0, 80], [0, 0, 80]], "closed": true}},
        {"op": "revolve", "profile": "profile", "origin": [0, 0, 0], "axis": [0, 0, 1], "angle": 360},
        {"op": "cylinder", "label": "pin", "radius": 4, "height": 30},
        {"op": "copy", "target": "pin", "frame": {"origin": [100, 0, 0], "z": [1, 0, 0], "x": [0, 0, -1]}},
        {"op": "copy", "target": "plate", "frame": {"origin": [0, 300, 0], "z": [1, 2, 2], "x": [2, 1, -2]}},
        {"op": "sphere", "label": "ball", "radius": 10},
        {"op": "copy", "target": "ball", "frame": {"origin": [0, 0, 400], "z": [0, 1, 1], "x": [1, 0, 0]}},
        {"op": "torus", "label": "ring", "major": 30, "minor": 8},
        {"op": "copy", "target": "ring", "frame": {"origin": [0, 0, 500], "z": [1, 1, 1], "x": [1, -1, 0]}}
    ])
}

/// The corners of `values`' box under `low` and `high`.
fn box_of(values: &[NamedValue], low: &str, high: &str) -> ([f64; 3], [f64; 3]) {
    let point = |name: &str| match values.iter().find(|v| v.name == name).map(|v| &v.value) {
        Some(Held::Point(p)) => *p,
        other => panic!("{name}: {other:?} in {values:?}"),
    };
    (point(low), point(high))
}

#[test]
fn a_faces_box_and_a_bodys_are_as_tight_as_measure_finds_them_after_any_sweep_or_turned_copy() {
    let ran = run(None, &request("mm", &swept_and_turned()));
    assert_eq!(ran.failed, None);
    let model = ran.model.as_ref().unwrap();
    let band = model.tolerance().length().millimetres();
    let mut ids: Vec<EntityRef> = model.ids::<Face>().iter().map(|f| f.entity()).collect();
    ids.extend(model.ids::<Body>().iter().map(|b| b.entity()));
    let mut loose = Vec::new();
    for (id, said) in ids.iter().zip(describe(model, &ids)) {
        let text = json!({"schema": 1, "units": {"length": "mm", "angle": "deg"},
                          "steps": [{"op": "measure", "target": id.text(), "what": ["bounds"]}]});
        let asked = Request::parse(&text.to_string()).unwrap();
        let tight = occam_ops::measure(model, &asked.steps[0], &occam_ops::Labels::default()).unwrap();
        let (low, high) = box_of(&said.values, "low", "high");
        let (true_low, true_high) = box_of(&tight.values, "bounds-low", "bounds-high");
        let gaps: Vec<(f64, f64)> = (0..3).map(|k| (true_low[k] - low[k], high[k] - true_high[k])).collect();
        assert!(
            gaps.iter().all(|&(under, over)| under >= -1e-9 && over >= -1e-9),
            "{} holds {true_low:?}..{true_high:?}: {low:?}..{high:?}",
            id.text()
        );
        if gaps.iter().any(|&(under, over)| under > band || over > band) {
            loose.push(format!("{} is {true_low:?}..{true_high:?}, described {low:?}..{high:?}", id.text()));
        }
    }
    assert!(loose.is_empty(), "{} of {} boxes are looser than the band:\n{}", loose.len(), ids.len(), loose.join("\n"));
}
