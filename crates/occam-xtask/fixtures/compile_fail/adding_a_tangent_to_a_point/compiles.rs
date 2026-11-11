use occam_units::{Point, Tangent, Vector, World};

pub fn moved(p: Point<World>, t: Tangent<World>, step: f64) -> Point<World> {
    p + Vector::from_vec3(t.rate() * step)
}
