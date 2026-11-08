//! The library never prints: diagnostics travel as values.

pub fn probe() {
    eprintln!("x"); // fires: clippy::print_stderr
}
