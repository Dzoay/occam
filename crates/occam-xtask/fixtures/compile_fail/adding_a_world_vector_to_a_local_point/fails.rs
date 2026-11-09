// expect: E0308
use occam_units::{Local, Point, Vector, World};

pub fn sum(a: Point<Local>, b: Vector<World>) -> Point<Local> {
    a + b
}
