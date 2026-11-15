//! Source: original
//! Points that are closed and come back to the first close on it: the last is that point again, not another edge,
//! unless only two are given.

use occam_report::{Code, Input};
use occam_topo::Edge;
use serde_json::json;

use super::*;
use crate::construct::tests::built;

#[test]
fn points_that_come_back_to_the_first_close_on_it_without_another_edge() {
    let (model, _) =
        built(json!([{"op": "wire", "points": [[0, 0, 0], [10, 0, 0], [10, 10, 0], [0, 0, 0]], "closed": true}]));
    assert_eq!(model.ids::<Edge>().len(), 3);
}

#[test]
fn two_points_closed_are_two_points_even_where_the_second_comes_back() {
    // A request closes no fewer than three points, but two that are closed give an edge from the first to the second,
    // of no length, refused at the second, and not the first point alone.
    let points = vec![(Vec3::new(0.0, 0.0, 0.0), Input::at("first")), (Vec3::new(0.0, 0.0, 1e-8), Input::at("second"))];
    let report = made(&WireArgs::Points { points, closed: true, at: Input::at("wire") }, 1e-7).map(|_| ()).unwrap_err();
    assert_eq!((report.code, report.input.clone()), (Code::NoLength, Some(Input::at("second"))));
}
