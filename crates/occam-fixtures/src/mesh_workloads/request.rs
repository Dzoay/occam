//! Source: original
//! P9's plates and solids of revolution as a request's steps, which construction makes as it makes any other: a plate
//! is a face with round holes extruded, the sphere, torus, cylinder and cone are primitives, and a bolt is its profile
//! revolved whole. Every number is in millimetres, every point in the world's frame.

use core::f64::consts::FRAC_PI_4;

use occam_math::sin_cos;
use serde_json::{Value, json};

use super::parts::{Frame, Shape};
use super::spline::world;

/// The point `local` in `frame`, as a request writes it.
fn at(frame: &Frame, local: [f64; 3]) -> Value {
    let p = world(frame, local).to_vec3();
    json!([p.x, p.y, p.z])
}

/// The frame a primitive is placed in: its origin, and its x axis turned about +Z.
fn placed(frame: &Frame) -> Value {
    let (sin, cos) = sin_cos(frame.turn);
    json!({"origin": frame.origin, "x": [cos, sin, 0.0]})
}

/// A plate `w` by `d` by `h` with round holes through it, a face then its extrusion.
fn plate(frame: &Frame, label: &str, (w, d, h): (f64, f64, f64), holes: &[(f64, f64, f64)]) -> Vec<Value> {
    let corners: Vec<Value> =
        [[0.0, 0.0], [w, 0.0], [w, d], [0.0, d]].iter().map(|c| at(frame, [c[0], c[1], 0.0])).collect();
    let circles: Vec<Value> = holes
        .iter()
        .map(|&(x, y, radius)| json!({"edges": [{"circle": {"centre": at(frame, [x, y, 0.0]), "radius": radius, "normal": [0, 0, 1]}}]}))
        .collect();
    vec![
        json!({"op": "face", "label": label, "outer": {"points": corners, "closed": true}, "holes": circles}),
        json!({"op": "extrude", "face": label, "vector": [0.0, 0.0, h]}),
    ]
}

/// The point at `angle` on the circle about `(rho, z)` of `radius`, in the profile's plane.
fn round(frame: &Frame, (rho, z): (f64, f64), radius: f64, angle: f64) -> Value {
    let (sin, cos) = sin_cos(angle);
    at(frame, [rho + radius * cos, 0.0, z + radius * sin])
}

/// A bolt's profile, from the axis out under its shank, up it, round a fillet under its head, out, up the head and
/// over a dome back to the axis, closed down the axis; revolved whole about +Z.
fn bolt(
    frame: &Frame,
    label: &str,
    (radius, length): (f64, f64),
    (head, fillet, height): (f64, f64, f64),
) -> Vec<Value> {
    let point = |rho: f64, z: f64| at(frame, [rho, 0.0, z]);
    let (under, top) = (length + fillet, length + fillet + height);
    let fillet_centre = (radius + fillet, length);
    let edges = json!([
        {"line": [point(0.0, 0.0), point(radius, 0.0)]},
        {"line": [point(radius, 0.0), point(radius, length)]},
        {"arc": [point(radius, length), round(frame, fillet_centre, fillet, 3.0 * FRAC_PI_4), point(radius + fillet, under)]},
        {"line": [point(radius + fillet, under), point(head, under)]},
        {"line": [point(head, under), point(head, top)]},
        {"arc": [point(head, top), round(frame, (0.0, top), head, FRAC_PI_4), point(0.0, top + head)]}
    ]);
    vec![
        json!({"op": "face", "label": label, "outer": {"edges": edges, "closed": true}}),
        json!({"op": "revolve", "profile": label, "origin": frame.origin, "axis": [0, 0, 1], "angle": 360}),
    ]
}

/// The steps that make `shape` in `frame`, its labels from `label`; none for a shape no request makes.
pub(crate) fn steps(frame: &Frame, label: &str, shape: &Shape) -> Vec<Value> {
    let frame_json = placed(frame);
    match *shape {
        Shape::Plate { size, ref holes } => plate(frame, label, size, holes),
        Shape::Sphere { radius } => vec![json!({"op": "sphere", "radius": radius, "frame": frame_json})],
        Shape::Torus { major, minor } => {
            vec![json!({"op": "torus", "major": major, "minor": minor, "frame": frame_json})]
        }
        Shape::Cylinder { radius, length } => {
            vec![json!({"op": "cylinder", "radius": radius, "height": length, "frame": frame_json})]
        }
        Shape::Cone { radius, length } => {
            vec![json!({"op": "cone", "radius1": radius, "radius2": 0, "height": length, "frame": frame_json})]
        }
        Shape::Bolt { radius, length, head, fillet, height } => {
            bolt(frame, label, (radius, length), (head, fillet, height))
        }
        Shape::Panel { .. } | Shape::Quilt { .. } => Vec::new(),
    }
}

/// A request of `steps`, in millimetres and degrees.
pub(crate) fn request(steps: &[Value]) -> String {
    json!({"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": steps}).to_string()
}
