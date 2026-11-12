//! Source: original
//! The mesh-backed surface: triangles whose vertices each have a point in space and a place in the parameters (S4,
//! L21). A parameter in a triangle gives that triangle's point there, so the surface is continuous and no smoother: its
//! first derivatives are the triangle's, and it has none above them. A parameter in no triangle is in a hole, inside the
//! domain or not, and there the surface is undefined, NaN in every lane, which a batch reports point by point. Its foot
//! is the nearest point of any triangle. It presents its triangles.

use occam_math::Real;
use occam_report::{Code, Fault, NamedValue, Quantity, Report};
use occam_units::{Bounds, ParamKind, Point, Tangent, Tolerance, UvPoint, Vec3, World};

use crate::batch::{Jet1, Jet2, Scratch};
use crate::bspline::{checked_poles, wrong_count};
use crate::cells::Cells;
use crate::domain::{Domain, ParamRange, UvBox, Wrap};
use crate::facet::Facet;
use crate::identity::{Content, FamilyName, Shape};
use crate::lanewise::{MAX_LANES, from_lanes, lanes};
use crate::support::{everything, padded, splat};
use crate::surface::{Parametric, TriangleView};

/// A surface made of triangles.
#[derive(Clone, Debug, PartialEq)]
pub struct MeshSurface {
    /// The vertices' points.
    points: Vec<Vec3>,
    /// The vertices' places in the parameters.
    uv: Vec<UvPoint>,
    /// The triangles, in the order given.
    facets: Vec<Facet>,
    /// Which triangles reach each part of the parameters.
    cells: Cells,
    domain: Domain,
}

/// A count as a number.
fn number(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

/// The refusal of triangle `index`.
fn bad_triangle(index: usize) -> Report {
    Report::new(Code::TriangleNotValid, Fault::InvalidInput).with_value(NamedValue::new(
        "index",
        Quantity::Count,
        number(index),
    ))
}

/// The first triangle that overlaps one before it in the parameters, and that one: only those listed in the same cells
/// of the grid are compared.
fn overlap(facets: &[Facet], spans: &[UvBox], cells: &Cells) -> Option<(usize, usize)> {
    facets.iter().zip(spans).enumerate().find_map(|(index, (facet, span))| {
        let earlier = cells.reaching(*span).filter_map(|j| usize::try_from(j).ok()).filter(|j| *j < index);
        earlier.into_iter().find(|j| facets.get(*j).is_some_and(|f| facet.overlaps(f))).map(|j| (index, j))
    })
}

/// The lower and the higher of two numbers.
fn ordered(a: f64, b: f64) -> (f64, f64) {
    if b < a { (b, a) } else { (a, b) }
}

/// The smallest box in the parameters holding every one of `boxes`, of which there is at least one.
fn hull(boxes: &[UvBox]) -> UvBox {
    let first = boxes.first().copied().unwrap_or(UvBox::new(UvPoint::new(0.0, 0.0), UvPoint::new(0.0, 0.0)));
    boxes.iter().fold(first, |a, b| {
        let lo = UvPoint::new(ordered(a.lo.u(), b.lo.u()).0, ordered(a.lo.v(), b.lo.v()).0);
        let hi = UvPoint::new(ordered(a.hi.u(), b.hi.u()).1, ordered(a.hi.v(), b.hi.v()).1);
        UvBox::new(lo, hi)
    })
}

impl MeshSurface {
    /// The surface of `triangles`, each three indices into the vertices, whose points are `points` and whose places in
    /// the parameters are `uv`. Each triangle must run anticlockwise with some area in the parameters, and no two may
    /// overlap there, though they may share edges and corners. The domain is the smallest box holding every triangle,
    /// and a parameter in it that no triangle holds is a hole.
    ///
    /// # Errors
    ///
    /// `units.not-finite` for a point or a parameter that is not finite, `geom.wrong-count` for no triangles or for
    /// a different number of places from points, and `geom.triangle-not-valid` for a triangle naming a vertex there is
    /// not, not running anticlockwise with some area in the parameters, or overlapping an earlier one there, which it
    /// names as `other`.
    pub fn new(points: &[Point<World>], uv: Vec<UvPoint>, triangles: &[[u32; 3]]) -> Result<Self, Report> {
        checked_poles(points, None)?;
        if uv.len() != points.len() {
            return Err(wrong_count(uv.len(), points.len()));
        }
        if let Some(x) = uv.iter().flat_map(|p| [p.u(), p.v()]).find(|x| !x.is_finite()) {
            return Err(Report::new(Code::NotFinite, Fault::InvalidInput).with_value(NamedValue::new(
                "value",
                Quantity::Ratio,
                x,
            )));
        }
        if triangles.is_empty() {
            return Err(wrong_count(0, 1));
        }
        let points: Vec<Vec3> = points.iter().map(|p| p.to_vec3()).collect();
        let corner = |i: u32| usize::try_from(i).ok().and_then(|i| points.get(i).copied().zip(uv.get(i).copied()));
        let facet = |&[a, b, c]: &[u32; 3]| {
            let [(pa, ua), (pb, ub), (pc, uc)] = [corner(a)?, corner(b)?, corner(c)?];
            Facet::new([a, b, c], [ua, ub, uc], [pa, pb, pc])
        };
        let facets = (0..)
            .zip(triangles)
            .map(|(index, t)| facet(t).ok_or_else(|| bad_triangle(index)))
            .collect::<Result<Vec<Facet>, Report>>()?;
        let spans: Vec<UvBox> = facets.iter().map(|f| UvBox::new(f.span().0, f.span().1)).collect();
        let window = hull(&spans);
        let range = |lo: f64, hi: f64| ParamRange { lo, hi, wrap: Wrap::Open, kind: ParamKind::Knot };
        let domain = Domain { u: range(window.lo.u(), window.hi.u()), v: range(window.lo.v(), window.hi.v()) };
        let cells = Cells::new(window, &spans);
        if let Some((index, other)) = overlap(&facets, &spans, &cells) {
            let other = NamedValue::new("other", Quantity::Count, number(other));
            return Err(bad_triangle(index).with_value(other));
        }
        Ok(Self { points, uv, facets, cells, domain })
    }

    /// The first triangle holding `uv`, if any does.
    fn locate(&self, uv: UvPoint) -> Option<&Facet> {
        if !self.domain.contains(uv) {
            return None;
        }
        let candidates = self.cells.near(uv).iter().filter_map(|i| self.facets.get(usize::try_from(*i).ok()?));
        candidates.into_iter().find(|f| f.holds(uv))
    }

    /// The point at `uv` and its first derivatives, or NaN in a hole.
    fn jet(&self, uv: UvPoint) -> [Vec3; 3] {
        self.locate(uv).map_or([Vec3::new(f64::NAN, f64::NAN, f64::NAN); 3], |f| f.jet(uv))
    }

    /// The point and its first derivatives in each lane, each lane alone: lanes gain nothing here (S4: 0.80 times).
    #[inline(always)]
    fn rungs<R: Real>(&self, uv: UvPoint<R>) -> [Vec3<R>; 3] {
        const { assert!(R::LANES <= MAX_LANES, "more lanes than a lane-at-a-time evaluation holds") };
        let (us, vs) = (lanes(uv.u()), lanes(uv.v()));
        let none = Vec3::new(f64::NAN, f64::NAN, f64::NAN);
        let each: [[Vec3; 3]; MAX_LANES] = std::array::from_fn(|k| match (us.get(k), vs.get(k)) {
            (Some(&u), Some(&v)) if k < R::LANES => self.jet(UvPoint::new(u, v)),
            _ => [none; 3],
        });
        std::array::from_fn(|slot| from_lanes(|k| each.get(k).and_then(|e| e.get(slot)).copied().unwrap_or(none)))
    }
}

impl TriangleView for MeshSurface {
    fn triangle_count(&self) -> usize {
        self.facets.len()
    }

    fn triangle(&self, index: usize) -> Option<[Point<World>; 3]> {
        self.facets.get(index).map(|f| f.points.map(Point::from_vec3))
    }
}

impl Parametric for MeshSurface {
    #[inline(always)]
    fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R> {
        let [point, ..] = self.rungs(uv);
        Point::from_vec3(point)
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R> {
        let [point, du, dv] = self.rungs(uv);
        Jet1 { point: Point::from_vec3(point), du: Tangent::new(du), dv: Tangent::new(dv) }
    }

    /// The first derivatives, and NaN for the second, which a mesh does not have.
    #[inline(always)]
    fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R> {
        let none = Tangent::new(splat(Vec3::new(f64::NAN, f64::NAN, f64::NAN)));
        Jet2 { first: self.eval_first(uv), duu: none, duv: none, dvv: none }
    }

    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        _scratch: &mut Scratch,
    ) -> Result<(), Report> {
        if order >= 1 {
            let [_, du, dv] = self.jet(uv);
            out.extend([Tangent::new(du), Tangent::new(dv)]);
        }
        Ok(())
    }

    fn domain(&self) -> Domain {
        self.domain
    }

    fn highest_derivative(&self) -> Option<u8> {
        Some(1)
    }

    /// The box of the corners of the triangles whose places reach the patch, padded for rounding; all of space for a
    /// patch that is not finite or that no triangle reaches.
    fn bounds(&self, patch: UvBox) -> Bounds {
        let ((u0, u1), (v0, v1)) = (ordered(patch.lo.u(), patch.hi.u()), ordered(patch.lo.v(), patch.hi.v()));
        if ![u0, u1, v0, v1].into_iter().all(f64::is_finite) {
            return everything();
        }
        let reaching = self.facets.iter().filter(|f| {
            let (lo, hi) = f.span();
            lo.u() <= u1 && hi.u() >= u0 && lo.v() <= v1 && hi.v() >= v0
        });
        Bounds::of(reaching.flat_map(|f| f.points.map(Point::from_vec3)))
            .map_or_else(everything, |hull| padded(hull, 3.0))
    }

    /// The nearest point of every triangle, the first of those as near.
    fn invert(
        &self,
        p: Point<World>,
        _seed: Option<UvPoint>,
        _tol: Tolerance,
        _scratch: &mut Scratch,
    ) -> Result<UvPoint, Report> {
        let q = p.to_vec3();
        let start = (self.domain_corner(), f64::INFINITY);
        let (at, _) = self
            .facets
            .iter()
            .map(|f| f.nearest(q))
            .fold(start, |best, next| if next.1 < best.1 { next } else { best });
        Ok(at)
    }

    fn family(&self) -> FamilyName {
        FamilyName("mesh")
    }

    fn content(&self, out: &mut Content) {
        out.push(number(self.points.len()));
        out.push(number(self.facets.len()));
        for (p, uv) in self.points.iter().zip(&self.uv) {
            out.push_vec3(*p);
            out.push(uv.u());
            out.push(uv.v());
        }
        for corner in self.facets.iter().flat_map(|f| f.corners) {
            out.push(f64::from(corner));
        }
    }

    fn shape(&self) -> Shape {
        let count = |name: &str, n: usize| NamedValue::new(name, Quantity::Count, number(n));
        Shape {
            form: "mesh",
            values: vec![count("vertices", self.points.len()), count("triangles", self.facets.len())],
        }
    }

    fn triangles(&self) -> Option<&dyn TriangleView> {
        Some(self)
    }
}

impl MeshSurface {
    /// The domain's lowest corner.
    const fn domain_corner(&self) -> UvPoint {
        UvPoint::new(self.domain.u.lo, self.domain.v.lo)
    }
}

#[cfg(test)]
mod tests {
    use occam_math::Level;

    use super::*;
    use crate::batch::{Jets, Order};
    use crate::conformance::{self, Check, local, wrong_first};
    use crate::surface::Surface;

    fn at(u: f64, v: f64) -> UvPoint {
        UvPoint::new(u, v)
    }

    /// A grid of `nu` by `nv` cells over `u` from 0 to 1 and `v` from 0 to 2, each cut along its rising diagonal, on a
    /// curved sheet placed off the axes; without the two triangles of cell `hole`.
    #[expect(clippy::arithmetic_side_effects, reason = "small indices in a test")]
    fn grid(nu: u32, nv: u32, hole: Option<(u32, u32)>) -> MeshSurface {
        let place = |i: u32, j: u32| at(f64::from(i) / f64::from(nu), 2.0 * f64::from(j) / f64::from(nv));
        let point = |p: UvPoint| {
            let (u, v) = (p.u(), p.v());
            Point::from_vec3(local(4.0 * u + 0.3 * v * v, 3.0 * v - 0.5 * u, u * u - 0.5 * v + 0.3 * u * v))
        };
        let index = |i: u32, j: u32| j * (nu + 1) + i;
        let uv: Vec<UvPoint> = (0..=nv).flat_map(|j| (0..=nu).map(move |i| place(i, j))).collect();
        let points: Vec<Point<World>> = uv.iter().map(|p| point(*p)).collect();
        let cells = (0..nv).flat_map(|j| (0..nu).map(move |i| (i, j))).filter(|cell| Some(*cell) != hole);
        let triangles: Vec<[u32; 3]> = cells
            .flat_map(|(i, j)| {
                let (a, b, c, d) = (index(i, j), index(i + 1, j), index(i + 1, j + 1), index(i, j + 1));
                [[a, b, c], [a, c, d]]
            })
            .collect();
        MeshSurface::new(&points, uv, &triangles).unwrap()
    }

    fn window(lo: (f64, f64), hi: (f64, f64)) -> UvBox {
        UvBox::new(at(lo.0, lo.1), at(hi.0, hi.1))
    }

    #[test]
    fn the_mesh_passes_the_conformance_suite_to_its_first_derivative() {
        let s = grid(7, 5, None);
        for w in [window((0.0, 0.0), (1.0, 2.0)), window((0.1, 0.3), (0.8, 1.5))] {
            let failures = conformance::surface(&s, w);
            assert!(failures.is_empty(), "{w:?}: {failures:#?}");
        }
        let holed = grid(7, 5, Some((3, 2)));
        let failures = conformance::surface(&holed, window((0.0, 0.0), (0.4, 2.0)));
        assert!(failures.is_empty(), "beside a hole: {failures:#?}");
    }

    #[test]
    fn the_suite_catches_a_mesh_with_a_wrong_first_derivative() {
        let whole = window((0.0, 0.0), (1.0, 2.0));
        let failed = conformance::checks(&conformance::surface(&wrong_first(grid(7, 5, None)), whole));
        assert_eq!(failed, [Check::Orders, Check::Differences]);
    }

    #[test]
    fn a_mesh_has_no_derivative_above_the_first() {
        let s = grid(7, 5, None);
        let (uv, mut out, mut scratch) = (at(0.3, 0.7), vec![], Scratch::default());
        let refused = Surface::derivatives(&s, uv, 2, &mut out, &mut scratch).unwrap_err();
        let named = |name: &str| refused.value(name).and_then(NamedValue::number);
        assert_eq!((refused.code, named("order"), named("highest")), (Code::NoDerivative, Some(2.0), Some(1.0)));
        Surface::derivatives(&s, uv, 1, &mut out, &mut scratch).unwrap();
        let jet = s.second(uv);
        assert_eq!(out, [jet.first.du, jet.first.dv]);
        assert!([jet.duu, jet.duv, jet.dvv].iter().all(|t| t.rate().x.is_nan() && t.rate().z.is_nan()));
        assert_eq!(Surface::highest_derivative(&s), Some(1));
        let (mut jets, points) = (Jets::default(), [at(0.1, 0.1), at(0.5, 1.5), at(0.9, 1.9)]);
        Surface::eval_batch(&s, &points, Order::Second, &mut jets, &mut scratch, Level::baseline());
        assert_eq!(jets.defined, [false; 3], "a batch to the second order asks for what a mesh has not");
        Surface::eval_batch(&s, &points, Order::First, &mut jets, &mut scratch, Level::baseline());
        assert_eq!(jets.defined, [true; 3]);
    }

    #[test]
    fn a_point_in_a_hole_is_undefined_and_the_batch_says_where() {
        let s = grid(7, 5, Some((3, 2)));
        let hole = at(3.5 / 7.0, 1.0);
        assert!(s.point(hole).to_vec3().x.is_nan() && s.first(hole).du.rate().y.is_nan());
        let mut out = vec![];
        Surface::derivatives(&s, hole, 1, &mut out, &mut Scratch::default()).unwrap();
        assert!(out.iter().all(|t| t.rate().z.is_nan()), "inside the domain, so not refused, and NaN as the rungs are");
        let (mut jets, mut scratch) = (Jets::default(), Scratch::default());
        let points = [at(0.1, 0.1), hole, at(0.9, 1.9)];
        Surface::eval_batch(&s, &points, Order::First, &mut jets, &mut scratch, Level::baseline());
        assert_eq!((jets.defined.clone(), jets.first_undefined()), (vec![true, false, true], Some(1)));
        // Above the middle of the hole: its foot is on the hole's rim.
        let corners = [at(3.0 / 7.0, 0.8), at(4.0 / 7.0, 0.8), at(3.0 / 7.0, 1.2), at(4.0 / 7.0, 1.2)];
        let middle = corners.iter().map(|c| s.point(*c).to_vec3()).fold(Vec3::new(0.0, 0.0, 0.0), |a, p| a + p * 0.25);
        let foot =
            Surface::closest(&s, Point::from_vec3(middle), None, conformance::tolerance(), &mut scratch).unwrap();
        let rim = |x: f64, lo: f64, hi: f64| (x - lo).abs() < 1e-9 || (x - hi).abs() < 1e-9;
        assert!(rim(foot.at.u(), 3.0 / 7.0, 4.0 / 7.0) || rim(foot.at.v(), 0.8, 1.2), "{foot:?}");
        assert!(foot.point.to_vec3().x.is_finite() && foot.distance.millimetres() > 0.1, "{foot:?}");
    }

    #[test]
    fn a_point_on_an_edge_or_a_vertex_is_held() {
        let s = grid(7, 5, None);
        for i in 0..=14 {
            for j in 0..=10 {
                let on = at(f64::from(i) / 14.0, 2.0 * f64::from(j) / 10.0);
                assert!(s.point(on).to_vec3().x.is_finite(), "{on:?}");
            }
        }
        assert!(s.point(at(1.0 + 1e-9, 1.0)).to_vec3().x.is_nan(), "outside the domain");
    }

    #[test]
    fn a_flat_mesh_is_its_plane_exactly() {
        let corner = |x: f64, y: f64| Point::from_vec3(Vec3::new(x, y, 0.0));
        let points = [corner(0.0, 0.0), corner(2.0, 0.0), corner(2.0, 1.0), corner(0.0, 1.0)];
        let uv = vec![at(0.0, 0.0), at(2.0, 0.0), at(2.0, 1.0), at(0.0, 1.0)];
        let s = MeshSurface::new(&points, uv, &[[0, 1, 2], [0, 2, 3]]).unwrap();
        for p in [at(0.3, 0.7), at(1.9, 0.1), at(1.0, 0.5)] {
            let jet = s.first(p);
            assert_eq!(jet.point.to_vec3(), Vec3::new(p.u(), p.v(), 0.0));
            assert_eq!((jet.du.rate(), jet.dv.rate()), (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)));
        }
        let d = Surface::domain(&s);
        assert_eq!((d.u.lo, d.u.hi, d.v.lo, d.v.hi), (0.0, 2.0, 0.0, 1.0));
        assert_eq!((d.u.wrap, d.v.kind), (Wrap::Open, ParamKind::Knot));
    }

    #[test]
    fn a_mesh_with_the_wrong_numbers_is_refused() {
        let corner = |x: f64, y: f64| Point::from_vec3(Vec3::new(x, y, 0.0));
        let points = [corner(0.0, 0.0), corner(1.0, 0.0), corner(0.0, 1.0)];
        let uv = || vec![at(0.0, 0.0), at(1.0, 0.0), at(0.0, 1.0)];
        let refused = |points: &[Point<World>], uv: Vec<UvPoint>, triangles: &[[u32; 3]]| {
            MeshSurface::new(points, uv, triangles).map(|_| ()).unwrap_err()
        };
        let index = |report: Report| (report.code, report.value("index").and_then(NamedValue::number));
        assert_eq!(index(refused(&points, uv(), &[[0, 1, 2], [0, 1, 3]])), (Code::TriangleNotValid, Some(1.0)));
        assert_eq!(index(refused(&points, uv(), &[[0, 2, 1]])), (Code::TriangleNotValid, Some(0.0)), "clockwise");
        assert_eq!(index(refused(&points, uv(), &[[0, 1, 1]])), (Code::TriangleNotValid, Some(0.0)), "no area");
        assert_eq!(refused(&points, uv(), &[]).code, Code::WrongCount);
        assert_eq!(refused(&points, vec![at(0.0, 0.0)], &[[0, 1, 2]]).code, Code::WrongCount);
        let nan = vec![at(0.0, 0.0), at(f64::NAN, 0.0), at(0.0, 1.0)];
        assert_eq!(refused(&points, nan, &[[0, 1, 2]]).code, Code::NotFinite);
        let far = [points[0], points[1], Point::from_vec3(Vec3::new(0.0, f64::INFINITY, 0.0))];
        assert_eq!(refused(&far, uv(), &[[0, 1, 2]]).code, Code::NotFinite);
        assert!(MeshSurface::new(&points, uv(), &[[0, 1, 2]]).is_ok());
    }

    #[test]
    fn triangles_that_overlap_in_the_parameters_are_refused_and_both_named() {
        // A square of two triangles, and a third over part of the first: refused, naming it and the one it overlaps.
        let corner = |x: f64, y: f64| Point::from_vec3(Vec3::new(x, y, x * y));
        let places = [at(0.0, 0.0), at(2.0, 0.0), at(2.0, 2.0), at(0.0, 2.0), at(0.5, 0.2), at(1.5, 0.2), at(1.0, 0.9)];
        let points: Vec<Point<World>> = places.iter().map(|p| corner(p.u(), p.v())).collect();
        let square = [[0, 1, 2], [0, 2, 3]];
        assert!(MeshSurface::new(&points, places.to_vec(), &square).is_ok(), "two sharing an edge");
        let report = MeshSurface::new(&points, places.to_vec(), &[square[0], square[1], [4, 5, 6]]).unwrap_err();
        let named = |name: &str| report.value(name).and_then(NamedValue::number);
        assert_eq!((report.code, named("index"), named("other")), (Code::TriangleNotValid, Some(2.0), Some(0.0)));
        let twice = MeshSurface::new(&points, places.to_vec(), &[square[0], square[1], square[1]]).unwrap_err();
        assert_eq!(twice.value("other").and_then(NamedValue::number), Some(1.0), "a triangle listed twice");
    }

    #[test]
    fn it_presents_its_triangles_and_names_what_defines_it() {
        let s = grid(2, 1, None);
        let view = Surface::triangles(&s).unwrap();
        assert_eq!((view.triangle_count(), view.triangle(4)), (4, None));
        assert_eq!(view.triangle(1).map(|t| t[2]), Some(s.point(at(0.0, 2.0))));
        let shape = Surface::shape(&s);
        let numbers: Vec<Option<f64>> = shape.values.iter().map(NamedValue::number).collect();
        assert_eq!((shape.form, numbers), ("mesh", vec![Some(6.0), Some(4.0)]));
        let mut content = Content::default();
        Surface::content(&s, &mut content);
        assert_eq!(content.words().len(), 1 + 4 + 2 + 6 * 5 + 4 * 3);
        assert_eq!(content.words().last(), Some(&4f64.to_bits()), "the last triangle's last corner");
        assert_eq!(Surface::to_bspline(&s, window((0.0, 0.0), (1.0, 2.0))), None);
        assert_eq!(Surface::analytic(&s, conformance::tolerance()), None);
        let everywhere = Point::from_vec3(Vec3::new(1e300, -1e300, 0.0));
        assert!(Surface::bounds(&s, window((0.0, f64::NEG_INFINITY), (1.0, 2.0))).contains(everywhere));
        assert!(Surface::bounds(&s, window((5.0, 5.0), (6.0, 6.0))).contains(everywhere), "no triangle reaches it");
        let grid = grid(7, 5, None);
        let corner = Surface::bounds(&grid, window((0.0, 0.0), (0.1, 0.2)));
        assert!(corner.contains(grid.point(at(0.05, 0.1))) && !corner.contains(grid.point(at(1.0, 0.0))), "{corner:?}");
        let back_to_front = Surface::bounds(&s, window((1.0, 2.0), (0.0, 0.0)));
        assert!(!back_to_front.contains(everywhere), "{back_to_front:?}");
        assert!(
            [at(0.0, 0.0), at(1.0, 2.0), at(0.5, 1.0), at(0.0, 2.0)]
                .iter()
                .all(|uv| back_to_front.contains(s.point(*uv)))
        );
    }
}
