//! Source: original
//! The topology's fixtures: clean models built through `Edit` as any caller would, what tests build beside them, and
//! the plates `gates perf` counts. occam-topo's unit tests compile `models` and `testing` from this source, so those
//! two name occam-topo by its name.

#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::missing_panics_doc,
    reason = "test models, written as the tests they came from are: one that cannot be built stops the test"
)]
pub mod models;
#[expect(
    clippy::unwrap_used,
    clippy::missing_panics_doc,
    reason = "test helpers, written as the tests they came from are: an edit that refuses one stops the test"
)]
pub mod testing;
pub mod workloads;
