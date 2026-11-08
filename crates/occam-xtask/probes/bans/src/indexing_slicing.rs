//! Indexing can panic (L19).

pub fn probe(x: &[u8]) -> u8 {
    x[0] // fires: clippy::indexing_slicing
}
