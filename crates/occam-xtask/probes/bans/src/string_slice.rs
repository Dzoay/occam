//! Slicing a string can panic on a character boundary (L19).

pub fn probe(x: &str) -> &str {
    &x[1..] // fires: clippy::string_slice
}
