// expect: E0308
use occam_units::{Angle, Length};

pub fn sum(a: Length, b: Angle) -> Length {
    a + b
}
