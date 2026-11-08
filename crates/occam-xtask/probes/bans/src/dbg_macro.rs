//! No debugging output left behind.

pub fn probe(x: u8) -> u8 {
    dbg!(x) // fires: clippy::dbg_macro
}
