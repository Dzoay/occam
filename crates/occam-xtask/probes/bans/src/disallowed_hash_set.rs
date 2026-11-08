//! Hashed collections iterate in an unspecified order, which breaks determinism (L23).

pub struct Probe(pub std::collections::HashSet<u8>); // fires: clippy::disallowed_types std::collections::HashSet
