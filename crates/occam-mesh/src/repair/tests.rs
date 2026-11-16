//! Source: original

use std::num::NonZeroUsize;

use occam_topo::Changes;
use occam_units::{Angle, Length};

use super::*;
use crate::again::again;
use crate::buffers::Buffers;
use crate::edges::sample_edges;
use crate::faces::mesh_faces;
use crate::merge::{Mesh, mesh};
use crate::part::mesh_part;
use crate::remesh::remesh;
use crate::testing::{lens, model, notched};
use crate::triangulation::Fail;

/// A deflection at which the arc of [`notched`] is one chord.
fn coarse() -> Limits {
    Limits::new(Length::from_millimetres(1.0), Angle::from_radians(core::f64::consts::PI)).unwrap()
}

/// The notched face, its wall, and their edges: the notch's two lines, the arc they share and the rest, as
/// [`notched`] lists them.
fn notch(x: f64, gap: f64) -> (Snapshot, [Id<Face>; 2], Vec<Id<Edge>>) {
    let (snapshot, edges) = model(&[notched(x, gap)]);
    let [flat, wall] = snapshot.ids::<Face>()[..] else { panic!("two faces") };
    (snapshot, [flat, wall], edges.into_iter().next().unwrap())
}

#[test]
fn a_chord_across_a_notch_is_cut_where_it_collided_until_it_clears_it_and_no_other_edge_is() {
    for (x, gap) in [(0.0, 0.1), (0.3, 1e-2), (0.3, 1e-4), (0.6, 1e-6)] {
        let (snapshot, [flat, wall], edges) = notch(x, gap);
        let first = sample_edges(&Exec::serial(), &snapshot, coarse());
        assert_eq!(first.get(edges[3]).unwrap().unwrap().params.len(), 2, "the arc is one chord");
        let refused = mesh_faces(&Exec::serial(), &snapshot, &first).get(flat).unwrap().unwrap_err().clone();
        assert_eq!((refused.code, refused.reason), (Code::FaceNotMeshed, Some(Reason::BoundaryCrosses)));
        assert!(refused.entities.contains(&edges[3].entity()), "the arc collided: {refused:#?}");
        let cut = cuts(&snapshot, flat, &first);
        assert!(cut.contains_key(&edges[3]), "{x}, {gap}: {cut:?}");
        assert!(cut.keys().all(|edge| edges[..4].contains(edge)), "only the notch's lines and the arc: {cut:?}");
        let made = mesh(&Exec::serial(), &snapshot, coarse());
        assert_eq!((made.refused.clone(), made.open.clone()), (vec![], vec![]), "{x}, {gap}");
        assert_eq!(made.edges.repaired(), cut.keys().copied().collect::<Vec<_>>());
        for (edge, samples) in made.edges.iter() {
            let (now, was) = (samples.unwrap(), first.get(edge).unwrap().unwrap());
            let added = cut.get(&edge).map_or(0, Vec::len);
            assert_eq!(now.params.len(), was.params.len() + added, "{edge:?}");
            assert!(was.params.iter().all(|t| now.params.contains(t)), "{edge:?} keeps its first samples");
        }
        assert!(made.block(wall).is_some(), "the wall, along the arc cut, meshed again with its cuts");
        assert_eq!(cuts(&snapshot, wall, &first), Cuts::new(), "the wall's own boundary needs none");
    }
}

#[test]
fn a_face_that_needs_no_repair_cuts_nothing_and_its_mesh_is_as_it_was() {
    let (snapshot, [flat, _], _) = notch(0.3, 0.5);
    let first = sample_edges(&Exec::serial(), &snapshot, coarse());
    assert_eq!(cuts(&snapshot, flat, &first), Cuts::new());
    let made = mesh(&Exec::serial(), &snapshot, coarse());
    assert_eq!(made.edges, first, "nothing repaired");
    assert_eq!(made.edges.repaired(), []);
}

#[test]
fn repair_stops_at_its_tries_and_at_the_cap_on_samples_and_keeps_no_cut() {
    let (snapshot, [flat, _], edges) = notch(0.3, 1e-4);
    let first = sample_edges(&Exec::serial(), &snapshot, coarse());
    let tries = |n: u32| {
        let mut cuts = Cuts::new();
        for _ in 0..n {
            let local = local(&snapshot, flat, &first).cut(&snapshot, &cuts);
            match collided(&snapshot, flat, &local) {
                Some(pieces) if cut(&mut cuts, &pieces, &first) => {}
                _ => return Some(cuts),
            }
        }
        None
    };
    assert!(tries(1).is_none(), "one try does not cure it");
    let cured = tries(Limits::REPAIRS_PER_FACE).unwrap();
    assert_eq!(cured, cuts(&snapshot, flat, &first));
    // Pieces with no middle strictly inside them, and an edge already at the cap.
    let at = |from: f64, to: f64| Piece { edge: edges[3], from, to };
    assert!(!cut(&mut Cuts::new(), &[at(1.0, 1.0_f64.next_up())], &first), "its middle rounds to its lower end");
    let up = 1.0_f64.next_up();
    assert_eq!(0.5 * up + 0.5 * up.next_up(), up.next_up());
    assert!(!cut(&mut Cuts::new(), &[at(up, up.next_up())], &first), "its middle rounds to its upper end");
    let full = vec![0.0; usize::try_from(Limits::SAMPLES_PER_EDGE).unwrap() - 2];
    let mut capped = Cuts::from([(edges[3], full)]);
    assert!(!cut(&mut capped, &[at(1.0, 2.0)], &first), "two samples and the cap less two, and one more");
    let mut room = Cuts::from([(edges[3], vec![0.0; usize::try_from(Limits::SAMPLES_PER_EDGE).unwrap() - 3])]);
    assert!(cut(&mut room, &[at(1.0, 2.0)], &first), "the cap exactly");
}

#[test]
fn a_failure_finer_samples_cannot_cure_names_no_piece() {
    let (snapshot, [flat, _], edges) = notch(0.3, 1e-2);
    let first = sample_edges(&Exec::serial(), &snapshot, coarse());
    let view = snapshot.view(flat).unwrap();
    let outline = boundary::of(&snapshot, view, &first).unwrap();
    for fail in [Fail::Coincide(0, 1), Fail::Undecided, Fail::Broken] {
        assert_eq!(outline.collided(fail), [], "{fail:?}");
    }
    // The chord of the arc, from (s, s) to (-s, s), and the notch's first line, from (-s, 0) to the notch.
    let [chord, line] = [3, 0].map(|k| outline.segments.iter().position(|s| outline.from[s[1]] == edges[k]).unwrap());
    let ends = |s: usize| outline.segments[s];
    let pieces = outline.collided(Fail::Crosses(ends(chord), ends(line)));
    assert_eq!(pieces.iter().map(|p| p.edge).collect::<Vec<_>>(), [edges[3], edges[0]]);
    assert_eq!(outline.collided(Fail::Touches(ends(chord), 1)).len(), 1);
    let [a, b] = ends(chord);
    assert_eq!(outline.collided(Fail::Touches([b, a], 1)), outline.collided(Fail::Touches([a, b], 1)), "either way");
    // The report names the edges whose pieces collided: the chord's, not that of the point the loop opened at.
    let mut both = vec![edges[0], edges[3]];
    both.sort_unstable();
    assert_eq!(outline.involved(Fail::Crosses(ends(chord), ends(line))), both);
    assert_eq!(outline.involved(Fail::Touches(ends(chord), 1)), {
        let mut named = vec![edges[3], outline.from[1]];
        named.sort_unstable();
        named
    });
    assert_eq!(outline.involved(Fail::Coincide(a, a)), [outline.from[a]]);
    assert_eq!(outline.involved(Fail::Undecided), []);
}

#[test]
fn an_edge_cut_twice_at_one_place_takes_one_sample_there_in_order_and_keeps_its_first_samples() {
    let (snapshot, _, edges) = notch(0.3, 0.5);
    let first = sample_edges(&Exec::serial(), &snapshot, coarse());
    let arc = edges[3];
    let was = first.get(arc).unwrap().unwrap().clone();
    let [lo, hi] = [was.params[0], was.params[1]];
    let (middle, quarter) = (0.5 * lo + 0.5 * hi, 0.75 * lo + 0.25 * hi);
    let cut = first.cut(&snapshot, &Cuts::from([(arc, vec![middle, quarter, middle])]));
    let now = cut.get(arc).unwrap().unwrap();
    assert_eq!(now.params, [lo, quarter, middle, hi]);
    assert_eq!((now.points[0], now.points[3]), (was.points[0], was.points[1]), "the ends at the vertices, bit for bit");
    let circle = snapshot.view(arc).unwrap();
    assert_eq!(now.points[1], circle.curve().unwrap().first(quarter).0, "a cut's point is its curve's there");
    assert_eq!(cut.repaired(), [arc]);
    assert_eq!(cut.get(edges[0]), first.get(edges[0]), "an edge not cut is as it was");
}

#[test]
fn a_loop_of_two_points_is_short_its_pieces_both_its_arcs_and_repair_cuts_both() {
    let (snapshot, edges) = model(&[lens()]);
    let (face, arcs) = (snapshot.ids::<Face>()[0], edges[0].clone());
    let first = sample_edges(&Exec::serial(), &snapshot, coarse());
    let outline = boundary::of(&snapshot, snapshot.view(face).unwrap(), &first).unwrap();
    let ends = |p: &Piece| (p.edge, p.from, p.to);
    let pieces: Vec<_> = outline.short_pieces().unwrap().iter().map(ends).collect();
    let quarter = core::f64::consts::FRAC_PI_4;
    assert_eq!(pieces, [(arcs[0], 5.0 * quarter, 7.0 * quarter), (arcs[1], quarter, 3.0 * quarter)]);
    let (notched_model, [flat, _], _) = notch(0.3, 0.5);
    let full = sample_edges(&Exec::serial(), &notched_model, coarse());
    assert_eq!(boundary::of(&notched_model, notched_model.view(flat).unwrap(), &full).unwrap().short_pieces(), None);
    let made = mesh(&Exec::serial(), &snapshot, coarse());
    assert_eq!((made.refused.clone(), made.open.clone(), made.edges.repaired()), (vec![], vec![], &arcs[..]));
}

/// `limits` with the faces of `model` meshed in parts, each face alone, laid end to end.
fn in_parts(snapshot: &Snapshot) -> Vec<Mesh> {
    snapshot.ids::<Face>().iter().map(|&face| mesh_part(&Exec::serial(), snapshot, coarse(), &[face])).collect()
}

#[test]
fn each_face_meshed_alone_takes_the_cuts_its_neighbour_makes_as_the_whole_mesh_does() {
    let (snapshot, [flat, wall], edges) = notch(0.3, 1e-4);
    let whole = mesh(&Exec::serial(), &snapshot, coarse());
    let four = Exec::threads(NonZeroUsize::new(4).unwrap()).unwrap();
    assert_eq!(mesh(&four, &snapshot, coarse()), whole, "the same bits at four threads");
    let parts = in_parts(&snapshot);
    for (part, face) in parts.iter().zip([flat, wall]) {
        assert_eq!(part.refused, [], "{face:?}");
        assert_eq!(part.edges.get(edges[3]), whole.edges.get(edges[3]), "{face:?}: the arc, cut as the whole cuts it");
    }
    let wall_alone = &parts[1];
    assert_eq!(wall_alone.edges.repaired(), [edges[3]], "the wall's part cut the arc, for its neighbour's collision");
    let laid = Buffers::of(&parts[0]).vertices() + Buffers::of(&parts[1]).vertices();
    assert_eq!(laid, Buffers::of(&whole).vertices());
    let block = |mesh: &Mesh, face: Id<Face>| mesh.own(face).unwrap().unwrap();
    assert_eq!(block(wall_alone, wall), block(&whole, wall), "the wall's block, bit for bit");
}

/// What an edit that rewrote `entities` says.
fn rewrote(entities: &[occam_report::EntityRef]) -> Changes {
    Changes { changed: entities.to_vec(), ..Changes::default() }
}

#[test]
fn an_edit_that_makes_a_face_collide_cuts_its_edges_and_meshes_its_neighbour_again() {
    // The same ids, the notch moved under the arc: the flat face and its lines are rewritten, the wall is not.
    let (clear, _, _) = notch(0.3, 0.5);
    let (close, [flat, wall], edges) = notch(0.3, 1e-4);
    let before = mesh(&Exec::serial(), &clear, coarse());
    assert_eq!(before.edges.repaired(), []);
    let redone = remesh(&Exec::serial(), &close, &before, [&rewrote(&[flat.entity()])]);
    assert_eq!(redone.mesh, mesh(&Exec::serial(), &close, coarse()));
    assert_eq!(redone.meshed, [flat, wall], "the wall, untouched, along the arc cut");
    assert!(redone.sampled.contains(&edges[3]));
    // A mesh before that repair cut is meshed whole.
    let after = remesh(&Exec::serial(), &close, &redone.mesh, [&rewrote(&[wall.entity()])]);
    assert_eq!((after.meshed.as_slice(), after.mesh.clone()), (&[flat, wall][..], redone.mesh));
    // And the page: the wall too, as the flat face cuts the arc otherwise than it did.
    let page = again(&Exec::serial(), &clear, &close, coarse());
    assert_eq!(page.faces, [flat, wall]);
    let back = again(&Exec::serial(), &close, &clear, coarse());
    assert_eq!(back.faces, [flat, wall], "and back");
    assert_eq!(again(&Exec::serial(), &close, &close, coarse()).faces, [], "and nothing where nothing changed");
}
