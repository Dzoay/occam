//! The library never prints: diagnostics travel as values.

pub fn probe() {
    println!("x"); // fires: clippy::print_stdout
}
