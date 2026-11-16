//! Source: original
//! `intersection.hits` on the corpus (docs/plan/intersection.md, P8): the box test removes scans and changes no digest.
//! Each `occam-oracle-case/1` file under `tools/corpus` has its operands built through `occam`'s runner, the case's
//! primitives and rotations translated into the runner's steps; no boolean runs. Every edge of one operand of each
//! boolean is then tried against every face of the other, through `hits`, which tests their boxes first, and `scan`,
//! which does not, and the two are compared by digest. A case whose booleans' operands need an operation the runner
//! cannot build yet, such as another boolean or a fillet, is skipped and counted by its reason. It takes minutes in a
//! debug build, so it runs nightly with the rest of the full corpus, through `just corpus-hits`.
#![cfg(not(target_family = "wasm"))]
#![allow(
    clippy::unwrap_used,
    clippy::expect_used,
    clippy::indexing_slicing,
    clippy::print_stdout,
    reason = "a test: a malformed case should fail it, and its totals are its output"
)]

use std::collections::{BTreeMap, BTreeSet};
use std::f64::consts::PI;
use std::fs;
use std::path::Path;

use occam_geom::{Curve, Domain, ParamCurve, ParamRange, Surface, UvBox, Wrap};
use occam_intersect::{Budget, Hits, Span, boxes_meet, hits, scan};
use occam_math::{Digest, sin_cos};
use occam_topo::{Body, Snapshot};
use occam_units::{Tolerance, UvPoint, Vec3};
use serde_json::{Value, json};

/// The work a scan may do: enough that no pair of the corpus runs out of it.
const BUDGET: Budget = Budget { steps: 1_000_000 };

/// The corpus's operations the runner builds today, by the case format's name.
const BUILT: [&str; 6] = ["box", "cylinder", "cone", "sphere", "torus", "rotate"];

/// The case format's booleans, whose operands are the pairs.
const BOOLEANS: [&str; 3] = ["fuse", "cut", "common"];

/// Every case of the corpus, by file name, in order.
fn cases() -> Vec<(String, Value)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus");
    let mut found = Vec::new();
    for dir in ["generated", "imported"] {
        for entry in fs::read_dir(root.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "json") {
                let case: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
                assert_eq!(case["schema"], "occam-oracle-case/1", "{}", path.display());
                found.push((path.file_name().unwrap().to_string_lossy().into_owned(), case));
            }
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

fn three(value: &Value) -> [f64; 3] {
    let v: Vec<f64> = value.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    [v[0], v[1], v[2]]
}

fn vec3([x, y, z]: [f64; 3]) -> Vec3 {
    Vec3::new(x, y, z)
}

/// The runner's frame at `origin` with its Z along `axis`, and an X it accepts across it.
fn frame(origin: &Value, axis: Option<&Value>) -> Value {
    let z = axis.map_or([0.0, 0.0, 1.0], three);
    let x = if z[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    json!({"origin": origin, "z": z, "x": x})
}

/// `v` turned by `angle` radians about the unit `axis`, right-handed (Rodrigues).
fn turned(v: Vec3, axis: Vec3, angle: f64) -> Vec3 {
    let (sin, cos) = sin_cos(angle);
    v * cos + axis.cross(v) * sin + axis * (axis.dot(v) * (1.0 - cos))
}

/// A rotation of the case format, as the frame the runner's `copy` places the world's at: the turn about the axis
/// through `axis_origin`, which takes the origin to `o - R o`.
fn rotation(op: &Value) -> Value {
    let (origin, axis) = (vec3(three(&op["axis_origin"])), vec3(three(&op["axis"])));
    let axis = axis * (1.0 / axis.norm());
    let angle = op["angle_degrees"].as_f64().unwrap() * PI / 180.0;
    let at = origin - turned(origin, axis, angle);
    let (z, x) = (turned(Vec3::new(0.0, 0.0, 1.0), axis, angle), turned(Vec3::new(1.0, 0.0, 0.0), axis, angle));
    json!({"origin": [at.x, at.y, at.z], "z": [z.x, z.y, z.z], "x": [x.x, x.y, x.z]})
}

/// The runner's step for a case's operation, which is one it builds, labelled with its id.
fn step(op: &Value) -> Value {
    let (id, origin) = (&op["id"], &op["origin"]);
    match op["op"].as_str().unwrap() {
        "box" => json!({"op": "box", "label": id, "size": op["size"], "frame": frame(origin, None)}),
        "sphere" => json!({"op": "sphere", "label": id, "radius": op["radius"], "frame": frame(origin, None)}),
        "cylinder" => json!({"op": "cylinder", "label": id, "radius": op["radius"], "height": op["height"],
            "frame": frame(origin, op.get("axis"))}),
        "cone" => json!({"op": "cone", "label": id, "radius1": op["radius1"], "radius2": op["radius2"],
            "height": op["height"], "frame": frame(origin, op.get("axis"))}),
        "torus" => json!({"op": "torus", "label": id, "major": op["major_radius"], "minor": op["minor_radius"],
            "frame": frame(origin, op.get("axis"))}),
        _ => json!({"op": "copy", "label": id, "target": op["target"], "frame": rotation(op)}),
    }
}

/// The ids `id` is built from, itself last, or why it cannot be built yet.
fn needs<'c>(ops: &BTreeMap<&'c str, &'c Value>, id: &'c str, out: &mut BTreeSet<&'c str>) -> Result<(), String> {
    let op = ops.get(id).ok_or_else(|| format!("unknown id {id}"))?;
    let kind = op["op"].as_str().unwrap();
    if !BUILT.contains(&kind) {
        return Err(format!("an operand is made by {kind}"));
    }
    if kind == "rotate" {
        needs(ops, op["target"].as_str().unwrap(), out)?;
    }
    out.insert(id);
    Ok(())
}

/// What a case gives: the request building every operand of its booleans that can be built, their pairs by label, and
/// why each of its other booleans cannot be built.
type Operands = (String, Vec<(String, String)>, Vec<String>);

/// The case's [`Operands`], or why it gives none: the first reason a boolean of it gave.
fn operands(case: &Value) -> Result<Operands, String> {
    let all = case["ops"].as_array().unwrap();
    let ops: BTreeMap<&str, &Value> = all.iter().map(|op| (op["id"].as_str().unwrap(), op)).collect();
    let booleans: Vec<&Value> = all.iter().filter(|op| BOOLEANS.contains(&op["op"].as_str().unwrap())).collect();
    let (mut built, mut pairs, mut why) = (BTreeSet::new(), Vec::new(), Vec::new());
    for boolean in &booleans {
        let (target, tool) = (boolean["target"].as_str().unwrap(), boolean["tool"].as_str().unwrap());
        let mut needed = BTreeSet::new();
        match needs(&ops, target, &mut needed).and_then(|()| needs(&ops, tool, &mut needed)) {
            Ok(()) => {
                built.extend(needed);
                pairs.push((target.to_string(), tool.to_string()));
            }
            Err(reason) => why.push(reason),
        }
    }
    if pairs.is_empty() {
        return Err(why.into_iter().next().unwrap_or_else(|| "no boolean".to_string()));
    }
    let steps: Vec<Value> = all.iter().filter(|op| built.contains(op["id"].as_str().unwrap())).map(step).collect();
    let request = json!({"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": steps});
    Ok((request.to_string(), pairs, why))
}

/// The smallest box holding both.
fn union(a: UvBox, b: UvBox) -> UvBox {
    let pick = |x: f64, y: f64, low: bool| if (y < x) == low { y } else { x };
    let lo = UvPoint::new(pick(a.lo.u(), b.lo.u(), true), pick(a.lo.v(), b.lo.v(), true));
    UvBox::new(lo, UvPoint::new(pick(a.hi.u(), b.hi.u(), false), pick(a.hi.v(), b.hi.v(), false)))
}

/// `patch` cut back into `domain` where a parameter does not come round: the images' boxes are padded for rounding, and
/// may reach a few ulps past a pole.
fn within(patch: UvBox, domain: Domain) -> UvBox {
    let cut = |lo: f64, hi: f64, range: ParamRange| {
        if range.wrap == Wrap::Periodic {
            return (lo, hi);
        }
        (if lo < range.lo { range.lo } else { lo }, if hi > range.hi { range.hi } else { hi })
    };
    let (u, v) = (cut(patch.lo.u(), patch.hi.u(), domain.u), cut(patch.lo.v(), patch.hi.v(), domain.v));
    UvBox::new(UvPoint::new(u.0, v.0), UvPoint::new(u.1, v.1))
}

/// A body's faces, each its surface over the box of its trims' images in its parameters, each image over its edge's
/// range, or its whole domain where it has no trims and the domain is finite; and its edges, each its curve over its
/// range.
type Faces<'s> = Vec<(&'s dyn Surface, UvBox)>;
type Edges<'s> = Vec<(&'s dyn Curve, Span)>;

fn parts(model: &Snapshot, body: occam_topo::Id<Body>) -> (Faces<'_>, Edges<'_>) {
    let (mut faces, mut edges) = (Vec::new(), BTreeSet::new());
    let shells = model.view(body).unwrap().shells();
    for face in shells.into_iter().flat_map(|shell| model.view(shell).unwrap().faces()) {
        let view = model.view(face).unwrap();
        let surface = view.surface().unwrap();
        let uses = view.loops().into_iter().flat_map(|l| model.view(l).unwrap().uses());
        let boxes = uses.filter_map(|id| {
            let image = model.view(id).unwrap();
            let (lo, hi) = model.view(image.edge()).unwrap().range();
            image.pcurve().map(|p: &dyn ParamCurve| p.bounds(lo, hi))
        });
        let domain = surface.domain();
        let whole = UvBox::new(UvPoint::new(domain.u.lo, domain.v.lo), UvPoint::new(domain.u.hi, domain.v.hi));
        let patch = within(boxes.reduce(union).unwrap_or(whole), domain);
        if [patch.lo.u(), patch.lo.v(), patch.hi.u(), patch.hi.v()].iter().all(|x| x.is_finite()) {
            faces.push((surface, patch));
        }
        edges.extend(view.edges());
    }
    let edges = edges.into_iter().filter_map(|edge| {
        let view = model.view(edge).unwrap();
        let (lo, hi) = view.range();
        (lo < hi).then(|| view.curve().map(|curve| (curve, Span { lo, hi })))?
    });
    (faces, edges.collect())
}

/// Everything found, but not the tally, which a culled pair has none of: every number in its shortest round-tripping
/// form, which names its bits.
fn digest(found: &Hits) -> u64 {
    let mut out = Digest::EMPTY;
    let text = format!("{:?} {:?} {:?} {:?}", found.crossings, found.contacts, found.overlaps, found.failures);
    out.write_bytes(text.as_bytes());
    out.finish()
}

/// `count` grown by `by`.
fn more(count: &mut usize, by: usize) {
    *count = count.saturating_add(by);
}

/// One more of `key` in `map`.
fn one(map: &mut BTreeMap<String, usize>, key: String) {
    more(map.entry(key).or_default(), 1);
}

/// What the corpus gave.
#[derive(Default)]
struct Totals {
    cases: usize,
    built: usize,
    pairs: usize,
    removed: usize,
    points: usize,
    failures: BTreeMap<String, usize>,
    skipped: BTreeMap<String, usize>,
    /// The booleans of cases built that are not tried, by why.
    booleans_skipped: BTreeMap<String, usize>,
}

impl Totals {
    /// Every edge of `edges` against every face of `faces`, culled and unculled, which must agree.
    fn compare(&mut self, case: &str, edges: &Edges<'_>, faces: &Faces<'_>, tol: Tolerance) {
        for (curve, span) in edges {
            for (surface, patch) in faces {
                let unculled = scan(*curve, *span, *surface, *patch, tol, BUDGET);
                let culled = hits(*curve, *span, *surface, *patch, tol, BUDGET);
                assert_eq!(digest(&culled), digest(&unculled), "{case}: {culled:?} against {unculled:?}");
                more(&mut self.pairs, 1);
                more(&mut self.removed, usize::from(!boxes_meet(*curve, *span, *surface, *patch, tol)));
                for found in [&unculled.crossings, &unculled.contacts] {
                    more(&mut self.points, found.len());
                }
                more(&mut self.points, unculled.overlaps.len());
                for report in &unculled.failures {
                    one(&mut self.failures, report.code.text().to_string());
                }
            }
        }
    }

    /// The case's operands built and every pair of its booleans compared, or the reason it is skipped counted.
    fn case(&mut self, name: &str, case: &Value) {
        more(&mut self.cases, 1);
        let (request, pairs, others) = match operands(case) {
            Ok(built) => built,
            Err(reason) => return one(&mut self.skipped, reason),
        };
        for reason in others {
            one(&mut self.booleans_skipped, reason);
        }
        let ran = occam::run(None, &request);
        if let Some(failed) = &ran.failed {
            let codes: Vec<&str> = failed.reports.iter().map(|r| r.code.text()).collect();
            return one(&mut self.skipped, format!("the runner refused it: {}", codes.join(", ")));
        }
        let model = ran.model.as_ref().unwrap();
        let labels = ran.done.iter().filter_map(|done| Some((done.label.clone()?, done.made?)));
        let bodies: BTreeMap<String, occam_topo::Id<Body>> =
            labels.map(|(label, made)| (label, model.resolve::<Body>(made).unwrap())).collect();
        for (a, b) in pairs {
            let ((a_faces, a_edges), (b_faces, b_edges)) = (parts(model, bodies[&a]), parts(model, bodies[&b]));
            self.compare(name, &a_edges, &b_faces, model.tolerance());
            self.compare(name, &b_edges, &a_faces, model.tolerance());
        }
        more(&mut self.built, 1);
    }

    /// Both together.
    fn add(mut self, other: Self) -> Self {
        let counts = [(&mut self.cases, other.cases), (&mut self.built, other.built), (&mut self.pairs, other.pairs)];
        for (count, by) in
            counts.into_iter().chain([(&mut self.removed, other.removed), (&mut self.points, other.points)])
        {
            more(count, by);
        }
        let maps = [(&mut self.failures, other.failures), (&mut self.skipped, other.skipped)];
        for (map, others) in maps.into_iter().chain([(&mut self.booleans_skipped, other.booleans_skipped)]) {
            for (key, count) in others {
                more(map.entry(key).or_default(), count);
            }
        }
        self
    }
}

#[test]
#[ignore = "the full corpus runs nightly, not per PR (docs/PLAN.md section 5): `just corpus-hits`"]
fn on_the_corpus_the_box_test_removes_scans_and_changes_no_digest() {
    let all = cases();
    let threads = std::thread::available_parallelism().map_or(1, |n| n.get().min(8));
    let chunk = all.len().div_ceil(threads);
    let totals = std::thread::scope(|scope| {
        let workers: Vec<_> = all
            .chunks(chunk)
            .map(|part| {
                scope.spawn(move || {
                    let mut totals = Totals::default();
                    for (name, case) in part {
                        totals.case(name, case);
                    }
                    totals
                })
            })
            .collect();
        workers.into_iter().map(|worker| worker.join().unwrap()).fold(Totals::default(), Totals::add)
    });
    let Totals { cases, built, pairs, removed, points, failures, skipped, booleans_skipped } = &totals;
    println!(
        "corpus: {cases} cases, {built} built, {pairs} edge and face pairs, {removed} scans removed by the box test"
    );
    println!("corpus: {points} hits found, failures by code {failures:?}");
    for (reason, count) in skipped {
        println!("corpus: cases skipped {count}: {reason}");
    }
    for (reason, count) in booleans_skipped {
        println!("corpus: booleans of cases built, skipped {count}: {reason}");
    }
    assert!(*built > 0 && *pairs > 0 && *points > 0, "nothing was compared");
    assert!(*removed > 0, "the box test removes no scan");
}
