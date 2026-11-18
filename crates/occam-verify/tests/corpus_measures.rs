//! Source: original
//! occam's measures against the corpus's recorded answers (L11, L12). Every `occam-oracle-case/1` file under
//! `tools/corpus` that records an `expect` and whose every operation the runner builds is built through `occam`'s
//! runner and measured, and [`judge`] must say the result agrees with the case's answer, within the tolerances the case
//! gives. The answers are closed-form, written by `tools/corpus/generate.py`, never copied from a run of the oracle or
//! of occam (`tools/oracle/README.md`). The oracle's own results are not in the repository (docs/PLAN.md, O6), so
//! nothing here reads them.
//!
//! A case that is not compared is counted by why, and the counts are pinned ([`LEFT_OUT`]): a case the runner
//! refuses, a disagreement, a count that moves or an empty corpus fails the test ([`faults`]). A `tessellate`
//! makes no shape and is passed over, so a case's `watertight` is not judged here.
#![cfg(not(target_family = "wasm"))]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stdout,
    clippy::arithmetic_side_effects,
    reason = "a test: a malformed case should fail it, its totals are its output, and its counts are small"
)]

use std::collections::{BTreeMap, BTreeSet};

use occam_topo::{Body, Face, Id, Snapshot};
use occam_verify::{Closeness, Measure, Measured, Reading, Tolerances, Verdict, judge};
use serde_json::{Value, json};

mod corpus;

use corpus::{BUILT, cases, step, three};

/// The cases that are not compared, by why, and how many: a count that moves is looked at, and changed here.
const LEFT_OUT: [(&str, usize); 6] = [
    ("it needs a common", 45),
    ("it needs a cut", 93),
    ("it needs a fillet", 16),
    ("it needs a fuse", 41),
    ("it records no answer", 306),
    ("its expect has no bbox", 1),
];

/// A measure's tolerance in a case's `expect`: `rel_tol`, `abs_tol`, or both.
fn closeness(measure: &Value) -> Closeness {
    let part = |name: &str| measure.get(name).map_or(0.0, |x| x.as_f64().unwrap());
    Closeness { rel: part("rel_tol"), abs: part("abs_tol") }
}

/// What a case records of its result, with the tolerances it gives, or the first measure it does not record. A result
/// is valid and closed unless the case says otherwise (`tools/oracle/README.md`, the invariants).
fn recorded(expect: &Value) -> Result<(Measured, Tolerances), String> {
    let part = |name: &str| expect.get(name).ok_or_else(|| format!("its expect has no {name}"));
    let (volume, area, bbox, euler) = (part("volume")?, part("area")?, part("bbox")?, part("euler")?);
    let flag = |name: &str| expect.get(name).is_none_or(|flag| flag["value"].as_bool().unwrap());
    let measured = Measured {
        volume: volume["value"].as_f64().unwrap(),
        area: area["value"].as_f64().unwrap(),
        bbox: Some((three(&bbox["min"]), three(&bbox["max"]))),
        euler: euler["value"].as_i64().unwrap(),
        valid: flag("valid"),
        closed: flag("closed"),
    };
    let tolerances =
        Tolerances { volume: closeness(volume), area: closeness(area), bbox: bbox["abs_tol"].as_f64().unwrap() };
    Ok((measured, tolerances))
}

/// The request that builds a case's result and measures it, or why it cannot be built yet.
fn request(case: &Value) -> Result<String, String> {
    let ops = case["ops"].as_array().unwrap().iter().filter(|op| op["op"] != "tessellate");
    let mut steps = Vec::new();
    for op in ops {
        let kind = op["op"].as_str().unwrap();
        if !BUILT.contains(&kind) {
            return Err(format!("it needs a {kind}"));
        }
        steps.push(step(op));
    }
    steps.push(json!({"op": "measure", "target": case["result"]}));
    Ok(json!({"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": steps}).to_string())
}

/// `x` as a signed count.
fn signed(x: usize) -> i64 {
    i64::try_from(x).unwrap()
}

/// A body's Euler characteristic, `V - E + 2F - L`, and whether every edge of it is used an even number of times.
fn shape(model: &Snapshot, body: Id<Body>) -> (i64, bool) {
    let shells = model.view(body).unwrap().shells();
    let faces: Vec<_> = shells.into_iter().flat_map(|shell| model.view(shell).unwrap().faces()).collect();
    let loops: usize = faces.iter().map(|face| model.view(*face).unwrap().loops().len()).sum();
    let edges: BTreeSet<_> = faces.iter().flat_map(|face| model.view(*face).unwrap().edges()).collect();
    let ends = edges.iter().flat_map(|edge| <[_; 2]>::from(model.view(*edge).unwrap().ends()));
    let vertices: BTreeSet<_> = ends.collect();
    let closed = edges.iter().all(|edge| model.view(*edge).unwrap().uses().len().is_multiple_of(2));
    (signed(vertices.len()) - signed(edges.len()) + 2 * signed(faces.len()) - signed(loops), closed)
}

/// What occam measures of a run's last step's target, a body. A run the runner accepted is valid: it checks each step.
fn ours(ran: &occam::Ran) -> Measured {
    let measured = ran.done.last().unwrap().measured.as_ref().unwrap();
    let value = |name: &str| measured.values.iter().find(|value| value.name == name).unwrap();
    let number = |name: &str| value(name).number().unwrap();
    let corner = |name: &str| three(&serde_json::to_value(value(name)).unwrap()["value"]);
    let model = ran.model.as_ref().unwrap();
    let (euler, closed) = shape(model, model.resolve::<Body>(measured.target).unwrap());
    let bbox = Some((corner("bounds-low"), corner("bounds-high")));
    Measured { volume: number("volume"), area: number("area"), bbox, euler, valid: ran.failed.is_none(), closed }
}

/// Everything wrong with what the corpus gave; nothing, if the test passes.
fn faults(tally: &Tally) -> Vec<String> {
    if tally.cases == 0 {
        return vec!["the corpus is empty".to_string()];
    }
    let refused = tally.refused.iter().map(|(case, codes)| format!("{case}: the runner refused it: {codes}"));
    let disagreed = tally
        .disagreed
        .iter()
        .map(|(case, measures)| format!("{case}: occam disagrees with the recorded answer on {measures:?}"));
    let mut found: Vec<String> = refused.chain(disagreed).collect();
    let pinned: BTreeMap<String, usize> = LEFT_OUT.iter().map(|(why, count)| ((*why).to_string(), *count)).collect();
    if tally.left_out != pinned {
        found.push(format!("the cases left out are {:?}, and {pinned:?} are pinned", tally.left_out));
    }
    if tally.agreed == 0 {
        found.push("nothing was compared".to_string());
    }
    found
}

/// What the corpus gave.
#[derive(Default)]
struct Tally {
    cases: usize,
    agreed: usize,
    /// The cases whose measures differ from their answers, each with the measures.
    disagreed: BTreeMap<String, Vec<Measure>>,
    /// The cases the runner refused, each with its reports' codes.
    refused: BTreeMap<String, String>,
    /// The cases not compared, by why.
    left_out: BTreeMap<String, usize>,
}

impl Tally {
    /// The case built, measured and judged, or the reason it is left out counted.
    fn case(&mut self, name: &str, case: &Value) {
        self.cases = self.cases.saturating_add(1);
        let Some(expect) = case.get("expect") else {
            return self.leave_out("it records no answer");
        };
        let (request, (answer, tolerances)) = match request(case).and_then(|r| Ok((r, recorded(expect)?))) {
            Ok(both) => both,
            Err(why) => return self.leave_out(&why),
        };
        let ran = occam::run(None, &request);
        if let Some(failed) = &ran.failed {
            let codes: Vec<&str> = failed.reports.iter().map(|r| r.code.text()).collect();
            self.refused.insert(name.to_string(), codes.join(", "));
            return;
        }
        match judge(&ours(&ran), &Reading::Shape(answer), tolerances) {
            Verdict::Agrees => self.agreed = self.agreed.saturating_add(1),
            Verdict::Disagrees(found) => {
                self.disagreed.insert(name.to_string(), found.iter().map(|d| d.measure).collect());
            }
            Verdict::OracleCouldNot | Verdict::Nothing(_) => self.leave_out("it has no reading"),
        }
    }

    fn leave_out(&mut self, why: &str) {
        let count = self.left_out.entry(why.to_string()).or_default();
        *count = count.saturating_add(1);
    }
}

/// Every case of `all` tried.
fn tried(all: &[(String, Value)]) -> Tally {
    let mut tally = Tally::default();
    for (name, case) in all {
        tally.case(name, case);
    }
    tally
}

#[test]
fn on_the_corpus_occams_measures_agree_with_the_recorded_answers() {
    let tally = tried(&cases());
    println!("corpus: {} cases, {} agree with their answers", tally.cases, tally.agreed);
    for (why, count) in &tally.left_out {
        println!("corpus: cases left out {count}: {why}");
    }
    for (case, measures) in &tally.disagreed {
        println!("corpus: {case} disagrees on {measures:?}");
    }
    assert_eq!(faults(&tally), Vec::<String>::new());
}

/// The `expect` of a case whose result has these measures, to a billionth.
fn answer(volume: f64, area: f64, bbox: [[f64; 3]; 2], euler: i64) -> Value {
    json!({"volume": {"value": volume, "rel_tol": 1e-9}, "area": {"value": area, "rel_tol": 1e-9},
        "euler": {"value": euler}, "bbox": {"min": bbox[0], "max": bbox[1], "abs_tol": 1e-9}})
}

/// A case of one box, 2 by 3 by 4 with its low corner at (1, 1, 1), and its answer worked by hand: 2 x 3 x 4 = 24,
/// 2 (6 + 8 + 12) = 52, and 8 - 12 + 2 x 6 - 6 = 2.
fn a_box() -> Value {
    json!({"schema": "occam-oracle-case/1", "result": "a",
        "ops": [{"op": "box", "id": "a", "size": [2.0, 3.0, 4.0], "origin": [1.0, 1.0, 1.0]}],
        "expect": answer(24.0, 52.0, [[1.0, 1.0, 1.0], [3.0, 4.0, 5.0]], 2)})
}

fn one_case(case: Value) -> Tally {
    tried(&[("seeded".to_string(), case)])
}

/// What the corpus gives when nothing is wrong, in the pinned counts.
fn sound() -> Tally {
    let left_out = LEFT_OUT.iter().map(|(why, count)| ((*why).to_string(), *count)).collect();
    Tally { cases: 578, agreed: 76, left_out, ..Tally::default() }
}

#[test]
fn a_box_agrees_with_its_answer_worked_by_hand_and_a_sound_tally_has_no_fault() {
    let tally = one_case(a_box());
    assert_eq!((tally.cases, tally.agreed, &tally.disagreed), (1, 1, &BTreeMap::new()));
    assert_eq!(faults(&sound()), Vec::<String>::new());
}

#[test]
fn each_measure_that_is_wrong_is_a_disagreement_and_a_fault_that_names_the_case() {
    let seeded = [
        (answer(24.001, 52.0, [[1.0, 1.0, 1.0], [3.0, 4.0, 5.0]], 2), Measure::Volume),
        (answer(24.0, 52.001, [[1.0, 1.0, 1.0], [3.0, 4.0, 5.0]], 2), Measure::Area),
        (answer(24.0, 52.0, [[1.0, 1.0, 1.001], [3.0, 4.0, 5.0]], 2), Measure::Bbox),
        (answer(24.0, 52.0, [[1.0, 1.0, 1.0], [3.0, 4.0, 5.0]], 0), Measure::Euler),
    ];
    for (expect, measure) in seeded {
        let mut case = a_box();
        case["expect"] = expect;
        let tally = one_case(case);
        assert_eq!((tally.agreed, tally.disagreed.get("seeded")), (0, Some(&vec![measure])));
        let found = faults(&Tally { disagreed: tally.disagreed, ..sound() });
        assert_eq!(found, [format!("seeded: occam disagrees with the recorded answer on {:?}", [measure])]);
    }
    // An answer that says the result is open or invalid is not occam's closed, accepted box.
    for (flag, measure) in [("closed", Measure::Closed), ("valid", Measure::Valid)] {
        let mut case = a_box();
        case["expect"][flag] = json!({"value": false});
        assert_eq!(one_case(case).disagreed.get("seeded"), Some(&vec![measure]));
    }
}

#[test]
fn a_sheet_is_open_and_a_solid_is_closed() {
    // A rectangle: 4 - 4 + 2 x 1 - 1 = 1, each edge used once. A box: 2, each edge used twice.
    let request = json!({"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
        {"op": "face", "outer": {"points": [[0, 0, 0], [4, 0, 0], [4, 2, 0], [0, 2, 0]], "closed": true}},
        {"op": "box", "size": [1, 2, 3]}
    ]});
    let ran = occam::run(None, &request.to_string());
    assert_eq!(ran.failed, None);
    let model = ran.model.as_ref().unwrap();
    let face = model.resolve::<Face>(ran.done[0].made.unwrap()).unwrap();
    let sheet = model.view(model.view(face).unwrap().shell()).unwrap().body();
    let solid = model.resolve::<Body>(ran.done[1].made.unwrap()).unwrap();
    assert_eq!((shape(model, sheet), shape(model, solid)), ((1, false), (2, true)));
}

#[test]
fn a_case_the_runner_refuses_is_a_fault_that_names_it() {
    let mut case = a_box();
    case["ops"][0]["size"] = json!([2.0, 3.0, 0.0]);
    let tally = one_case(case);
    assert_eq!((tally.agreed, tally.refused.len()), (0, 1));
    let found = faults(&Tally { refused: tally.refused, ..sound() });
    assert!(found.len() == 1 && found[0].starts_with("seeded: the runner refused it: "), "{found:?}");
}

#[test]
fn a_case_that_cannot_be_compared_is_left_out_by_why_and_a_count_that_moves_is_a_fault() {
    let mut no_answer = a_box();
    no_answer.as_object_mut().unwrap().remove("expect");
    let mut cut = a_box();
    cut["ops"].as_array_mut().unwrap().push(json!({"op": "cut", "id": "c", "target": "a", "tool": "a"}));
    let mut no_box = a_box();
    no_box["expect"].as_object_mut().unwrap().remove("bbox");
    let named = |name: &str, case: Value| (name.to_string(), case);
    let tally = tried(&[named("a", no_answer), named("b", cut), named("c", no_box), named("d", a_box())]);
    let want = [("it needs a cut", 1), ("it records no answer", 1), ("its expect has no bbox", 1)];
    assert_eq!(tally.left_out, want.map(|(why, count)| (why.to_string(), count)).into());
    assert_eq!((tally.cases, tally.agreed), (4, 1));
    let found = faults(&Tally { left_out: tally.left_out, ..sound() });
    assert!(found.len() == 1 && found[0].starts_with("the cases left out are "), "{found:?}");
}

#[test]
fn an_empty_corpus_and_one_where_nothing_agrees_are_faults() {
    assert_eq!(faults(&tried(&[])), ["the corpus is empty"]);
    assert_eq!(faults(&Tally { agreed: 0, ..sound() }), ["nothing was compared"]);
}
