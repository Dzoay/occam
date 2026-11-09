// expect: E0308
use occam_units::{Point, World};

pub fn sum(a: Point<World>, b: Point<World>) -> Point<World> {
    a + b
}
