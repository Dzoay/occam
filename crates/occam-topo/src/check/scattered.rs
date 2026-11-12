//! Source: original
//! A commit checks a shell's joining region by region: scattered changes read what they touch, however large the
//! shell, and only a region whose border falls apart, as when a shell is cut, sends the search round the whole shell.

use core::num::NonZeroU32;

use occam_report::{EntityKind, EntityRef, Reason};

use super::incremental::whole_searches;
use super::read::reads;
use super::tests::{Failure, clean, failures, second_shell, split, split_square, whole};
use crate::edit::{Changes, Edit};
use crate::kind::sealed::Sealed;
use crate::kind::{
    Body, CurveGeom, Edge, EdgeUse, Face, Id, Kind, Loop, ParamCurveGeom, Shell, SurfaceGeom, Vertex, Wire,
};
use crate::models::{Built, Spec, build, plate, ring, straight_pcurves};
use crate::snapshot::Snapshot;
use crate::testing::{serial, tolerance};

/// `spec` built clean, then `how` made in a second edit: what its commit reports, what the whole-model check finds in
/// the model it was given, and how many whole shells the commit searched.
fn cut(spec: &Spec, how: impl FnOnce(&mut Edit, &Built)) -> (Vec<Failure>, Vec<Failure>, usize) {
    let (snapshot, built) = clean(spec);
    let mut edit = snapshot.edit();
    how(&mut edit, &built);
    let _ = crate::geometric::measure(&mut edit);
    let next = edit.next.clone();
    let searched = whole_searches();
    let commit = edit.commit(&serial()).map_or_else(|reports| failures(&reports), |_| Vec::new());
    (commit, failures(&whole(&next)), whole_searches().saturating_sub(searched))
}

#[test]
fn what_a_commit_reads_follows_the_edit_when_its_changes_are_scattered() {
    // Sixteen top squares spread over the plate, four by four, each split by its diagonal in one edit.
    let read = |k: usize| {
        let (snapshot, built) = clean(&plate(k));
        let mut edit = snapshot.edit();
        let spread = |n: usize| k * (2 * n + 1) / 8;
        for (a, b) in (0..4).flat_map(|a| (0..4).map(move |b| (a, b))) {
            split(&mut edit, built.faces[k * k + spread(b) * k + spread(a)]);
        }
        let (before, searched) = (reads(), whole_searches());
        let (after, _) = edit.commit(&serial()).unwrap();
        let used = reads() - before;
        assert_eq!(after.ids::<Face>().len(), 2 * k * k + 4 * k + 16);
        assert_eq!(whole_searches(), searched, "no shell searched whole");
        used
    };
    let (small, large) = (read(22), read(70));
    assert!(small > 0);
    assert_eq!(small, large, "records read splitting sixteen scattered faces of 1,056 faces, and of 10,080");
}

#[test]
fn a_shell_cut_in_two_by_a_ring_of_faces_is_not_joined() {
    // A plate 12 squares across loses the band round it at row 6 (top and bottom rows, and the two walls at their ends)
    // to a second shell, which leaves two pieces.
    let k = 12;
    let band: Vec<usize> = (0..k)
        .flat_map(|i| [k * k + 6 * k + i, 6 * k + i])
        .chain([2 * k * k + 4 * 6 + 2, 2 * k * k + 4 * 6 + 3])
        .collect();
    let mut first = None;
    let (commit, whole, searched) = cut(&plate(k), |edit, built| {
        first = built.shell;
        second_shell(edit, built, &band);
    });
    let first = first.unwrap().entity();
    assert!(whole.iter().any(|(reason, e)| *reason == Reason::ShellNotConnected && e[0] == first), "{whole:?}");
    assert_eq!(commit, whole);
    assert_eq!(searched, 1, "only the cut shell searched whole");
}

#[test]
fn a_band_cut_from_a_ring_leaves_it_joined_the_long_way_round() {
    // The ring 9 across loses the band at square 4 of its first side (top, bottom, outer and inner wall) to a second
    // shell: what is left is joined round the other three sides, which no region's border reaches.
    let (m, c) = (9, 4);
    let a = 4 * (m - 1);
    let band = [c, a + c, 2 * a + 4 * c, 2 * a + 4 * m + 4 * (c - 1)];
    let mut first = None;
    let (commit, whole, searched) = cut(&ring(m), |edit, built| {
        first = built.shell;
        second_shell(edit, built, &band);
    });
    let first = first.unwrap().entity();
    assert!(!whole.iter().any(|(reason, e)| *reason == Reason::ShellNotConnected && e[0] == first), "{whole:?}");
    assert!(whole.iter().any(|(reason, _)| *reason == Reason::EdgeShared), "the band's edges are shared: {whole:?}");
    assert_eq!(commit, whole);
    assert_eq!(searched, 1, "the ring searched whole, the band's own shell not");
}

#[test]
fn the_ring_is_a_clean_model_of_genus_one() {
    let (snapshot, built) = clean(&ring(5));
    assert_eq!(built.faces.len(), 2 * 16 + 4 * 5 + 4 * 3);
    assert_eq!(failures(&whole(&snapshot.model)), []);
}

/// `entity` of kind `K` in `edit` set back to its record in `before`, or removed if `before` has none.
fn back<K: Kind>(edit: &mut Edit, before: &Snapshot, entity: EntityRef) {
    let Some(generation) = NonZeroU32::new(entity.generation) else { return };
    let id = Id::<K>::new(entity.slot, generation);
    match K::table(&before.model).live(entity.slot, generation).cloned() {
        Some(record) => edit.set(id, record).unwrap(),
        None => edit.remove(id).unwrap(),
    }
}

/// An edit of `after` that undoes `changes`, made from `before`: what they created removed, what they changed set back.
fn undo(before: &Snapshot, after: &Snapshot, changes: &Changes) -> Edit {
    let mut edit = after.edit();
    for &entity in changes.created.iter().chain(&changes.changed) {
        match entity.kind {
            EntityKind::Body => back::<Body>(&mut edit, before, entity),
            EntityKind::Shell => back::<Shell>(&mut edit, before, entity),
            EntityKind::Face => back::<Face>(&mut edit, before, entity),
            EntityKind::Loop => back::<Loop>(&mut edit, before, entity),
            EntityKind::Wire => back::<Wire>(&mut edit, before, entity),
            EntityKind::EdgeUse => back::<EdgeUse>(&mut edit, before, entity),
            EntityKind::Edge => back::<Edge>(&mut edit, before, entity),
            EntityKind::Vertex => back::<Vertex>(&mut edit, before, entity),
            EntityKind::Surface => back::<SurfaceGeom>(&mut edit, before, entity),
            EntityKind::Curve => back::<CurveGeom>(&mut edit, before, entity),
            EntityKind::ParamCurve => back::<ParamCurveGeom>(&mut edit, before, entity),
            EntityKind::Model | EntityKind::Section | EntityKind::Region => {}
        }
    }
    edit
}

#[test]
fn what_a_commit_reads_follows_the_edit_when_it_undoes_a_split() {
    // The split triangle and its diagonal taken away again, and the square's loop and uses set back: a face, a loop and
    // an edge fewer, which a count of the shell's changes decides with no search of the whole shell. Removing the
    // diagonal's curve reads every face, edge and use to find what lies on it; beyond that, the same however large.
    let read = |k: usize| {
        let (snapshot, built) = clean(&plate(k));
        let (after, changes) = split_square(&snapshot, &built, k).unwrap();
        let edit = undo(&snapshot, &after, &changes);
        let (before, searched) = (reads(), whole_searches());
        let (merged, _) = edit.commit(&serial()).unwrap();
        let scanned = merged.ids::<Face>().len() + merged.ids::<Edge>().len() + merged.ids::<EdgeUse>().len();
        let used = reads() - before - scanned;
        assert_eq!(merged.ids::<Face>().len(), 2 * k * k + 4 * k);
        assert_eq!(whole_searches(), searched, "no shell searched whole");
        used
    };
    let (small, large) = (read(22), read(70));
    assert!(small > 0);
    assert_eq!(small, large, "records read beyond the scan undoing a split of 1,056 faces, and of 10,080");
}

#[test]
fn what_a_commit_reads_beyond_finding_what_lies_on_a_replaced_parameter_curve_follows_the_edit() {
    // A plate with straight parameter curves, one of which is replaced in place by itself. Finding what lies on it reads
    // every face, edge and use; beyond that, a commit reads the same however large the plate.
    let read = |k: usize| {
        let mut edit = Snapshot::empty(tolerance()).edit();
        let built = build(&mut edit, &plate(k));
        straight_pcurves(&mut edit, &built);
        let (snapshot, _) = edit.commit(&serial()).unwrap();
        let p = snapshot.view(built.uses[0]).unwrap().pcurve_id().unwrap();
        let mut edit = snapshot.edit();
        edit.set(p, ParamCurveGeom::table(&snapshot.model).live(p.slot(), p.generation()).cloned().unwrap()).unwrap();
        let scanned = snapshot.ids::<Face>().len() + snapshot.ids::<Edge>().len() + snapshot.ids::<EdgeUse>().len();
        let before = reads();
        edit.commit(&serial()).unwrap();
        reads() - before - scanned
    };
    let (small, large) = (read(6), read(12));
    assert!(small > 0);
    assert_eq!(small, large, "records read beyond the scan, on 96 faces and on 336");
}

#[test]
fn a_shell_cut_in_two_is_named_from_where_each_check_begins() {
    // As above, but the cut shell then begins from a face of the far piece, past the band. The commit searches from the
    // first face the edit touched, square 72 of the bottom row 6 (face 72), and names the first touched face it does not
    // reach, face 84, on row 7; the whole-model check names the first face not joined to the one the shell begins from,
    // face 0. Both refuse the edit, and name every other failure alike.
    let k = 12;
    let band: Vec<usize> = (0..k)
        .flat_map(|i| [k * k + 6 * k + i, 6 * k + i])
        .chain([2 * k * k + 4 * 6 + 2, 2 * k * k + 4 * 6 + 3])
        .collect();
    let mut faces = Vec::new();
    let (commit, whole, searched) = cut(&plate(k), |edit, built| {
        second_shell(edit, built, &band);
        let far = built.faces[k * k + 9 * k + 4];
        super::tests::change(edit, built.shell.unwrap(), |r: &mut crate::records::ShellRecord| r.face = far);
        faces = built.faces.clone();
    });
    let apart = |found: &[Failure]| {
        found.iter().filter(|(r, _)| *r == Reason::ShellNotConnected).map(|(_, e)| e[1]).collect::<Vec<_>>()
    };
    assert_eq!((apart(&commit), apart(&whole)), (vec![faces[84].entity()], vec![faces[0].entity()]));
    let rest =
        |found: &[Failure]| found.iter().filter(|(r, _)| *r != Reason::ShellNotConnected).cloned().collect::<Vec<_>>();
    assert_eq!(rest(&commit), rest(&whole));
    assert_eq!(searched, 1);
}

#[test]
fn what_a_commit_reads_follows_the_edit_when_a_face_is_set_to_itself() {
    // A face's record written again unchanged: its whole count, +1, is the same before and after, so the shell's count
    // is decided unchanged with no search of the whole shell.
    let read = |k: usize| {
        let (snapshot, built) = clean(&plate(k));
        let mut edit = snapshot.edit();
        let face = built.faces[k * k + (k / 2) * k + k / 2];
        let record = Face::table(&snapshot.model).live(face.slot(), face.generation()).cloned().unwrap();
        edit.set(face, record).unwrap();
        let (before, searched) = (reads(), whole_searches());
        edit.commit(&serial()).unwrap();
        assert_eq!(whole_searches(), searched, "no shell searched whole");
        reads() - before
    };
    let (small, large) = (read(22), read(70));
    assert!(small > 0);
    assert_eq!(small, large, "records read writing one face of 1,056 faces again, and of 10,080");
}
