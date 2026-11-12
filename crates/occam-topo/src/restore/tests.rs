//! Source: original
//! A model read back slot by slot is the model saved, to the digest and the ids the next edit gives, and one with a
//! record that is wrong is refused, naming it.

use std::sync::Arc;

use occam_report::{Code, Reason};
use occam_units::Length;

use super::*;
use crate::kind::{Body, CurveGeom, Edge, EdgeUse, Face, Id, Loop, ParamCurveGeom, Shell, SurfaceGeom, Vertex, Wire};
use crate::models::{Spec, build, cube, cylinder, disc, frame, holed_sheet, sheet, straight_pcurves, wire};
use crate::records::{SurfaceRecord, VertexRecord};
use crate::testing::{Unfindable, number, serial, tolerance, vertex};

/// `spec` built and committed, with `extra` run on the edit first.
fn committed(spec: &Spec, extra: impl FnOnce(&mut crate::Edit, &crate::models::Built)) -> Snapshot {
    let mut edit = Snapshot::empty(tolerance()).edit();
    let built = build(&mut edit, spec);
    extra(&mut edit, &built);
    edit.commit(&serial()).unwrap().0
}

/// Kind `K`'s slots of `s`, as a file would hand them back, each changed by `change`.
fn copied<K: Kind>(s: &Snapshot, change: &impl Fn(Id<K>, &mut Saved<K::Record>)) -> Vec<Saved<K::Record>> {
    (0_u32..)
        .zip(s.saved::<K>())
        .map(|(slot, saved)| {
            let mut copy = Saved { generation: saved.generation, record: saved.record.cloned() };
            change(Id::new(slot, saved.generation), &mut copy);
            copy
        })
        .collect()
}

/// Leaves a slot as it was.
const fn same<K: Kind>(_: Id<K>, _: &mut Saved<K::Record>) {}

/// Every table of `s` handed to a [`Restore`], unchanged.
fn restore(s: &Snapshot) -> Restore {
    let mut r = Restore::new(s.tolerance(), s.step());
    r.table::<Body>(copied(s, &same::<Body>));
    r.table::<Shell>(copied(s, &same::<Shell>));
    r.table::<Face>(copied(s, &same::<Face>));
    r.table::<Loop>(copied(s, &same::<Loop>));
    r.table::<Wire>(copied(s, &same::<Wire>));
    r.table::<EdgeUse>(copied(s, &same::<EdgeUse>));
    r.table::<Edge>(copied(s, &same::<Edge>));
    r.table::<Vertex>(copied(s, &same::<Vertex>));
    r.table::<SurfaceGeom>(copied(s, &same::<SurfaceGeom>));
    r.table::<CurveGeom>(copied(s, &same::<CurveGeom>));
    r.table::<ParamCurveGeom>(copied(s, &same::<ParamCurveGeom>));
    r
}

#[test]
fn every_clean_model_comes_back_with_its_digest() {
    for spec in [cube(), frame(), cylinder(), sheet(), wire(), disc(), holed_sheet()] {
        let s = committed(&spec, |_, _| {});
        let back = restore(&s).finish(&serial()).unwrap_or_else(|r| panic!("{r:#?}"));
        assert_eq!(back.digest(), s.digest());
        assert_eq!((back.step(), back.tolerance()), (s.step(), s.tolerance()));
    }
    let with_pcurves = committed(&cube(), straight_pcurves);
    assert_eq!(restore(&with_pcurves).finish(&serial()).unwrap().digest(), with_pcurves.digest());
}

#[test]
fn free_slots_come_back_and_are_reused_in_the_same_order() {
    let s = committed(&cube(), |_, _| {});
    let mut edit = s.edit();
    let lone = [edit.add(vertex(5.0)), edit.add(vertex(6.0)), edit.add(vertex(7.0))];
    let s = edit.commit(&serial()).unwrap().0;
    let mut edit = s.edit();
    for v in [lone[2], lone[0]] {
        edit.remove(v).unwrap();
    }
    let s = edit.commit(&serial()).unwrap().0;
    let back = restore(&s).finish(&serial()).unwrap();
    assert_eq!(back.digest(), s.digest());
    let next = |snapshot: &Snapshot| {
        let mut edit = snapshot.edit();
        [edit.add(vertex(8.0)), edit.add(vertex(9.0)), edit.add(vertex(10.0))]
    };
    let (theirs, ours) = (next(&s), next(&back));
    assert_eq!(theirs, ours);
    assert_eq!(
        theirs[0],
        Id::<Vertex>::new(lone[0].slot(), core::num::NonZeroU32::new(2).unwrap()),
        "lowest first, reused"
    );
}

#[test]
fn a_deviation_a_file_states_is_replaced_by_the_one_measured() {
    let s = committed(&cube(), |_, _| {});
    let mut r = restore(&s);
    r.table::<Edge>(copied(&s, &|_: Id<Edge>, saved: &mut Saved<crate::records::EdgeRecord>| {
        if let Some(record) = &mut saved.record {
            record.deviation = Length::from_millimetres(1e-9);
        }
    }));
    let back = r.finish(&serial()).unwrap();
    assert_eq!(back.digest(), s.digest(), "every deviation measured again");
}

#[test]
fn a_record_that_is_wrong_is_refused_naming_it() {
    let s = committed(&cube(), |_, _| {});
    let moved = s.ids::<Vertex>()[0];
    let mut r = restore(&s);
    r.table::<Vertex>(copied(&s, &|id, saved: &mut Saved<VertexRecord>| {
        if let Some(record) = saved.record.as_mut().filter(|_| id == moved) {
            record.point = crate::testing::at(9.0, 9.0, 9.0);
        }
    }));
    let reports = r.finish(&serial()).unwrap_err();
    let off: Vec<_> = reports.iter().filter(|r| r.reason == Some(Reason::VertexOffEdge)).collect();
    assert!(!off.is_empty(), "{reports:#?}");
    assert!(off.iter().all(|r| r.entities.first() == Some(&moved.entity())), "{reports:#?}");
    let mut r = restore(&s);
    let gone = s.ids::<Face>()[2];
    r.table::<Face>(copied(&s, &|id, saved: &mut Saved<crate::records::FaceRecord>| {
        if id == gone {
            saved.record = None;
        }
    }));
    let reports = r.finish(&serial()).unwrap_err();
    assert!(
        reports.iter().any(|r| r.reason == Some(Reason::DeadLink) && r.entities.contains(&gone.entity())),
        "{reports:#?}"
    );
}

#[test]
fn an_edge_whose_distance_from_a_face_cannot_be_measured_is_refused() {
    let s = committed(&cube(), |_, _| {});
    let top = s.view(s.ids::<Face>()[1]).unwrap().surface_id();
    let mut r = restore(&s);
    r.table::<SurfaceGeom>(copied(&s, &|id, saved: &mut Saved<SurfaceRecord>| {
        if id == top {
            let plane = occam_geom::Plane::placed(&occam_units::Placement::identity());
            saved.record = Some(SurfaceRecord(Arc::new(Unfindable(plane))));
        }
    }));
    let reports = r.finish(&serial()).unwrap_err();
    let unmeasured: Vec<_> = reports.iter().filter(|r| r.reason == Some(Reason::DeviationNotMeasured)).collect();
    assert_eq!(unmeasured.len(), 4, "the top's four edges: {reports:#?}");
    assert!(unmeasured.iter().all(|r| r.code == Code::InvariantFailed && number(r, "parameter") == Some(0.0)));
}
