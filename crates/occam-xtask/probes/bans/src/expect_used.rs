//! No panics: errors are the agent API (L1).

pub fn probe(x: Option<u8>) -> u8 {
    x.expect("x") // fires: clippy::expect_used
}
