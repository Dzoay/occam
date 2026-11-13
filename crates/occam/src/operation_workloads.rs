//! Source: original
//! The construction operations' requests that `gates perf` times, in instruction counts natively and in milliseconds in
//! the browser, so that both measure the same work. Each is the plan's tests for its step, as one request.

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
