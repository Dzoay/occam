use occam_units::{Local, Point, Vector};

pub fn sum(a: Point<Local>, b: Vector<Local>) -> Point<Local> {
    a + b
}
