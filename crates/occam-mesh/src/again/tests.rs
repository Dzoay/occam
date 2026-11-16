//! Source: original

use occam_topo::Changes;
use occam_units::{Angle, Length};

use super::*;
use crate::merge::{Mesh, mesh};
use crate::part::mesh_part;
use crate::remesh::remesh;
use crate::testing::{disc_drawn, model, plane_and_cylinder, undrawn};

/// What every test here meshes to: at 0.05 mm the cylinder's line is cut in two at a radius of 2, and in four at 1.
fn at() -> Limits {
    Limits::new(Length::from_millimetres(0.05), Angle::from_radians(0.35)).unwrap()
}

/// Every face of `model`, then every edge its faces use, each in id order.
fn everything(model: &Snapshot) -> Vec<EntityRef> {
    let faces = model.ids::<Face>();
    let edges = users(model, &faces).into_keys().map(Id::entity);
    faces.iter().map(|face| face.entity()).chain(edges).collect()
}

/// Whether each face of `after` meshed whole has the block, or the refusal, that `before` meshed whole gives it where
/// `again` keeps it, and that meshing only `again`'s faces gives it where it does not: what the page assembles.
fn assembled(before: &Snapshot, after: &Snapshot, again: &Again) -> bool {
    let (was, now): (Mesh, Mesh) = (mesh(&Exec::serial(), before, at()), mesh(&Exec::serial(), after, at()));
    let part = mesh_part(&Exec::serial(), after, at(), &again.faces);
    after.ids::<Face>().into_iter().all(|face| {
        let from = if again.faces.contains(&face) { &part } else { &was };
        from.own(face) == now.own(face)
    })
}

#[test]
fn a_model_given_again_as_it_was_is_meshed_nowhere_and_keeps_everything() {
    let (snapshot, _) = model(&[plane_and_cylinder(2.0), disc_drawn(1.0)]);
    let again = again(&Exec::serial(), &snapshot, &snapshot, at());
    assert_eq!(again, Again { faces: Vec::new(), kept: everything(&snapshot) });
}

#[test]
fn a_face_changed_is_meshed_again_with_each_face_along_an_edge_whose_samples_changed() {
    let (gentle, _) = model(&[plane_and_cylinder(2.0), disc_drawn(1.0)]);
    let (sharp, _) = model(&[plane_and_cylinder(1.0), disc_drawn(1.0)]);
    let [plane, cylinder, disc] = sharp.ids::<Face>()[..] else { panic!("three faces") };
    let again = again(&Exec::serial(), &gentle, &sharp, at());
    assert_eq!(again.faces, [plane, cylinder], "the plane, untouched, along the line whose samples changed");
    let disc_edges = users(&sharp, &[disc]).into_keys().map(Id::entity);
    let kept: Vec<EntityRef> = [disc.entity()].into_iter().chain(disc_edges).collect();
    assert_eq!(again.kept, kept, "the disc and its circle");
    assert!(assembled(&gentle, &sharp, &again));
    let written = Changes { changed: sharp.written_since(&gentle), ..Changes::default() };
    let remeshed = remesh(&Exec::serial(), &sharp, &mesh(&Exec::serial(), &gentle, at()), [&written]);
    assert_eq!(again.faces, remeshed.meshed, "the faces remesh meshes again after the edit");
}

#[test]
fn a_face_made_is_meshed_alone_one_refused_keeps_its_refusal_and_one_gone_meshes_nothing() {
    let (first, _) = model(&[undrawn(disc_drawn(1.0), 0), plane_and_cylinder(2.0)]);
    let (more, _) = model(&[undrawn(disc_drawn(1.0), 0), plane_and_cylinder(2.0), disc_drawn(3.0)]);
    let grown = again(&Exec::serial(), &first, &more, at());
    let faces = more.ids::<Face>();
    assert_eq!(grown.faces, [faces[3]], "the new disc alone");
    assert_eq!(grown.kept, everything(&first), "the undrawn disc and its refusal among what is kept");
    assert!(assembled(&first, &more, &grown));
    let shrunk = again(&Exec::serial(), &more, &first, at());
    assert_eq!(shrunk, Again { faces: Vec::new(), kept: everything(&first) }, "the disc's block goes with it");
    assert!(assembled(&more, &first, &shrunk));
}

#[test]
fn every_face_is_meshed_again_after_an_empty_model_and_the_same_at_four_threads() {
    let (snapshot, _) = model(&[plane_and_cylinder(2.0), disc_drawn(1.0)]);
    let empty = Snapshot::empty(snapshot.tolerance());
    let four = Exec::threads(std::num::NonZeroUsize::new(4).unwrap()).unwrap();
    let all = again(&four, &empty, &snapshot, at());
    assert_eq!(all, Again { faces: snapshot.ids::<Face>(), kept: Vec::new() });
    let (sharp, _) = model(&[plane_and_cylinder(1.0), disc_drawn(1.0)]);
    assert_eq!(again(&four, &snapshot, &sharp, at()), again(&Exec::serial(), &snapshot, &sharp, at()));
}
