//! Source: original
//! What replaces a shell's list of faces: a shell's faces are those that name it, joined through their edges to the face
//! it begins from. A commit and the whole-model check agree on each change to that membership.

use core::num::NonZeroU32;

use occam_report::Reason;

use super::tests::{Failure, change, clean, failures, split_square, whole};
use crate::edit::Edit;
use crate::kind::{Face, Id};
use crate::models::{Built, build, cube, plate};
use crate::records::{FaceRecord, ShellRecord};
use crate::snapshot::Snapshot;
use crate::testing::{serial, tolerance};

/// Two cubes, each a body of one shell, committed.
fn two_cubes() -> (Snapshot, Built, Built) {
    let mut edit = Snapshot::empty(tolerance()).edit();
    let (a, b) = (build(&mut edit, &cube()), build(&mut edit, &cube()));
    (edit.commit(&serial()).unwrap().0, a, b)
}

/// `how` made to `snapshot` in one edit: what its commit reports, and what the whole-model check finds in the model it
/// was given.
fn changed(snapshot: &Snapshot, how: impl FnOnce(&mut Edit)) -> (Vec<Failure>, Vec<Failure>) {
    let mut edit = snapshot.edit();
    how(&mut edit);
    let next = edit.next.clone();
    let commit = edit.commit(&serial()).map_or_else(|reports| failures(&reports), |_| Vec::new());
    (commit, failures(&whole(&next)))
}

#[test]
fn a_shell_may_begin_from_any_of_its_faces() {
    let (snapshot, a, _) = two_cubes();
    let found = changed(&snapshot, |edit| change(edit, a.shell.unwrap(), |r: &mut ShellRecord| r.face = a.faces[3]));
    assert_eq!(found, (vec![], vec![]));
}

#[test]
fn a_shell_that_begins_from_another_shells_face_names_both() {
    let (snapshot, a, b) = two_cubes();
    let found = changed(&snapshot, |edit| change(edit, a.shell.unwrap(), |r: &mut ShellRecord| r.face = b.faces[2]));
    let want = vec![(Reason::ListedElsewhere, vec![a.shell.unwrap().entity(), b.faces[2].entity()])];
    assert_eq!(found, (want.clone(), want));
}

#[test]
fn a_shell_that_begins_from_a_face_no_longer_there_names_both() {
    let (snapshot, a, _) = two_cubes();
    let f = a.faces[2];
    let gone = Id::<Face>::new(f.slot(), NonZeroU32::new(f.generation().get() + 1).unwrap());
    let found = changed(&snapshot, |edit| change(edit, a.shell.unwrap(), |r: &mut ShellRecord| r.face = gone));
    let want = vec![(Reason::DeadLink, vec![a.shell.unwrap().entity(), gone.entity()])];
    assert_eq!(found, (want.clone(), want));
}

#[test]
fn a_face_given_to_another_shell_without_joining_it_is_not_connected() {
    let (snapshot, a, b) = two_cubes();
    let (moved, to) = (a.faces[2], b.shell.unwrap());
    let (commit, whole) = changed(&snapshot, |edit| change(edit, moved, |r: &mut FaceRecord| r.shell = to));
    assert!(whole.contains(&(Reason::ShellNotConnected, vec![to.entity(), moved.entity()])), "{whole:?}");
    assert_eq!(commit, whole);
}

#[test]
fn a_face_that_names_its_shell_and_shares_an_edge_with_it_joins_it() {
    let (snapshot, built) = clean(&plate(3));
    let (after, _) = split_square(&snapshot, &built, 3).unwrap();
    let half = after.ids::<Face>().into_iter().find(|f| !snapshot.ids::<Face>().contains(f)).unwrap();
    let faces = after.view(built.shell.unwrap()).unwrap().faces();
    let mut sorted = faces.clone();
    sorted.sort();
    assert_eq!(sorted, after.ids::<Face>(), "every face of the plate once, the new one included");
    assert!(faces.contains(&half));
    assert_eq!(failures(&whole(&after.model)), []);
}
