use occam_units::{Point, Vector, World};

pub fn difference(a: Point<World>, b: Point<World>) -> Vector<World> {
    a - b
}
