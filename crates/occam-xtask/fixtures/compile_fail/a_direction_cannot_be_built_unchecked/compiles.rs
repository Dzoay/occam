use occam_report::Report;
use occam_units::{Direction, Tolerance, Vec3, Vector, World};

pub fn checked(tolerance: Tolerance) -> Result<Direction<World>, Report> {
    Direction::along(Vector::from_vec3(Vec3::new(3.0, 0.0, 0.0)), tolerance)
}
