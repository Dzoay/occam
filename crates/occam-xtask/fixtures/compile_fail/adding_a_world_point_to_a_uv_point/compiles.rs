use occam_units::{Point, Vector, World};

pub fn sum(a: Point<World>, b: Vector<World>) -> Point<World> {
    a + b
}
