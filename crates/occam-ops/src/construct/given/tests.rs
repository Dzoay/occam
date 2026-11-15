//! Source: original
//! A radius is a length above nought: one of nought is refused as the request is read.

use occam_report::Code;
use serde_json::json;

use crate::construct::tests::{number, refused};

#[test]
fn a_circle_of_no_radius_is_refused_as_no_size() {
    let circle = json!({"circle": {"centre": [0, 0, 0], "radius": 0, "normal": [0, 0, 1]}});
    let report = refused(json!([{"op": "wire", "edges": [circle]}]));
    let at = report.input.as_ref().map(|input| input.parameter.as_str());
    assert_eq!(
        (report.code, at, number(&report, "size")),
        (Code::SizeNotPositive, Some("steps[0].edges[0].circle.radius"), 0.0)
    );
}
