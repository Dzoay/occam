// expect: E0015
use occam_report::Report;
use occam_units::{Angle, Length, Tolerance};

static TOLERANCE: Result<Tolerance, Report> =
    Tolerance::new(Length::from_millimetres(1e-7), Angle::from_radians(1e-9));

pub fn tolerance() -> &'static Result<Tolerance, Report> {
    &TOLERANCE
}
