//! Source: original
//! [`Axes`]: a placement's origin and its X, Y and Z axes in `World`, which the plane and the conics are written in.

use occam_math::{Interval, Real};
use occam_report::{NamedValue, Quantity, Value};
use occam_units::{Direction, Local, Placement, Point, Vec3, World};

use crate::domain::ParamRange;
use crate::identity::Content;
use crate::support::{exact, splat};

/// An origin and two perpendicular unit axes in `World`, with the normal they face.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Axes {
    /// The origin.
    pub(crate) origin: Vec3,
    /// The first axis.
    pub(crate) x: Vec3,
    /// The second axis.
    pub(crate) y: Vec3,
    /// The third, which the plane of the first two faces.
    pub(crate) normal: Direction<World>,
}

impl Axes {
    /// The origin and axes of `placement`, in `World`.
    pub(crate) fn of(placement: &Placement<Local, World>) -> Self {
        let axis = |x, y, z| placement.direction(Direction::normalised(Vec3::new(x, y, z)));
        Self {
            origin: placement.point(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0))).to_vec3(),
            x: axis(1.0, 0.0, 0.0).get(),
            y: axis(0.0, 1.0, 0.0).get(),
            normal: axis(0.0, 0.0, 1.0),
        }
    }

    /// `a x + b y`.
    #[inline(always)]
    pub(crate) fn along<R: Real>(&self, a: R, b: R) -> Vec3<R> {
        splat(self.x) * a + splat(self.y) * b
    }

    /// The point `origin + a x + b y`.
    #[inline(always)]
    pub(crate) fn at<R: Real>(&self, a: R, b: R) -> Point<World, R> {
        Point::from_vec3(splat(self.origin) + self.along(a, b))
    }

    /// `p`'s coordinates along the first two axes, from the origin.
    pub(crate) fn coordinates(&self, p: Point<World>) -> (f64, f64) {
        let d = p.to_vec3() - self.origin;
        (d.dot(self.x), d.dot(self.y))
    }

    /// Intervals holding the exact coordinates of `origin + a x + b y` for every `a` and `b` in theirs.
    pub(crate) fn reach(&self, first: Interval, second: Interval) -> [Interval; 3] {
        let (at, along_x, along_y) = (self.origin, self.x, self.y);
        let coordinate = |o: f64, x: f64, y: f64| exact(o) + exact(x) * first + exact(y) * second;
        [
            coordinate(at.x, along_x.x, along_y.x),
            coordinate(at.y, along_x.y, along_y.y),
            coordinate(at.z, along_x.z, along_y.z),
        ]
    }

    /// Adds the numbers that define them.
    pub(crate) fn push(&self, out: &mut Content) {
        for v in [self.origin, self.x, self.y, self.normal.get()] {
            out.push_vec3(v);
        }
    }

    /// Adds what defines a curve placed on these axes: the axes, its `sizes`, and its range's ends.
    pub(crate) fn push_curve(&self, out: &mut Content, sizes: &[f64], range: ParamRange) {
        self.push(out);
        for x in sizes.iter().copied().chain([range.lo, range.hi]) {
            out.push(x);
        }
    }

    /// What a description shows of a curve placed on these axes: [`Axes::values`], its `sizes`, and its range's ends
    /// as `from` and `to`.
    pub(crate) fn curve_values(&self, name: &str, sizes: &[NamedValue], range: ParamRange) -> Vec<NamedValue> {
        let mut values = self.values(name);
        values.extend_from_slice(sizes);
        let quantity = range.quantity();
        values.extend([NamedValue::new("from", quantity, range.lo), NamedValue::new("to", quantity, range.hi)]);
        values
    }

    /// The origin under `name`, the normal and the first axis, as a description shows them.
    pub(crate) fn values(&self, name: &str) -> Vec<NamedValue> {
        let o = self.origin;
        vec![
            NamedValue::point(name, [o.x, o.y, o.z]),
            direction("normal", self.normal.get()),
            direction("x-axis", self.x),
        ]
    }
}

/// A direction's three coordinates in a named value: they are ratios.
pub(crate) fn direction(name: &str, d: Vec3) -> NamedValue {
    NamedValue { name: name.to_string(), quantity: Quantity::Ratio, value: Value::Point([d.x, d.y, d.z]) }
}
