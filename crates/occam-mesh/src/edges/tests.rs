//! Source: original
#![expect(clippy::arithmetic_side_effects, reason = "small indices and counts in tests")]

use core::f64::consts::{FRAC_PI_2, PI};
use std::num::NonZeroUsize;

use occam_geom::{BsplineCurve, Circle, Line, SplineCurve};
use occam_math::{larger, smaller};
use occam_report::{Code, DisplayUnits, NamedValue, RemedyKind, followed_remedy};
use occam_units::{Angle, Direction, Placement};

use super::*;
use crate::testing::{Layout, collapsed_patch, disc, model, plane_and_cylinder};

fn limits(deflection: f64, angle: f64) -> Limits {
    Limits::new(Length::from_millimetres(deflection), Angle::from_radians(angle)).unwrap()
}

/// The one model of `layouts`, and its edges, in their order.
fn built(layouts: &[Layout]) -> (Snapshot, Vec<Id<Edge>>) {
    let (snapshot, edges) = model(layouts);
    (snapshot, edges.into_iter().flatten().collect())
}

/// The samples of `edge`, which must not have been refused.
fn of(edges: &Edges, edge: Id<Edge>) -> &EdgeSamples {
    edges.get(edge).unwrap().unwrap()
}

/// How far each piece of `curve` between neighbouring samples strays from its chord, at 16 steps along it.
fn strays(curve: &dyn Curve, samples: &EdgeSamples) -> Vec<f64> {
    let pieces = samples.params.windows(2).zip(samples.points.windows(2));
    let piece = |(t, p): (&[f64], &[Point<World>])| {
        let along = |k: u32| curve.point(t[0] + (t[1] - t[0]) * f64::from(k) / 16.0).to_vec3();
        (1..16).map(|k| off_chord(along(k), p[0].to_vec3(), p[1].to_vec3())).fold(0.0, larger)
    };
    pieces.map(piece).collect()
}

/// The lengths of the chords between neighbouring samples.
fn chords(samples: &EdgeSamples) -> Vec<f64> {
    samples.points.windows(2).map(|p| (p[1] - p[0]).to_vec3().norm()).collect()
}

#[test]
fn a_circles_samples_are_on_it_and_within_the_deflection_of_their_chords() {
    let (snapshot, edges) = built(&[disc(1.0)]);
    let sampled = sample_edges(&Exec::serial(), &snapshot, limits(1e-3, PI));
    let circle = of(&sampled, edges[0]);
    let curve = snapshot.view(edges[0]).unwrap().curve().unwrap();
    let off = strays(curve, circle);
    assert!(off.iter().all(|&d| d <= 1e-3), "{off:?}");
    // Each piece takes half the deflection, and halving a piece quarters its height, so the highest is above an eighth.
    assert!(off.iter().any(|&d| d > 1e-3 / 8.0), "{off:?}");
    assert_eq!((circle.params[0], circle.params[circle.params.len() - 1]), (0.0, 2.0 * PI));
    for (&t, &p) in circle.params.iter().zip(&circle.points) {
        assert!((p - curve.point(t)).to_vec3().norm() < 1e-15, "at {t}");
    }
    assert_eq!(circle.points[0], circle.points[circle.points.len() - 1], "both ends are the vertex");
}

#[test]
fn a_circle_turns_through_no_more_than_the_angle_between_samples() {
    let (snapshot, edges) = built(&[disc(1.0)]);
    let steps = |angle: f64| {
        let sampled = sample_edges(&Exec::serial(), &snapshot, limits(0.1, angle));
        of(&sampled, edges[0]).params.windows(2).map(|t| t[1] - t[0]).collect::<Vec<f64>>()
    };
    let each = |steps: &[f64], step: f64| steps.iter().all(|&s| (s - step).abs() < 1e-15);
    // The deflection alone cuts a unit circle into sixteenths, and an angle of 0.35 into 32nds.
    let (by_deflection, by_angle) = (steps(PI), steps(0.35));
    assert!(by_deflection.len() == 16 && each(&by_deflection, PI / 8.0), "{by_deflection:?}");
    assert!(by_angle.len() == 32 && each(&by_angle, PI / 16.0), "{by_angle:?}");
}

#[test]
fn a_closed_edge_is_four_pieces_however_coarse_the_deflection() {
    let (snapshot, edges) = built(&[disc(1.0)]);
    let sampled = sample_edges(&Exec::serial(), &snapshot, limits(10.0, PI));
    assert_eq!(of(&sampled, edges[0]).params, [0.0, FRAC_PI_2, PI, 1.5 * PI, 2.0 * PI]);
}

#[test]
fn a_shared_edge_is_sampled_as_finely_as_the_finest_of_its_faces_asks() {
    let (snapshot, edges) = built(&[plane_and_cylinder(2.0)]);
    let sampled = sample_edges(&Exec::serial(), &snapshot, limits(0.01, PI));
    // The cylinder of radius 2 asks for a chord 2 sqrt(0.01 * 2) long, about 0.28, of every edge it has: its straight
    // edges are cut into quarters, the one it shares with the plane too. The plane asks nothing of its own.
    let quarters = vec![0.25; 4];
    assert_eq!(chords(of(&sampled, edges[3])), quarters, "shared");
    assert_eq!(chords(of(&sampled, edges[6])), quarters, "the cylinder's own");
    for flat in [0, 1, 2] {
        assert_eq!(chords(of(&sampled, edges[flat])), [1.0], "the plane's own");
    }
    let asked = 2.0 * (0.01_f64 * 2.0).sqrt();
    for arc in [4, 5] {
        assert!(chords(of(&sampled, edges[arc])).iter().all(|&c| c <= asked), "arc {arc}");
    }
}

#[test]
fn p9s_bicubic_patch_with_a_collapsed_row_asks_for_no_more_than_the_cap() {
    let (snapshot, edges) = built(&[collapsed_patch()]);
    // Read on the collapsed row, the patch's noise would ask each unit edge for the floor, about one sample per
    // deflection; what the rest of the patch asks is far less.
    for (deflection, floor_alone) in [(1e-3, 1_000), (1e-5, 100_000)] {
        let sampled = sample_edges(&Exec::serial(), &snapshot, limits(deflection, 0.35));
        assert_eq!(sampled.refused().count(), 0, "at {deflection}");
        let cap = usize::try_from(Limits::SAMPLES_PER_EDGE).unwrap();
        let counts: Vec<usize> = edges.iter().map(|&e| of(&sampled, e).params.len()).collect();
        assert!(counts.iter().all(|&n| n <= cap && n < floor_alone / 4), "at {deflection}: {counts:?}");
    }
}

/// A circle of radius 1000 at a deflection of 1e-6 needs about 99,000 samples, six times the cap.
fn over_cap_model() -> (Snapshot, Id<Edge>) {
    let (snapshot, edges) = built(&[disc(1000.0)]);
    (snapshot, edges[0])
}

#[test]
fn an_edge_over_its_cap_is_named_and_meshing_at_its_remedys_deflection_succeeds() {
    let (snapshot, edge) = over_cap_model();
    let sampled = sample_edges(&Exec::serial(), &snapshot, limits(1e-6, 0.35));
    let refused: Vec<&Report> = sampled.refused().collect();
    assert_eq!(refused.len(), 1);
    let report = refused[0].clone();
    assert_eq!((report.code, report.named()), (Code::EdgeOverCap, vec![edge.entity()]));
    assert_eq!(sampled.get(edge), Some(Err(&report)));
    assert_eq!(
        report.message(DisplayUnits::default()),
        "The edge would need more than 16384 samples at a deflection of 1e-6 mm."
    );
    followed_remedy!(report, Code::EdgeOverCap, RemedyKind::UseDeflection, |remedy| {
        let deflection = remedy.value("deflection").and_then(NamedValue::number).unwrap_or(f64::NAN);
        let retried = sample_edges(&Exec::serial(), &snapshot, limits(deflection, 0.35));
        retried.refused().next().map_or(Ok(()), |again| Err(again.clone()))
    });
}

#[test]
fn the_remedys_deflection_is_the_first_doubling_that_fits() {
    let (snapshot, edge) = over_cap_model();
    let sampled = sample_edges(&Exec::serial(), &snapshot, limits(1e-6, 0.35));
    let report = sampled.get(edge).unwrap().unwrap_err();
    let fits =
        report.remedy(RemedyKind::UseDeflection).and_then(|r| r.value("deflection")).and_then(NamedValue::number);
    // 256 times 1e-6 cuts each quarter of the circle into 2^11 pieces; 128 times would need twice as many.
    assert_eq!(fits, Some(256e-6));
    let half = sample_edges(&Exec::serial(), &snapshot, limits(128e-6, 0.35));
    assert_eq!(half.refused().count(), 1, "half of it does not fit");
}

#[test]
fn the_samples_are_the_same_bits_at_one_thread_and_at_four() {
    let (snapshot, _) = built(&[disc(1.0), collapsed_patch(), plane_and_cylinder(2.0), disc(1000.0)]);
    let bits = |edges: &Edges| {
        let words = |s: &EdgeSamples| {
            let points = s.points.iter().map(|p| p.to_vec3()).flat_map(|v| [v.x, v.y, v.z]).map(f64::to_bits);
            s.params.iter().map(|t| t.to_bits()).chain(points).collect::<Vec<u64>>()
        };
        edges.iter().map(|(e, r)| (e, r.map(words).map_err(Report::clone))).collect::<Vec<_>>()
    };
    let four = Exec::threads(NonZeroUsize::new(4).unwrap()).unwrap();
    let serial = sample_edges(&Exec::serial(), &snapshot, limits(1e-5, 0.35));
    let threaded = sample_edges(&four, &snapshot, limits(1e-5, 0.35));
    assert_eq!(serial.iter().count(), 1 + 3 + 7 + 1, "every edge any face uses");
    assert_eq!(serial.refused().count(), 1, "the big disc's");
    assert_eq!(bits(&serial), bits(&threaded));
}

#[test]
fn edges_are_listed_in_id_order_and_one_no_face_uses_has_none() {
    let (snapshot, edges) = built(&[plane_and_cylinder(2.0)]);
    let sampled = sample_edges(&Exec::serial(), &snapshot, limits(0.01, PI));
    let mut ids = edges.clone();
    ids.sort();
    assert_eq!(sampled.iter().map(|(e, _)| e).collect::<Vec<_>>(), ids);
    let (lone, only) = built(&[disc(1.0)]);
    let one = sample_edges(&Exec::serial(), &lone, limits(0.01, PI));
    assert!(one.get(only[0]).is_some());
    assert!(edges.iter().filter(|&&e| e != only[0]).all(|&e| one.get(e).is_none()));
}

/// A piece of a straight line from 0 to 1 along X, held to `criteria`, its ends apart.
fn cut_line(criteria: &Criteria) -> Vec<f64> {
    let line = Line::new(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)), Direction::normalised(Vec3::new(1.0, 0.0, 0.0)));
    let piece = Piece { curve: &line, range: (0.0, 1.0), closed: false };
    piece.cut(criteria).unwrap().iter().map(|s| s.at).collect()
}

/// A polyline of degree 1 through `points`, its parameter from 0 to 1 with a knot at each inner point.
fn polyline(points: &[[f64; 3]]) -> SplineCurve {
    let inner = u32::try_from(points.len() - 2).unwrap();
    let mut knots = vec![0.0, 0.0];
    knots.extend((1..=inner).map(|k| f64::from(k) / f64::from(inner + 1)));
    knots.extend([1.0, 1.0]);
    let poles = points.iter().map(|&[x, y, z]| Point::from_vec3(Vec3::new(x, y, z))).collect();
    SplineCurve::new(&BsplineCurve { degree: 1, knots, poles, weights: None }).unwrap()
}

const OPEN: Criteria = Criteria { chord: f64::INFINITY, step: f64::INFINITY, floor: 0.0, turn: -1.0 };

#[test]
fn a_piece_exactly_as_long_as_the_step_or_as_high_as_the_chord_allows_is_not_cut() {
    assert_eq!(cut_line(&Criteria { step: 1.0, ..OPEN }), [0.0, 1.0]);
    assert_eq!(cut_line(&Criteria { step: 0.9, ..OPEN }), [0.0, 0.5, 1.0]);
    let tent = polyline(&[[0.0, 0.0, 0.0], [0.5, 0.25, 0.0], [1.0, 0.0, 0.0]]);
    let piece = Piece { curve: &tent, range: (0.0, 1.0), closed: false };
    let at = |criteria: &Criteria| piece.cut(criteria).unwrap().iter().map(|s| s.at).collect::<Vec<f64>>();
    assert_eq!(at(&Criteria { chord: 0.25, ..OPEN }), [0.0, 1.0]);
    assert_eq!(at(&Criteria { chord: 0.2, ..OPEN }), [0.0, 0.5, 1.0]);
}

#[test]
fn a_piece_turning_exactly_as_far_as_the_angle_allows_is_not_cut() {
    // At a whole turn a hairpin's derivatives at its ends and its middle are opposed, on the limit, so it stays whole.
    let hairpin = polyline(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [0.0, 0.0, 0.0]]);
    let piece = Piece { curve: &hairpin, range: (0.0, 1.0), closed: false };
    assert_eq!(piece.cut(&OPEN).unwrap().iter().map(|s| s.at).collect::<Vec<f64>>(), [0.0, 1.0]);
}

#[test]
fn a_piece_no_longer_than_twice_the_floor_is_never_cut() {
    // A line asks for steps of 0.1, but a floor of 0.2 stops it at quarters, each no longer than 0.4, and one of 0.3 at
    // halves.
    assert_eq!(cut_line(&Criteria { step: 0.1, floor: 0.2, ..OPEN }), [0.0, 0.25, 0.5, 0.75, 1.0]);
    assert_eq!(cut_line(&Criteria { step: 0.1, floor: 0.3, ..OPEN }), [0.0, 0.5, 1.0]);
    // A line exactly twice the floor long stays whole.
    assert_eq!(cut_line(&Criteria { step: 0.1, floor: 0.5, ..OPEN }), [0.0, 1.0]);
}

/// The parameter `bits` units in the last place above 1, where they are 2^-52 apart.
fn above_one(bits: u64) -> f64 {
    f64::from_bits(1.0_f64.to_bits() + bits)
}

/// A straight line along X from 0, cut over `range` where every piece asks to be cut: no step is short enough.
fn cut_everywhere(line: &Line, range: (f64, f64)) -> Option<Vec<f64>> {
    let piece = Piece { curve: line, range, closed: false };
    piece.cut(&Criteria { step: 0.0, ..OPEN }).map(|samples| samples.iter().map(|s| s.at).collect())
}

fn x_axis() -> Line {
    Line::new(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)), Direction::normalised(Vec3::new(1.0, 0.0, 0.0)))
}

#[test]
fn a_piece_one_bit_of_its_parameter_wide_is_never_cut_whichever_end_its_middle_rounds_to() {
    // The middle of 1 and the next double rounds to 1, whose last bit is even; that of the next two to the second.
    let line = x_axis();
    for range in [(1.0, above_one(1)), (above_one(1), above_one(2))] {
        assert_eq!(cut_everywhere(&line, range), Some(vec![range.0, range.1]));
    }
}

#[test]
fn an_edge_needing_exactly_the_caps_samples_fits_and_one_more_does_not() {
    // Cut wherever it can be, a range n bits wide ends with a sample at each of its n + 1 doubles.
    let line = x_axis();
    let cap = u64::from(Limits::SAMPLES_PER_EDGE);
    let fits = cut_everywhere(&line, (1.0, above_one(cap - 1))).unwrap();
    assert_eq!(fits, (0..cap).map(above_one).collect::<Vec<f64>>());
    assert_eq!(cut_everywhere(&line, (1.0, above_one(cap))), None);
}

#[test]
fn a_corner_is_cut_to_its_parameters_resolution_and_stops_there() {
    // The turn at the corner never goes, so only the floor or the parameter's last bit can stop the cutting.
    let corner = polyline(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0]]);
    let piece = Piece { curve: &corner, range: (0.0, 1.0), closed: false };
    let fine = piece.cut(&Criteria { turn: limits(1.0, 0.35).half_turn_cosine(), ..OPEN }).unwrap();
    assert!(fine.len() < 200, "{} samples", fine.len());
    let near = fine.windows(2).map(|s| s[1].at - s[0].at).fold(1.0, smaller);
    assert!(near <= f64::EPSILON, "the corner's pieces are a bit of the parameter wide, not {near}");
    let floored = piece.cut(&Criteria::new(limits(0.01, 0.35), 0.0)).unwrap();
    assert!(floored.len() < 20, "{} samples", floored.len());
}

#[test]
fn a_closed_edge_whose_parameter_is_not_an_angle_starts_as_four_pieces() {
    let ring = polyline(&[[0.0, 0.0, 0.0], [1.0, 0.0, 0.0], [1.0, 1.0, 0.0], [0.0, 0.0, 0.0]]);
    let piece = Piece { curve: &ring, range: (0.0, 1.0), closed: true };
    assert_eq!(piece.first_cuts(), [0.0, 0.25, 0.5, 0.75, 1.0]);
    let open = Piece { closed: false, ..piece };
    assert_eq!(open.first_cuts(), [0.0, 1.0]);
}

#[test]
fn an_arc_starts_as_pieces_of_at_most_a_quarter_turn() {
    let circle = Circle::placed(&Placement::identity(), Length::from_millimetres(1.0)).unwrap();
    let arc = |hi: f64| Piece { curve: &circle, range: (0.0, hi), closed: false }.first_cuts();
    assert_eq!(arc(FRAC_PI_2), [0.0, FRAC_PI_2]);
    assert_eq!(arc(PI), [0.0, FRAC_PI_2, PI]);
    assert_eq!(arc(1.5 * PI).len(), 5, "three quarters of a turn in four pieces");
    assert_eq!(arc(5.0 * PI).len(), 9, "never more than eight pieces");
    let later = Piece { curve: &circle, range: (PI, 2.0 * PI), closed: false }.first_cuts();
    assert_eq!(later, [PI, 1.5 * PI, 2.0 * PI], "half a turn from a half turn on");
}
