// expect: E0308
use occam_units::Length;

fn extrude(_distance: Length) {}

pub fn call() {
    extrude(10.0);
}
