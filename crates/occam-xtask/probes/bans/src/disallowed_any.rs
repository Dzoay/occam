//! No `Any` or downcasting across seams (L26).

pub fn probe(_: &dyn std::any::Any) {} // fires: clippy::disallowed_types std::any::Any
