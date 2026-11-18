//! Source: original
//! What the corpus's tests share: the `occam-oracle-case/1` files under `tools/corpus`, and each of a case's
//! operations that the runner builds, translated into the runner's step.

use std::f64::consts::PI;
use std::fs;
use std::path::Path;

use occam_math::sin_cos;
use occam_units::Vec3;
use serde_json::{Value, json};

/// The corpus's operations the runner builds today, by the case format's name.
pub const BUILT: [&str; 6] = ["box", "cylinder", "cone", "sphere", "torus", "rotate"];

/// Every case of the corpus, by file name, in order.
pub fn cases() -> Vec<(String, Value)> {
    let root = Path::new(env!("CARGO_MANIFEST_DIR")).join("../../tools/corpus");
    let mut found = Vec::new();
    for dir in ["generated", "imported"] {
        for entry in fs::read_dir(root.join(dir)).unwrap() {
            let path = entry.unwrap().path();
            if path.extension().is_some_and(|e| e == "json") {
                let case: Value = serde_json::from_str(&fs::read_to_string(&path).unwrap()).unwrap();
                assert_eq!(case["schema"], "occam-oracle-case/1", "{}", path.display());
                found.push((path.file_name().unwrap().to_string_lossy().into_owned(), case));
            }
        }
    }
    found.sort_by(|a, b| a.0.cmp(&b.0));
    found
}

pub fn three(value: &Value) -> [f64; 3] {
    let v: Vec<f64> = value.as_array().unwrap().iter().map(|x| x.as_f64().unwrap()).collect();
    [v[0], v[1], v[2]]
}

fn vec3([x, y, z]: [f64; 3]) -> Vec3 {
    Vec3::new(x, y, z)
}

/// The runner's frame at `origin` with its Z along `axis`, and an X it accepts across it.
fn frame(origin: &Value, axis: Option<&Value>) -> Value {
    let z = axis.map_or([0.0, 0.0, 1.0], three);
    let x = if z[0].abs() < 0.9 { [1.0, 0.0, 0.0] } else { [0.0, 1.0, 0.0] };
    json!({"origin": origin, "z": z, "x": x})
}

/// `v` turned by `angle` radians about the unit `axis`, right-handed (Rodrigues).
fn turned(v: Vec3, axis: Vec3, angle: f64) -> Vec3 {
    let (sin, cos) = sin_cos(angle);
    v * cos + axis.cross(v) * sin + axis * (axis.dot(v) * (1.0 - cos))
}

/// A rotation of the case format, as the frame the runner's `copy` places the world's at: the turn about the axis
/// through `axis_origin`, which takes the origin to `o - R o`.
fn rotation(op: &Value) -> Value {
    let (origin, axis) = (vec3(three(&op["axis_origin"])), vec3(three(&op["axis"])));
    let axis = axis * (1.0 / axis.norm());
    let angle = op["angle_degrees"].as_f64().unwrap() * PI / 180.0;
    let at = origin - turned(origin, axis, angle);
    let (z, x) = (turned(Vec3::new(0.0, 0.0, 1.0), axis, angle), turned(Vec3::new(1.0, 0.0, 0.0), axis, angle));
    json!({"origin": [at.x, at.y, at.z], "z": [z.x, z.y, z.z], "x": [x.x, x.y, x.z]})
}

/// The runner's step for a case's operation, which is one it builds, labelled with its id.
pub fn step(op: &Value) -> Value {
    let (id, origin) = (&op["id"], &op["origin"]);
    match op["op"].as_str().unwrap() {
        "box" => json!({"op": "box", "label": id, "size": op["size"], "frame": frame(origin, None)}),
        "sphere" => json!({"op": "sphere", "label": id, "radius": op["radius"], "frame": frame(origin, None)}),
        "cylinder" => json!({"op": "cylinder", "label": id, "radius": op["radius"], "height": op["height"],
            "frame": frame(origin, op.get("axis"))}),
        "cone" => json!({"op": "cone", "label": id, "radius1": op["radius1"], "radius2": op["radius2"],
            "height": op["height"], "frame": frame(origin, op.get("axis"))}),
        "torus" => json!({"op": "torus", "label": id, "major": op["major_radius"], "minor": op["minor_radius"],
            "frame": frame(origin, op.get("axis"))}),
        _ => json!({"op": "copy", "label": id, "target": op["target"], "frame": rotation(op)}),
    }
}
