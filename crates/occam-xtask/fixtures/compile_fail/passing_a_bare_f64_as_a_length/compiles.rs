use occam_units::Length;

fn extrude(_distance: Length) {}

pub fn call() {
    extrude(Length::from_millimetres(10.0));
}
