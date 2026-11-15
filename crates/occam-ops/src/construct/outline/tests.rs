//! Source: original
//! A run read back, as the checks of a wire and a face use it: its samples follow each edge the way the run takes it,
//! the plane through its points goes through the first of those furthest out, and of the points off that plane the
//! first furthest is named, a point exactly the band off it being in it.

use occam_report::{Code, EntityKind, Report};
use occam_topo::Snapshot;
use serde_json::{Value, json};

use super::*;
use crate::construct::tests::{Built, build, number, refused, request};
use crate::default_tolerance;

/// The point `report` names.
fn point(report: &Report) -> Option<[f64; 3]> {
    match report.value("point").map(|v| v.value) {
        Some(occam_report::Value::Point(p)) => Some(p),
        _ => None,
    }
}

/// The request `steps` built in a band of 0.5.
fn coarse(steps: Value) -> Result<Built, Vec<Report>> {
    let mut text = request(steps);
    text["tolerance"] = json!({"length": 0.5, "angle": 1e-6});
    build(&text)
}

#[test]
fn a_run_s_samples_follow_each_edge_the_way_the_run_takes_it() {
    let mut edit = Snapshot::empty(default_tolerance().unwrap()).edit();
    let (o, x, y) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
    let (line, circle) = (Bent::Line { origin: o, along: x }, Bent::Circle { centre: o, x, y, radius: 2.0 });
    let parts = [(line, (1.0, 3.0), true), (circle, (0.5, 2.0), false), (circle, (0.25, 1.5), true)];
    let run = Run {
        edges: parts.iter().map(|&(_, _, back)| (edit.reserve(), back)).collect(),
        bents: parts.iter().map(|&(bent, _, _)| bent).collect(),
        ranges: parts.iter().map(|&(_, range, _)| range).collect(),
        turned: false,
    };
    // Each edge from its start, which is the end of its curve's part where the run takes it backwards; a circle's part
    // in sixteenths; and, the run being open, the last edge to its end.
    let along = |from: f64, to: f64, count: u32| (0..count).map(move |k| from + (to - from) * f64::from(k) / 16.0);
    let wanted: Vec<Vec3> = core::iter::once(line.point(3.0))
        .chain(along(0.5, 2.0, 16).map(|t| circle.point(t)))
        .chain(along(1.5, 0.25, 17).map(|t| circle.point(t)))
        .collect();
    let samples = run.samples();
    assert_eq!(samples.len(), wanted.len());
    assert!(samples.iter().zip(&wanted).all(|(&p, &q)| (p - q).norm() < 1e-12), "{samples:?}");
}

#[test]
fn the_plane_goes_through_the_first_of_the_points_furthest_out() {
    // (5, 1, 0) and (5, 0, 1) are each 1 from the line through the first point and (10, 0, 0), the furthest from it: the
    // plane goes through the first of them, z = 0, which the second is 1 from.
    let outer = json!({"points": [[0, 0, 0], [5, 1, 0], [10, 0, 0], [5, 0, 1]], "closed": true});
    let report = refused(json!([{"op": "face", "outer": outer}]));
    assert_eq!(
        (report.code, point(&report), number(&report, "distance")),
        (Code::NotPlanar, Some([5.0, 0.0, 1.0]), 1.0)
    );
}

#[test]
fn of_the_points_off_the_plane_the_first_furthest_is_named() {
    // A square 100 across in z = 0, its right side bowed out of it at three points 1, 2 and 2 above.
    let points = json!([[0, 0, 0], [0, 100, 0], [100, 100, 0], [100, 75, 1], [100, 50, 2], [100, 25, 2], [100, 0, 0]]);
    let report = refused(json!([{"op": "face", "outer": {"points": points, "closed": true}}]));
    let named = (report.code, point(&report), number(&report, "distance"));
    assert_eq!(named, (Code::NotPlanar, Some([100.0, 50.0, 2.0]), 2.0));
}

#[test]
fn a_point_exactly_the_band_out_of_the_plane_is_in_it() {
    // In a band of 0.5, (100, 50, 0.5) is as far out of z = 0 as the band allows: the face is made.
    let points = json!([[0, 0, 0], [0, 100, 0], [100, 100, 0], [100, 50, 0.5], [100, 0, 0]]);
    let (_, made) = coarse(json!([{"op": "face", "outer": {"points": points, "closed": true}}])).unwrap();
    assert_eq!(made[0].map(|face| face.kind), Some(EntityKind::Face));
}

#[test]
fn points_in_line_but_for_exactly_the_band_span_no_plane() {
    // In a band of 0.5, (5, 0.5, 0) is as far from the line through the others as the band allows.
    let outer = json!({"points": [[0, 0, 0], [10, 0, 0], [5, 0.5, 0]], "closed": true});
    let reports = coarse(json!([{"op": "face", "outer": outer}])).map(|_| ()).unwrap_err();
    assert_eq!(reports.iter().map(|r| (r.code, number(r, "clearance"))).collect::<Vec<_>>(), [(Code::NoInterior, 0.0)]);
}
