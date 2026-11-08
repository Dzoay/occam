//! Integer arithmetic can overflow (L19).

pub fn probe(a: u8, b: u8) -> u8 {
    a + b // fires: clippy::arithmetic_side_effects
}
