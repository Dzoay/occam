//! Content, never addresses: pointer identity says "different" where content is equal, and an address differs from run
//! to run (P8, L23).

use std::rc::Rc;
use std::sync::Arc;

pub fn probe(a: &Arc<u8>, b: &Rc<u8>, c: &u8) -> [bool; 3] {
    [
        Arc::ptr_eq(a, a), // fires: clippy::disallowed_methods std::sync::Arc::ptr_eq
        Rc::ptr_eq(b, b), // fires: clippy::disallowed_methods std::rc::Rc::ptr_eq
        std::ptr::eq(c, c), // fires: clippy::disallowed_methods std::ptr::eq
    ]
}
