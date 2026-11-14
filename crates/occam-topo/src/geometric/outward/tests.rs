//! Source: original
//! Where a solid reaches furthest along an axis, and which way its faces point there: at a vertex, where each face's
//! corner spans its angle, and along an edge, where each face spans half a turn. A body whose furthest point is not
//! certain along any axis is not decided.

use core::f64::consts::{FRAC_PI_2, PI, TAU};
use core::num::NonZeroU32;
use std::sync::Arc;

use occam_geom::{Circle, Plane, Sphere};
use occam_report::Code;
use occam_units::{Length, Placement, Point};

use super::*;
use crate::edit::Edit;
use crate::models::{Built, Spec, build, cube, cylinder, facing as placed_facing, holed_sheet, straight_pcurves};
use crate::records::{CurveRecord, EdgeRecord, SurfaceRecord};
use crate::snapshot::Snapshot;
use crate::testing::{Unfindable, at, serial, tolerance, turn_inside_out};

const X: Vec3 = Vec3::new(1.0, 0.0, 0.0);
const Y: Vec3 = Vec3::new(0.0, 1.0, 0.0);
const Z: Vec3 = Vec3::new(0.0, 0.0, 1.0);

/// `spec` built, with `extra` run on the edit, and committed.
fn committed(spec: &Spec, extra: impl FnOnce(&mut Edit, &Built)) -> (Snapshot, Built) {
    let mut edit = Snapshot::empty(tolerance()).edit();
    let built = build(&mut edit, spec);
    extra(&mut edit, &built);
    (edit.commit(&serial()).unwrap().0, built)
}

/// Nothing more.
fn nothing(_: &mut Edit, _: &Built) {}

/// `snapshot`'s body turned inside out, every face with its loops, not committed.
fn turned(snapshot: &Snapshot, built: &Built) -> Edit {
    let mut edit = snapshot.edit();
    built.faces.iter().for_each(|&f| turn_inside_out(&mut edit, f));
    edit
}

fn near(a: f64, b: f64) -> bool {
    (a - b).abs() < 1e-12
}

#[test]
fn a_box_reaches_along_an_axis_to_its_corner_furthest_that_way() {
    let b = Bounds::of([at(1.0, 2.0, 3.0), at(4.0, 5.0, 6.0)]).unwrap();
    let reach = |x, y, z| along(b, Vec3::new(x, y, z));
    let six = [reach(1., 0., 0.), reach(-1., 0., 0.), reach(0., 1., 0.), reach(0., -1., 0.), reach(0., 0., 1.)];
    assert_eq!(six, [4.0, -1.0, 5.0, -2.0, 6.0]);
    assert_eq!((reach(0.0, 0.0, -1.0), reach(2.0, 0.0, -1.0), reach(0.0, 0.0, 0.0)), (-3.0, 5.0, 0.0));
    let unbounded = Bounds::around(at(0.0, 0.0, 0.0), Length::from_millimetres(f64::INFINITY));
    assert_eq!(along(unbounded, X), f64::INFINITY, "a box without end, and no part of it across the axis");
}

#[test]
fn each_axis_is_tried_along_it_and_then_against_it_x_first() {
    let order = AXES.map(|a| [a.x, a.y, a.z]);
    let (x, y, z) = ([1.0, 0.0, 0.0], [0.0, 1.0, 0.0], [0.0, 0.0, 1.0]);
    let against = |[a, b, c]: [f64; 3]| [-a, -b, -c];
    assert_eq!(order, [x, against(x), y, against(y), z, against(z)]);
}

#[test]
fn of_two_numbers_or_places_as_far_the_first_is_kept() {
    assert_eq!((larger(1.0, 2.0), larger(2.0, 1.0), smaller(1.0, 2.0), smaller(2.0, 1.0)), (2.0, 2.0, 1.0, 1.0));
    assert!(larger(0.0, -0.0).is_sign_positive() && smaller(-0.0, 0.0).is_sign_negative());
    let place = |slot, reach| Furthest {
        place: Place::Vertex(Id::new(slot, NonZeroU32::MIN)),
        point: Vec3::new(reach, 0.0, 0.0),
        reach,
    };
    let (a, b, c) = (place(0, 1.0), place(1, 2.0), place(2, 1.0));
    assert_eq!([further(Some(a), Some(b)), further(Some(b), Some(a))], [Some(b), Some(b)]);
    assert_eq!(
        [further(Some(a), Some(c)), further(None, Some(c)), further(Some(a), None)],
        [Some(a), Some(c), Some(a)]
    );
    assert_eq!(further(None, None), None);
    assert!(
        past(1.0 + 1e-6, 1.0, 4e-7) && !past(1.0 + 1e-7, 1.0, 4e-7) && !past(1.5, 1.0, 0.5) && past(1.5, 1.0, 0.25)
    );
}

#[test]
fn the_slack_is_four_bands() {
    let (s, _) = committed(&cube(), nothing);
    assert_eq!(slack(&s.model), 4.0 * 1e-7);
}

#[test]
fn a_curve_reaches_furthest_where_its_samples_narrowed_say() {
    let circle = Circle::placed(&Placement::identity(), Length::from_millimetres(2.0)).unwrap();
    let (t, p) = peak(&circle, (-1.0, 2.0), X);
    assert!(t.abs() < 1e-7 && near(p.x, 2.0), "{t} {p:?}");
    let (t, p) = peak(&circle, (0.0, 3.0), Y);
    assert!((t - FRAC_PI_2).abs() < 1e-7 && near(p.y, 2.0), "{t} {p:?}");
    assert_eq!(peak(&circle, (0.2, 0.9), -X).0, 0.9, "furthest at its end, the end itself");
    assert_eq!(peak(&circle, (0.2, 0.9), X).0, 0.2);
    let across = occam_geom::Line::new(at(0.0, 0.0, 0.0), occam_units::Direction::normalised(Y));
    assert_eq!(peak(&across, (0.2, 0.9), X).0, 0.2, "as far all along: the first sample");
}

#[test]
fn golden_section_narrows_to_where_a_function_is_greatest() {
    assert!((narrow(&|x: f64| -(x - 0.3) * (x - 0.3), (0.0, 1.0)) - 0.3).abs() < 1e-7);
    assert!((narrow(&|x: f64| -(x - 0.8) * (x - 0.8), (0.0, 1.0)) - 0.8).abs() < 1e-7);
    assert!(narrow(&|x: f64| -x, (0.0, 1.0)) < 1e-12);
    assert!(narrow(&|x: f64| x, (0.0, 1.0)) > 1.0 - 1e-12);
    assert!(narrow(&|_: f64| 0.0, (0.0, 1.0)) > 1.0 - 1e-12, "as far everywhere: the step's end beyond moves");
    let (level, rising) = (narrow(&|_: f64| 0.0, (0.0, 1.0)), narrow(&|x: f64| x, (0.0, 1.0)));
    assert_eq!(level, rising, "as far at the last two points: the one nearer the end beyond, as where it rises");
}

#[test]
fn an_edge_reaches_furthest_at_an_end_unless_its_box_reaches_past_both() {
    // The cylinder's bottom circle from a half turn's start at (1, 0, 0) to its end at (-1, 0, 0).
    let (s, b) = committed(&cylinder(), nothing);
    let arc = view(&s.model, b.edges[2]).unwrap();
    let curve = arc.curve().unwrap();
    let slack = slack(&s.model);
    let reach = |axis| along(curve.bounds(0.0, PI), axis);
    assert_eq!(on_edge(arc, curve, X, reach(X), slack).place, Place::Vertex(b.vertices[0]));
    assert_eq!(on_edge(arc, curve, -X, reach(-X), slack).place, Place::Vertex(b.vertices[1]));
    let top = on_edge(arc, curve, Y, reach(Y), slack);
    let Place::Edge(e, t) = top.place else { panic!("{top:?}") };
    assert!(e == b.edges[2] && (t - FRAC_PI_2).abs() < 1e-7 && near(top.reach, 1.0) && near(top.point.y, 1.0));
    let end = on_edge(arc, curve, Y, 0.5 * slack, slack);
    assert_eq!(end.place, Place::Vertex(b.vertices[1]), "a box within the slack of an end keeps the end");
    let high = Vec3::new(0.0, 0.0, 1.0);
    let level = on_edge(arc, curve, high, reach(high), slack);
    assert_eq!(level.place, Place::Vertex(b.vertices[0]), "ends as far: the start");
    let line = view(&s.model, b.edges[0]).unwrap();
    let up = on_edge(line, line.curve().unwrap(), high, 10.0, slack);
    assert_eq!(up.place, Place::Vertex(b.vertices[2]), "searched, and furthest at its end: the end");
}

#[test]
fn a_body_reaches_furthest_at_a_vertex_or_along_an_edge_where_that_is_certain() {
    let (s, b) = committed(&cube(), nothing);
    let parts = Parts::of(&s.model, b.body).unwrap();
    let corner = furthest(&s.model, &parts, X, slack(&s.model)).unwrap();
    assert!(matches!(corner.place, Place::Vertex(_)) && near(corner.reach, 1.0), "{corner:?}");
    let (s, b) = committed(&cylinder(), nothing);
    let parts = Parts::of(&s.model, b.body).unwrap();
    let side = furthest(&s.model, &parts, Y, slack(&s.model)).unwrap();
    assert!(matches!(side.place, Place::Edge(..)) && near(side.reach, 1.0), "{side:?}");
}

/// The cube, with or without parameter curves, its bottom face on a plane whose analytic form is not known.
fn unknown_bottom(pcurves: bool) -> Edit {
    let (s, b) = committed(&cube(), |edit, built| {
        if pcurves {
            straight_pcurves(edit, built);
        }
    });
    let mut edit = s.edit();
    let surface = edit.record(b.faces[0]).unwrap().surface;
    let plane = Plane::placed(&placed_facing(Vec3::new(0.0, 0.0, -1.0), Vec3::new(0.5, 0.5, 0.0)));
    edit.set(surface, SurfaceRecord(Arc::new(Unfindable(plane)))).unwrap();
    edit
}

#[test]
fn a_face_on_an_unknown_surface_keeps_within_its_box_over_the_box_its_parameter_curves_span() {
    for (pcurves, certain) in [(false, false), (true, true)] {
        let edit = unknown_bottom(pcurves);
        let body = edit.view(edit.view(Id::<Face>::new(0, NonZeroU32::MIN)).unwrap().shell()).unwrap().body();
        let parts = Parts::of(&edit.next, body).unwrap();
        let found = AXES.map(|axis| furthest(&edit.next, &parts, axis, slack(&edit.next)).is_some());
        assert_eq!(found, [certain; 6], "with parameter curves: {pcurves}");
    }
    let edit = unknown_bottom(true);
    let face = view(&edit.next, Id::<Face>::new(0, NonZeroU32::MIN)).unwrap();
    let span = region(&edit.next, face).unwrap();
    assert!(span.lo.u() <= -0.5 && span.hi.u() >= 0.5 && span.lo.v() <= -0.5 && span.hi.v() >= 0.5, "{span:?}");
    assert!(span.hi.u() - span.lo.u() < 1.0 + 1e-9 && span.hi.v() - span.lo.v() < 1.0 + 1e-9, "{span:?}");
}

#[test]
fn a_face_holds_a_point_strictly_inside_it_and_not_one_in_its_hole_or_outside_it() {
    // The holed sheet, 3 by 3 with a hole 1 by 1 in its middle, on its own plane: a point on its outer loop or on its
    // hole's could be either side, so the face may reach as far as it.
    let (s, b) = committed(&holed_sheet(), nothing);
    let face = view(&s.model, b.faces[0]).unwrap();
    let held = |x, y| holds(&s.model, face, face.surface().unwrap(), Vec3::new(x, y, 0.0), Z + X);
    let Inside::Point(found) = held(0.5, 0.5) else { panic!("inside it") };
    assert!(matches!(found.place, Place::Face(f, _) if f == b.faces[0]), "{found:?}");
    assert_eq!((found.point, found.reach), (Vec3::new(0.5, 0.5, 0.0), 0.5));
    assert!(matches!(held(1.5, 1.5), Inside::Nothing), "in its hole");
    assert!(matches!(held(5.0, 1.5), Inside::Nothing), "outside it");
    assert!(matches!(held(1.5, 0.0), Inside::Boxed(r) if near(r, 1.5)), "on its outer loop");
    assert!(matches!(held(1.5, 1.0), Inside::Boxed(r) if near(r, 1.5)), "on its hole");
}

#[test]
fn a_surface_has_a_unit_normal_turned_over_where_reversed() {
    let sphere = Sphere::placed(&Placement::identity(), Length::from_millimetres(2.0)).unwrap();
    let uv = UvPoint::new(0.3, 0.2);
    let n = normal(&sphere, false, uv);
    let out = sphere.point(uv).to_vec3() * 0.5;
    assert!(near(n.norm(), 1.0) && near(n.dot(out).abs(), 1.0), "{n:?} {out:?}");
    assert_eq!(normal(&sphere, true, uv), -n);
}

#[test]
fn a_use_runs_from_its_start_to_its_finish_the_way_it_runs() {
    let (snapshot, built) = committed(&cube(), nothing);
    let model = &snapshot.model;
    for &id in &built.uses {
        let used = view(model, id).unwrap();
        let edge = view(model, used.edge()).unwrap();
        let ((lo, hi), curve) = (edge.range(), edge.curve().unwrap());
        let (from, to) = if used.reversed() { (hi, lo) } else { (lo, hi) };
        let way = (curve.point(to).to_vec3() - curve.point(from).to_vec3()) * (1.0 / (hi - lo));
        let (start, finish) = (travel(model, used, End::Start).unwrap(), travel(model, used, End::Finish).unwrap());
        assert_eq!((start.0, finish.0), (from, to));
        assert_eq!((start.1, finish.1), (curve.point(from).to_vec3(), curve.point(to).to_vec3()));
        assert!((start.2 - way).norm() < 1e-12 && (finish.2 - way).norm() < 1e-12);
    }
}

#[test]
fn a_corner_spans_its_angle_anticlockwise_and_none_at_a_cusp() {
    assert_eq!((spanned(FRAC_PI_2), spanned(PI)), (Some(FRAC_PI_2), Some(PI)));
    assert!(spanned(-FRAC_PI_2).is_some_and(|a| near(a, 1.5 * PI)));
    assert_eq!([spanned(0.0), spanned(-0.0), spanned(1e-7), spanned(-1e-7), spanned(f64::NAN)], [None; 5]);
    assert!(spanned(2e-6).is_some() && spanned(-2e-6).is_some_and(|a| a < TAU));
    assert_eq!((spanned(CUSP), spanned(-CUSP)), (None, None), "exactly the cusp's angle from none or a whole turn");
    assert!(clear(0.1) && clear(-0.1) && clear(PI) && !clear(0.099) && !clear(-0.099) && !clear(f64::NAN));
}

#[test]
fn the_faces_at_a_cube_s_corner_each_span_a_quarter_turn_there() {
    let (s, b) = committed(&cube(), nothing);
    let parts = Parts::of(&s.model, b.body).unwrap();
    let (point, v) = (Vec3::new(1.0, 1.0, 1.0), b.vertices[6]);
    let ending: Vec<_> = parts.uses.iter().filter(|(u, _)| u.vertices().unwrap().1 == v).collect();
    assert_eq!(ending.len(), 3);
    for &&(u, f) in &ending {
        let (angle, n) = corner(&s.model, (u, f), point, slack(&s.model)).unwrap();
        assert!(near(angle, FRAC_PI_2) && near(n.dot(Vec3::new(1.0, 1.0, 1.0)), 1.0), "{angle} {n:?}");
        assert_eq!(corner(&s.model, (u, f), point + Vec3::new(0.0, 0.0, 1e-3), slack(&s.model)), None);
        // Both curves end exactly the slack from the point, 2^-20 so that the distance is exact: near enough.
        let room = 1.0 / 1_048_576.0;
        assert!(corner(&s.model, (u, f), point + Vec3::new(0.0, 0.0, room), room).is_some(), "{room}");
    }
    let at = Furthest { place: Place::Vertex(v), point, reach: 1.0 };
    assert!(facing(&s.model, &parts, at, X, slack(&s.model)).is_some_and(|f| near(f, FRAC_PI_2)));
    assert!(facing(&s.model, &parts, at, -X, slack(&s.model)).is_some_and(|f| near(f, -FRAC_PI_2)));
}

#[test]
fn a_corner_one_of_whose_curves_ends_away_from_it_spans_nothing_that_can_be_told() {
    // The cube's corner (1, 1, 1), one of whose edges is moved a millimetre's thousandth up: each corner of a face that
    // edge runs into or out of has one curve end off it.
    let (s, b) = committed(&cube(), nothing);
    let mut edit = s.edit();
    let v = b.vertices[6];
    let moved = edit.view(v).unwrap().edges()[0];
    let record = edit.record(moved).unwrap();
    let line = edit.record(record.curve).unwrap().0;
    let (start, along) = line.first(0.0);
    let lifted = occam_geom::Line::new(
        Point::from_vec3(start.to_vec3() + Vec3::new(0.0, 0.0, 1e-3)),
        occam_units::Direction::normalised(along.rate()),
    );
    let curve = edit.add(CurveRecord(Arc::new(lifted)));
    edit.set(moved, EdgeRecord { curve, ..record }).unwrap();
    let parts = Parts::of(&edit.next, b.body).unwrap();
    let corner = Vec3::new(1.0, 1.0, 1.0);
    let mut told = Vec::new();
    for &(u, f) in parts.uses.iter().filter(|(u, _)| u.vertices().unwrap().1 == v) {
        let touches = u.edge() == moved || view(&edit.next, u.next().unwrap()).unwrap().edge() == moved;
        told.push((touches, corner_of(&edit.next, (u, f), corner).is_some()));
    }
    told.sort_unstable();
    assert_eq!(told, [(false, true), (true, false), (true, false)]);
}

/// The corner `corner` gives at `point`, to the slack.
fn corner_of(model: &Model, at: (View<'_, EdgeUse>, View<'_, Face>), point: Vec3) -> Option<(f64, Vec3)> {
    corner(model, at, point, slack(model))
}

#[test]
fn the_faces_along_an_edge_each_span_half_a_turn_there() {
    let (s, b) = committed(&cylinder(), nothing);
    let parts = Parts::of(&s.model, b.body).unwrap();
    let at = Furthest { place: Place::Edge(b.edges[2], FRAC_PI_2), point: Vec3::new(0.0, 1.0, 0.0), reach: 1.0 };
    // The side faces along y there, half a turn, and the bottom across it.
    assert!(facing(&s.model, &parts, at, Y, slack(&s.model)).is_some_and(|f| near(f, PI)));
    let inside = turned(&s, &b);
    let parts = Parts::of(&inside.next, b.body).unwrap();
    assert!(facing(&inside.next, &parts, at, Y, slack(&inside.next)).is_some_and(|f| near(f, -PI)));
}

#[test]
fn a_solid_that_faces_out_passes_and_one_inside_out_is_named_where_it_reaches_furthest() {
    let (s, b) = committed(&cube(), nothing);
    assert_eq!(inside_out(&s.model, b.body), None);
    let edit = turned(&s, &b);
    let report = inside_out(&edit.next, b.body).unwrap();
    assert_eq!((report.code, report.reason), (Code::InvariantFailed, Some(Reason::ShellInsideOut)));
    assert_eq!(report.entities[0], b.shell.unwrap().entity());
    let EntityRef { kind, .. } = report.entities[1];
    assert_eq!(kind, occam_report::EntityKind::Vertex);
    assert_eq!(report.value("facing").and_then(NamedValue::number).map(|f| near(f, -FRAC_PI_2)), Some(true));
    let furthest = report.value("furthest").unwrap().value;
    assert!(matches!(furthest, occam_report::Value::Point([x, _, _]) if near(x, 1.0)), "{furthest:?}");
    let (s, b) = committed(&cylinder(), nothing);
    let edit = turned(&s, &b);
    let report = inside_out(&edit.next, b.body).unwrap();
    assert_eq!(report.entities[0], b.shell.unwrap().entity());
    assert_eq!(inside_out(&edit.next, Id::new(9, NonZeroU32::MIN)), None, "no such body");
}

#[test]
fn a_solid_no_axis_decides_passes_whichever_way_it_faces() {
    // The cube turned inside out, its bottom on a plane whose analytic form is not known and with no parameter curves:
    // that face could reach anywhere, so the cube's furthest point is not certain along any axis.
    let edit = unknown_bottom(false);
    let body = edit.view(edit.view(Id::<Face>::new(0, NonZeroU32::MIN)).unwrap().shell()).unwrap().body();
    let shells = edit.view(body).unwrap().shells();
    let faces = edit.view(shells[0]).unwrap().faces();
    let mut edit = edit;
    for f in faces {
        turn_inside_out(&mut edit, f);
    }
    assert_eq!(inside_out(&edit.next, body), None);
}
