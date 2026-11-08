//! Hashed collections iterate in an unspecified order, which breaks determinism (L23).

pub struct Probe(pub std::collections::HashMap<u8, u8>); // fires: clippy::disallowed_types std::collections::HashMap
