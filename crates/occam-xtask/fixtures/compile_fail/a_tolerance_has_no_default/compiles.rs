use occam_report::Report;
use occam_units::{Angle, Length, Tolerance};

pub fn tolerance(length: Length, angle: Angle) -> Result<Tolerance, Report> {
    Tolerance::new(length, angle)
}
