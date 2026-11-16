//! Source: original

use occam_report::EntityKind;
use occam_topo::{Edge, Loop, View};
use occam_units::{Angle, Length};

use super::*;
use crate::limits::Limits;
use crate::merge::mesh;
use crate::testing::{disc_drawn, model, plane_and_cylinder, undrawn};

fn limits(deflection: f64) -> Limits {
    Limits::new(Length::from_millimetres(deflection), Angle::from_radians(0.35)).unwrap()
}

/// What an edit that rewrote `entities` and made nothing says.
fn rewrote(entities: &[EntityRef]) -> Changes {
    Changes { changed: entities.to_vec(), ..Changes::default() }
}

/// `model` meshed whole, at 0.05.
fn whole(model: &Snapshot) -> Mesh {
    mesh(&Exec::serial(), model, limits(0.05))
}

/// The edges `face` uses, in id order.
fn edges_of(model: &Snapshot, face: Id<Face>) -> Vec<Id<Edge>> {
    let mut edges = model.view(face).unwrap().edges();
    edges.sort_unstable();
    edges
}

#[test]
fn an_edit_that_touched_nothing_meshes_nothing_and_gives_the_same_mesh() {
    let (snapshot, _) = model(&[plane_and_cylinder(2.0), disc_drawn(1.0)]);
    let before = whole(&snapshot);
    let again = remesh(&Exec::serial(), &snapshot, &before, &[Changes::default()]);
    assert_eq!(again, Remeshed { mesh: before, meshed: Vec::new(), sampled: Vec::new() });
}

#[test]
fn a_face_touched_is_meshed_again_alone_where_the_samples_of_its_edges_hold() {
    let (snapshot, _) = model(&[plane_and_cylinder(2.0), disc_drawn(1.0)]);
    let before = whole(&snapshot);
    let plane = snapshot.ids::<Face>()[0];
    let again = remesh(&Exec::serial(), &snapshot, &before, [&rewrote(&[plane.entity()])]);
    assert_eq!(again.meshed, [plane], "not the cylinder, along the line it shares, nor the disc");
    assert_eq!(again.sampled, edges_of(&snapshot, plane), "the line the cylinder shares among them");
    assert_eq!(again.mesh, before);
}

#[test]
fn anything_a_face_is_drawn_from_touches_it() {
    let (snapshot, edges) = model(&[plane_and_cylinder(2.0)]);
    let before = whole(&snapshot);
    let plane = snapshot.view(snapshot.ids::<Face>()[0]).unwrap();
    // The plane's edge from (1, 0) to (1, 1), which the cylinder does not use, and its use, curve and first vertex.
    let edge = snapshot.view(edges[0][1]).unwrap();
    let round = snapshot.view(plane.loops()[0]).map(View::<Loop>::uses).unwrap();
    let side = round.into_iter().find(|&u| snapshot.view(u).unwrap().edge() == edge.id()).unwrap();
    let pcurve = snapshot.view(side).unwrap().pcurve_id().unwrap();
    let parts = [
        plane.id().entity(),
        plane.surface_id().entity(),
        plane.loops()[0].entity(),
        side.entity(),
        pcurve.entity(),
        edge.id().entity(),
        edge.curve_id().entity(),
        edge.ends().0.entity(),
    ];
    for part in parts {
        let again = remesh(&Exec::serial(), &snapshot, &before, [&rewrote(&[part])]);
        assert_eq!(again.meshed, [plane.id()], "{part:?}");
        assert_eq!(again.mesh, before, "{part:?}");
    }
}

#[test]
fn every_face_along_an_edge_whose_samples_changed_is_meshed_again() {
    // At 0.05 the line the plane shares is cut in two where the cylinder's radius is 2, and in four where it is 1. The
    // two models have the same ids, and only the cylinder's face, and what it alone is drawn from, differ.
    let (gentle, _) = model(&[plane_and_cylinder(2.0)]);
    let (sharp, edges) = model(&[plane_and_cylinder(1.0)]);
    let before = whole(&gentle);
    let [plane, cylinder] = sharp.ids::<Face>()[..] else { panic!("two faces") };
    let again = remesh(&Exec::serial(), &sharp, &before, [&rewrote(&[cylinder.entity()])]);
    let line = edges[0][3];
    let count = |mesh: &Mesh| mesh.edges.get(line).unwrap().unwrap().params.len();
    assert_eq!((count(&before), count(&again.mesh)), (3, 5));
    assert_eq!(again.meshed, [plane, cylinder], "the plane, untouched, along the line whose samples changed");
    assert_eq!(again.sampled, edges_of(&sharp, cylinder));
    assert_eq!(again.mesh, whole(&sharp));
}

/// Every entity `model` holds that `before` does not.
fn new_in(model: &Snapshot, before: &Snapshot) -> Vec<EntityRef> {
    let had = |e: &EntityRef| before.resolve_entity(*e).is_ok();
    EntityKind::ALL.iter().flat_map(|&kind| model.entities(kind)).filter(|e| !had(e)).collect()
}

#[test]
fn a_face_made_is_meshed_one_gone_takes_its_block_and_one_refused_keeps_its_refusal() {
    let (first, _) = model(&[undrawn(disc_drawn(1.0), 0), plane_and_cylinder(2.0)]);
    let (more, _) = model(&[undrawn(disc_drawn(1.0), 0), plane_and_cylinder(2.0), disc_drawn(3.0)]);
    let before = whole(&first);
    assert_eq!(before.refused.len(), 1, "the undrawn disc");
    let made = Changes { created: new_in(&more, &first), ..Changes::default() };
    let grown = remesh(&Exec::serial(), &more, &before, [&made]);
    let disc = more.ids::<Face>()[3];
    assert_eq!((grown.meshed, &grown.sampled), (vec![disc], &edges_of(&more, disc)));
    assert_eq!(grown.mesh, whole(&more));
    let shrunk = remesh(&Exec::serial(), &first, &grown.mesh, [&Changes::default()]);
    assert_eq!((shrunk.meshed, shrunk.sampled), (Vec::new(), Vec::new()), "the undrawn disc's refusal is kept");
    assert_eq!(shrunk.mesh, before, "the new disc's block and its circle's samples go with it");
}

#[test]
fn the_mesh_again_is_the_same_bits_at_one_thread_and_at_four() {
    let (gentle, _) = model(&[plane_and_cylinder(2.0), disc_drawn(1.0)]);
    let (sharp, _) = model(&[plane_and_cylinder(1.0), disc_drawn(1.0)]);
    let before = whole(&gentle);
    let cylinder = rewrote(&[sharp.ids::<Face>()[1].entity()]);
    let four = Exec::threads(std::num::NonZeroUsize::new(4).unwrap()).unwrap();
    let serial = remesh(&Exec::serial(), &sharp, &before, [&cylinder]);
    assert_eq!(serial.mesh.digest(), remesh(&four, &sharp, &before, [&cylinder]).mesh.digest());
    assert_eq!(serial.mesh.digest(), whole(&sharp).digest());
}
