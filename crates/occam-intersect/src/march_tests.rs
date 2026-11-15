//! Source: original
//! The march's step control, against curves whose answer is known: the shortest and longest steps, the sag a chord
//! may have and the turn a step may take, the way a trace first goes, and how it recovers where the corrector fails.

use crate::trace::{End, Station, Trace};
use crate::trace_tests::{ball, gap, level, on_both, point, start_near, tracer};

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
        assert!(trace.stations.iter().all(|s| (gap(s.at, point(0.0, 0.0, 0.0)) - 0.1).abs() < 1e-6));
        on_both(&trace, &plane, &sphere);
    }
}
