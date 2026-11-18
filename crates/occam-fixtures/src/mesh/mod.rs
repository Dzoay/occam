//! Source: original
//! The mesher's fixtures: sheets of faces to mesh, and what `gates determinism` hashes of their meshes.

#[expect(
    clippy::unwrap_used,
    clippy::indexing_slicing,
    clippy::missing_panics_doc,
    reason = "test models, written as the tests they came from are: one that cannot be built stops the test"
)]
pub mod layouts;
pub mod probe;
