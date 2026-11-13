//! Source: original
//! The checks that read geometry: every clean model passes them, and each seeded corruption is caught and named.

use core::f64::consts::{FRAC_PI_2, PI};
use std::sync::Arc;

use occam_geom::Plane;
use occam_report::{Code, EntityRef, Fault, Reason, Report};
use occam_units::{Angle, Direction, Length, Local, Placement, Point, Tolerance, UvPoint, Vec3, Vector, World};

use crate::edit::Edit;
use crate::kind::{Edge, EdgeUse, Face, Id, Loop, Vertex};
use crate::models::{
    Built, Spec, build, cube, cylinder, cylinder_from, cylinder_pcurves, disc, frame, holed_sheet, point, sheet,
    sheet_holed_by_its_reversal, straight_pcurves, wire,
};
use crate::records::{
    Around, CurveRecord, EdgeRecord, EdgeRole, EdgeUseRecord, End, FaceRecord, LoopRecord, Sheet, SurfaceRecord,
    UseOwner, VertexRecord,
};
use crate::snapshot::Snapshot;
use crate::testing::{at, number, record, serial, tolerance, x_axis};

/// `spec` built in one edit, with `extra` run on the edit before it commits.
fn attempt(spec: &Spec, extra: impl FnOnce(&mut Edit, &Built)) -> (Result<Snapshot, Vec<Report>>, Built) {
    let mut edit = Snapshot::empty(tolerance()).edit();
    let built = build(&mut edit, spec);
    extra(&mut edit, &built);
    (edit.commit(&serial()).map(|(s, _)| s), built)
}

/// `spec` built clean, with `extra`: the commit must succeed.
fn clean(spec: &Spec, extra: impl FnOnce(&mut Edit, &Built)) -> (Snapshot, Built) {
    let (result, built) = attempt(spec, extra);
    (result.unwrap_or_else(|reports| panic!("{:#?}", failures(&reports))), built)
}

/// Nothing more.
fn nothing(_: &mut Edit, _: &Built) {}

/// Each report's reason and entities, every one a `check.invariant-failed` of fault internal.
fn failures(reports: &[Report]) -> Vec<(Reason, Vec<EntityRef>)> {
    for report in reports {
        assert_eq!((report.code, report.fault), (Code::InvariantFailed, Some(Fault::Internal)), "{report:?}");
    }
    reports.iter().map(|r| (r.reason.unwrap(), r.entities.clone())).collect()
}

/// The reports for `reason`.
fn with(reports: &[Report], reason: Reason) -> Vec<&Report> {
    reports.iter().filter(|r| r.reason == Some(reason)).collect()
}

/// Every check that reads geometry, over every live entity of `s`, as a check of the whole model runs them.
fn everything(s: &Snapshot) -> Vec<Report> {
    let mut out = Vec::new();
    let entities = [
        s.ids::<Vertex>().into_iter().map(Id::entity).collect::<Vec<_>>(),
        s.ids::<Edge>().into_iter().map(Id::entity).collect(),
        s.ids::<EdgeUse>().into_iter().map(Id::entity).collect(),
        s.ids::<Loop>().into_iter().map(Id::entity).collect(),
        s.ids::<Face>().into_iter().map(Id::entity).collect(),
    ];
    for entity in entities.into_iter().flatten() {
        super::entity(&s.model, entity, &mut out);
    }
    out
}

/// `edit` committed: it must fail.
fn refused(edit: Edit) -> Vec<Report> {
    edit.commit(&serial()).map(|(s, _)| s.step()).unwrap_err()
}

#[test]
fn every_clean_model_passes_the_checks_that_read_geometry() {
    for spec in [cube(), frame(), sheet(), wire(), cylinder(), cylinder_from(0.0), disc(), holed_sheet()] {
        let (s, _) = clean(&spec, nothing);
        assert_eq!(failures(&everything(&s)), [], "the whole model, entity by entity");
    }
    for spec in [cube(), frame(), sheet(), holed_sheet()] {
        let (s, b) = clean(&spec, straight_pcurves);
        assert!(s.view(b.uses[0]).unwrap().pcurve().is_some());
        assert_eq!(failures(&everything(&s)), [], "with parameter curves");
    }
    let (s, _) = clean(&cylinder_from(0.0), |e, b| cylinder_pcurves(e, b, 0.0, 0.0));
    assert_eq!(failures(&everything(&s)), []);
}

#[test]
fn a_vertex_moved_off_its_edges_is_caught_naming_it_and_each_edge() {
    let (s, b) = clean(&cube(), nothing);
    let corner = b.vertices[6];
    let mut edit = s.edit();
    let moved = VertexRecord { point: point(1.0, 1.0, 1.001), ..record(&s, corner).unwrap().clone() };
    edit.set(corner, moved).unwrap();
    let reports = refused(edit);
    let edges = s.view(corner).unwrap().edges();
    assert_eq!(edges.len(), 3);
    let want: Vec<(Reason, Vec<EntityRef>)> =
        edges.iter().map(|e| (Reason::VertexOffEdge, vec![corner.entity(), e.entity()])).collect();
    assert_eq!(failures(&reports), want);
    let gap = number(&reports[0], "distance").unwrap();
    assert!((gap - 0.001).abs() < 1e-12, "{gap}");
    assert_eq!(number(&reports[0], "tolerance-in-force"), Some(1e-7));
}

/// One edge along the x axis from 0 to 1, its start vertex at `x` with a certified radius of `radius`, to `tol`.
fn one_edge(tol: Tolerance, x: f64, radius: f64) -> (Result<Snapshot, Vec<Report>>, Id<Vertex>, Id<Edge>) {
    let mut edit = Snapshot::empty(tol).edit();
    let curve = edit.add(x_axis());
    let (a, b, e) = (edit.reserve::<Vertex>(), edit.reserve::<Vertex>(), edit.reserve::<Edge>());
    let (start, finish) = (Around { edge: e, end: End::Start }, Around { edge: e, end: End::Finish });
    let record = EdgeRecord {
        curve,
        range: (0.0, 1.0),
        ends: (a, b),
        around: (start, finish),
        first_use: None,
        role: EdgeRole::Ordinary,
        deviation: Length::from_millimetres(0.0),
        born: edit.provenance(occam_report::StepRole::Made),
    };
    edit.set(e, record).unwrap();
    let radius = Length::from_millimetres(radius);
    edit.set(a, VertexRecord { point: at(x, 0.0, 0.0), radius, star: Some(start) }).unwrap();
    edit.set(b, VertexRecord { point: at(1.0, 0.0, 0.0), radius: Length::from_millimetres(0.0), star: Some(finish) })
        .unwrap();
    (edit.commit(&serial()).map(|(s, _)| s), a, e)
}

#[test]
fn a_vertex_exactly_its_radius_and_the_band_from_its_edge_passes_and_any_further_fails() {
    let (quarter, half) = (1.0 / 16_777_216.0, 1.0 / 8_388_608.0);
    let tol = Tolerance::new(Length::from_millimetres(quarter), Angle::from_radians(1e-9)).unwrap();
    assert!(one_edge(tol, -half, quarter).0.is_ok(), "radius and band, exactly");
    let (refused, a, e) = one_edge(tol, (-half).next_down(), quarter);
    let reports = refused.unwrap_err();
    assert_eq!(failures(&reports), [(Reason::VertexOffEdge, vec![a.entity(), e.entity()])]);
    assert_eq!(number(&reports[0], "radius"), Some(quarter));
    assert_eq!(number(&reports[0], "parameter"), Some(0.0), "at the start of its range");
    assert!(one_edge(tol, -quarter, 0.0).0.is_ok() && one_edge(tol, -half, 0.0).0.is_err(), "the band alone");
}

#[test]
fn an_edges_range_outside_its_curves_domain_is_caught() {
    let (s, b) = clean(&cylinder(), nothing);
    for range in [(0.0, 4.0), (1.0, 1.0), (-1.0, 3.0)] {
        let mut edit = s.edit();
        let arc = b.edges[2];
        let old = record(&s, arc).unwrap().clone();
        edit.set(arc, EdgeRecord { range, ..old.clone() }).unwrap();
        let reports = refused(edit);
        let found = with(&reports, Reason::OutsideDomain);
        assert_eq!(found.len(), 1, "{range:?}: {:#?}", failures(&reports));
        assert_eq!(found[0].entities, [arc.entity(), old.curve.entity()]);
        assert_eq!((number(found[0], "start"), number(found[0], "finish")), (Some(range.0), Some(range.1)));
    }
}

#[test]
fn a_parameter_curve_on_another_faces_surface_is_caught_naming_its_edge_use_and_face() {
    let (s, b) = clean(&cube(), straight_pcurves);
    let edge = b.edges[0];
    let uses = s.view(edge).unwrap().uses();
    let (mine, theirs) = (record(&s, uses[0]).unwrap().clone(), record(&s, uses[1]).unwrap().clone());
    let mut edit = s.edit();
    edit.set(uses[0], EdgeUseRecord { pcurve: theirs.pcurve, ..mine.clone() }).unwrap();
    let reports = refused(edit);
    let UseOwner::Loop(owner) = s.view(uses[0]).unwrap().owner() else { panic!("a face's use") };
    let face = s.view(owner).unwrap().face();
    let found = with(&reports, Reason::ParamCurveOffEdge);
    assert_eq!(found.len(), 1, "{:#?}", failures(&reports));
    assert_eq!(found[0].entities, [edge.entity(), uses[0].entity(), face.entity()]);
    assert!(number(found[0], "gap").unwrap() > 0.1);
    let mut lone = s.edit();
    lone.set(uses[0], EdgeUseRecord { pcurve: None, ..mine }).unwrap();
    assert!(lone.commit(&serial()).is_ok(), "a use may have none");
}

#[test]
fn a_witness_outside_its_loop_or_within_the_band_of_it_is_caught() {
    let (s, b) = clean(&cube(), nothing);
    let (face, outer) = (b.faces[0], b.loops[0]);
    let moved = |witness: UvPoint| {
        let mut edit = s.edit();
        let old = record(&s, outer).unwrap().clone();
        edit.set(outer, LoopRecord { witness, ..old }).unwrap();
        edit.commit(&serial()).map(|(s, _)| s.step())
    };
    for witness in [UvPoint::new(5.0, 5.0), UvPoint::new(0.5, 0.0), UvPoint::new(0.5 - 0.5e-7, 0.0)] {
        let reports = moved(witness).unwrap_err();
        assert_eq!(
            failures(&reports),
            [(Reason::WitnessNotInside, vec![outer.entity(), face.entity()])],
            "{witness:?}"
        );
    }
    assert_eq!(number(&moved(UvPoint::new(5.0, 5.0)).unwrap_err()[0], "winding"), Some(0.0));
    assert!(moved(UvPoint::new(0.5 - 2e-7, 0.0)).is_ok(), "twice the band inside");
    assert!(moved(UvPoint::new(-0.49, 0.49)).is_ok(), "near a corner");
}

#[test]
fn a_hole_attached_to_its_own_reversal_is_caught() {
    let (refused, b) = attempt(&sheet_holed_by_its_reversal(), nothing);
    let reports = refused.unwrap_err();
    let want = [(Reason::LoopsNotNested, vec![b.faces[0].entity(), b.loops[0].entity(), b.loops[1].entity()])];
    assert_eq!(failures(&reports), want);
    let (s, _) = clean(&holed_sheet(), nothing);
    assert_eq!(s.ids::<Loop>().len(), 2, "a true hole passes");
}

#[test]
fn a_hole_outside_its_outer_loop_is_caught() {
    let (s, b) = clean(&holed_sheet(), nothing);
    let mut edit = s.edit();
    let old = record(&s, b.loops[1]).unwrap().clone();
    edit.set(b.loops[1], LoopRecord { witness: UvPoint::new(5.0, 0.0), ..old }).unwrap();
    let reports = refused(edit);
    let nested = with(&reports, Reason::LoopsNotNested);
    assert_eq!(nested.len(), 1, "{:#?}", failures(&reports));
    assert_eq!(nested[0].entities, [b.faces[0].entity(), b.loops[0].entity(), b.loops[1].entity()]);
}

#[test]
fn a_face_across_its_seam_is_caught_with_parameter_curves_or_without() {
    let start = FRAC_PI_2;
    let (without, b) = attempt(&cylinder_from(start), nothing);
    let want = [(Reason::OutsideSheet, vec![b.loops[1].entity(), b.faces[1].entity()])];
    assert_eq!(failures(&without.unwrap_err()), want);
    let (with_curves, _) = attempt(&cylinder_from(start), |e, b| cylinder_pcurves(e, b, start, 0.0));
    let reports = with_curves.unwrap_err();
    assert_eq!(failures(&reports), want);
    assert!(number(&reports[0], "u").unwrap() > 2.0 * PI);
    assert_eq!((number(&reports[0], "low"), number(&reports[0], "high")), (Some(0.0), Some(2.0 * PI)));
}

#[test]
fn a_point_within_the_band_of_the_seam_belongs_to_the_chart_the_faces_sheet_names() {
    for curves in [false, true] {
        let pcurves = |start: f64| {
            move |e: &mut Edit, b: &Built| {
                if curves {
                    cylinder_pcurves(e, b, start, 0.0);
                }
            }
        };
        let within = 5e-8;
        assert!(attempt(&cylinder_from(within), pcurves(within)).0.is_ok(), "half the band past 2π");
        let beyond = 3e-7;
        let (refused, b) = attempt(&cylinder_from(beyond), pcurves(beyond));
        assert_eq!(
            failures(&refused.unwrap_err()),
            [(Reason::OutsideSheet, vec![b.loops[1].entity(), b.faces[1].entity()])]
        );
    }
}

#[test]
fn a_faces_loops_lie_in_the_period_its_sheet_names() {
    let next_sheet = |e: &mut Edit, b: &Built| {
        for &face in &b.faces[..2] {
            let old = e.record(face).unwrap();
            e.set(face, FaceRecord { sheet: Sheet { u: 1, v: 0 }, ..old }).unwrap();
        }
    };
    let wit = |e: &mut Edit, b: &Built| {
        for &l in &b.loops[..2] {
            let old = e.record(l).unwrap();
            let witness = UvPoint::new(old.witness.u() + 2.0 * PI, old.witness.v());
            e.set(l, LoopRecord { witness, ..old }).unwrap();
        }
    };
    let moved = |e: &mut Edit, b: &Built| {
        next_sheet(e, b);
        wit(e, b);
    };
    assert!(attempt(&cylinder(), moved).0.is_ok(), "found on the surface, then placed in the sheet");
    let shifted = |e: &mut Edit, b: &Built| {
        moved(e, b);
        cylinder_pcurves(e, b, 0.0, 2.0 * PI);
    };
    assert!(attempt(&cylinder_from(0.0), shifted).0.is_ok(), "curves a turn on");
    let unshifted = |e: &mut Edit, b: &Built| {
        moved(e, b);
        cylinder_pcurves(e, b, 0.0, 0.0);
    };
    let (refused, b) = attempt(&cylinder_from(0.0), unshifted);
    let reports = refused.unwrap_err();
    let off: Vec<Vec<EntityRef>> = with(&reports, Reason::OutsideSheet).iter().map(|r| r.entities.clone()).collect();
    assert_eq!(off, [vec![b.loops[0].entity(), b.faces[0].entity()], vec![b.loops[1].entity(), b.faces[1].entity()]]);
    let flat = |e: &mut Edit, b: &Built| {
        let old = e.record(b.faces[2]).unwrap();
        e.set(b.faces[2], FaceRecord { sheet: Sheet { u: 0, v: 1 }, ..old }).unwrap();
    };
    let (refused, b) = attempt(&cylinder(), flat);
    let reports = refused.unwrap_err();
    assert_eq!(
        failures(&reports),
        [(Reason::OutsideSheet, vec![b.loops[2].entity(), b.faces[2].entity()])],
        "a plane has one"
    );
}

#[test]
fn a_deviation_the_caller_wrote_is_replaced_by_the_measured_one() {
    let claimed = |e: &mut Edit, b: &Built| {
        for &edge in &b.edges {
            let old = e.record(edge).unwrap();
            e.set(edge, EdgeRecord { deviation: Length::from_millimetres(7.0), ..old }).unwrap();
        }
    };
    let (s, b) = clean(&cube(), claimed);
    for &edge in &b.edges {
        assert!(s.view(edge).unwrap().deviation().millimetres() < 1e-15, "{edge:?}");
    }
    let (s, b) = clean(&sheet(), nothing);
    let edge = b.edges[0];
    let mut edit = s.edit();
    let lifted = edit.add(CurveRecord(Arc::new(occam_geom::Line::new(
        point(0.0, 0.0, 5e-8),
        Direction::normalised(Vec3::new(1.0, 0.0, 0.0)),
    ))));
    let old = record(&s, edge).unwrap().clone();
    edit.set(edge, EdgeRecord { curve: lifted, deviation: Length::from_millimetres(0.0), ..old }).unwrap();
    let (after, changes) = edit.commit(&serial()).unwrap();
    let deviation = after.view(edge).unwrap().deviation().millimetres();
    assert!((deviation - 5e-8).abs() < 1e-21, "{deviation}");
    assert_eq!(changes.changed, [edge.entity()]);
    assert!(s.view(b.edges[1]).unwrap().deviation().millimetres() < 1e-15, "an edge on its face");
}

#[test]
fn a_closed_edge_is_checked_at_both_its_ends() {
    let (s, b) = clean(&disc(), nothing);
    let mut edit = s.edit();
    let arc = b.edges[0];
    let old = record(&s, arc).unwrap().clone();
    edit.set(arc, EdgeRecord { range: (0.0, 6.0), ..old }).unwrap();
    let reports = refused(edit);
    let off = with(&reports, Reason::VertexOffEdge);
    assert_eq!(off.len(), 1, "{:#?}", failures(&reports));
    assert_eq!(
        (off[0].entities.clone(), number(off[0], "parameter")),
        (vec![b.vertices[0].entity(), arc.entity()], Some(6.0))
    );
}

/// The sheet's plane, its parameters along X and Y from (0.5, 0.5), lifted by `z`.
fn sheet_plane_lifted(z: f64) -> SurfaceRecord {
    let lift = Placement::<World, World>::translation(Vector::from_vec3(Vec3::new(0.5, 0.5, z)));
    SurfaceRecord(Arc::new(Plane::placed(&Placement::<Local, World>::identity().then(&lift))))
}

#[test]
fn a_step_and_a_chord_are_over_the_range_they_are_given() {
    assert_eq!((super::step((1.0, 3.0), super::STEPS / 2), super::step((1.0, 3.0), super::STEPS)), (2.0, 3.0));
    assert!((super::chord(x_axis().0.as_ref(), (1.0, 5.0)) - 4.0).abs() < 1e-12);
}

#[test]
fn a_parameter_curve_may_stray_from_its_edge_by_the_band_and_the_edges_measured_deviation() {
    // A square sheet with straight parameter curves, its plane then lifted 0.8 bands: each edge strays that far from the
    // face, and each parameter curve as far from its edge, which the band less the deviation would refuse.
    let (snapshot, built) = clean(&sheet(), straight_pcurves);
    let mut edit = snapshot.edit();
    let surface = edit.record(built.faces[0]).unwrap().surface;
    let lift = 0.8 * tolerance().band().millimetres();
    edit.set(surface, sheet_plane_lifted(lift)).unwrap();
    let (after, _) = edit.commit(&serial()).unwrap_or_else(|reports| panic!("{:#?}", failures(&reports)));
    for e in after.view(built.faces[0]).unwrap().edges() {
        assert!((after.view(e).unwrap().deviation().millimetres() - lift).abs() < 1e-12, "{e:?}");
    }
}

#[test]
fn the_edges_of_a_face_whose_surface_is_replaced_in_place_are_measured_again() {
    // A sheet's one face, its plane lifted 0.01: its edges, which no other face uses, stray beyond the cap.
    let (snapshot, built) = clean(&sheet(), nothing);
    let mut edit = snapshot.edit();
    let surface = edit.record(built.faces[0]).unwrap().surface;
    edit.set(surface, sheet_plane_lifted(0.01)).unwrap();
    assert_eq!(with(&refused(edit), Reason::EdgeOffFace).len(), 4);
}

#[test]
fn an_edge_whose_curve_is_replaced_in_place_is_measured_again() {
    // A cube's first edge's line replaced by an arc through its ends that bulges 0.01 across it, beyond the cap.
    let (snapshot, built) = clean(&cube(), nothing);
    let e = built.edges[0];
    let mut edit = snapshot.edit();
    let record = edit.record(e).unwrap();
    let ends = [record.ends.0, record.ends.1].map(|v| edit.record(v).unwrap().point.to_vec3());
    let along = ends[1] - ends[0];
    let across = if along.x.abs() > 0.5 { Vec3::new(0.0, 0.0, 1.0) } else { Vec3::new(1.0, 0.0, 0.0) };
    let middle = (ends[0] + ends[1]) * 0.5 + across * 0.02;
    let poles = vec![Point::from_vec3(ends[0]), Point::from_vec3(middle), Point::from_vec3(ends[1])];
    let arc = occam_geom::SplineCurve::bezier(poles, None).unwrap();
    edit.set(record.curve, CurveRecord(Arc::new(arc))).unwrap();
    let off = with(&refused(edit), Reason::EdgeOffFace).into_iter().map(|r| r.entities[0]).collect::<Vec<_>>();
    assert_eq!(off, [e.entity()]);
}
