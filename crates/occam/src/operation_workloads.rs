//! Source: original
//! The construction operations' requests that `gates perf` times, in instruction counts natively and in milliseconds in
//! the browser, so that both measure the same work. Each is the plan's tests for its step, as one request, but for
//! [`polygon_extruded`], counted natively alone, whose size is the point.

use core::f64::consts::TAU;

use occam_math::sin_cos;

/// A request making each primitive once, in millimetres.
pub const PRIMITIVES: &str = r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
    {"op": "box", "size": [10, 20, 30]},
    {"op": "wedge", "size": [10, 5, 4], "top": {"start": 2, "width": 3}},
    {"op": "cylinder", "radius": 5, "height": 10},
    {"op": "cone", "radius1": 5, "radius2": 2, "height": 10},
    {"op": "sphere", "radius": 5},
    {"op": "torus", "major": 10, "minor": 2}
]}"#;

/// The plan's tests for sweeps, as one request: the plate extruded, and a rectangle turned a whole turn off its axis.
pub const SWEEPS: &str = r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
    {"op": "wire", "label": "outline", "points": [[0, 0, 0], [40, 0, 0], [40, 20, 0], [0, 20, 0]], "closed": true},
    {"op": "wire", "label": "hole", "edges": [{"circle": {"centre": [20, 10, 0], "radius": 3, "normal": [0, 0, 1]}}]},
    {"op": "face", "label": "plate", "outer": "outline", "holes": ["hole"]},
    {"op": "extrude", "face": "plate", "vector": [0, 0, 5]},
    {"op": "wire", "label": "side", "points": [[50, 0, 0], [60, 0, 0], [60, 0, 5], [50, 0, 5]], "closed": true},
    {"op": "face", "label": "rect", "outer": "side"},
    {"op": "revolve", "profile": "rect", "origin": [0, 0, 0], "axis": [0, 0, 1], "angle": 360}
]}"#;

/// The plan's tests for transforms, as one request: a box copied and moved, a cylinder scaled alike, and one stretched
/// along a tilted frame, which makes its side exact B-splines.
pub const TRANSFORMS: &str = r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
    {"op": "box", "label": "box", "size": [10, 20, 30]},
    {"op": "copy", "target": "box", "frame": {"origin": [100, 0, 0]}},
    {"op": "cylinder", "label": "round", "radius": 5, "height": 10},
    {"op": "transform", "target": "round", "scale": 2.5, "about": {"origin": [1, 2, 3]}},
    {"op": "copy", "target": "round", "scale": [2, 1, 0.5], "about": {"origin": [1, -2, 3], "z": [1, 2, 2], "x": [2, -1, 0]}}
]}"#;

/// A request measuring everything each primitive has, by its body's id.
pub const MEASURES: &str = r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
    {"op": "measure", "target": "body:0/1"},
    {"op": "measure", "target": "body:1/1"},
    {"op": "measure", "target": "body:2/1"},
    {"op": "measure", "target": "body:3/1"},
    {"op": "measure", "target": "body:4/1"},
    {"op": "measure", "target": "body:5/1"}
]}"#;

/// How many corners the polygon `gates perf` counts has: enough that a cost growing with the square of its edges would
/// stand far above the cost of the edges themselves.
pub const POLYGON_CORNERS: u32 = 1000;

/// A request making a closed polygon of `corners` straight edges round the circle of radius 10 about the origin, a face
/// of it, and that face extruded 1 up, each step committed: work that should grow with the edges, and once grew with
/// their square.
#[must_use]
pub fn polygon_extruded(corners: u32) -> String {
    let count = f64::from(corners);
    let corner = |k: u32| {
        let (sin, cos) = sin_cos(TAU * f64::from(k) / count);
        format!("[{}, {}, 0]", 10.0 * cos, 10.0 * sin)
    };
    let points: Vec<String> = (0..corners).map(corner).collect();
    format!(
        r#"{{"schema": 1, "units": {{"length": "mm", "angle": "deg"}}, "steps": [
    {{"op": "wire", "label": "outline", "points": [{}], "closed": true}},
    {{"op": "face", "label": "polygon", "outer": "outline"}},
    {{"op": "extrude", "face": "polygon", "vector": [0, 0, 1]}}
]}}"#,
        points.join(", ")
    )
}

#[cfg(test)]
mod tests {
    use occam_topo::{Body, BodyKind, Edge, Face, Vertex};
    use occam_units::Vec3;

    use super::*;

    #[test]
    fn the_polygon_extruded_is_a_solid_prism_with_a_side_for_each_corner_spaced_evenly_round_the_circle() {
        let ran = crate::run(None, &polygon_extruded(12));
        assert!(ran.failed.is_none(), "{:?}", ran.failed);
        assert_eq!(ran.done.len(), 3);
        let model = ran.model.unwrap();
        let bodies = model.ids::<Body>();
        assert_eq!(bodies.len(), 1);
        assert_eq!(model.view(bodies[0]).unwrap().kind(), BodyKind::Solid);
        assert_eq!((model.ids::<Face>().len(), model.ids::<Edge>().len()), (14, 36));
        let points: Vec<Vec3> =
            model.ids::<Vertex>().into_iter().map(|v| model.view(v).unwrap().point().to_vec3()).collect();
        for k in 0..12 {
            let (sin, cos) = sin_cos(TAU * f64::from(k) / 12.0);
            for z in [0.0, 1.0] {
                let corner = Vec3::new(10.0 * cos, 10.0 * sin, z);
                assert!(points.iter().any(|p| (*p - corner).norm() < 1e-9), "corner {k} at height {z}: {points:?}");
            }
        }
    }
}
