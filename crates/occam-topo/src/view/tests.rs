//! Source: original
//! The views' tests: every entity listed once, and every adjacency query as counted by hand, on models built through
//! `Edit`.
#![expect(clippy::arithmetic_side_effects, reason = "small indices in tests")]

use occam_units::{Point, UvPoint, Vec3, World};

use super::*;
use crate::models::{Built, Spec, build, cube, cylinder, frame, point, sheet, wire};
use crate::testing::{serial, tolerance};

fn built(spec: &Spec) -> (Snapshot, Built) {
    let mut edit = Snapshot::empty(tolerance()).edit();
    let built = build(&mut edit, spec);
    (edit.commit(&serial()).unwrap().0, built)
}

/// How many of each kind, bodies to vertices.
fn counts(s: &Snapshot) -> [usize; 8] {
    [
        s.ids::<Body>().len(),
        s.ids::<Shell>().len(),
        s.ids::<Face>().len(),
        s.ids::<Loop>().len(),
        s.ids::<Wire>().len(),
        s.ids::<EdgeUse>().len(),
        s.ids::<Edge>().len(),
        s.ids::<Vertex>().len(),
    ]
}

fn near(a: Point<World>, b: Point<World>) -> bool {
    let d = a.to_vec3() - b.to_vec3();
    d.dot(d) < 1e-24
}

/// Each loop's or wire's uses meet end to start, round the loop or along the wire.
fn uses_meet(s: &Snapshot, uses: &[Id<EdgeUse>], closed: bool) {
    let ends: Vec<(Id<Vertex>, Id<Vertex>)> = uses.iter().map(|&u| s.view(u).unwrap().vertices().unwrap()).collect();
    let pairs = ends.len() - usize::from(!closed);
    for k in 0..pairs {
        assert_eq!(ends[k].1, ends[(k + 1) % ends.len()].0, "use {k} of {uses:?}");
    }
}

/// Each edge's curve passes through its vertices at the ends of its range.
fn curves_meet_vertices(s: &Snapshot) {
    for e in s.ids::<Edge>() {
        let edge = s.view(e).unwrap();
        let (curve, (lo, hi), (a, b)) = (edge.curve().unwrap(), edge.range(), edge.ends());
        let (start, finish) = (s.view(a).unwrap().point(), s.view(b).unwrap().point());
        assert!(near(curve.point(lo), start) && near(curve.point(hi), finish), "{e:?}");
    }
}

/// The outer loop's uses of `face`.
fn outer_uses(s: &Snapshot, face: Id<Face>) -> Vec<Id<EdgeUse>> {
    s.view(s.view(face).unwrap().outer()).unwrap().uses()
}

#[test]
fn every_entity_is_listed_once_in_slot_order() {
    let expected = [
        (cube(), [1, 1, 6, 6, 0, 24, 12, 8]),
        (frame(), [1, 1, 16, 16, 0, 64, 32, 16]),
        (cylinder(), [1, 1, 4, 4, 0, 12, 6, 4]),
        (sheet(), [1, 1, 1, 1, 0, 4, 4, 4]),
        (wire(), [1, 0, 0, 0, 1, 3, 3, 4]),
    ];
    for (spec, count) in expected {
        let (s, _) = built(&spec);
        assert_eq!(counts(&s), count);
        assert!(s.ids::<Face>().windows(2).all(|pair| pair[0] < pair[1]));
        assert!(s.ids::<EdgeUse>().windows(2).all(|pair| pair[0].slot() < pair[1].slot()));
    }
}

#[test]
fn a_cube_answers_every_adjacency_query_as_counted_by_hand() {
    let (s, b) = built(&cube());
    let (es, fs, vs) = (&b.edges, &b.faces, &b.vertices);
    let edges = |face: Id<Face>| s.view(face).unwrap().edges();
    assert_eq!(edges(fs[0]), [es[0], es[1], es[2], es[3]], "bottom");
    assert_eq!(edges(fs[2]), [es[3], es[8], es[4], es[9]], "front");
    assert_eq!(edges(fs[4]), [es[9], es[7], es[10], es[0]], "left");
    let faces = |edge: Id<Edge>| s.view(edge).unwrap().faces();
    assert_eq!(
        (faces(es[0]), faces(es[4]), faces(es[8])),
        (vec![fs[0], fs[4]], vec![fs[1], fs[2]], vec![fs[2], fs[5]])
    );
    let at = |vertex: Id<Vertex>| s.view(vertex).unwrap().edges();
    assert_eq!((at(vs[0]), at(vs[6])), (vec![es[0], es[3], es[9]], vec![es[5], es[6], es[11]]));
    for &edge in es {
        let uses = s.view(edge).unwrap().uses();
        let directions: Vec<bool> = uses.iter().map(|&u| s.view(u).unwrap().reversed()).collect();
        assert_eq!((faces(edge).len(), directions.len()), (2, 2));
        assert_ne!(directions[0], directions[1], "{edge:?} is used once each way");
    }
    assert!(vs.iter().all(|&vertex| at(vertex).len() == 3));
    for &face in fs {
        let view = s.view(face).unwrap();
        assert_eq!((view.loops(), view.shell(), view.born().step), (vec![view.outer()], b.shell.unwrap(), 1));
        uses_meet(&s, &outer_uses(&s, face), true);
    }
    let body = s.view(b.body).unwrap();
    assert_eq!((body.kind(), body.shells(), body.wires()), (BodyKind::Solid, vec![b.shell.unwrap()], vec![]));
    assert_eq!(body.born().step, 1);
    let shell = s.view(b.shell.unwrap()).unwrap();
    assert_eq!((shell.faces(), shell.body(), shell.next()), (fs.clone(), b.body, None));
    curves_meet_vertices(&s);
}

#[test]
fn a_cubes_faces_lie_on_planes_facing_out_through_their_witnesses() {
    let (s, b) = built(&cube());
    let centre = Vec3::new(0.5, 0.5, 0.5);
    for (k, &face) in b.faces.iter().enumerate() {
        let view = s.view(face).unwrap();
        let witness = s.view(view.outer()).unwrap().witness();
        let jet = view.surface().unwrap().first(witness);
        let out = jet.du.rate().cross(jet.dv.rate());
        assert!(out.dot(jet.point.to_vec3() - centre) > 0.49, "face {k} faces out");
        assert!(!view.reversed() && view.sheet() == Sheet::default() && view.next() == b.faces.get(k + 1).copied());
    }
}

#[test]
fn a_block_with_a_hole_through_it_has_every_edge_twice_and_four_at_each_corner() {
    let (s, b) = built(&frame());
    for &edge in &b.edges {
        assert_eq!(s.view(edge).unwrap().faces().len(), 2, "{edge:?}");
    }
    let valence: Vec<usize> = b.vertices.iter().map(|&v| s.view(v).unwrap().edges().len()).collect();
    assert_eq!(valence, [4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4, 4]);
    for &face in &b.faces {
        uses_meet(&s, &outer_uses(&s, face), true);
    }
    curves_meet_vertices(&s);
}

#[test]
fn a_cylinder_split_at_its_seam_answers_every_adjacency_query_as_counted_by_hand() {
    let (s, b) = built(&cylinder());
    let (es, fs, vs) = (&b.edges, &b.faces, &b.vertices);
    let edges = |face: Id<Face>| s.view(face).unwrap().edges();
    assert_eq!((edges(fs[0]), edges(fs[2])), (vec![es[2], es[1], es[4], es[0]], vec![es[3], es[2]]));
    let faces = |edge: Id<Edge>| s.view(edge).unwrap().faces();
    assert_eq!(
        (faces(es[0]), faces(es[2]), faces(es[5])),
        (vec![fs[0], fs[1]], vec![fs[0], fs[2]], vec![fs[1], fs[3]])
    );
    let at = |vertex: Id<Vertex>| s.view(vertex).unwrap().edges();
    assert_eq!((at(vs[0]), at(vs[3])), (vec![es[0], es[2], es[3]], vec![es[1], es[4], es[5]]));
    assert_eq!((s.view(es[0]).unwrap().role(), s.view(es[1]).unwrap().role()), (EdgeRole::Seam, EdgeRole::Ordinary));
    for &face in fs {
        uses_meet(&s, &outer_uses(&s, face), true);
    }
    let side = s.view(fs[0]).unwrap().surface().unwrap();
    assert!(near(side.point(UvPoint::new(core::f64::consts::FRAC_PI_2, 1.0)), point(0.0, 1.0, 1.0)));
    assert_eq!(s.view(fs[0]).unwrap().surface_id(), s.view(fs[1]).unwrap().surface_id(), "two patches of one surface");
    curves_meet_vertices(&s);
}

#[test]
fn a_sheet_and_a_wire_answer_adjacency_with_one_use_of_each_edge() {
    let (s, b) = built(&sheet());
    assert_eq!(s.view(b.body).unwrap().kind(), BodyKind::Sheet);
    for (&edge, &vertex) in b.edges.iter().zip(&b.vertices) {
        assert_eq!((s.view(edge).unwrap().faces(), s.view(edge).unwrap().uses().len()), (vec![b.faces[0]], 1));
        assert_eq!(s.view(vertex).unwrap().edges().len(), 2);
    }
    let (s, b) = built(&wire());
    let (e, v) = (&b.edges, &b.vertices);
    let body = s.view(b.body).unwrap();
    assert_eq!((body.kind(), body.shells(), body.wires()), (BodyKind::Wire, vec![], b.wires.clone()));
    let line = s.view(b.wires[0]).unwrap();
    assert_eq!((line.uses(), line.body(), line.next()), (b.uses.clone(), b.body, None));
    assert_eq!(s.view(b.uses[2]).unwrap().next(), None, "an open wire ends");
    uses_meet(&s, &b.uses, false);
    let at = |vertex: Id<Vertex>| s.view(vertex).unwrap().edges();
    assert_eq!((at(v[0]), at(v[1]), at(v[3])), (vec![e[0]], vec![e[0], e[1]], vec![e[2]]));
    assert!(e.iter().all(|&edge| s.view(edge).unwrap().faces().is_empty()));
    assert_eq!(s.view(b.uses[0]).unwrap().owner(), UseOwner::Wire(b.wires[0]));
    curves_meet_vertices(&s);
}

#[test]
fn a_view_reads_each_field_of_its_record() {
    let (s, b) = built(&cylinder());
    let seam = s.view(b.edges[0]).unwrap();
    assert_eq!((seam.id(), seam.range(), seam.deviation().millimetres()), (b.edges[0], (0.0, 2.0), 0.0));
    assert_eq!(seam.around(End::Start), Around { edge: b.edges[2], end: End::Start });
    assert_eq!(seam.around(End::Finish), Around { edge: b.edges[4], end: End::Start });
    assert_eq!((seam.uses(), seam.born().step), (vec![b.uses[3], b.uses[5]], 1));
    let corner = s.view(b.vertices[0]).unwrap();
    let star = Some(Around { edge: b.edges[0], end: End::Start });
    assert_eq!((corner.star(), corner.radius().millimetres(), corner.point()), (star, 0.0, point(1.0, 0.0, 0.0)));
    let first = s.view(b.uses[0]).unwrap();
    assert_eq!(
        (first.edge(), first.radial(), first.pcurve_id(), first.next()),
        (b.edges[2], b.uses[9], None, Some(b.uses[1]))
    );
    assert!(first.pcurve().is_none());
    let bottom = s.view(b.loops[2]).unwrap();
    assert_eq!((bottom.face(), bottom.next(), bottom.witness()), (b.faces[2], None, UvPoint::new(0.0, 0.0)));
    assert_eq!(s.view(seam.curve_id()).map(|c| c.get().family().0), Some("line"));
    assert_eq!(s.view(s.view(b.faces[0]).unwrap().surface_id()).map(|c| c.get().family().0), Some("cylinder"));
    let mut edit = s.edit();
    edit.remove(b.edges[5]).unwrap();
    assert!(s.view(b.edges[5]).is_some(), "an edit changes no snapshot");
    let newer = Id::<Edge>::new(b.edges[5].slot(), core::num::NonZeroU32::new(2).unwrap());
    assert!(s.view(newer).is_none(), "nor is a stale id read");
}

#[test]
fn a_walk_stops_where_its_chain_ends_comes_back_or_runs_too_long() {
    assert_eq!(walk(Some(0), 9, |x| (x < 3).then_some(x + 1)), [0, 1, 2, 3], "it ends");
    assert_eq!(walk(Some(0), 9, |x| Some((x + 1) % 3)), [0, 1, 2], "it comes back");
    assert_eq!(walk(Some(0), 5, |x| Some(1 + x % 2)), [0, 1, 2, 1, 2], "it never comes back");
    assert_eq!(walk(None::<u8>, 5, Some), Vec::<u8>::new());
    assert_eq!(once([3, 1, 3, 2, 1]), [3, 1, 2]);
}

/// `id`'s record in `s`, changed in place by `how`, as no edit may: for reading back what a view reads.
fn written<K: Kind>(s: &mut Snapshot, id: Id<K>, how: impl FnOnce(&mut K::Record)) {
    how(K::table_mut(&mut s.model).live_mut(id.slot(), id.generation()).unwrap());
}

#[test]
fn a_view_reads_each_link_and_flag_as_its_record_holds_it() {
    // Links that no clean model of one shell, one wire and one loop a face sets, written in place and read back.
    let (mut s, ids) = built(&cube());
    let (shell, face, other) = (ids.shell.unwrap(), ids.faces[0], ids.loops[1]);
    written(&mut s, shell, |r: &mut crate::records::ShellRecord| r.next = Some(shell));
    written(&mut s, face, |r: &mut crate::records::FaceRecord| r.reversed = true);
    written(&mut s, ids.loops[0], |r: &mut crate::records::LoopRecord| r.next = Some(other));
    assert_eq!(s.view(shell).unwrap().next(), Some(shell));
    assert!(s.view(face).unwrap().reversed());
    assert_eq!(s.view(ids.loops[0]).unwrap().next(), Some(other));
    let (mut w, ids) = built(&wire());
    let first = ids.wires[0];
    written(&mut w, first, |r: &mut crate::records::WireRecord| r.next = Some(first));
    assert_eq!(w.view(first).unwrap().next(), Some(first));
    let (mut p, ids) = built(&sheet());
    let given = Id::<crate::kind::ParamCurveGeom>::new(0, core::num::NonZeroU32::MIN);
    written(&mut p, ids.uses[0], |r: &mut crate::records::EdgeUseRecord| r.pcurve = Some(given));
    assert_eq!(p.view(ids.uses[0]).unwrap().pcurve_id(), Some(given));
}
