//! Deliberate violations, to prove CI goes red for the right reason. Never merged.

pub fn untested (x: Option<u8>)  -> u8 {
    x.unwrap()
}
