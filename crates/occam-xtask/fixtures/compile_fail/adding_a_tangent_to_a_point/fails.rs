// expect: E0308
use occam_units::{Point, Tangent, World};

pub fn moved(p: Point<World>, t: Tangent<World>) -> Point<World> {
    p + t
}
