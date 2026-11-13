// expect: E0451
use std::marker::PhantomData;

use occam_units::{Direction, Vec3, World};

pub fn unchecked() -> Direction<World> {
    Direction { unit: Vec3::new(3.0, 0.0, 0.0), frame: PhantomData }
}
