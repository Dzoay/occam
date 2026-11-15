//! Source: original
//! Which way the solids a request builds face (PARITY 1.10): every one passes the checker as built, facing out, and is
//! refused turned inside out whole, each face over with its loops round, so that only the whole body shows it.

use core::f64::consts::TAU;
use occam_math::Exec;

use occam_report::{Code, EntityKind, NamedValue, Reason, Report};
use occam_topo::{EdgeUseRecord, Face, FaceRecord, Snapshot};
use serde_json::json;

use crate::construct::tests::built;

/// The reports refusing `model` with every face turned inside out: over, and each of its loops round, each use running
/// the other way and coming after the use that came after it, so that each face still agrees with its loops and with
/// its neighbours across their edges.
fn turned_inside_out(model: &Snapshot) -> Vec<Report> {
    let mut edit = model.edit();
    for face in model.ids::<Face>() {
        for l in model.view(face).unwrap().loops() {
            let uses = model.view(l).unwrap().uses();
            let mut befores = uses.clone();
            befores.rotate_right(1);
            for (u, before) in uses.into_iter().zip(befores) {
                let record = edit.record(u).unwrap();
                edit.set(u, EdgeUseRecord { next: Some(before), reversed: !record.reversed, ..record }).unwrap();
            }
        }
        let record = edit.record(face).unwrap();
        edit.set(face, FaceRecord { reversed: !record.reversed, ..record }).unwrap();
    }
    edit.commit(&Exec::serial()).map(|_| ()).unwrap_err()
}

#[test]
fn every_solid_turned_inside_out_is_refused_as_facing_in() {
    // A slanting extrusion of a disc, whose sides lie on surfaces of linear extrusion; a lens, on surfaces of
    // revolution; a sphere and a torus turned so that they reach furthest inside a face along every axis; a disc
    // turned a whole turn about a slanting axis, a torus; and part of a turn of a triangle, on a cone and planes.
    let disc = json!([
        {"op": "wire", "label": "c", "edges": [{"circle": {"centre": [0, 0, 0], "radius": 2, "normal": [0, 0, 1]}}]},
        {"op": "face", "label": "disc", "outer": "c"},
        {"op": "extrude", "face": "disc", "vector": [1, 2, 5]}
    ]);
    let r = 21.0_f64.sqrt();
    let lens = json!([
        {"op": "wire", "label": "lens", "edges": [{"arc": [[0, 0, -r], [3, 0, 0], [0, 0, r]]}, {"line": [[0, 0, r], [0, 0, -r]]}]},
        {"op": "face", "label": "f", "outer": "lens"},
        {"op": "revolve", "profile": "f", "origin": [0, 0, 0], "axis": [0, 0, 1], "angle": 360}
    ]);
    let ball = json!([{"op": "sphere", "radius": 2, "frame": {"origin": [1, 2, 3], "z": [1, 1, 1], "x": [1, -1, 0]}}]);
    let tyre =
        json!([{"op": "torus", "major": 5, "minor": 1, "frame": {"z": [0.3, -0.8, 0.52], "x": [1, 0.2, -0.15]}}]);
    let ring = json!([
        {"op": "wire", "label": "c", "edges": [{"circle": {"centre": [10, 0, 0], "radius": 3, "normal": [0, 1, 0]}}]},
        {"op": "face", "label": "disc", "outer": "c"},
        {"op": "revolve", "profile": "disc", "origin": [0, 0, 0], "axis": [0.2, 0.0, 1.0], "angle": 360}
    ]);
    let wedge = json!([
        {"op": "wire", "label": "t", "points": [[0, 0, 2], [4, 0, 2], [0, 0, 12]], "closed": true},
        {"op": "face", "label": "tri", "outer": "t"},
        {"op": "revolve", "profile": "tri", "origin": [0, 0, 0], "axis": [0, 0, 1], "angle": 90}
    ]);
    // Where each reaches furthest along the first axis that decides, and whether the faces there span a whole turn: the
    // two patches of a surface along an edge between them, or four round a vertex, or one face about a point inside it.
    let all = [
        ("disc", disc, EntityKind::Vertex, false),
        ("lens", lens, EntityKind::Edge, true),
        ("ball", ball, EntityKind::Face, true),
        ("tyre", tyre, EntityKind::Face, true),
        ("ring", ring, EntityKind::Vertex, true),
        ("wedge", wedge, EntityKind::Vertex, false),
    ];
    for (name, steps, at, whole) in all {
        let (model, _) = built(steps);
        let reports = turned_inside_out(&model);
        let reasons: Vec<_> = reports.iter().map(|r| (r.code, r.reason)).collect();
        assert_eq!(reasons, [(Code::InvariantFailed, Some(Reason::ShellInsideOut))], "{name}: {reports:#?}");
        assert_eq!(reports[0].entities.get(1).map(|e| e.kind), Some(at), "{name}");
        let facing = reports[0].value("facing").and_then(NamedValue::number).unwrap();
        assert!(facing < -1.0 && ((facing + TAU).abs() < 1e-9) == whole, "{name}: {facing}");
    }
}
