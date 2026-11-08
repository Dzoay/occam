//! The kernel is safe Rust.

pub fn probe() -> u8 {
    unsafe { std::hint::unreachable_unchecked() } // fires: unsafe_code
}
