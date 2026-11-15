//! Source: original
//! Placements and arcs at the limits of their numbers: a normal whose part across the z axis is no more than 1e-12 is
//! taken as the z axis, and an arc whose ends are nearer than rounding can tell apart, through a point across the circle
//! from them, turns a whole turn.

use core::f64::consts::TAU;

use super::*;

#[test]
fn a_normal_no_more_than_1e_12_across_the_z_axis_is_taken_as_it() {
    // Its part across z is exactly 1e-12, and its length rounds to 1: no turn at all.
    let placed = frame(Vec3::new(0.0, 0.0, 0.0), Vec3::new(1e-12, 0.0, 1.0), None);
    let axis = |v: Vec3| placed.vector(Vector::from_vec3(v)).to_vec3();
    assert_eq!(
        (axis(Vec3::new(1.0, 0.0, 0.0)), axis(Vec3::new(0.0, 0.0, 1.0))),
        (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 1.0))
    );
}

#[test]
fn an_arc_whose_ends_are_nearer_than_rounding_turns_a_whole_turn() {
    // From (1, 0, 0) to a point 2^-53 off it each way, through a point 2 away: the angle between the ends, seen from the
    // centre, rounds to nought, and the arc turns the whole circle less that, which rounds to a whole turn.
    let tiny = f64::EPSILON / 2.0;
    let (a, m, b) = (Vec3::new(1.0, 0.0, 0.0), Vec3::new(-1.0, 0.25, 0.0), Vec3::new(1.0 - tiny, tiny, 0.0));
    let arc = through(a, m, b, 1e-20).unwrap();
    assert!((arc.sweep - TAU).abs() < 1e-12, "{}", arc.sweep);
}
