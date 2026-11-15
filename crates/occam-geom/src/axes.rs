//! Source: original
//! [`Axes`]: a placement's origin and its X, Y and Z axes in `World`, which the plane, the conics and the surfaces
//! turned about an axis are written in.

use occam_math::{Interval, Real};
use occam_report::{Code, Fault, NamedValue, Quantity, Report, Value};
use occam_units::{Affine, Direction, Local, Placement, Point, Similar, Vec3, Vector, World};

use crate::domain::{ParamRange, UvMap};
use crate::identity::Content;
use crate::restore::{Numbers, wrap_code};
use crate::support::{exact, finite, splat};

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

    /// Axes about `normal` through `origin`: `x` the world axis least along `normal` with its part along `normal` taken
    /// away, and `y` the normal across it, so that the three are right-handed. The same axes for the same numbers.
    pub(crate) fn about(origin: Vec3, normal: Direction<World>) -> Self {
        let n = normal.get();
        let world = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
        let least = world.into_iter().fold((Vec3::new(1.0, 0.0, 0.0), f64::INFINITY), |best, axis| {
            let along = axis.dot(n).abs();
            if along < best.1 { (axis, along) } else { best }
        });
        let across = least.0 - n * least.0.dot(n);
        let x = across * (1.0 / across.norm());
        Self { origin, x, y: n.cross(x), normal }
    }

    /// These axes moved by the similarity `similar`: the origin placed after the scale, and the axes turned, then made
    /// square again, so that axes moved over and over keep within what [`Axes::restore`] reads back.
    pub(crate) fn moved(&self, similar: &Similar) -> Self {
        let Similar { placement, scale } = *similar;
        let origin = placement.point(Point::from_vec3(self.origin * scale)).to_vec3();
        let turned = |v: Vec3| placement.vector(Vector::from_vec3(v)).to_vec3();
        let normal = Direction::<World>::normalised(turned(self.normal.get()));
        let n = normal.get();
        let x = turned(self.x);
        let x = Direction::<World>::normalised(x - n * x.dot(n)).get();
        Self { origin, x, y: n.cross(x), normal }
    }

    /// The axes of the plane through these that `map` makes, and how the plane's parameters move, which are lengths
    /// along the axes: the origin goes where `map` takes it, `x` along where it takes `x`, and the normal across where it
    /// takes `x` and `y`. A point `u` along `x` and `v` along `y` goes to one `|M x| u + (M y . x') v` along the new `x`
    /// and `(M y . y') v` along the new `y`, where `M` is the map's linear part.
    pub(crate) fn stretched(&self, map: &Affine) -> (Self, UvMap) {
        let origin = map.point(Point::from_vec3(self.origin)).to_vec3();
        let (mx, my) = (map.vector(self.x), map.vector(self.y));
        let normal = Direction::<World>::normalised(mx.cross(my));
        let n = normal.get();
        let x = Direction::<World>::normalised(mx).get();
        let y = n.cross(x);
        (Self { origin, x, y, normal }, UvMap { u: [mx.norm(), my.dot(x), 0.0], v: [0.0, my.dot(y), 0.0] })
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

    /// `p`'s coordinates along the three axes, from the origin.
    pub(crate) fn local(&self, p: Point<World>) -> Vec3 {
        let d = p.to_vec3() - self.origin;
        Vec3::new(d.dot(self.x), d.dot(self.y), d.dot(self.normal.get()))
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

    /// Adds what defines a curve placed on these axes: the axes, its `sizes`, its range's ends, and how the range wraps,
    /// since an arc trimmed to a whole turn stops at its seam where an untrimmed one carries on round.
    pub(crate) fn push_curve(&self, out: &mut Content, sizes: &[f64], range: ParamRange) {
        self.push(out);
        for x in sizes.iter().copied().chain([range.lo, range.hi, wrap_code(range.wrap)]) {
            out.push(x);
        }
    }

    /// Axes read back as [`Axes::push`] wrote them. The two axes and the normal must each be a unit axis, square to
    /// each other within 64 machine epsilons and right-handed, as a placement's are, and the origin finite.
    ///
    /// # Errors
    ///
    /// `geom.wrong-count` if too few numbers are left, `units.not-finite` for an origin that is not finite, and
    /// `geom.degenerate-direction` for axes that are not as a placement makes them.
    pub(crate) fn restore(numbers: &mut Numbers<'_>) -> Result<Self, Report> {
        let origin = numbers.vec3()?;
        finite(Point::from_vec3(origin))?;
        let (x, y) = (numbers.vec3()?, numbers.vec3()?);
        let normal = Direction::<World>::kept(numbers.vec3()?)?;
        Direction::<World>::kept(x)?;
        Direction::<World>::kept(y)?;
        let n = normal.get();
        let worst =
            [x.dot(y), x.dot(n), y.dot(n)].into_iter().fold(0.0, |a: f64, d| if d.abs() > a { d.abs() } else { a });
        if worst <= 64.0 * f64::EPSILON && x.cross(y).dot(n) > 0.0 {
            Ok(Self { origin, x, y, normal })
        } else {
            Err(Report::new(Code::DegenerateDirection, Fault::InvalidInput).with_value(NamedValue::new(
                "square",
                Quantity::Ratio,
                worst,
            )))
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

#[cfg(test)]
mod tests {
    use occam_units::Angle;

    use super::*;

    #[test]
    fn axes_moved_over_and_over_stay_square_enough_to_be_read_back() {
        // A turn about a skew axis and a scale, eighty times: rounding left square to the axes would double each time.
        let turn = Placement::rotation(Direction::normalised(Vec3::new(1.0, -2.0, 0.5)), Angle::from_radians(0.3));
        let similar = Similar { placement: turn, scale: 1.1 };
        let mut axes = Axes::about(Vec3::new(1.0, 2.0, 3.0), Direction::normalised(Vec3::new(0.3, 0.4, 0.9)));
        for _ in 0..80 {
            axes = axes.moved(&similar);
        }
        let n = axes.normal.get();
        let square = [axes.x.dot(n), axes.y.dot(n), axes.x.dot(axes.y)].map(f64::abs);
        assert!(square.iter().all(|d| *d <= 4.0 * f64::EPSILON), "{square:?}");
        let mut content = Content::default();
        axes.push(&mut content);
        let numbers: Vec<f64> = content.words().iter().map(|w| f64::from_bits(*w)).collect();
        assert_eq!(Axes::restore(&mut Numbers::new(&numbers)).map(|a| a == axes), Ok(true));
    }

    #[test]
    fn axes_about_a_normal_start_from_the_first_world_axis_least_along_it() {
        // Along X, the Y and Z axes are both square to it: the first, Y, is taken.
        let x_axis = Axes::about(Vec3::new(1.0, 2.0, 3.0), Direction::normalised(Vec3::new(1.0, 0.0, 0.0)));
        assert_eq!((x_axis.x, x_axis.y), (Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)));
        // Along the diagonal, every axis is as far along it: X, the first, with its part along the normal taken away.
        let n = Vec3::new(1.0, 1.0, 1.0) * (1.0 / 3f64.sqrt());
        let diagonal = Axes::about(Vec3::new(0.0, 0.0, 0.0), Direction::normalised(n));
        let want = Vec3::new(2.0, -1.0, -1.0) * (1.0 / 6f64.sqrt());
        assert!((diagonal.x - want).norm() < 1e-15 && diagonal.x.dot(n).abs() < 1e-15, "{:?}", diagonal.x);
        assert!((diagonal.y - n.cross(want)).norm() < 1e-15);
    }

    #[test]
    fn unit_axes_more_than_64_machine_epsilons_from_square_are_refused_naming_the_worst() {
        // The normal leans off the second axis twice as far as off the first, the other way round: that lean is named.
        let leaning = |lean: f64| {
            let normal = Vec3::new(lean / 2.0, -lean, 1.0);
            let axes = [Vec3::new(0.0, 0.0, 0.0), Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), normal];
            let numbers: Vec<f64> = axes.iter().flat_map(|v| [v.x, v.y, v.z]).collect();
            Axes::restore(&mut Numbers::new(&numbers))
        };
        for lean in [128.0 * f64::EPSILON, 1e-9] {
            let square = NamedValue::new("square", Quantity::Ratio, lean);
            let refused = Report::new(Code::DegenerateDirection, Fault::InvalidInput).with_value(square);
            assert_eq!(leaning(lean), Err(refused), "{lean}");
        }
        assert!(leaning(32.0 * f64::EPSILON).is_ok(), "within 64 machine epsilons");
    }
}
