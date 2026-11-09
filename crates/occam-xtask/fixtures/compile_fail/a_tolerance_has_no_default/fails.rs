// expect: E0599
use occam_units::Tolerance;

pub fn tolerance() -> Tolerance {
    Tolerance::default()
}
