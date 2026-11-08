//! No panics: errors are the agent API (L1).

pub fn probe(x: Option<u8>) -> u8 {
    x.unwrap() // fires: clippy::unwrap_used
}
