//! Source: do-carmo-1976
//! The offset surface, `S + d N` for any surface `S` and its unit normal `N` (do Carmo, chapter 3: a parallel surface;
//! PARITY 2.2). Its partials to an order need the base's to the order above, through the normal's, which normal.rs
//! gives exactly, so it has one order fewer than its base: a base with no second derivative, as a mesh has, has no
//! continuous normal and is refused. Where `Su x Sv` vanishes, at a singular point, the offset is undefined: NaN there.
//! A degenerate line of the base that declares its normal, as a sphere's pole does, is the limit of the normals beside
//! it, so the offset collapses there too, to the base's point moved along that normal, and declares it; one without a
//! normal, as a cone's apex, it does not. The base is held by type, so the first two rungs run in lanes; the second,
//! which needs the base's third derivatives that no rung gives, runs a lane at a time.

use occam_math::Real;
use occam_report::{Code, Fault, NamedValue, Quantity, Report};
use occam_units::{Angle, Bounds, Length, Point, Tangent, Tolerance, UvPoint, Vec3, World};

use crate::analytic::Analytic;
use crate::batch::{Jet1, Jet2, Scratch};
use crate::bspline::BsplineSurface;
use crate::domain::{Degeneracy, Domain, UvBox};
use crate::foot::{foot, polish};
use crate::held::{push_surface, with_surface};
use crate::identity::{Content, FamilyName, Shape};
use crate::lanewise::{MAX_LANES, from_lanes, lanes};
use crate::normal;
use crate::spline::triangle;
use crate::support::{finite_size, padded};
use crate::surface::Parametric;

/// How many cells a side the foot's search over a finite domain samples.
const SAMPLES: u32 = 16;

/// A surface moved along its normal.
#[derive(Clone, Debug, PartialEq)]
pub struct OffsetSurface<S> {
    base: S,
    distance: f64,
    /// The base's degenerate lines that declare a normal, moved along it.
    lines: Vec<Degeneracy>,
}

/// The offset's partials to `order` into `out`, from the base's to `order + 1` in `base`, the point first: `S + d N`,
/// at [`crate::spline::index`]. `m` and `lengths` are workspace as long as `out`.
#[inline(always)]
fn offset<R: Real>(
    order: usize,
    base: &[Vec3<R>],
    distance: f64,
    (m, lengths): (&mut [Vec3<R>], &mut [R]),
    out: &mut [Vec3<R>],
) {
    normal::surface(order, base, m, lengths);
    for (slot, (s, n)) in out.iter_mut().zip(base.iter().zip(m.iter())).take(triangle(order)) {
        *slot = *s + *n * R::splat(distance);
    }
}

/// The finest tolerance there is, so that a form presented within it is the base exactly, as a B-spline must be.
fn exactly() -> Option<Tolerance> {
    Tolerance::new(Length::from_millimetres(f64::MIN_POSITIVE), Angle::from_radians(f64::MIN_POSITIVE)).ok()
}

/// The refusal of a base with derivatives only to `highest`, where an offset needs the second.
fn not_smooth(highest: u8) -> Report {
    Report::new(Code::NotSmooth, Fault::InvalidInput)
        .with_value(NamedValue::new("highest", Quantity::Count, f64::from(highest)))
        .with_value(NamedValue::new("needed", Quantity::Count, 2.0))
}

impl<S: Parametric> OffsetSurface<S> {
    /// `base` moved `distance` along its normal, `Su x Sv` over its length, so a negative distance moves it the other way.
    ///
    /// # Errors
    ///
    /// `units.not-finite` for a distance that is not finite, and `geom.not-smooth` for a base without second
    /// derivatives, whose normal is not continuous.
    pub fn new(base: S, distance: Length) -> Result<Self, Report> {
        let distance = finite_size(distance)?;
        if let Some(highest) = base.highest_derivative().filter(|h| *h < 2) {
            return Err(not_smooth(highest));
        }
        let moved = |line: &Degeneracy| {
            let normal = line.normal?;
            Some(Degeneracy { point: Point::from_vec3(line.point.to_vec3() + normal.get() * distance), ..*line })
        };
        let lines = base.degeneracies().iter().filter_map(moved).collect();
        Ok(Self { base, distance, lines })
    }

    /// The point and its partials to `order` at `uv` as numbers, from the base's `derivatives` in the caller's
    /// `rates` and `scratch`, so that a call's lanes share one workspace: NaN where the base has none.
    fn jets(&self, uv: UvPoint, order: u8, (rates, scratch): (&mut Vec<Tangent<World>>, &mut Scratch)) -> [Vec3; 6] {
        rates.clear();
        let nan = Vec3::new(f64::NAN, f64::NAN, f64::NAN);
        let mut base = [nan; 10];
        if Parametric::derivatives(&self.base, uv, order.saturating_add(1), rates, scratch).is_ok() {
            let point = std::iter::once(self.base.eval_point(uv).to_vec3());
            for (slot, value) in base.iter_mut().zip(point.chain(rates.iter().map(|t| t.rate()))) {
                *slot = value;
            }
        }
        let (mut m, mut lengths, mut out) = ([nan; 6], [f64::NAN; 6], [nan; 6]);
        offset(usize::from(order), &base, self.distance, (&mut m, &mut lengths), &mut out);
        out
    }
}

impl<S: Parametric> Parametric for OffsetSurface<S> {
    #[inline(always)]
    fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R> {
        let jet = self.base.eval_first(uv);
        let base = [jet.point.to_vec3(), jet.du.rate(), jet.dv.rate()];
        let zero = R::splat(0.0);
        let none = Vec3::new(zero, zero, zero);
        let (mut m, mut lengths, mut out) = ([none], [zero], [none]);
        offset(0, &base, self.distance, (&mut m, &mut lengths), &mut out);
        let [point] = out;
        Point::from_vec3(point)
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R> {
        let jet = self.base.eval_second(uv);
        let first = jet.first;
        let base =
            [first.point.to_vec3(), first.du.rate(), first.dv.rate(), jet.duu.rate(), jet.duv.rate(), jet.dvv.rate()];
        let zero = R::splat(0.0);
        let none = Vec3::new(zero, zero, zero);
        let (mut m, mut lengths, mut out) = ([none; 3], [zero; 3], [none; 3]);
        offset(1, &base, self.distance, (&mut m, &mut lengths), &mut out);
        let [point, du, dv] = out;
        Jet1 { point: Point::from_vec3(point), du: Tangent::new(du), dv: Tangent::new(dv) }
    }

    /// A lane at a time, in one workspace: the second partials need the base's third, which no rung gives.
    fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R> {
        const { assert!(R::LANES <= MAX_LANES, "more lanes than a lane-at-a-time evaluation holds") };
        let (us, vs) = (lanes(uv.u()), lanes(uv.v()));
        let nan = Vec3::new(f64::NAN, f64::NAN, f64::NAN);
        let (mut rates, mut scratch) = (Vec::with_capacity(9), Scratch::default());
        let each: [[Vec3; 6]; MAX_LANES] = std::array::from_fn(|k| match (us.get(k), vs.get(k)) {
            (Some(&u), Some(&v)) if k < R::LANES => self.jets(UvPoint::new(u, v), 2, (&mut rates, &mut scratch)),
            _ => [nan; 6],
        });
        let at = |slot: usize| from_lanes::<R>(|k| each.get(k).and_then(|e| e.get(slot)).copied().unwrap_or(nan));
        let first = Jet1 { point: Point::from_vec3(at(0)), du: Tangent::new(at(1)), dv: Tangent::new(at(2)) };
        Jet2 { first, duu: Tangent::new(at(3)), duv: Tangent::new(at(4)), dvv: Tangent::new(at(5)) }
    }

    /// From the base's partials to the order above, through the caller's `out` and `scratch`.
    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        let above = normal::above(order)?;
        let start = out.len();
        Parametric::derivatives(&self.base, uv, above, out, scratch)?;
        let (wide, own) = (triangle(usize::from(above)), triangle(usize::from(order)));
        let (lengths, vectors) = scratch.reals_and_vectors(own, wide.saturating_add(own.saturating_mul(2)));
        let (base, rest) = vectors.split_at_mut(wide);
        let (m, results) = rest.split_at_mut(own);
        normal::take(self.base.eval_point(uv).to_vec3(), (out, start), base);
        offset(usize::from(order), base, self.distance, (m, lengths), results);
        out.extend(results.iter().skip(1).map(|v| Tangent::new(*v)));
        Ok(())
    }

    fn domain(&self) -> Domain {
        self.base.domain()
    }

    fn highest_derivative(&self) -> Option<u8> {
        self.base.highest_derivative().map(|h| h.saturating_sub(1))
    }

    /// The base's box over the patch, grown by the distance and padded for the rounding in the normal.
    fn bounds(&self, patch: UvBox) -> Bounds {
        let b = self.base.bounds(patch);
        let grown = Length::from_millimetres(self.distance.abs());
        padded(Bounds::around(b.low(), grown).union(&Bounds::around(b.high(), grown)), 8.0)
    }

    /// From the base's foot of `p`: where the distance is within the base's radii of curvature on the side it moves to,
    /// the normal through the base's foot meets the offset at its foot. Over a finite domain the shared search over the
    /// whole, seeded there, also finds a foot where it does not, as past a sphere's centre; over an unbounded one,
    /// Newton's method from the seed finds the nearest of its valley.
    fn invert(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<UvPoint, Report> {
        let start = self.base.invert(p, seed, tol, scratch)?;
        let d = self.base.domain();
        let window = UvBox::new(UvPoint::new(d.u.lo, d.v.lo), UvPoint::new(d.u.hi, d.v.hi));
        let bounded = [window.lo.u(), window.lo.v(), window.hi.u(), window.hi.v()].into_iter().all(f64::is_finite);
        Ok(if bounded {
            foot(self, p, window, (SAMPLES, SAMPLES), Some(start), scratch)
        } else {
            polish(self, p, start, window)
        })
    }

    fn family(&self) -> FamilyName {
        FamilyName("offset")
    }

    fn content(&self, out: &mut Content) {
        out.push(self.distance);
        push_surface(out, &self.base);
    }

    fn degeneracies(&self) -> &[Degeneracy] {
        &self.lines
    }

    fn shape(&self) -> Shape {
        Shape { form: "offset", values: with_surface(vec![NamedValue::length("distance", self.distance)], &self.base) }
    }

    /// The base's form moved along its normal, where that stays in the form: a plane moved, and a cylinder, a sphere
    /// and a torus's tube grown by the distance while it stays above zero, facing as before. A cone's two nappes move
    /// to two cones, and a form shrunk through its axis or centre faces inward, which no form here says: neither.
    fn analytic(&self, tol: Tolerance) -> Option<Analytic> {
        let d = self.distance;
        let grown = |r: Length| (r.millimetres() + d > 0.0).then(|| Length::from_millimetres(r.millimetres() + d));
        match self.base.analytic(tol)? {
            Analytic::Plane { origin, normal } => {
                Some(Analytic::Plane { origin: Point::from_vec3(origin.to_vec3() + normal.get() * d), normal })
            }
            Analytic::Cylinder { on_axis, axis, radius } => {
                Some(Analytic::Cylinder { on_axis, axis, radius: grown(radius)? })
            }
            Analytic::Sphere { centre, radius } => Some(Analytic::Sphere { centre, radius: grown(radius)? }),
            Analytic::Torus { centre, axis, major, minor } => {
                let minor = grown(minor).filter(|m| m.millimetres() < major.millimetres())?;
                Some(Analytic::Torus { centre, axis, major, minor })
            }
            Analytic::Cone { .. } => None,
        }
    }

    /// The base's B-spline moved as the offset moves it, where the base is a form whose offset is an affine map of it:
    /// a plane translated along its normal, a sphere scaled about its centre, and a cylinder scaled across its axis,
    /// each by `(r + d) / r`. Any other offset has no B-spline form.
    fn to_bspline(&self, patch: UvBox) -> Option<BsplineSurface> {
        let form = self.base.analytic(exactly()?)?;
        let mut spline = self.base.to_bspline(patch)?;
        let d = self.distance;
        let moved = |p: Vec3| -> Option<Vec3> {
            match form {
                Analytic::Plane { normal, .. } => Some(p + normal.get() * d),
                Analytic::Sphere { centre, radius } => {
                    let c = centre.to_vec3();
                    Some(c + (p - c) * ((radius.millimetres() + d) / radius.millimetres()))
                }
                Analytic::Cylinder { on_axis, axis, radius } => {
                    let (o, a) = (on_axis.to_vec3(), axis.get());
                    let along = a * (p - o).dot(a);
                    Some(o + along + (p - o - along) * ((radius.millimetres() + d) / radius.millimetres()))
                }
                Analytic::Cone { .. } | Analytic::Torus { .. } => None,
            }
        };
        for pole in &mut spline.poles {
            *pole = Point::from_vec3(moved(pole.to_vec3())?);
        }
        Some(spline)
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_2, TAU};

    use occam_units::{Angle, Direction};

    use super::*;
    use crate::conformance::{self, Check, arc, local, placement, tolerance, wrong_second};
    use crate::surface::Surface;
    use crate::{Cone, Cylinder, MeshSurface, Plane, Revolution, Sphere, SplineSurface, Torus};

    fn mm(x: f64) -> Length {
        Length::from_millimetres(x)
    }

    fn window(lo: (f64, f64), hi: (f64, f64)) -> UvBox {
        UvBox::new(UvPoint::new(lo.0, lo.1), UvPoint::new(hi.0, hi.1))
    }

    fn moved<S: Parametric>(base: S, distance: f64) -> OffsetSurface<S> {
        OffsetSurface::new(base, mm(distance)).unwrap()
    }

    /// A gentle rational biquadratic Bézier, its radii of curvature well above the distances moved.
    fn gentle() -> SplineSurface {
        let pole = |i: u32, j: u32| {
            let (x, y) = (f64::from(i), f64::from(j));
            Point::from_vec3(local(4.0 * x + 0.3 * y, 4.0 * y, 0.6 * (x - 1.0) * (y - 1.0) + 0.2 * x))
        };
        let poles = (0..3).flat_map(|j| (0..3).map(move |i| pole(i, j))).collect();
        SplineSurface::bezier((3, 3), poles, Some(vec![1.0, 1.2, 1.0, 0.9, 1.0, 1.1, 1.0, 0.8, 1.0])).unwrap()
    }

    fn cylinder() -> Cylinder {
        Cylinder::placed(&placement(), mm(6.0)).unwrap()
    }

    fn sphere() -> Sphere {
        Sphere::placed(&placement(), mm(5.0)).unwrap()
    }

    fn torus() -> Torus {
        Torus::placed(&placement(), mm(8.0), mm(2.0)).unwrap()
    }

    fn cone(radius: f64) -> Cone {
        Cone::placed(&placement(), mm(radius), Angle::from_radians(0.4)).unwrap()
    }

    fn passes(s: &dyn Surface, windows: &[UvBox]) {
        for w in windows {
            let failures = conformance::surface(s, *w);
            assert!(failures.is_empty(), "{:?} over {w:?}: {failures:#?}", s.shape().values);
        }
    }

    #[test]
    fn the_offset_of_every_surface_family_passes_the_suite() {
        let across_seam = window((-1.0, -4.0), (2.5, 6.0));
        passes(&moved(Plane::placed(&placement()), 1.5), &[window((-3.0, 2.0), (4.0, 9.5))]);
        passes(&moved(cylinder(), 0.8), &[across_seam]);
        passes(&moved(cylinder(), -2.0), &[across_seam]);
        passes(&moved(cone(3.0), 0.5), &[window((-1.0, 1.0), (2.5, 6.0))]);
        passes(
            &moved(sphere(), 0.7),
            &[window((-1.0, -1.2), (2.5, 1.3)), window((-1.0, -FRAC_PI_2), (2.5, FRAC_PI_2))],
        );
        passes(&moved(sphere(), -1.5), &[window((0.5, -0.4), (6.0, 1.0))]);
        passes(&moved(torus(), 0.5), &[window((-1.0, 1.0), (2.5, 4.0))]);
        passes(&moved(gentle(), 0.4), &[window((0.0, 0.0), (1.0, 1.0)), window((0.2, 0.1), (0.7, 0.9))]);
        let up = placement().direction(Direction::normalised(occam_units::Vec3::new(0.0, 0.0, 1.0)));
        let turned = Revolution::new(Point::from_vec3(local(0.0, 0.0, 0.0)), up, arc());
        passes(&moved(turned, 0.3), &[window((-1.0, 0.5), (2.5, 4.0))]);
    }

    #[test]
    fn the_suite_catches_an_offset_with_a_wrong_second_derivative() {
        let s = wrong_second(moved(sphere(), 0.7));
        let failed = conformance::checks(&conformance::surface(&s, window((-1.0, -1.2), (2.5, 1.3))));
        assert_eq!(failed, [Check::Orders, Check::Differences]);
    }

    #[test]
    fn the_offset_of_a_plane_a_cylinder_and_a_sphere_is_the_expected_analytic_surface_within_the_band() {
        let tol = tolerance();
        let plane = Plane::placed(&placement());
        let Some(Analytic::Plane { origin, normal }) = Surface::analytic(&plane, tol) else { panic!("a plane") };
        let want = Analytic::Plane { origin: Point::from_vec3(origin.to_vec3() + normal.get() * 1.5), normal };
        assert_eq!(Surface::analytic(&moved(plane, 1.5), tol), Some(want));
        let Some(Analytic::Cylinder { on_axis, axis, .. }) = Surface::analytic(&cylinder(), tol) else { panic!() };
        let want = Analytic::Cylinder { on_axis, axis, radius: mm(6.8) };
        assert_eq!(Surface::analytic(&moved(cylinder(), 0.8), tol), Some(want));
        let Some(Analytic::Sphere { centre, .. }) = Surface::analytic(&sphere(), tol) else { panic!("a sphere") };
        assert_eq!(Surface::analytic(&moved(sphere(), -1.5), tol), Some(Analytic::Sphere { centre, radius: mm(3.5) }));
        // The suite's normals check holds every point and normal to the form presented, within the band.
        let named: [(Box<dyn Surface>, UvBox); 3] = [
            (Box::new(moved(Plane::placed(&placement()), 1.5)), window((-3.0, 2.0), (4.0, 9.5))),
            (Box::new(moved(cylinder(), 0.8)), window((-1.0, -4.0), (2.5, 6.0))),
            (Box::new(moved(sphere(), -1.5)), window((0.5, -0.4), (6.0, 1.0))),
        ];
        for (s, w) in named {
            assert!(s.analytic(tol).is_some());
            let failed = conformance::checks(&conformance::surface(s.as_ref(), w));
            assert!(!failed.contains(&Check::Normals) && failed.is_empty(), "{failed:?}");
        }
    }

    #[test]
    fn a_form_shrunk_through_its_axis_or_a_cone_presents_none() {
        let tol = tolerance();
        assert_eq!(Surface::analytic(&moved(cylinder(), -6.5), tol), None, "inward");
        assert_eq!(Surface::analytic(&moved(sphere(), -5.0), tol), None, "a point");
        assert_eq!(Surface::analytic(&moved(cone(3.0), 0.5), tol), None, "two nappes, two cones");
        let Some(Analytic::Torus { centre, axis, major, .. }) = Surface::analytic(&torus(), tol) else { panic!() };
        let want = Analytic::Torus { centre, axis, major, minor: mm(2.5) };
        assert_eq!(Surface::analytic(&moved(torus(), 0.5), tol), Some(want));
        assert_eq!(Surface::analytic(&moved(torus(), 6.0), tol), None, "past the axis");
        assert_eq!(Surface::analytic(&moved(gentle(), 0.4), tol), None);
    }

    #[test]
    fn its_bspline_is_the_bases_moved_where_that_is_an_affine_map_and_none_elsewhere() {
        let whole = window((0.0, 0.0), (TAU, 2.0));
        let shown: [(Box<dyn Surface>, UvBox); 4] = [
            (Box::new(moved(Plane::placed(&placement()), 1.5)), window((-3.0, 2.0), (4.0, 9.5))),
            (Box::new(moved(cylinder(), -2.0)), whole),
            (Box::new(moved(sphere(), 0.7)), window((0.0, -1.2), (TAU, 1.3))),
            (Box::new(moved(sphere(), -7.0)), window((0.5, -0.4), (2.0, 1.0))),
        ];
        for (s, w) in shown {
            assert!(s.to_bspline(w).is_some(), "{:?}", s.shape().values);
            let failures = conformance::presented_surface(s.as_ref(), w);
            assert!(failures.is_empty(), "{w:?}: {failures:#?}");
        }
        assert_eq!(Surface::to_bspline(&moved(torus(), 0.5), whole), None);
        assert_eq!(Surface::to_bspline(&moved(gentle(), 0.4), window((0.0, 0.0), (1.0, 1.0))), None);
    }

    #[test]
    fn a_pole_moves_along_its_normal_and_an_apex_without_one_is_undefined() {
        let s = moved(sphere(), 0.7);
        let poles: Vec<f64> = Surface::degeneracies(&s).iter().map(|line| line.at).collect();
        assert_eq!(poles, [-FRAC_PI_2, FRAC_PI_2]);
        for (line, base) in Surface::degeneracies(&s).iter().zip(Surface::degeneracies(&sphere())) {
            let want = base.point.to_vec3() + base.normal.unwrap().get() * 0.7;
            assert!((line.point.to_vec3() - want).norm() < 1e-12 && line.normal == base.normal, "{line:?}");
        }
        let pointed = moved(cone(0.0), 0.5);
        assert!(pointed.point(UvPoint::new(1.0, 0.0)).to_vec3().z.is_nan(), "the apex");
        assert!(Surface::degeneracies(&pointed).is_empty(), "an apex declares no normal");
    }

    #[test]
    fn a_base_without_a_continuous_normal_or_a_distance_not_finite_is_refused() {
        let points = [(0.0, 0.0), (1.0, 0.0), (1.0, 1.0), (0.0, 1.0)].map(|(u, v)| Point::from_vec3(local(u, v, 0.0)));
        let uv = vec![UvPoint::new(0.0, 0.0), UvPoint::new(1.0, 0.0), UvPoint::new(1.0, 1.0), UvPoint::new(0.0, 1.0)];
        let mesh = MeshSurface::new(&points, uv, &[[0, 1, 2], [0, 2, 3]]).unwrap();
        let refused = OffsetSurface::new(mesh, mm(1.0)).unwrap_err();
        let named = |name: &str| refused.value(name).and_then(NamedValue::number);
        assert_eq!((refused.code, named("highest"), named("needed")), (Code::NotSmooth, Some(1.0), Some(2.0)));
        for bad in [f64::NAN, f64::INFINITY] {
            assert_eq!(OffsetSurface::new(Plane::placed(&placement()), mm(bad)).unwrap_err().code, Code::NotFinite);
        }
    }

    #[test]
    fn its_shape_and_content_name_what_defines_it() {
        let s = moved(cylinder(), 0.8);
        let shape = Surface::shape(&s);
        assert_eq!((shape.form, shape.values[0].clone()), ("offset", NamedValue::length("distance", 0.8)));
        assert_eq!(shape.values[1].name, "base-origin");
        let (mut content, mut base) = (Content::default(), Content::default());
        Surface::content(&s, &mut content);
        base.push_family(FamilyName("cylinder"));
        Parametric::content(&cylinder(), &mut base);
        assert!(content.words().ends_with(base.words()));
        assert_eq!(content.words()[7], 0.8f64.to_bits(), "the name, then the distance");
        assert_eq!((Surface::family(&s), Surface::highest_derivative(&s)), (FamilyName("offset"), None));
    }

    #[test]
    fn an_offset_has_one_derivative_fewer_than_a_base_that_has_few() {
        let s = OffsetSurface::new(conformance::capped(cylinder(), 3), mm(0.5)).unwrap();
        assert_eq!(Surface::highest_derivative(&s), Some(2));
        let rough = OffsetSurface::new(conformance::capped(cylinder(), 1), mm(0.5)).map(|_| ()).unwrap_err();
        assert_eq!(rough.code, Code::NotSmooth);
    }
}
