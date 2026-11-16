//! Source: original

use core::num::NonZeroUsize;

use occam_math::Exec;
use occam_report::EntityKind;
use serde_json::{Value, json};

use super::*;
use crate::Session;
use crate::session::Encoding;

/// A cube of side 2 from the origin, as `occam run` builds it.
fn cube() -> Session {
    let mut session = Session::new(Encoding::Json);
    let (ran, _) = session.run(
        r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [{"op": "box", "size": [2, 2, 2]}]}"#,
    );
    assert!(ran.failed.is_none(), "{ran:?}");
    session
}

fn pick(session: &mut Session, exec: &Exec, from: [f64; 3], direction: [f64; 3], tolerance: f64) -> Picked {
    session.pick(exec, 0.1, 0.35, Pick { from, direction, tolerance }).unwrap()
}

/// The coordinates of every vertex of `face`'s block, in `session`'s mesh.
fn block(session: &mut Session, face: EntityRef) -> Vec<[f64; 3]> {
    let meshed = session.mesh(&Exec::serial(), 0.1, 0.35).unwrap();
    let range = meshed.buffers.faces.iter().find(|f| f.face.entity() == face).unwrap().vertices.clone();
    let (first, count) = (range.start as usize, range.len());
    meshed.buffers.positions.chunks(3).skip(first).take(count).map(|p| [p[0], p[1], p[2]]).collect()
}

/// A ray through the centre of each face of the cube, from outside it along the face's normal, and the axis and side
/// the face lies on.
fn through_centres() -> Vec<([f64; 3], [f64; 3], usize, f64)> {
    let mut rays = Vec::new();
    for axis in 0..3 {
        for (side, way) in [(0.0, -1.0), (2.0, 1.0)] {
            let (mut from, mut direction) = ([1.0; 3], [0.0; 3]);
            from[axis] = side + 3.0 * way;
            direction[axis] = -way;
            rays.push((from, direction, axis, side));
        }
    }
    rays
}

#[test]
fn a_ray_through_the_centre_of_each_face_of_a_cube_picks_that_face_at_its_centre() {
    let mut session = cube();
    let mut faces = Vec::new();
    for (from, direction, axis, side) in through_centres() {
        let picked = pick(&mut session, &Exec::serial(), from, direction, 0.1);
        let hit = picked.hit.clone().unwrap();
        assert_eq!(hit.entity.kind, EntityKind::Face, "{picked:?}");
        let mut centre = [1.0; 3];
        centre[axis] = side;
        assert_eq!((hit.point, hit.distance), (centre, 3.0), "{picked:?}");
        assert!(
            block(&mut session, hit.entity).iter().all(|p| p[axis].to_bits() == side.to_bits()),
            "{} lies at {side}",
            hit.entity.text()
        );
        faces.push(hit.entity);
    }
    faces.sort();
    faces.dedup();
    assert_eq!(faces.len(), 6, "each ray its own face: {faces:?}");
}

#[test]
fn a_ray_within_the_tolerance_of_an_edge_of_the_cube_picks_the_edge_and_one_just_beyond_it_the_face() {
    let mut session = cube();
    let exec = Exec::serial();
    let within = pick(&mut session, &exec, [1.0, 0.04, 5.0], [0.0, 0.0, -1.0], 0.05).hit.unwrap();
    assert_eq!((within.entity.kind, within.point), (EntityKind::Edge, [1.0, 0.0, 2.0]), "the top's edge at y = 0");
    let samples = session.samples(&exec, 0.1, 0.35, within.entity).unwrap().unwrap();
    assert!(samples.len() >= 2 && samples.iter().all(|p| p[1..] == [0.0, 2.0]), "{samples:?}");
    let beyond = pick(&mut session, &exec, [1.0, 0.0501, 5.0], [0.0, 0.0, -1.0], 0.05).hit.unwrap();
    assert_eq!(beyond.entity.kind, EntityKind::Face);
    assert!(block(&mut session, beyond.entity).iter().all(|p| p[2..] == [2.0]), "the top");
    assert_eq!(session.samples(&exec, 0.1, 0.35, beyond.entity).unwrap(), None, "a face has no samples");
}

#[test]
fn the_same_rays_give_the_same_answers_at_one_thread_and_four_and_again_from_the_mesh_kept() {
    let four = Exec::threads(NonZeroUsize::new(4).unwrap()).unwrap();
    let (mut once, mut again) = (cube(), cube());
    for (from, direction, ..) in through_centres() {
        let first = pick(&mut once, &Exec::serial(), from, direction, 0.3);
        assert_eq!(first, pick(&mut again, &four, from, direction, 0.3), "at four threads");
        assert_eq!(first, pick(&mut again, &four, from, direction, 0.3), "from the mesh kept");
    }
    let edge = pick(&mut once, &Exec::serial(), [-1.0, 0.1, -1.0], [1.0, 0.0, 1.0], 0.3);
    assert_eq!(edge.hit.as_ref().map(|hit| hit.entity.kind), Ok(EntityKind::Edge), "at a corner's edge: {edge:?}");
    let _ = once.run(
        r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
        {"op": "transform", "target": "body:0/1", "frame": {"origin": [0, 0, 10]}}]}"#,
    );
    let moved = pick(&mut once, &Exec::serial(), [1.0, 1.0, 15.0], [0.0, 0.0, -1.0], 0.0);
    assert_eq!(moved.hit.map(|hit| hit.point), Ok([1.0, 1.0, 12.0]), "a run changes the mesh picked");
}

#[test]
fn a_pick_is_said_in_its_one_json_form_and_in_words() {
    let mut session = cube();
    let digest = session.digest().map(crate::digest_text);
    let picked = pick(&mut session, &Exec::serial(), [1.0, 1.0, 5.0], [0.0, 0.0, -2.0], 0.0);
    let entity = picked.hit.as_ref().unwrap().entity.text();
    let value: Value = serde_json::from_str(&picked.json()).unwrap();
    let want = json!({"schema": 2, "ok": true, "digest": digest, "from": [1.0, 1.0, 5.0], "direction": [0.0, 0.0, -1.0],
        "tolerance": 0.0, "entity": entity, "point": [1.0, 1.0, 2.0], "distance": 3.0, "reports": []});
    assert_eq!(value, want);
    assert_eq!(picked.words(), [(true, format!("The ray hits {entity} at (1, 1, 2) mm, 3 mm along it."))]);
    let missed = pick(&mut session, &Exec::serial(), [1.0, 1.0, 5.0], [0.0, 0.0, 1.0], 0.0);
    let value: Value = serde_json::from_str(&missed.json()).unwrap();
    assert_eq!(
        (&value["ok"], &value["entity"], &value["reports"][0]["code"]),
        (&json!(false), &Value::Null, &json!("mesh.ray-missed"))
    );
    assert!(value.get("point").is_none(), "{value}");
    let said = missed.words();
    assert_eq!(said.len(), 1);
    assert!(!said[0].0 && said[0].1.starts_with("The ray from (1, 1, 5) mm hits no face"), "{said:?}");
}

#[test]
fn a_ray_or_a_tolerance_that_cannot_be_picked_with_is_refused_by_name() {
    let mut session = cube();
    let exec = Exec::serial();
    let refused = |session: &mut Session, from: [f64; 3], direction: [f64; 3], tolerance: f64| {
        let asked = Pick { from, direction, tolerance };
        session.pick(&exec, 0.1, 0.35, asked).unwrap_err().iter().map(|r| r.code).collect::<Vec<_>>()
    };
    assert_eq!(refused(&mut session, [0.0; 3], [0.0; 3], 0.0), [Code::InputNoDirection]);
    assert_eq!(refused(&mut session, [f64::NAN, 0.0, 0.0], [0.0, 0.0, 1.0], 0.0), [Code::NotFinite]);
    assert_eq!(refused(&mut session, [0.0; 3], [0.0, 0.0, 1.0], -0.1), [Code::InputNegative]);
    assert_eq!(refused(&mut session, [0.0; 3], [0.0, 0.0, 1.0], f64::INFINITY), [Code::NotFinite]);
    let limits = session.pick(&exec, 0.0, 0.35, Pick { from: [0.0; 3], direction: [0.0, 0.0, 1.0], tolerance: 0.0 });
    assert_eq!(limits.unwrap_err()[0].code, Code::LimitNotPositive);
    assert_eq!(
        session.samples(&exec, 0.1, -1.0, EntityRef::new(EntityKind::Edge, 0, 1)).unwrap_err()[0].code,
        Code::LimitNotPositive
    );
}
