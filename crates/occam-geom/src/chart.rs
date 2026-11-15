//! Source: original
//! [`Chart`]: a surface's own parameters in closed form, which the images of curves on it are read through. The seam
//! presents a plane, a sphere, a cylinder, a cone or a torus without saying how the surface's parameters run over it
//! (S4: what a surface can present, it is asked for), so the chart is read from the surface's point and first
//! derivatives at one place in its domain, then held to its points at four others. A surface whose parameters run
//! otherwise, as a plane's B-spline with poles spaced unevenly does, has no chart.

use occam_math::{atan2, sin_cos};
use occam_units::{Direction, Point, UvPoint, Vec3, World};

use crate::analytic::Analytic;
use crate::domain::ParamRange;
use crate::identity::Content;
use crate::surface::Surface;

/// How a turned surface's second parameter runs along its meridian.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Meridian {
    /// A cylinder's: `v` the height along the axis from the origin, at `radius` from it.
    Upright { radius: f64 },
    /// A cone's: with the origin at the apex, `v = apex + s ρ sin α + z cos α`, where `s` is −1 on the nappe below
    /// the apex.
    Slanted { apex: f64, slope: (f64, f64) },
    /// A sphere's: `v` the latitude, at `radius` from the centre.
    Centred { radius: f64 },
    /// A torus's: `v` the angle about the circle of the tube's centres, `major` from the axis, of the tube's `minor`
    /// radius.
    Tube { major: f64, minor: f64 },
}

/// The chart's closed form.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Form {
    /// `p = origin + (u - at.0) du + (v - at.1) dv`, read back by the dual axes: `u = at.0 + (p - origin) · dual.0`.
    Flat { origin: Vec3, at: (f64, f64), du: Vec3, dv: Vec3, dual: (Vec3, Vec3) },
    /// Turned about the axis through `origin` along `z`: `u` the angle about it from `x` towards `y`, which with `z` need
    /// not be right-handed, and `v` as the meridian runs.
    Turned { origin: Vec3, x: Vec3, y: Vec3, z: Vec3, meridian: Meridian },
}

/// A surface's parameters as a closed form of its point, recovered from its evaluation.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Chart(pub(crate) Form);

fn unit(v: Vec3) -> Vec3 {
    v * (1.0 / v.norm())
}

/// `v` less its part along the unit `along`.
fn across(v: Vec3, along: Vec3) -> Vec3 {
    v - along * v.dot(along)
}

/// A parameter's middle where its range is finite, or 0, and two others to hold the chart to.
fn spread(range: ParamRange) -> [f64; 3] {
    if range.lo.is_finite() && range.hi.is_finite() {
        let at = |f: f64| range.lo + (range.hi - range.lo) * f;
        [at(0.5), at(0.2), at(0.7)]
    } else {
        [0.0, -1.3, 1.1]
    }
}

/// A turned chart about the axis through `origin` along `up`, from the outward unit `out` and the derivative `du` at
/// the angle `u`: `x` and `y` are `out` and `du`'s direction turned back by `u`.
fn turned(origin: Vec3, out: Vec3, du: Vec3, up: Vec3, u: f64, meridian: Meridian) -> Form {
    let ahead = unit(across(across(du, up), out));
    let (sin, cos) = sin_cos(u);
    Form::Turned { origin, x: out * cos - ahead * sin, y: out * sin + ahead * cos, z: up, meridian }
}

/// The chart read from `surface`'s point and first derivatives at `at`.
fn read(surface: &dyn Surface, form: Analytic, at: UvPoint) -> Form {
    let jet = surface.first(at);
    let (point, du, dv) = (jet.point.to_vec3(), jet.du.rate(), jet.dv.rate());
    match form {
        Analytic::Plane { .. } => {
            let normal = du.cross(dv);
            let square = normal.dot(normal);
            let dual = (dv.cross(normal) * (1.0 / square), normal.cross(du) * (1.0 / square));
            Form::Flat { origin: point, at: (at.u(), at.v()), du, dv, dual }
        }
        Analytic::Cylinder { on_axis, axis, radius } => {
            let (up, offset) = (axis.get(), point - on_axis.to_vec3());
            let origin = on_axis.to_vec3() + up * (offset.dot(up) - at.v());
            let meridian = Meridian::Upright { radius: radius.millimetres() };
            turned(origin, unit(across(offset, up)), du, up, at.u(), meridian)
        }
        Analytic::Cone { apex, axis, half_angle } => {
            let (up, offset) = (axis.get(), point - apex.to_vec3());
            let side = if offset.dot(up) < 0.0 { -1.0 } else { 1.0 };
            let slope = sin_cos(half_angle.radians());
            let from_apex = side * across(offset, up).norm() * slope.0 + offset.dot(up) * slope.1;
            let meridian = Meridian::Slanted { apex: at.v() - from_apex, slope };
            turned(apex.to_vec3(), unit(across(offset, up)) * side, du * side, up, at.u(), meridian)
        }
        Analytic::Sphere { centre, radius } => {
            let size = radius.millimetres();
            let (out, rising) = ((point - centre.to_vec3()) * (1.0 / size), dv * (1.0 / size));
            let (sin, cos) = sin_cos(at.v());
            let up = unit(out * sin + rising * cos);
            turned(centre.to_vec3(), unit(across(out, up)), du, up, at.u(), Meridian::Centred { radius: size })
        }
        Analytic::Torus { centre, axis, major, minor } => {
            let (up, offset) = (axis.get(), point - centre.to_vec3());
            let meridian = Meridian::Tube { major: major.millimetres(), minor: minor.millimetres() };
            turned(centre.to_vec3(), unit(across(offset, up)), du, up, at.u(), meridian)
        }
    }
}

impl Chart {
    /// The chart of `surface`, which presents `form`, or `None` where its parameters do not run as the chart's: its
    /// points at four places of its domain are held to the chart's within a part in 10^11 of the size of its point and
    /// first derivatives where the chart is read, at the middle of its domain.
    #[must_use]
    pub fn of(surface: &dyn Surface, form: Analytic) -> Option<Self> {
        let domain = surface.domain();
        let (us, mut vs) = (spread(domain.u), spread(domain.v));
        // A point on a line that collapses, a cone's apex or a sphere's pole, has no first derivatives to read.
        if surface.degeneracies().iter().any(|line| (line.at - vs[0]).abs() < 0.25) {
            vs = vs.map(|v| v + 0.5);
        }
        let at = UvPoint::new(us[0], vs[0]);
        let chart = Self(read(surface, form, at));
        // A chart read where the surface has no derivatives is NaN, which no probe is within any distance of.
        let probes = [(us[1], vs[1]), (us[1], vs[2]), (us[2], vs[1]), (us[2], vs[2])];
        let jet = surface.first(at);
        let size = jet.point.to_vec3().norm() + jet.du.rate().norm() + jet.dv.rate().norm();
        let fits = probes.iter().all(|&(u, v)| {
            let uv = UvPoint::new(u, v);
            (chart.point(uv).to_vec3() - surface.point(uv).to_vec3()).norm() <= 1e-11 * size
        });
        fits.then_some(chart)
    }

    /// The surface's point at `uv`, by the chart.
    #[must_use]
    pub fn point(&self, uv: UvPoint) -> Point<World> {
        Point::from_vec3(match self.0 {
            Form::Flat { origin, at, du, dv, .. } => origin + du * (uv.u() - at.0) + dv * (uv.v() - at.1),
            Form::Turned { origin, x, y, z, meridian } => {
                let (sin, cos) = sin_cos(uv.u());
                let out = x * cos + y * sin;
                origin
                    + match meridian {
                        Meridian::Upright { radius } => out * radius + z * uv.v(),
                        Meridian::Slanted { apex, slope } => (out * slope.0 + z * slope.1) * (uv.v() - apex),
                        Meridian::Centred { radius } => {
                            let (sin, cos) = sin_cos(uv.v());
                            (out * cos + z * sin) * radius
                        }
                        Meridian::Tube { major, minor } => {
                            let (sin, cos) = sin_cos(uv.v());
                            out * (major + minor * cos) + z * (minor * sin)
                        }
                    }
            }
        })
    }

    /// The parameters of `point`, a point of the surface, by the chart: where a parameter is undefined, at a pole or an
    /// apex, any value serves: an angle there is the angle of two noughts, 0 or a half turn by their signs.
    #[must_use]
    pub fn parameters(&self, point: Point<World>) -> UvPoint {
        match self.0 {
            Form::Flat { origin, at, dual, .. } => {
                let offset = point.to_vec3() - origin;
                UvPoint::new(at.0 + offset.dot(dual.0), at.1 + offset.dot(dual.1))
            }
            Form::Turned { origin, x, y, z, meridian } => {
                let offset = point.to_vec3() - origin;
                let (east, north, height) = (offset.dot(x), offset.dot(y), offset.dot(z));
                let rho = (east * east + north * north).sqrt();
                match meridian {
                    Meridian::Upright { .. } => UvPoint::new(atan2(north, east), height),
                    Meridian::Slanted { apex, slope } => {
                        let side = if height < 0.0 { -1.0 } else { 1.0 };
                        let v = apex + side * rho * slope.0 + height * slope.1;
                        UvPoint::new(atan2(side * north, side * east), v)
                    }
                    Meridian::Centred { .. } => UvPoint::new(atan2(north, east), atan2(height, rho)),
                    Meridian::Tube { major, .. } => UvPoint::new(atan2(north, east), atan2(height, rho - major)),
                }
            }
        }
    }

    /// The points where the surface's lines of constant `v` collapse and its parameters jump along a curve through
    /// them: a sphere's poles.
    #[must_use]
    pub fn poles(&self) -> Vec<Point<World>> {
        match self.0 {
            Form::Turned { origin, z, meridian: Meridian::Centred { radius }, .. } => {
                vec![Point::from_vec3(origin + z * radius), Point::from_vec3(origin - z * radius)]
            }
            Form::Flat { .. } | Form::Turned { .. } => Vec::new(),
        }
    }

    /// On a cone, the ruling from its apex along `along` in its parameters: the apex's parameters on it, at the
    /// ruling's angle, and the way `v` runs with the distance along it, 1 or −1. A ruling crosses the apex, where the
    /// angle about the axis is undefined, so its image is this line of constant `u`. `None` on any other surface.
    #[must_use]
    pub fn ruling(&self, along: Direction<World>) -> Option<(UvPoint, f64)> {
        let Form::Turned { x, y, z, meridian: Meridian::Slanted { apex, .. }, .. } = self.0 else { return None };
        let g = along.get();
        let up = if g.dot(z) < 0.0 { -1.0 } else { 1.0 };
        Some((UvPoint::new(atan2(up * g.dot(y), up * g.dot(x)), apex), up))
    }

    /// Adds the numbers that define the chart.
    pub(crate) fn push(&self, out: &mut Content) {
        match self.0 {
            Form::Flat { origin, at, du, dv, .. } => {
                out.push(0.0);
                for v in [origin, du, dv] {
                    out.push_vec3(v);
                }
                out.push(at.0);
                out.push(at.1);
            }
            Form::Turned { origin, x, y, z, meridian } => {
                out.push(1.0);
                for v in [origin, x, y, z] {
                    out.push_vec3(v);
                }
                let numbers = match meridian {
                    Meridian::Upright { radius } => [0.0, radius, 0.0, 0.0],
                    Meridian::Slanted { apex, slope } => [1.0, apex, slope.0, slope.1],
                    Meridian::Centred { radius } => [2.0, radius, 0.0, 0.0],
                    Meridian::Tube { major, minor } => [3.0, major, minor, 0.0],
                };
                for x in numbers {
                    out.push(x);
                }
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use occam_units::{Angle, Length};

    use super::*;
    use crate::conformance::{placement, tolerance};
    use crate::{BsplineSurface, Cone, Cylinder, Plane, Sphere, SplineSurface, Torus};

    fn mm(x: f64) -> Length {
        Length::from_millimetres(x)
    }

    fn surfaces() -> Vec<Box<dyn Surface>> {
        vec![
            Box::new(Plane::placed(&placement())),
            Box::new(Cylinder::placed(&placement(), mm(0.8)).unwrap()),
            Box::new(Cone::placed(&placement(), mm(0.5), Angle::from_radians(0.4)).unwrap()),
            Box::new(Sphere::placed(&placement(), mm(1.5)).unwrap()),
            Box::new(Torus::placed(&placement(), mm(2.0), mm(0.5)).unwrap()),
        ]
    }

    fn chart(s: &dyn Surface) -> Option<Chart> {
        Chart::of(s, s.analytic(tolerance())?)
    }

    #[test]
    fn a_chart_gives_a_surfaces_points_and_reads_their_parameters_back() {
        for s in surfaces() {
            let c = chart(s.as_ref()).unwrap_or_else(|| panic!("{:?} has a chart", s.family()));
            for (i, j) in (0..6).flat_map(|i| (0..5).map(move |j| (i, j))) {
                // Down to v = -3, below the cone's apex at -1.28, where its parameters are read on the far side.
                let uv = UvPoint::new(0.3 + 0.5 * f64::from(i), -3.0 + 0.9 * f64::from(j));
                let p = s.point(uv);
                let gap = (c.point(uv).to_vec3() - p.to_vec3()).norm();
                assert!(gap <= 1e-12 * (1.0 + p.to_vec3().norm()), "{:?} at {uv:?}: {gap}", s.family());
                let back = s.point(c.parameters(p));
                let gap = (back.to_vec3() - p.to_vec3()).norm();
                assert!(gap <= 1e-12 * (1.0 + p.to_vec3().norm()), "{:?} reads {uv:?} back {gap} away", s.family());
            }
        }
    }

    #[test]
    fn a_surface_whose_parameters_run_otherwise_has_no_chart() {
        // Poles in a plane, bunched towards one side, so that the point moves unevenly with `u`.
        let at = |x: f64, y: f64| placement().point(Point::from_vec3(Vec3::new(x, y, 0.0)));
        let poles = vec![at(-2.0, -2.0), at(-1.5, -2.0), at(2.0, -2.0), at(-2.0, 2.0), at(-1.5, 2.0), at(2.0, 2.0)];
        let (knots_u, knots_v) = (vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0], vec![0.0, 0.0, 1.0, 1.0]);
        let form = BsplineSurface { degree: (2, 1), count: (3, 2), knots_u, knots_v, poles, weights: None };
        let bunched = SplineSurface::new(&form).unwrap();
        assert!(Surface::analytic(&bunched, tolerance()).is_some(), "it presents its plane");
        assert_eq!(chart(&bunched), None);
        let plane = Plane::placed(&placement());
        let sphere = Sphere::placed(&placement(), mm(1.0)).unwrap();
        assert_eq!(Chart::of(&plane, Surface::analytic(&sphere, tolerance()).unwrap()), None, "a form it is not");
    }

    #[test]
    fn a_spheres_chart_has_its_poles_and_a_cones_its_rulings() {
        let sphere = Sphere::placed(&placement(), mm(1.5)).unwrap();
        let c = chart(&sphere).unwrap();
        let poles: Vec<Vec3> = c.poles().into_iter().map(Point::to_vec3).collect();
        let want = Surface::degeneracies(&sphere).iter().rev().map(|d| d.point.to_vec3());
        for (got, want) in poles.iter().zip(want) {
            assert!((*got - want).norm() <= 1e-12, "{got:?} against {want:?}");
        }
        assert_eq!(poles.len(), 2);
        let cone = Cone::placed(&placement(), mm(0.5), Angle::from_radians(0.4)).unwrap();
        let c = chart(&cone).unwrap();
        // Down the ruling at u = 1.1 from the apex: its points are at that `u`, `v` falling with the distance.
        let down = Surface::first(&cone, UvPoint::new(1.1, 0.0)).dv.rate() * -1.0;
        let (apex, up) = c.ruling(Direction::normalised(down)).unwrap();
        assert_eq!(up, -1.0, "the ruling runs down the axis");
        let there = Surface::point(&cone, UvPoint::new(apex.u(), apex.v() + up * 3.0)).to_vec3();
        let want = Surface::point(&cone, apex).to_vec3() + down * 3.0;
        assert!((there - want).norm() <= 1e-12 * 8.0, "{apex:?}: {there:?} against {want:?}");
        let flat = chart(&Plane::placed(&placement())).unwrap();
        assert!(c.poles().is_empty() && flat.ruling(Direction::normalised(down)).is_none() && flat.poles().is_empty());
    }

    /// A quadratic Bézier plane patch about `(60, 80, 0)`, 20 along `u` and 2 along `v`, whose middle row is moved
    /// `off` along `u`: its point strays from its chart by `2 off (u - 1/2)^2`, most at the probe at `u = 0.2`.
    fn bowed(off: f64) -> SplineSurface {
        let at = |x: f64, y: f64| Point::from_vec3(Vec3::new(50.0 + x, 79.0 + y, 0.0));
        let poles =
            vec![at(0.0, 0.0), at(10.0 + off, 0.0), at(20.0, 0.0), at(0.0, 2.0), at(10.0 + off, 2.0), at(20.0, 2.0)];
        let (knots_u, knots_v) = (vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0], vec![0.0, 0.0, 1.0, 1.0]);
        SplineSurface::new(&BsplineSurface { degree: (2, 1), count: (3, 2), knots_u, knots_v, poles, weights: None })
            .unwrap()
    }

    #[test]
    fn a_chart_holds_its_surface_to_a_part_in_1e11_of_the_size_of_its_point_and_derivatives() {
        // Read at (1/2, 1/2), where the point is 100 from the origin and the derivatives 20 and 2 long: a patch that strays
        // by 0.99 of the band has a chart, and one by 1.01 has none.
        let jet = Surface::first(&bowed(0.0), UvPoint::new(0.5, 0.5));
        let size = jet.point.to_vec3().norm() + jet.du.rate().norm() + jet.dv.rate().norm();
        assert!((size - 122.0).abs() < 1e-9, "{size}");
        for (share, kept) in [(0.99, true), (1.01, false)] {
            let off = share * 1e-11 * size / (2.0 * 0.09);
            assert_eq!(chart(&bowed(off)).is_some(), kept, "strays by {share} of its band");
        }
    }

    #[test]
    fn a_part_of_a_surface_has_its_chart_read_away_from_its_turns_start_and_its_apex() {
        use crate::{TrimmedSurface, UvBox};
        let window = |u: (f64, f64), v: (f64, f64)| UvBox::new(UvPoint::new(u.0, v.0), UvPoint::new(u.1, v.1));
        let cone = || Cone::placed(&placement(), mm(1.0), Angle::from_radians(0.5)).unwrap();
        let apex = -1.0 / occam_math::sin(0.5);
        let parts: Vec<Box<dyn Surface>> = vec![
            Box::new(
                TrimmedSurface::new(Cylinder::placed(&placement(), mm(0.8)).unwrap(), window((0.2, 1.4), (1.0, 3.0)))
                    .unwrap(),
            ),
            Box::new(TrimmedSurface::new(cone(), window((0.2, 1.4), (apex - 1.0, apex + 1.0))).unwrap()),
            Box::new(TrimmedSurface::new(cone(), window((4.0, 5.5), (apex - 3.0, apex - 1.0))).unwrap()),
            Box::new(
                TrimmedSurface::new(Sphere::placed(&placement(), mm(1.5)).unwrap(), window((0.2, 1.4), (0.2, 1.2)))
                    .unwrap(),
            ),
            Box::new(
                TrimmedSurface::new(
                    Torus::placed(&placement(), mm(2.0), mm(0.5)).unwrap(),
                    window((0.2, 1.4), (0.5, 2.5)),
                )
                .unwrap(),
            ),
        ];
        for s in parts {
            let c = chart(s.as_ref()).unwrap_or_else(|| panic!("{:?} has a chart", s.family()));
            let d = s.domain();
            for (fu, fv) in [(0.1, 0.1), (0.5, 0.9), (0.9, 0.4)] {
                let uv = UvPoint::new(d.u.lo + fu * (d.u.hi - d.u.lo), d.v.lo + fv * (d.v.hi - d.v.lo));
                let gap = (c.point(uv).to_vec3() - s.point(uv).to_vec3()).norm();
                assert!(gap <= 1e-12 * 8.0, "{:?} at {uv:?}: {gap}", s.family());
            }
        }
    }

    #[test]
    fn a_plane_whose_parameters_run_unevenly_on_a_domain_about_nought_has_no_chart() {
        // Its `v` runs from -1 to 1, its middle row bunched towards its first: probed across `v`, it strays.
        let at = |x: f64, y: f64| Point::from_vec3(Vec3::new(x, y, 0.0));
        let poles = vec![at(-2.0, -2.0), at(2.0, -2.0), at(-2.0, -1.5), at(2.0, -1.5), at(-2.0, 2.0), at(2.0, 2.0)];
        let (knots_u, knots_v) = (vec![0.0, 0.0, 1.0, 1.0], vec![-1.0, -1.0, -1.0, 1.0, 1.0, 1.0]);
        let form = BsplineSurface { degree: (1, 2), count: (2, 3), knots_u, knots_v, poles, weights: None };
        let bunched = SplineSurface::new(&form).unwrap();
        assert!(Surface::analytic(&bunched, tolerance()).is_some());
        assert_eq!(chart(&bunched), None);
    }

    #[test]
    fn a_charts_content_is_its_numbers() {
        let words = |c: Chart| {
            let mut out = Content::default();
            c.push(&mut out);
            out.words().to_vec()
        };
        let charts = surfaces().iter().filter_map(|s| chart(s.as_ref())).map(words).collect::<Vec<_>>();
        assert_eq!(charts.len(), 5);
        for (i, one) in charts.iter().enumerate() {
            assert!(one.len() > 4, "{one:?}");
            assert!(charts.iter().skip(i + 1).all(|other| other != one), "every chart's numbers differ");
        }
    }
}
