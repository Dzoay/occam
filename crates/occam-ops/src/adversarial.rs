//! Source: original
//! P9's adversarial catalogue rebuilt as models through construction (view.hardening), each meshed whole: every case
//! meshes watertight within its deflection, measured to the triangle (L2 of P9's lessons), or is named with its reason.
//! P9's integer grid failed three of its 48 cases, the hole 1e-7 from an arc and the edges of 1e-8 and 1e-10, which the
//! exact predicates mesh; its holes under a chord are cured by repair, cutting only the edges that collided. The
//! forty-eighth case, P9's bicubic patch with a collapsed row, is a spline construction cannot make, so occam-mesh
//! meshes it in its own tests; and the plate with 1,024 holes takes about a minute in a debug build, half of it the
//! checker's and a third the mesher's, every hole alike, so it runs nightly (`just mesh-holes`).

use core::f64::consts::{FRAC_1_SQRT_2, PI};

use occam_math::{Exec, cos, sin};
use occam_mesh::{Block, FaceMesh, Limits, Mesh, deviation, mesh, mesh_faces, sample_edges};
use occam_report::{Code, Reason, Report};
use occam_topo::{Edge, Id, Snapshot};
use occam_units::{Angle, Length, Vec3};
use serde_json::{Value, json};

use crate::construct::tests::{build, plate};

/// One case of the catalogue: its family, what varies in it, the steps that build it, the tolerance its model is built
/// to, and the deflection it is meshed to.
struct Case {
    family: &'static str,
    value: f64,
    steps: Value,
    tolerance: f64,
    deflection: f64,
}

/// The default angular tolerance, 1e-9 rad, in degrees, as a request gives it.
const ANGLE: f64 = 5.729_577_951_308_232e-8;

/// The model `case` builds, at its tolerance, every check passed.
fn built(case: &Case) -> Snapshot {
    let request = json!({
        "schema": 1,
        "units": {"length": "mm", "angle": "deg"},
        "tolerance": {"length": case.tolerance, "angle": ANGLE},
        "steps": case.steps,
    });
    let (model, _) = build(&request).unwrap_or_else(|reports| panic!("{} {:e}: {reports:#?}", case.family, case.value));
    assert_eq!(
        model.check_all(&Exec::serial()),
        [],
        "{} {:e}: the whole model passes the checker",
        case.family,
        case.value
    );
    model
}

/// The limits the catalogue meshes `case` to: its deflection, and P9's angle of 0.35.
fn limits(case: &Case) -> Limits {
    Limits::new(Length::from_millimetres(case.deflection), Angle::from_radians(0.35)).unwrap()
}

/// `x`, `y`, `z` as a point in a step.
fn at(x: f64, y: f64, z: f64) -> Value {
    json!([x, y, z])
}

/// The steps of `first` and then `then`.
fn and(first: &Value, then: &[Value]) -> Value {
    let mut steps = first.as_array().cloned().unwrap_or_default();
    steps.extend(then.iter().cloned());
    Value::Array(steps)
}

/// A face labelled `face` bounded by the wire of `edges`.
fn bounded(edges: &Value) -> Value {
    json!([{"op": "wire", "label": "rim", "edges": edges}, {"op": "face", "label": "face", "outer": "rim"}])
}

/// A face labelled `face` bounded by the closed polygon through `points`.
fn polygon(points: &Value) -> Value {
    json!([
        {"op": "wire", "label": "rim", "points": points, "closed": true},
        {"op": "face", "label": "face", "outer": "rim"}
    ])
}

/// What extruding the face labelled `face` by 1 along +Z adds.
fn extruded() -> Value {
    json!({"op": "extrude", "face": "face", "vector": [0, 0, 1]})
}

/// What revolving the face labelled `face` by `angle` degrees about `axis` through `origin` adds.
fn revolved(origin: &Value, axis: &Value, angle: f64) -> Value {
    json!({"op": "revolve", "profile": "face", "origin": origin, "axis": axis, "angle": angle})
}

/// A bolt: a head of radius 6 and a shank of radius 3 joined by a fillet of radius 1, revolved a whole turn about +Z
/// from a profile that runs up its axis, so that it has a pole at each end.
fn bolt() -> Value {
    let half = FRAC_1_SQRT_2;
    let edges = json!([
        {"line": [at(0.0, 0.0, 0.0), at(6.0, 0.0, 0.0)]},
        {"line": [at(6.0, 0.0, 0.0), at(6.0, 0.0, 4.0)]},
        {"line": [at(6.0, 0.0, 4.0), at(4.0, 0.0, 4.0)]},
        {"arc": [at(4.0, 0.0, 4.0), at(4.0 - half, 0.0, 5.0 - half), at(3.0, 0.0, 5.0)]},
        {"line": [at(3.0, 0.0, 5.0), at(3.0, 0.0, 24.0)]},
        {"line": [at(3.0, 0.0, 24.0), at(0.0, 0.0, 24.0)]},
        {"line": [at(0.0, 0.0, 24.0), at(0.0, 0.0, 0.0)]}
    ]);
    and(&bounded(&edges), &[revolved(&at(0.0, 0.0, 0.0), &at(0.0, 0.0, 1.0), 360.0)])
}

/// Seams and degenerate points: two poles, two seams, an apex, a seam alone, and a bolt with a fillet and two poles.
fn seams() -> Vec<Case> {
    let shapes = [
        (json!([{"op": "sphere", "radius": 2}]), 0.02),
        (json!([{"op": "torus", "major": 5, "minor": 1}]), 0.02),
        (json!([{"op": "cone", "radius1": 0, "radius2": 3, "height": 4}]), 0.02),
        (json!([{"op": "cylinder", "radius": 4, "height": 10}]), 0.02),
        (bolt(), 0.05),
    ];
    let case = |(k, (steps, deflection)): (u8, (Value, f64))| Case {
        family: "seams and degenerate points",
        value: f64::from(k),
        steps,
        tolerance: 1e-7,
        deflection,
    };
    (0..).zip(shapes).map(case).collect()
}

/// Sliver walls: the plate of 40 by 20 with a hole of radius 3, extruded `thickness`.
fn sliver(thickness: f64) -> Case {
    let steps = and(&plate(), &[json!({"op": "extrude", "face": "plate", "vector": [0, 0, thickness]})]);
    Case { family: "sliver walls", value: thickness, steps, tolerance: thickness / 100.0, deflection: 0.01 }
}

/// A strip: a ring of `width` about +Z, from radius 5, 1 high, its top and bottom flat strips that wide.
fn strip(width: f64) -> Case {
    let points = json!([at(5.0, 0.0, 0.0), at(5.0 + width, 0.0, 0.0), at(5.0 + width, 0.0, 1.0), at(5.0, 0.0, 1.0)]);
    let steps = and(&polygon(&points), &[revolved(&at(0.0, 0.0, 0.0), &at(0.0, 0.0, 1.0), 360.0)]);
    Case { family: "strip width", value: width, steps, tolerance: width / 100.0, deflection: 0.01 }
}

/// How far an arc of `radius` stands, `across` from its middle, from the line through its ends, which are `chord` apart:
/// written so that nothing cancels.
fn rise(radius: f64, chord: f64, across: f64) -> f64 {
    let sag = |half: f64| half * half / (radius + (radius * radius - half * half).sqrt());
    sag(chord / 2.0) - sag(across)
}

/// A crescent `width` wide between two arcs over one chord, of radius 10 and 500, so that the walls along them,
/// extruded, bend 50 to 1 apart, meshed to a tenth of its width so that each wall is cut as its bend asks. Meshed more
/// coarsely than it is wide, each wall can be drawn as the plane strip between the tips, the two the same, and their
/// chords between the tips' middles, one at one place used by both, read as a crack.
fn crescent(width: f64) -> Case {
    let (near, far) = (10.0, 500.0);
    let chord = (8.0 * width / (1.0 / near - 1.0 / far)).sqrt();
    let tip = |side: f64| at(side * chord / 2.0, 0.0, 0.0);
    let edges = json!([
        {"arc": [tip(-1.0), at(0.0, rise(near, chord, 0.0), 0.0), tip(1.0)]},
        {"arc": [tip(1.0), at(0.0, rise(far, chord, 0.0), 0.0), tip(-1.0)]}
    ]);
    let steps = and(&bounded(&edges), &[extruded()]);
    Case { family: "crescent", value: width, steps, tolerance: width / 1000.0, deflection: width / 10.0 }
}

/// A needle: two arcs from one tip `tip` radians apart, of radius 1 and 100, bending away from each other, closed by a
/// line and extruded, so that its walls bend 100 to 1 apart, at the default band of 1e-7. The arcs of the needle 1e-7
/// radians wide stay within that band of each other for 4.4e-4 from its tip, a sharp corner, as two pieces that meet do
/// near where they meet.
fn needle(tip: f64) -> Case {
    let (near, far, half) = (1.0, 100.0, tip / 2.0);
    // Round its centre above the tip, the near arc leaves it at half the tip above +X; round its centre below, the far
    // arc leaves it at half the tip below.
    let upper = |turn: f64| at(near * (sin(half + turn) - sin(half)), near * (cos(half) - cos(half + turn)), 0.0);
    let lower = |turn: f64| at(far * (sin(half + turn) - sin(half)), far * (cos(half + turn) - cos(half)), 0.0);
    let (reach, other) = (0.5, 0.5 * near / far);
    let edges = json!([
        {"arc": [at(0.0, 0.0, 0.0), lower(other / 2.0), lower(other)]},
        {"line": [lower(other), upper(reach)]},
        {"arc": [upper(reach), upper(reach / 2.0), at(0.0, 0.0, 0.0)]}
    ]);
    let steps = and(&bounded(&edges), &[extruded()]);
    Case { family: "needle", value: tip, steps, tolerance: 1e-7, deflection: 0.01 }
}

/// A square of 10 whose top bulges in, as an arc down to 8 in its middle, with a hole of radius 1 `gap` under it.
fn bulge(gap: f64) -> Case {
    let steps = json!([
        {"op": "wire", "label": "rim", "edges": [
            {"line": [at(0.0, 0.0, 0.0), at(10.0, 0.0, 0.0)]},
            {"line": [at(10.0, 0.0, 0.0), at(10.0, 10.0, 0.0)]},
            {"arc": [at(10.0, 10.0, 0.0), at(5.0, 8.0, 0.0), at(0.0, 10.0, 0.0)]},
            {"line": [at(0.0, 10.0, 0.0), at(0.0, 0.0, 0.0)]}
        ]},
        {"op": "wire", "label": "hole", "edges": [{"circle": {"centre": at(5.0, 7.0 - gap, 0.0), "radius": 1, "normal": [0, 0, 1]}}]},
        {"op": "face", "label": "face", "outer": "rim", "holes": ["hole"]},
        extruded()
    ]);
    Case { family: "hole by a bulging arc", value: gap, steps, tolerance: gap / 100.0, deflection: 0.01 }
}

/// Where the hole of [`under_a_chord`] at `gap` is centred.
fn under(gap: f64) -> [f64; 2] {
    let centre = 10.0 - gap - 0.25;
    [centre * cos(PI / 32.0), centre * sin(PI / 32.0)]
}

/// A sector of radius 10 a sixteenth of a turn either side of +X, extruded 5: at 0.1 its arc is sampled at its ends and
/// its middle, so the chord from (10, 0) sags about 0.048 inside it. A hole of radius 0.25 comes within `gap` of the arc
/// under the middle of that chord, so that for a gap below 0.048 the chord cuts across the hole's samples, in both caps.
pub(crate) fn under_a_chord(gap: f64) -> Value {
    let (x, y) = (10.0 * cos(PI / 16.0), 10.0 * sin(PI / 16.0));
    let [cx, cy] = under(gap);
    json!([
        {"op": "wire", "label": "rim", "edges": [{"arc": [at(x, -y, 0.0), at(10.0, 0.0, 0.0), at(x, y, 0.0)]}, {"line": [at(x, y, 0.0), at(0.0, 0.0, 0.0)]}, {"line": [at(0.0, 0.0, 0.0), at(x, -y, 0.0)]}]},
        {"op": "wire", "label": "hole", "edges": [{"circle": {"centre": at(cx, cy, 0.0), "radius": 0.25, "normal": [0, 0, 1]}}]},
        {"op": "face", "label": "face", "outer": "rim", "holes": ["hole"]},
        {"op": "extrude", "face": "face", "vector": [0, 0, 5]}
    ])
}

/// [`under_a_chord`] as a case.
fn chord(gap: f64) -> Case {
    Case {
        family: "hole under a chord",
        value: gap,
        steps: under_a_chord(gap),
        tolerance: gap / 100.0,
        deflection: 0.1,
    }
}

/// A profile from radius 2 to 12, 10 high, with a step of `length` in its top at radius 7, revolved a quarter turn
/// about +Z: one edge of that length between long ones, in the faces either side of it and round it.
fn short_edge(length: f64) -> Case {
    let top = 10.0 + length;
    let points = json!([
        at(2.0, 0.0, 0.0),
        at(12.0, 0.0, 0.0),
        at(12.0, 0.0, 10.0),
        at(7.0, 0.0, 10.0),
        at(7.0, 0.0, top),
        at(2.0, 0.0, top)
    ]);
    let steps = and(&polygon(&points), &[revolved(&at(0.0, 0.0, 0.0), &at(0.0, 0.0, 1.0), 90.0)]);
    Case { family: "one short edge", value: length, steps, tolerance: length / 100.0, deflection: 0.01 }
}

/// Neighbouring faces bending `ratio` to 1 apart: a profile whose floor is an arc of radius 10 and whose end is a half
/// circle of radius `10 / ratio`, extruded, or revolved a quarter turn about an axis beside it if `turned`; the walls
/// along the two arcs share an edge, sampled as the sharper asks.
fn resolutions(ratio: f64, turned: bool) -> Case {
    let small = 10.0 / ratio;
    let edges = json!([
        {"arc": [at(0.0, 0.0, 0.0), at(5.0, -rise(10.0, 10.0, 0.0), 0.0), at(10.0, 0.0, 0.0)]},
        {"arc": [at(10.0, 0.0, 0.0), at(10.0 + small, small, 0.0), at(10.0, 2.0 * small, 0.0)]},
        {"line": [at(10.0, 2.0 * small, 0.0), at(0.0, 2.0 * small, 0.0)]},
        {"line": [at(0.0, 2.0 * small, 0.0), at(0.0, 0.0, 0.0)]}
    ]);
    let (sweep, deflection) =
        if turned { (revolved(&at(-5.0, 0.0, 0.0), &at(0.0, 1.0, 0.0), 90.0), 0.05) } else { (extruded(), 0.01) };
    let family = if turned { "resolutions, turned" } else { "resolutions, extruded" };
    Case { family, value: ratio, steps: and(&bounded(&edges), &[sweep]), tolerance: small / 1000.0, deflection }
}

/// A plate of 64 by 64 with 1,024 holes of radius 0.5 in a grid of 2, extruded 1.
fn holes() -> Case {
    let rim = json!({"op": "wire", "label": "rim", "points": [[0, 0, 0], [64, 0, 0], [64, 64, 0], [0, 64, 0]], "closed": true});
    let mut steps = vec![rim];
    let mut labels = Vec::new();
    for k in 0..1024_u32 {
        let (i, j) = (f64::from(k % 32), f64::from(k / 32));
        let circle = json!({"centre": at(1.0 + 2.0 * i, 1.0 + 2.0 * j, 0.0), "radius": 0.5, "normal": [0, 0, 1]});
        steps.push(json!({"op": "wire", "label": format!("hole{k}"), "edges": [{"circle": circle}]}));
        labels.push(format!("hole{k}"));
    }
    steps.extend([json!({"op": "face", "label": "face", "outer": "rim", "holes": labels}), extruded()]);
    Case { family: "1,024 holes", value: 1024.0, steps: Value::Array(steps), tolerance: 1e-7, deflection: 0.05 }
}

/// The mesh of the face of `block` in `made`, numbered from its block's start.
fn own(made: &Mesh, block: &Block) -> FaceMesh {
    let range = |r: &core::ops::Range<u32>| usize::try_from(r.start).unwrap()..usize::try_from(r.end).unwrap();
    let (vertices, first) = (range(&block.vertices), block.vertices.start);
    FaceMesh {
        points: made.points[vertices.clone()].to_vec(),
        normals: made.normals[vertices.clone()].to_vec(),
        uvs: made.uvs[vertices].to_vec(),
        triangles: made.triangles[range(&block.triangles)]
            .iter()
            .map(|t| t.map(|k| k.checked_sub(first).unwrap()))
            .collect(),
        sides: Vec::new(),
        capped: block.capped,
    }
}

/// A report as its code, its reason and what it names.
fn said(report: &Report) -> String {
    let entities: Vec<String> = report.entities.iter().map(|e| e.text()).collect();
    format!("{} {:?} {}", report.code.text(), report.reason, entities.join(" "))
}

/// What is wrong with `case` meshed whole: none where every face meshed, the mesh is watertight, and every face is
/// within the deflection, measured to the triangle.
fn faults(case: &Case) -> Vec<String> {
    let model = built(case);
    let made = mesh(&Exec::serial(), &model, limits(case));
    let mut found: Vec<String> = made.refused.iter().chain(made.edges.refused()).chain(&made.open).map(said).collect();
    for block in &made.blocks {
        let off = deviation(&own(&made, block), model.view(block.face).unwrap().surface().unwrap());
        if off > case.deflection {
            found.push(format!("{:?} strays {off}", block.face));
        }
    }
    found.into_iter().map(|fault| format!("{} {:e}: {fault}", case.family, case.value)).collect()
}

/// Asserts that every one of `cases` meshes watertight within its deflection.
fn meshes(cases: impl IntoIterator<Item = Case>) {
    let found: Vec<String> = cases.into_iter().flat_map(|case| faults(&case)).collect();
    assert_eq!(found, Vec::<String>::new());
}

#[test]
fn seams_poles_an_apex_and_a_bolt_mesh_watertight_within_their_deflection() {
    meshes(seams());
}

#[test]
fn sliver_walls_down_to_1e_11_mesh_watertight_within_their_deflection() {
    meshes([1e-1, 1e-3, 1e-5, 1e-7, 1e-9, 1e-11].map(sliver));
}

#[test]
fn strips_down_to_1e_10_wide_mesh_watertight_within_their_deflection() {
    meshes([1e-2, 1e-4, 1e-6, 1e-8, 1e-10].map(strip));
}

#[test]
fn crescents_down_to_1e_6_wide_mesh_watertight_within_their_deflection() {
    meshes([1e-1, 1e-2, 1e-3, 1e-4, 1e-5, 1e-6].map(crescent));
}

#[test]
fn needles_down_to_1e_7_radians_mesh_watertight_within_their_deflection() {
    meshes([1e-1, 1e-3, 1e-5, 1e-7].map(needle));
}

#[test]
fn a_hole_down_to_1e_7_from_an_arc_bulging_at_it_meshes_watertight_within_its_deflection() {
    meshes([1e-1, 1e-3, 1e-5, 1e-6, 1e-7].map(bulge));
}

#[test]
fn an_edge_down_to_1e_10_long_between_long_ones_meshes_watertight_within_its_deflection() {
    meshes([1e-2, 1e-4, 1e-8, 1e-10].map(short_edge));
}

#[test]
fn neighbours_bending_100_and_10000_to_1_apart_mesh_watertight_within_their_deflection() {
    meshes(
        [(100.0, false), (1e4, false), (100.0, true), (1e4, true)].map(|(ratio, turned)| resolutions(ratio, turned)),
    );
}

/// What an edge of [`under_a_chord`] at `gap` is, read from the middle of its range: `Some(true)` on the arc, `Some(false)`
/// on the hole, and `None` for a line.
fn round(model: &Snapshot, edge: Id<Edge>, gap: f64) -> Option<bool> {
    let view = model.view(edge).unwrap();
    let (lo, hi) = view.range();
    let p = view.curve().unwrap().first(0.5 * lo + 0.5 * hi).0.to_vec3();
    let [cx, cy] = under(gap);
    let from = |x: f64, y: f64| Vec3::new(p.x - x, p.y - y, 0.0).norm();
    if (from(0.0, 0.0) - 10.0).abs() < 1e-9 {
        Some(true)
    } else {
        ((from(cx, cy) - 0.25).abs() < 1e-9).then_some(false)
    }
}

#[test]
fn holes_under_a_chord_mesh_watertight_once_repair_cuts_the_edges_that_collided_and_no_other() {
    meshes([1e-1, 1e-2, 1e-3, 1e-4, 1e-5, 1e-6, 1e-8].map(chord));
    for gap in [1e-2, 1e-3, 1e-4, 1e-5, 1e-6, 1e-8] {
        let case = chord(gap);
        let model = built(&case);
        let first = sample_edges(&Exec::serial(), &model, limits(&case));
        let refused: Vec<Report> = mesh_faces(&Exec::serial(), &model, &first).refused().cloned().collect();
        assert_eq!(refused.len(), 2, "{gap}: both caps, before repair");
        for report in &refused {
            assert_eq!((report.code, report.reason), (Code::FaceNotMeshed, Some(Reason::BoundaryCrosses)), "{gap}");
            let named = report.entities[1..].iter().map(|&e| Id::<Edge>::from_entity(e).unwrap());
            assert!(named.map(|edge| round(&model, edge, gap)).all(|kind| kind.is_some()), "{gap}: {report:#?}");
        }
        let made = mesh(&Exec::serial(), &model, limits(&case));
        let kinds: Vec<Option<bool>> = made.edges.repaired().iter().map(|&edge| round(&model, edge, gap)).collect();
        assert!(kinds.iter().all(Option::is_some), "{gap}: only the arcs and the holes are cut: {kinds:?}");
        assert_eq!(kinds.iter().filter(|kind| **kind == Some(true)).count(), 2, "{gap}: both arcs");
        let most = usize::try_from(4 * Limits::REPAIRS_PER_FACE).unwrap();
        for &edge in made.edges.repaired() {
            let (now, was) = (made.edges.get(edge).unwrap().unwrap(), first.get(edge).unwrap().unwrap());
            assert!(now.params.len() > was.params.len() && now.params.len() <= was.params.len() + most, "{gap}");
        }
    }
}

#[test]
#[ignore = "about a minute in a debug build, so it runs nightly: `just mesh-holes`"]
fn a_plate_with_1024_holes_meshes_watertight_within_its_deflection() {
    meshes([holes()]);
}
