// expect: E0308
use occam_units::{Point, UvPoint, World};

pub fn sum(a: Point<World>, b: UvPoint) -> Point<World> {
    a + b
}
