//! No panics: errors are the agent API (L1).

pub fn probe() -> u8 {
    panic!("x") // fires: clippy::panic
}
