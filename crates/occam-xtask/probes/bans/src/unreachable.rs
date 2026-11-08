//! No panics: errors are the agent API (L1).

pub fn probe() -> u8 {
    unreachable!() // fires: clippy::unreachable
}
