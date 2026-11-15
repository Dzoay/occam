//! Source: original
//! Edges meet where one begins no further than the band from where the one before it ends: exactly the band apart, they
//! meet.

use occam_report::Code;
use occam_topo::Edge;
use serde_json::json;

use crate::construct::tests::{build, request};

#[test]
fn edges_exactly_the_band_apart_meet() {
    let wire = |gap: f64| {
        let mut text = request(
            json!([{"op": "wire", "edges": [{"line": [[0, 0, 0], [10, 0, 0]]}, {"line": [[10, gap, 0], [10, 10, 0]]}]}]),
        );
        text["tolerance"] = json!({"length": 0.5, "angle": 1e-6});
        build(&text).map(|(model, _)| model.ids::<Edge>().len()).map_err(|reports| reports.first().map(|r| r.code))
    };
    assert_eq!((wire(0.5), wire(0.75)), (Ok(2), Err(Some(Code::WireGap))));
}
