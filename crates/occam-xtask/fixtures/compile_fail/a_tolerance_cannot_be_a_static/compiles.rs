use occam_report::Report;
use occam_units::{Angle, Length, Tolerance};

pub fn tolerance() -> Result<Tolerance, Report> {
    Tolerance::new(Length::from_millimetres(1e-7), Angle::from_radians(1e-9))
}
