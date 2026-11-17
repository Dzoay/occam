//! Source: original
//! The runner's tests: a failing step leaves the model as the last good step left it, labels name what earlier steps
//! made, a new model takes the request's tolerance or the default and says which, and the run's JSON form. Every
//! operation is refused when a request is read until its step lands, so these drive the runner with steps that add
//! vertices.

use occam_ops::{Args, Operation, Request, Step, Units};
use occam_report::{AngleUnit, DisplayUnits, EntityKind, Input, LengthUnit, Reason, followed_remedy};
use occam_topo::{Derivation, Lineage, Vertex, VertexRecord};
use occam_units::{Angle, Length, Point};
use serde_json::{Map, Value, json};

use super::*;

/// `n` steps, each labelled `s<index>`.
fn steps(n: u32) -> Vec<Step> {
    let step = |index| Step {
        index,
        label: Some(format!("s{index}")),
        operation: Operation::Box,
        parameters: Map::new(),
        args: Args::None,
    };
    (0..n).map(step).collect()
}

/// A request of `steps` in `length` units and radians, with no tolerance.
fn request(steps: Vec<Step>, length: LengthUnit) -> Request {
    Request { units: Units { length, angle: AngleUnit::Radian }, tolerance: None, steps }
}

/// The vertex at `x` millimetres along the x axis.
fn vertex(x: f64) -> VertexRecord {
    let at = |v| Length::from_millimetres(v);
    VertexRecord { point: Point::new(at(x), at(0.0), at(0.0)), radius: at(0.0), star: None }
}

/// What the step that fails reports: a length, to show the wording in the request's units.
fn boom() -> Report {
    Report::new(Code::OutOfRange, Fault::InvalidInput).with_value(NamedValue::length("value", 20.0))
}

/// Each step adds a vertex at its index and makes it, until step `failing`, which reports [`boom`].
fn adds_vertices(failing: Option<u32>) -> impl FnMut(&mut Edit, &Step, &Labels) -> Result<Option<EntityRef>, Report> {
    move |edit, step, _| {
        if Some(step.index) == failing {
            return Err(boom());
        }
        Ok(Some(edit.add(vertex(f64::from(step.index))).entity()))
    }
}

/// A model with one vertex in it, at the default tolerance.
fn base() -> Snapshot {
    let mut edit = Snapshot::empty(default_tolerance().unwrap()).edit();
    edit.add(vertex(-1.0));
    edit.commit(&Exec::serial()).unwrap().0
}

fn vertex_ref(slot: u32) -> EntityRef {
    EntityRef::new(EntityKind::Vertex, slot, 1)
}

#[test]
fn a_failing_step_stops_the_run_and_leaves_the_model_as_the_last_good_step_left_it() {
    let base = base();
    let ran = run_request(Some(&base), &request(steps(4), LengthUnit::Millimetre), adds_vertices(Some(2)));
    assert_eq!(ran.failed, Some(Failed { step: Some(2), reports: vec![boom()] }));
    let made: Vec<Option<EntityRef>> = ran.done.iter().map(|done| done.made).collect();
    assert_eq!(made, [Some(vertex_ref(1)), Some(vertex_ref(2))]);
    let created: Vec<Vec<EntityRef>> = ran.done.iter().map(|done| done.changes.created.clone()).collect();
    assert_eq!(created, [vec![vertex_ref(1)], vec![vertex_ref(2)]]);
    let model = ran.model.unwrap();
    let two = run_request(Some(&base), &request(steps(2), LengthUnit::Millimetre), adds_vertices(None));
    assert_eq!((two.failed, two.done.len()), (None, 2));
    assert_eq!(model, two.model.unwrap(), "the model after the last good step");
    assert_eq!((model.step(), model.ids::<Vertex>().len()), (base.step() + 2, 3));
    assert_eq!(base.ids::<Vertex>().len(), 1, "the model given is unchanged");
}

#[test]
fn a_step_whose_edit_commit_refuses_stops_the_run_with_the_checkers_reports() {
    let execute = |edit: &mut Edit, step: &Step, _: &Labels| {
        if step.index == 1 {
            let _ = edit.reserve::<Vertex>();
            return Ok(None);
        }
        Ok(Some(edit.add(vertex(0.0)).entity()))
    };
    let ran = run_request(None, &request(steps(3), LengthUnit::Millimetre), execute);
    let failed = ran.failed.unwrap();
    assert_eq!(failed.step, Some(1));
    let reasons: Vec<(Code, Option<Reason>)> = failed.reports.iter().map(|r| (r.code, r.reason)).collect();
    assert_eq!(reasons, [(Code::InvariantFailed, Some(Reason::ReservedNotSet))]);
    assert_eq!(ran.done.len(), 1);
    assert_eq!(ran.model.unwrap().ids::<Vertex>().len(), 1, "the model the first step left");
}

#[test]
fn a_later_step_finds_what_a_labelled_step_made_and_its_lineage_is_given() {
    let mut seen = Vec::new();
    let execute = |edit: &mut Edit, step: &Step, labels: &Labels| {
        seen.push(labels.get("s0"));
        let made = edit.add(vertex(f64::from(step.index)));
        if let Some(earlier) = labels.get("s0") {
            edit.derive(made.entity(), earlier, Derivation::Copied)?;
        }
        Ok(Some(made.entity()))
    };
    let ran = run_request(None, &request(steps(2), LengthUnit::Millimetre), execute);
    assert_eq!(seen, [None, Some(vertex_ref(0))]);
    let lineage = Lineage { new: vertex_ref(1), old: vertex_ref(0), how: Derivation::Copied };
    assert_eq!(ran.done[1].changes.lineage, [lineage]);
}

/// A request of no steps, with `tolerance` after its units if given.
fn empty(tolerance: &str) -> String {
    format!(r#"{{"schema": 1, "units": {{"length": "mm", "angle": "rad"}}{tolerance}, "steps": []}}"#)
}

#[test]
fn a_new_model_takes_the_requests_tolerance_or_the_default_and_says_which() {
    let default = default_tolerance().unwrap();
    let ran = run(None, &empty(""));
    assert_eq!((ran.failed.clone(), ran.tolerance), (None, Some((default, ToleranceFrom::Default))));
    assert_eq!(ran.model.map(|m| (m.tolerance(), m.step())), Some((default, 0)));
    let asked = Tolerance::new(Length::from_millimetres(1e-6), Angle::from_radians(1e-8)).unwrap();
    let ran = run(None, &empty(r#", "tolerance": {"length": 1e-6, "angle": 1e-8}"#));
    assert_eq!(ran.tolerance, Some((asked, ToleranceFrom::Request)));
    assert_eq!(ran.model.map(|m| m.tolerance()), Some(asked));
}

#[test]
fn on_a_model_its_own_tolerance_governs_and_a_finer_one_is_refused_with_the_models() {
    let model = base();
    let coarser = run(Some(&model), &empty(r#", "tolerance": {"length": 1e-6, "angle": 1e-8}"#));
    assert_eq!(coarser.tolerance, Some((model.tolerance(), ToleranceFrom::Model)));
    assert_eq!(coarser.model.as_ref(), Some(&model));
    let finer = |length: f64, angle: f64| format!(r#", "tolerance": {{"length": {length:e}, "angle": {angle:e}}}"#);
    for (length, angle) in [(1e-8, 1e-9), (1e-7, 1e-10)] {
        let ran = run(Some(&model), &empty(&finer(length, angle)));
        assert_eq!((ran.model.as_ref(), ran.tolerance, ran.done.len()), (Some(&model), None, 0));
        let failed = ran.failed.unwrap();
        assert_eq!(failed.step, None);
        let report = failed.reports[0].clone();
        assert_eq!((report.code, report.input.clone()), (Code::ToleranceBelowModel, Some(Input::at("tolerance"))));
        assert_eq!(report.value("requested").and_then(NamedValue::number), Some(length));
        assert_eq!(report.value("requested-angle").and_then(NamedValue::number), Some(angle));
        followed_remedy!(report, Code::ToleranceBelowModel, RemedyKind::UseModelTolerance, |remedy| {
            let value = |name| remedy.value(name).and_then(NamedValue::number).unwrap_or(0.0);
            let ran = run(Some(&model), &empty(&finer(value("tolerance"), value("angle"))));
            ran.failed.map_or(Ok(ran.tolerance), |mut failed| Err(failed.reports.remove(0)))
        });
    }
}

#[test]
fn a_request_refused_when_it_is_read_runs_nothing_and_changes_nothing() {
    let text = r#"{"schema": 1, "units": {"length": "mm", "angle": "rad"}, "steps": [{"op": "loft"}]}"#;
    let ran = run(None, text);
    assert_eq!((ran.model, ran.tolerance, ran.done.len()), (None, None, 0));
    let failed = ran.failed.unwrap();
    assert_eq!((failed.step, failed.reports.len()), (None, 1));
    assert_eq!(failed.reports[0].code, Code::UnknownOperation);
    let model = base();
    assert_eq!(run(Some(&model), text).model, Some(model.clone()));
    assert_eq!(run(Some(&model), "not json").failed.map(|f| f.reports[0].code), Some(Code::RequestMalformed));
}

#[test]
fn a_request_refused_when_it_is_read_is_worded_in_its_own_units_once_they_are_read() {
    let words = |units: &str| {
        let text = format!(
            r#"{{"schema": 1, "units": {units}, "steps": [{{"op": "box", "size": [-1, 1, 1]}},
                {{"op": "revolve", "profile": "p", "origin": [0, 0, 0], "axis": [0, 0, 1], "angle": 400}}]}}"#
        );
        let ran = run(None, &text);
        let reports = ran.failed.map_or_default(|failed| failed.reports);
        (ran.units, reports.iter().map(|report| report.message(ran.units)).collect::<Vec<String>>())
    };
    let inch = DisplayUnits { length: LengthUnit::Inch, angle: AngleUnit::Degree };
    let size = |shown: &str| format!("The size given, {shown}, is not a finite number greater than zero.");
    let angle = "steps[1].angle is 400 deg, outside the range allowed: more than 0 deg and at most 360 deg.";
    assert_eq!(words(r#"{"length": "in", "angle": "deg"}"#), (inch, vec![size("-1 in"), angle.to_string()]));
    // Units that cannot be read leave every number in millimetres and radians, as it was read and as it is worded.
    let (units, unread) = words(r#"{"length": "in", "angle": "turns"}"#);
    assert_eq!((units, unread.get(1)), (DisplayUnits::default(), Some(&size("-1 mm"))));
}

#[test]
fn a_run_has_one_json_form_with_each_steps_changes_and_the_failure_in_the_requests_units() {
    let execute = |edit: &mut Edit, step: &Step, labels: &Labels| {
        let made = adds_vertices(Some(2))(edit, step, labels)?;
        if let (Some(made), Some(earlier)) = (made, labels.get("s0")) {
            edit.derive(made, earlier, Derivation::Copied)?;
        }
        Ok(made)
    };
    let ran = run_request(None, &request(steps(3), LengthUnit::Centimetre), execute);
    let value: Value = serde_json::from_str(&ran_json(&ran)).unwrap();
    let note = json!({
        "schema": 2, "code": "measure.tolerance-used", "severity": "note",
        "message": "The operation worked to a tolerance of 1e-8 cm.",
        "entities": [], "remedies": [],
        "values": [{"name": "tolerance", "quantity": "length", "value": 1e-7}, {"name": "angle", "quantity": "angle", "value": 1e-9}],
    });
    let boom = json!({
        "schema": 2, "code": "geom.out-of-range", "severity": "error", "fault": "invalid-input",
        "message": "The value given, 2 cm, is not strictly between {low} and {high}.",
        "entities": [], "remedies": [], "values": [{"name": "value", "quantity": "length", "value": 20.0}],
    });
    // Each vertex described as the run left it, in the request's units: a vertex keeps no step.
    let at = |slot: u32, x: f64, shown: &str| {
        json!({"entity": format!("vertex:{slot}/1"), "form": "vertex", "adjacent": [],
               "values": [{"name": "point", "quantity": "length", "value": [x, 0.0, 0.0], "shown": shown}]})
    };
    let want = json!({
        "schema": 2,
        "ok": false,
        "tolerance": {"length": 1e-7, "angle": 1e-9, "from": "default"},
        "steps": [
            {"step": 0, "label": "s0", "operation": "box", "made": "vertex:0/1", "created": ["vertex:0/1"],
             "changed": [], "deleted": [], "lineage": [], "described": [at(0, 0.0, "(0, 0, 0) cm")]},
            {"step": 1, "label": "s1", "operation": "box", "made": "vertex:1/1", "created": ["vertex:1/1"],
             "changed": [], "deleted": [], "lineage": [{"new": "vertex:1/1", "old": "vertex:0/1", "how": "copied"}],
             "described": [at(0, 0.0, "(0, 0, 0) cm"), at(1, 1.0, "(0.1, 0, 0) cm")]},
        ],
        "failed": {"step": 2, "reports": [boom]},
        "diagnostics": [note],
    });
    assert_eq!(value, want);
    let refused: Value = serde_json::from_str(&ran_json(&run(None, "[1"))).unwrap();
    assert_eq!(refused["ok"], json!(false));
    assert_eq!(
        (refused.get("tolerance"), refused["steps"].clone(), refused["diagnostics"].clone()),
        (None, json!([]), json!([]))
    );
    assert_eq!(refused["failed"].get("step"), None);
    assert_eq!(refused["failed"]["reports"][0]["code"], json!("request.malformed"));
}

#[test]
fn a_measure_changes_nothing_and_gives_its_values_in_millimetres_shown_in_the_requests_units() {
    let request = r#"{"schema": 1, "units": {"length": "cm", "angle": "rad"}, "steps": [
        {"op": "box", "label": "block", "size": [1, 2, 3]},
        {"op": "measure", "target": "block", "what": ["volume", "bounds"]}
    ]}"#;
    let ran = run(None, request);
    assert_eq!(ran.failed, None);
    let measure = &ran.done[1];
    assert_eq!((measure.made, &measure.changes), (None, &Changes::default()));
    assert_eq!(ran.model.as_ref().map(Snapshot::step), Some(1), "the box's step, and no other");
    let measured = measure.measured.as_ref().unwrap();
    assert_eq!(Some(measured.target), ran.done[0].made);
    let volume = measured.values[0].number().unwrap();
    assert!((volume - 6000.0).abs() <= 6000.0 * occam_ops::RELATIVE, "{volume}");
    let value: Value = serde_json::from_str(&ran_json(&ran)).unwrap();
    let step = &value["steps"][1];
    assert_eq!(
        (step["operation"].clone(), step["created"].clone(), step.get("made")),
        (json!("measure"), json!([]), None)
    );
    assert_eq!(step["measured"]["target"], json!(measured.target.text()));
    let shown = measured.values[0].shown(DisplayUnits { length: LengthUnit::Centimetre, angle: AngleUnit::Radian });
    assert_eq!(
        step["measured"]["values"][0],
        json!({"name": "volume", "quantity": "volume", "value": volume, "shown": shown})
    );
    // The box holds the block, rounding included: each corner a few roundings out, and shown in centimetres.
    let high = &measured.values[2];
    let occam_report::Value::Point(corner) = high.value else { panic!("{high:?}") };
    assert!(corner.iter().zip([10.0, 20.0, 30.0]).all(|(c, w)| *c >= w && *c - w < 1e-12), "{corner:?}");
    let shown = high.shown(DisplayUnits { length: LengthUnit::Centimetre, angle: AngleUnit::Radian });
    assert!(shown.starts_with("(1") && shown.ends_with(") cm"), "{shown}");
    let want = json!({"name": "bounds-high", "quantity": "length", "value": corner, "shown": shown});
    assert_eq!(step["measured"]["values"][2], want);
    assert_eq!(value["steps"][0].get("measured"), None, "a step that measures nothing says nothing of it");
    // A measure that fails stops the run there, as any step does, the model as the step before it left it.
    let refused = run(None, &request.replace("\"target\": \"block\"", "\"target\": \"plate\""));
    let failed = refused.failed.as_ref().map(|f| (f.step, f.reports[0].code));
    assert_eq!((failed, refused.done.len()), (Some((Some(1), Code::InputUnknownLabel)), 1));
    assert_eq!(refused.model.as_ref().map(Snapshot::step), Some(1));
}

#[test]
fn every_way_a_lineage_is_made_has_its_name() {
    let ways = [Derivation::Split, Derivation::Trimmed, Derivation::Merged, Derivation::Copied, Derivation::Swept];
    assert_eq!(ways.map(wire::how), ["split", "trimmed", "merged", "copied", "swept"]);
    let froms = [ToleranceFrom::Request, ToleranceFrom::Default, ToleranceFrom::Model].map(wire::from);
    assert_eq!(froms, ["request", "default", "model"]);
}

#[test]
fn a_runs_measurements_come_first_in_its_json_before_the_long_list_of_what_each_step_made() {
    // The blind run's c4 and c7 found their measurements at the end of 75 to 93 KB, after every step's descriptions.
    let text = r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
        {"op": "box", "label": "block", "size": [40, 20, 10]},
        {"op": "cylinder", "label": "pin", "radius": 4, "height": 30},
        {"op": "measure", "target": "block", "what": ["volume"]},
        {"op": "measure", "target": "pin", "what": ["volume", "area"]}
    ]}"#;
    let json = ran_json(&crate::run(None, text));
    let value: Value = serde_json::from_str(&json).unwrap();
    let first = |k: usize| {
        let measured = &value["steps"][k]["measured"];
        json!({"step": k, "target": measured["target"], "values": measured["values"]})
    };
    assert_eq!(value["measured"], json!([first(2), first(3)]), "each measure step's, in order: {json}");
    let (summary, steps) = (json.find(r#""measured":[{"step""#), json.find(r#""steps":"#));
    assert!(summary.is_some_and(|at| steps.is_some_and(|steps| at < steps)), "before the steps: {json}");
    let none = ran_json(&crate::run(None, r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": []}"#));
    assert!(!none.contains(r#""measured""#), "a run that measured nothing says nothing of it: {none}");
}

#[test]
fn a_step_taken_by_hand_leaves_what_the_runner_leaves_and_a_refused_one_gives_back_its_edit() {
    let ran = run_request(None, &request(steps(3), LengthUnit::Millimetre), adds_vertices(None));
    let mut model = Snapshot::empty(default_tolerance().unwrap());
    for index in 0..3_u32 {
        let add = |edit: &mut Edit| Ok(edit.add(vertex(f64::from(index))).entity());
        let Ok((next, made, changes)) = step(&model, index, Operation::Box.name(), add) else { panic!("step {index}") };
        assert_eq!((Some(made), &changes), (ran.done[index as usize].made, &ran.done[index as usize].changes));
        model = next;
    }
    let runners = ran.model.unwrap();
    assert_eq!(model.digest(), runners.digest());
    let notes = |model: &Snapshot| model.notes().map(|(step, note)| (step, note.clone())).collect::<Vec<_>>();
    assert_eq!(notes(&model), notes(&runners), "each step named as its request numbers and names it");
    assert_eq!(notes(&model).len(), 3);
    let refused = step(&model, 3, "box", |edit| {
        edit.add(vertex(9.0));
        Err::<(), _>(boom())
    });
    let Err((reports, edit)) = refused else { panic!("the step was not refused") };
    assert_eq!((reports, edit.base().digest()), (vec![boom()], model.digest()));
    assert_eq!(edit.note(edit.step()).map(|note| note.request_step), Some(3), "named before the work was done");
}
