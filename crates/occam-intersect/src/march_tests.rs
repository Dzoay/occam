//! Source: original
//! The march's step control, against curves whose answer is known: the shortest and longest steps, the sag a chord
//! may have and the turn a step may take, the way a trace first goes, and how it recovers where the corrector fails.

use occam_geom::{Circle, Extrusion, Line, Revolution, Surface};
use occam_units::{Direction, UvPoint, Vec3};

use crate::correct::foot;

use crate::trace::{End, Jump, Start, Station, Trace, Which};
use crate::trace_tests::{
    BAND, ball, gap, level, mm, moved, nurbs_cylinder, on_both, point, start_near, tracer, upright,
};

/// The angle between each chord of `trace` and the next.
fn turns(trace: &Trace) -> Vec<f64> {
    let chord = |w: &[Station]| w[1].at.to_vec3() - w[0].at.to_vec3();
    let chords: Vec<_> = trace.stations.windows(2).map(chord).collect();
    chords.windows(2).map(|c| occam_math::atan2(c[0].cross(c[1]).norm(), c[0].dot(c[1]))).collect()
}

#[test]
fn a_coarse_sag_on_a_tight_curve_is_held_to_the_turn_a_step_may_take() {
    // The circle of radius 0.1 where the plane z = 0 cuts a sphere about the origin: at a sag of 0.01 or more, the
    // turn of the tangent, at most 0.3 a step, sets the step, and a step that turns too far is taken again shorter.
    let (plane, sphere) = (level(0.0), ball(0.1, [0.0; 3]));
    let start = start_near(&plane, &sphere, point(0.1, 0.0, 0.0));
    for sag in [1e-2, 1.0] {
        let trace = tracer(sag, 1.0, 5000).trace(&plane, &sphere, start);
        assert_eq!(
            trace.ends,
            [End::Closed; 2],
            "sag {sag}: {} stations in {} steps",
            trace.stations.len(),
            trace.steps
        );
        let most = turns(&trace).into_iter().fold(0.0, |most: f64, turn| if turn > most { turn } else { most });
        assert!(most <= 0.31, "sag {sag}: a chord turned {most}");
        // Taken again at nine tenths of the step that turns by 0.3, a step turns by about 0.27: some 23 to a turn.
        assert!(trace.stations.len() <= 32, "sag {sag}: {} stations", trace.stations.len());
        assert!(trace.stations.iter().all(|s| (gap(s.at, point(0.0, 0.0, 0.0)) - 0.1).abs() < 1e-6));
        on_both(&trace, &plane, &sphere);
    }
}

/// The length of each chord of `trace`, in station order.
fn chords(trace: &Trace) -> Vec<f64> {
    trace.stations.windows(2).map(|w| gap(w[0].at, w[1].at)).collect()
}

#[test]
fn where_the_curvature_asks_for_less_a_step_is_the_shortest() {
    // The same circle of radius 0.1 at the band's sag asks for steps of sqrt(8 band 0.1), 2.8e-4, below the shortest
    // at a longest step of 4, sqrt(8 band 4). The first step, a quarter of the longest, finds nothing until it halves.
    let (plane, sphere) = (level(0.0), ball(0.1, [0.0; 3]));
    let start = start_near(&plane, &sphere, point(0.1, 0.0, 0.0));
    let trace = tracer(BAND, 4.0, 5000).trace(&plane, &sphere, start);
    assert_eq!(trace.ends, [End::Closed; 2]);
    let shortest = (8.0 * BAND * 4.0_f64).sqrt();
    // A step is measured along the tangent, so its chord is longer by the cosine of half its turn, 4e-5 here.
    let off: Vec<_> =
        chords(&trace).into_iter().enumerate().filter(|(_, c)| (c / shortest - 1.0).abs() >= 1e-4).collect();
    assert!(off.is_empty(), "{} chords: {off:?}", chords(&trace).len());
    on_both(&trace, &plane, &sphere);
}

#[test]
fn a_trace_first_goes_along_the_cross_product_of_the_normals_and_doubles_its_step_to_the_longest() {
    // The planes z = 0 and x = 0 meet in the Y axis. The first surface's normal is Z and the second's X, so a trace
    // goes along Z x X = Y, first by a quarter of the longest step, then doubling it, then by the longest.
    let (flat, upright) = (level(0.0), upright(0.0));
    let start = start_near(&flat, &upright, point(0.0, 0.3, 0.0));
    let trace = tracer(1e-4, 0.5, 8).trace(&flat, &upright, start);
    assert_eq!(trace.ends, [End::Stalled; 2], "a line has no end");
    let along: Vec<f64> = trace.stations.windows(2).map(|w| w[1].at.to_vec3().y - w[0].at.to_vec3().y).collect();
    let want = [0.125, 0.25, 0.5, 0.5, 0.5, 0.5, 0.5];
    assert_eq!(along.len(), want.len(), "{along:?}");
    assert!(along.iter().zip(want).all(|(got, want)| (got - want).abs() < 1e-12), "{along:?}");
    on_both(&trace, &flat, &upright);
}

#[test]
fn no_chord_sags_from_the_curve_by_more_than_twice_the_sag_allowed() {
    // A circle of radius 2: a chord of length c sags by c^2 / 16 from it, so at a sag of 1e-5 none is longer than
    // sqrt(16 2e-5), 0.0179. It starts at the sphere's angle 1, so its angle comes round past a whole turn.
    let (plane, sphere) = (level(0.0), ball(2.0, [0.0; 3]));
    let (sin, cos) = occam_math::sin_cos(1.0);
    let trace =
        tracer(1e-5, 1.0, 5000).trace(&plane, &sphere, start_near(&plane, &sphere, point(2.0 * cos, 2.0 * sin, 0.0)));
    assert_eq!(trace.ends, [End::Closed; 2]);
    let longest = chords(&trace).into_iter().fold(0.0, |most: f64, c| if c > most { c } else { most });
    assert!(longest * longest / 16.0 <= 2e-5 * 1.001, "a chord of {longest}");
    assert_eq!(trace.crossings, [], "the sphere's angle comes round, past nought and a whole turn, with no seam");
    on_both(&trace, &plane, &sphere);
}

#[test]
fn where_the_normals_are_parallel_at_the_start_the_surfaces_touch_there() {
    // Two planes that are one: their normals are the same, and there is no way along to start in.
    let (one, other) = (level(1.0), level(1.0));
    let trace = tracer(1e-4, 0.5, 100).trace(&one, &other, start_near(&one, &other, point(0.3, 0.2, 1.0)));
    assert_eq!((trace.ends, trace.stations.len()), ([End::Touch; 2], 1), "{trace:?}");
    on_both(&trace, &one, &other);
}

/// The surface the arc of the circle of radius 2 about Z from `turn.0` to `turn.1` sweeps along Z: its angle stops at
/// the arc's ends, and its height runs without end.
fn swept(turn: (f64, f64)) -> Extrusion<Circle> {
    let circle = Circle::placed(&moved([0.0; 3]), mm(2.0)).unwrap().trimmed(turn.0, turn.1).unwrap();
    Extrusion::new(circle, Direction::normalised(Vec3::new(0.0, 0.0, 1.0))).unwrap()
}

#[test]
fn an_arc_across_a_surface_whose_height_runs_without_end_ends_at_its_angles_ends() {
    // Wide and narrow: the nearer end of a parameter is the one it is less far from, whatever the range's width.
    for turn in [(0.5, 2.5), (1.0, 1.5)] {
        let (arc, plane) = (swept(turn), level(0.3));
        let middle = f64::midpoint(turn.0, turn.1);
        let (sin, cos) = occam_math::sin_cos(middle);
        let trace =
            tracer(1e-4, 0.5, 5000).trace(&arc, &plane, start_near(&arc, &plane, point(2.0 * cos, 2.0 * sin, 0.3)));
        assert_eq!(trace.ends, [End::Edge; 2], "{turn:?}");
        let mut ends = [trace.stations[0].on_a.u(), trace.stations[trace.stations.len() - 1].on_a.u()];
        ends.sort_by(f64::total_cmp);
        assert_eq!(ends, [turn.0, turn.1]);
        on_both(&trace, &arc, &plane);
    }
}

/// The disc of radius 3 at height 1 about Z, turned from the line along X whose parameter 2 is on the axis, a pole the
/// disc declares; its rim, at the parameter 5, is not one.
pub(crate) fn disc() -> Revolution<Line> {
    let line = Line::new(point(-2.0, 0.0, 1.0), Direction::normalised(Vec3::new(1.0, 0.0, 0.0)));
    let up = Direction::normalised(Vec3::new(0.0, 0.0, 1.0));
    Revolution::new(point(0.0, 0.0, 0.0), up, line.trimmed(2.0, 5.0).unwrap())
}

#[test]
fn a_trace_across_a_disc_goes_through_its_pole_and_ends_at_its_rim() {
    let (disc, plane) = (disc(), upright(0.0));
    let trace = tracer(1e-4, 0.5, 5000).trace(&disc, &plane, start_near(&disc, &plane, point(0.0, 1.5, 1.0)));
    assert_eq!(trace.ends, [End::Edge; 2], "{trace:?}");
    let kinds: Vec<_> = trace.crossings.iter().map(|c| (c.on, c.jump)).collect();
    assert_eq!(kinds, [(Which::A, Jump::Pole)]);
    let rims: Vec<f64> = [trace.stations[0], trace.stations[trace.stations.len() - 1]].map(|s| s.on_a.v()).to_vec();
    assert_eq!(rims, [5.0, 5.0]);
    on_both(&trace, &disc, &plane);
    assert!((chords(&trace).iter().sum::<f64>() - 6.0).abs() < 1e-6, "rim to rim through the centre");
}

#[test]
fn a_start_beyond_the_edge_of_a_patch_finds_nothing_there() {
    // The cylinder's patch runs from 0.5 to 2.5 about Z; at 3, where its formula would carry on, it is not.
    let (patch, plane) = (nurbs_cylinder((0.5, 2.5)), level(0.3));
    let (sin, cos) = occam_math::sin_cos(3.0);
    let near = point(2.0 * cos, 2.0 * sin, 0.3);
    let at_edge = UvPoint::new(patch.domain().u.hi, 0.3);
    let beyond = foot(&patch, near.to_vec3(), at_edge);
    assert!(beyond.u() > patch.domain().u.hi && gap(patch.point(beyond), near) < 1e-12, "{beyond:?}");
    let start = Start { on_a: beyond, on_b: UvPoint::new(2.0 * cos, 2.0 * sin) };
    let trace = tracer(1e-4, 0.5, 500).trace(&patch, &plane, start);
    assert_eq!((trace.ends, trace.stations.len()), ([End::Apart; 2], 0), "{trace:?}");
}

#[test]
fn a_curve_tighter_than_the_shortest_step_can_turn_by_is_stepped_at_the_shortest() {
    // A circle of radius 0.005 at a coarse sag, at a longest step of 4: the shortest step, sqrt(8 band 4), turns by
    // 0.36, more than the 0.3 a step may, and a step taken again for its turn is never shorter than it.
    let (plane, sphere) = (level(0.0), ball(0.005, [0.0; 3]));
    let start = start_near(&plane, &sphere, point(0.005, 0.0, 0.0));
    let trace = tracer(1.0, 4.0, 5000).trace(&plane, &sphere, start);
    assert_eq!(trace.ends, [End::Closed; 2]);
    let shortest = (8.0 * BAND * 4.0_f64).sqrt();
    assert!(chords(&trace).iter().all(|c| *c >= shortest), "{:?}", chords(&trace));
    on_both(&trace, &plane, &sphere);
}
