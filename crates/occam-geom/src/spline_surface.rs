//! Source: piegl-tiller-1997
//! The B-spline surface, rational or not, and the Bézier surface, which is one with a single span (PARITY 2.2):
//! `S(u, v) = sum N(i, p)(u) N(j, q)(v) w(i, j) P(i, j) / sum N(i, p)(u) N(j, q)(v) w(i, j)`. Its partial derivatives
//! are the basis's (A3.6), divided by Leibniz's rule where it is rational (A4.4). In lanes, a chunk of points on one span
//! reads that span's knots and poles once for every lane, which is where lanes gain; a chunk across spans is evaluated a
//! lane at a time, so that every lane has the bits of a single call. Its foot is searched for.

use occam_math::{Level, Real, width};
use occam_report::{NamedValue, Quantity, Report};
use occam_units::{Bounds, ParamKind, Point, Tangent, Tolerance, UvPoint, Vec3, World};

use crate::basis::{basis, rows_len, span};
use crate::batch::{Jet1, Jet2, Jets, Order, Scratch, eval_in_lanes};
use crate::bspline::{
    BsplineSurface, MAX_DEGREE, bezier_knots, cartesian, checked_knots, checked_poles, homogeneous, wrong_count,
};
use crate::domain::{Domain, ParamRange, UvBox, Wrap};
use crate::foot::foot;
use crate::identity::{BatchKey, Content, FamilyName, Shape};
use crate::knots::{Rows, canonical, transposed};
use crate::lanewise::{MAX_LANES, from_lanes, lanes};
use crate::spline::{CAPACITIES, index, surface_quotient, triangle};
use crate::support::{everything, padded};
use crate::surface::Parametric;

/// A B-spline or Bézier surface, rational or not.
#[derive(Clone, Debug, PartialEq)]
pub struct SplineSurface {
    degree: (usize, usize),
    count: (usize, usize),
    knots_u: Vec<f64>,
    knots_v: Vec<f64>,
    /// The homogeneous poles, `(w x, w y, w z, w)`, a row of constant `v` at a time.
    poles: Vec<[f64; 4]>,
    rational: bool,
    bezier: bool,
    domain: Domain,
}

/// A count as a number.
fn number(n: usize) -> f64 {
    f64::from(u32::try_from(n).unwrap_or(u32::MAX))
}

/// The range from knot `p` to knot `n`.
fn range(knots: &[f64], p: usize, n: usize) -> ParamRange {
    let at = |i: usize| knots.get(i).copied().unwrap_or(f64::NAN);
    ParamRange { lo: at(p), hi: at(n), wrap: Wrap::Open, kind: ParamKind::Knot }
}

/// `work` cut into pieces of `lengths`.
fn pieces<R, const N: usize>(mut work: &mut [R], lengths: [usize; N]) -> Option<[&mut [R]; N]> {
    let mut out: [&mut [R]; N] = std::array::from_fn(|_| <&mut [R]>::default());
    for (slot, length) in out.iter_mut().zip(lengths) {
        let (piece, rest) = std::mem::take(&mut work).split_at_mut_checked(length)?;
        (*slot, work) = (piece, rest);
    }
    Some(out)
}

/// The lower and the higher of two numbers.
fn ordered(a: f64, b: f64) -> (f64, f64) {
    if b < a { (b, a) } else { (a, b) }
}

impl SplineSurface {
    /// The surface `form` describes.
    ///
    /// # Errors
    ///
    /// `geom.out-of-range` for a degree outside 1 to 25, `geom.wrong-count` for too few poles or the wrong number of
    /// knots, poles or weights, `geom.knots-not-valid` for knots that decrease or repeat too often, `units.not-finite`
    /// for a knot or pole that is not finite, and `geom.weight-not-positive` for a weight that is not above zero.
    pub fn new(form: &BsplineSurface) -> Result<Self, Report> {
        let as_usize = |n: u32| usize::try_from(n).unwrap_or(usize::MAX);
        let (p, nu) = checked_knots(form.degree.0, as_usize(form.count.0), &form.knots_u)?;
        let (q, nv) = checked_knots(form.degree.1, as_usize(form.count.1), &form.knots_v)?;
        let needed = nu.saturating_mul(nv);
        if form.poles.len() != needed {
            return Err(wrong_count(form.poles.len(), needed));
        }
        checked_poles(&form.poles, form.weights.as_deref())?;
        let (knots_u, knots_v) = (canonical(&form.knots_u), canonical(&form.knots_v));
        let domain = Domain { u: range(&knots_u, p, nu), v: range(&knots_v, q, nv) };
        let poles = homogeneous(&form.poles, form.weights.as_deref());
        let rational = form.weights.is_some();
        Ok(Self { degree: (p, q), count: (nu, nv), knots_u, knots_v, poles, rational, bezier: false, domain })
    }

    /// The Bézier surface of `count.0` by `count.1` poles, a row of constant `v` at a time, and their weights, if it is
    /// rational: degree one less than the poles in each parameter, both from 0 to 1.
    ///
    /// # Errors
    ///
    /// As [`SplineSurface::new`]: a degree outside 1 to 25 is a count outside 2 to 26.
    pub fn bezier(count: (u32, u32), poles: Vec<Point<World>>, weights: Option<Vec<f64>>) -> Result<Self, Report> {
        let degree = |n: u32| u16::try_from(n.saturating_sub(1)).unwrap_or(u16::MAX);
        let (p, q) = (degree(count.0), degree(count.1));
        let knots = |d: u16| bezier_knots(usize::from(d.min(MAX_DEGREE)));
        let form = BsplineSurface { degree: (p, q), count, knots_u: knots(p), knots_v: knots(q), poles, weights };
        Ok(Self { bezier: true, ..Self::new(&form)? })
    }

    /// The spans holding `u` and `v`.
    fn spans(&self, u: f64, v: f64) -> (usize, usize) {
        let ((p, q), (nu, nv)) = (self.degree, self.count);
        (span(&self.knots_u, p, nu, u), span(&self.knots_v, q, nv, v))
    }

    /// How many numbers an evaluation to `order` works in: the basis's rows and derivatives in `u`, then in `v`, then
    /// the weight's partial derivatives.
    fn layout(&self, order: usize) -> [usize; 5] {
        let ((p, q), orders) = (self.degree, order.saturating_add(1));
        let ders = |d: usize| orders.saturating_mul(d.saturating_add(1));
        [rows_len(p, order), ders(p), rows_len(q, order), ders(q), triangle(order)]
    }

    /// Every partial derivative to `order` at `uv`, which lies on `span`, into `out` at [`index`]: the weighted form's
    /// sums of the basis's derivatives times the poles, then, where it is rational, their quotient. `work` holds
    /// [`SplineSurface::layout`]'s numbers.
    #[inline(always)]
    fn jets<R: Real>(&self, span: (usize, usize), uv: UvPoint<R>, order: usize, work: &mut [R], out: &mut [Vec3<R>]) {
        let Some([rows_u, ders_u, rows_v, ders_v, weights]) = pieces(work, self.layout(order)) else { return };
        basis(&self.knots_u, (self.degree.0, span.0), uv.u(), order, rows_u, ders_u);
        basis(&self.knots_v, (self.degree.1, span.1), uv.v(), order, rows_v, ders_v);
        if self.rational {
            self.sum::<R, true>(span, order, (ders_u, ders_v), out, weights);
            surface_quotient(order, out, weights);
        } else {
            self.sum::<R, false>(span, order, (ders_u, ders_v), out, weights);
        }
    }

    /// The weighted form's partial derivatives to `order` into `out`, and the weight's into `weights` where it is
    /// `RATIONAL`: for each row of poles on the span, the sum along `u` of each order's basis derivatives times the poles,
    /// then that times each order's basis derivative in `v`. Orders above a degree are zero and skipped.
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn sum<R: Real, const RATIONAL: bool>(
        &self,
        (su, sv): (usize, usize),
        order: usize,
        (ders_u, ders_v): (&[R], &[R]),
        out: &mut [Vec3<R>],
        weights: &mut [R],
    ) {
        let ((degree_u, degree_v), zero) = (self.degree, R::splat(0.0));
        out.fill(Vec3::new(zero, zero, zero));
        weights.fill(zero);
        let (wide_u, wide_v) = (degree_u.saturating_add(1), degree_v.saturating_add(1));
        let first_u = su.saturating_sub(degree_u);
        let parts = if RATIONAL { 4 } else { 3 };
        for (column, row_index) in (sv.saturating_sub(degree_v)..=sv).enumerate() {
            let start = row_index.saturating_mul(self.count.0).saturating_add(first_u);
            let row = self.poles.get(start..start.saturating_add(wide_u)).unwrap_or_default();
            for (in_u, along) in ders_u.chunks_exact(wide_u).take(order.min(degree_u).saturating_add(1)).enumerate() {
                let mut sums = [zero; 4];
                for (function, pole) in along.iter().zip(row) {
                    for (sum, part) in sums.iter_mut().zip(pole).take(parts) {
                        *sum = *sum + *function * R::splat(*part);
                    }
                }
                let [sum_x, sum_y, sum_z, sum_w] = sums;
                let orders_v = order.saturating_sub(in_u).min(degree_v).saturating_add(1);
                for (in_v, across) in ders_v.chunks_exact(wide_v).take(orders_v).enumerate() {
                    let (Some(&function), slot) = (across.get(column), index(in_u, in_v)) else { continue };
                    if let Some(jet) = out.get_mut(slot) {
                        *jet = *jet + Vec3::new(sum_x, sum_y, sum_z) * function;
                    }
                    if let (true, Some(weight)) = (RATIONAL, weights.get_mut(slot)) {
                        *weight = *weight + sum_w * function;
                    }
                }
            }
        }
    }

    /// [`SplineSurface::jets`] to `order` on the stack, with room for `M` functions of the basis.
    #[inline(always)]
    fn within<R: Real, const N: usize, const M: usize>(
        &self,
        span: (usize, usize),
        uv: UvPoint<R>,
        order: usize,
    ) -> [Vec3<R>; N] {
        let zero = R::splat(0.0);
        let mut work = [[zero; M]; 14];
        let mut out = [Vec3::new(zero, zero, zero); N];
        self.jets(span, uv, order, work.as_flattened_mut(), &mut out);
        out
    }

    /// [`SplineSurface::within`], with the least room the degrees need.
    #[inline(always)]
    fn at<R: Real, const N: usize>(&self, span: (usize, usize), uv: UvPoint<R>, order: usize) -> [Vec3<R>; N] {
        let [small, middle, _] = CAPACITIES;
        let widest = self.degree.0.max(self.degree.1);
        if widest < small {
            self.within::<R, N, 4>(span, uv, order)
        } else if widest < middle {
            self.within::<R, N, 8>(span, uv, order)
        } else {
            self.within::<R, N, 26>(span, uv, order)
        }
    }

    /// The point and its partial derivatives to `order`, `N` of them, at [`index`]: in lanes where every lane is on one
    /// span, and otherwise a lane at a time.
    #[inline(always)]
    fn rungs<R: Real, const N: usize>(&self, uv: UvPoint<R>, order: usize) -> [Vec3<R>; N] {
        const { assert!(R::LANES <= MAX_LANES, "more lanes than a lane-at-a-time evaluation holds") };
        let (us, vs) = (lanes(uv.u()), lanes(uv.v()));
        let mut spans = us.iter().zip(&vs).take(R::LANES).map(|(u, v)| self.spans(*u, *v));
        let first = spans.next().unwrap_or(self.degree);
        if spans.all(|s| s == first) {
            return self.at(first, uv, order);
        }
        let zero = Vec3::new(0.0, 0.0, 0.0);
        let each: [[Vec3; N]; MAX_LANES] = std::array::from_fn(|k| match (us.get(k), vs.get(k)) {
            (Some(&u), Some(&v)) if k < R::LANES => self.at::<f64, N>(self.spans(u, v), UvPoint::new(u, v), order),
            _ => [zero; N],
        });
        std::array::from_fn(|slot| from_lanes(|k| each.get(k).and_then(|e| e.get(slot)).copied().unwrap_or(zero)))
    }

    /// Where its foot is searched for: its domain, at a few samples across each span.
    fn search(&self) -> (UvBox, (u32, u32)) {
        let samples =
            |n: usize, p: usize| u32::try_from(n.saturating_add(p).saturating_mul(2).clamp(8, 128)).unwrap_or(8);
        let (d, (p, q), (nu, nv)) = (self.domain, self.degree, self.count);
        let window = UvBox::new(UvPoint::new(d.u.lo, d.v.lo), UvPoint::new(d.u.hi, d.v.hi));
        (window, (samples(nu, p), samples(nv, q)))
    }

    /// The poles of the spans that `patch` covers, as points.
    fn hull(&self, patch: UvBox) -> impl Iterator<Item = Point<World>> + '_ {
        let ((p, q), (u0, u1), (v0, v1)) =
            (self.degree, ordered(patch.lo.u(), patch.hi.u()), ordered(patch.lo.v(), patch.hi.v()));
        let (first, last) = (self.spans(u0, v0), self.spans(u1, v1));
        let columns = first.0.saturating_sub(p)..=last.0;
        (first.1.saturating_sub(q)..=last.1)
            .flat_map(move |j| {
                let row = j.saturating_mul(self.count.0);
                columns.clone().filter_map(move |i| self.poles.get(row.saturating_add(i)))
            })
            .map(|[x, y, z, w]| Point::from_vec3(Vec3::new(x / w, y / w, z / w)))
    }
}

impl Parametric for SplineSurface {
    #[inline(always)]
    fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R> {
        let [point] = self.rungs::<R, 1>(uv, 0);
        Point::from_vec3(point)
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R> {
        let [point, du, dv] = self.rungs::<R, 3>(uv, 1);
        Jet1 { point: Point::from_vec3(point), du: Tangent::new(du), dv: Tangent::new(dv) }
    }

    #[inline(always)]
    fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R> {
        let [point, du, dv, duu, duv, dvv] = self.rungs::<R, 6>(uv, 2);
        let first = Jet1 { point: Point::from_vec3(point), du: Tangent::new(du), dv: Tangent::new(dv) };
        Jet2 { first, duu: Tangent::new(duu), duv: Tangent::new(duv), dvv: Tangent::new(dvv) }
    }

    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        let order = usize::from(order);
        let len = self.layout(order).into_iter().fold(0, usize::saturating_add);
        let (work, jets) = scratch.reals_and_vectors(len, triangle(order));
        self.jets(self.spans(uv.u(), uv.v()), uv, order, work, jets);
        out.extend(jets.iter().skip(1).map(|v| Tangent::new(*v)));
        Ok(())
    }

    fn eval_batch(&self, uv: &[UvPoint], order: Order, out: &mut Jets, _scratch: &mut Scratch, level: Level) {
        eval_in_lanes::<{ width::SPLINE }, Self>(self, uv, order, out, level);
    }

    fn domain(&self) -> Domain {
        self.domain
    }

    /// The box of the poles of the spans the patch covers, which hold it where every weight is positive, padded for
    /// rounding. A patch outside the domain gives all of space.
    fn bounds(&self, patch: UvBox) -> Bounds {
        if !(self.domain.contains(patch.lo) && self.domain.contains(patch.hi)) {
            return everything();
        }
        let terms = number(self.degree.0.saturating_add(self.degree.1).saturating_add(2));
        Bounds::of(self.hull(patch)).map_or_else(everything, |hull| padded(hull, terms))
    }

    fn invert(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        _tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<UvPoint, Report> {
        let (window, steps) = self.search();
        Ok(foot(self, p, window, steps, seed, scratch))
    }

    fn family(&self) -> FamilyName {
        FamilyName("bspline")
    }

    fn batch_key(&self) -> BatchKey {
        let degree = |d: usize| u16::try_from(d).unwrap_or(u16::MAX);
        BatchKey { family: FamilyName("bspline"), degree: (degree(self.degree.0), degree(self.degree.1)) }
    }

    fn content(&self, out: &mut Content) {
        let ((p, q), (nu, nv)) = (self.degree, self.count);
        for x in [u8::from(self.bezier), u8::from(self.rational)] {
            out.push(f64::from(x));
        }
        for n in [p, q, nu, nv] {
            out.push(number(n));
        }
        for x in self.knots_u.iter().chain(&self.knots_v).chain(self.poles.iter().flatten()) {
            out.push(*x);
        }
    }

    fn shape(&self) -> Shape {
        let ((p, q), (nu, nv)) = (self.degree, self.count);
        let count = |name: &str, n: usize| NamedValue::new(name, Quantity::Count, number(n));
        let values = vec![
            count("u-degree", p),
            count("v-degree", q),
            count("u-poles", nu),
            count("v-poles", nv),
            count("rational", usize::from(self.rational)),
        ];
        Shape { form: if self.bezier { "bezier" } else { "bspline" }, values }
    }

    /// Itself from `patch.lo` to `patch.hi`, cut by knot insertion: in `u` along each row, then in `v` along each
    /// column.
    fn to_bspline(&self, patch: UvBox) -> Option<BsplineSurface> {
        let u = self.domain.u.trimmed(patch.lo.u(), patch.hi.u()).ok()?;
        let v = self.domain.v.trimmed(patch.lo.v(), patch.hi.v()).ok()?;
        let rows = self.poles.chunks(self.count.0.max(1)).map(<[[f64; 4]]>::to_vec).collect();
        let in_u = Rows { knots: self.knots_u.clone(), degree: self.degree.0, poles: rows }.restricted(u.lo, u.hi);
        let columns = Rows { knots: self.knots_v.clone(), degree: self.degree.1, poles: transposed(&in_u.poles) };
        let in_v = columns.restricted(v.lo, v.hi);
        let grid = transposed(&in_v.poles);
        let size = |n: usize| u32::try_from(n).unwrap_or(u32::MAX);
        let count = (size(grid.first().map_or(0, Vec::len)), size(grid.len()));
        let (poles, weights) = cartesian(&grid.concat(), self.rational);
        let degree = |d: usize| u16::try_from(d).unwrap_or(u16::MAX);
        let degree = (degree(self.degree.0), degree(self.degree.1));
        Some(BsplineSurface { degree, count, knots_u: in_u.knots, knots_v: in_v.knots, poles, weights })
    }
}

#[cfg(test)]
mod tests {
    use occam_math::Lanes;
    use occam_report::Code;

    use super::*;
    use crate::conformance::{self, Check, wrong_second};
    use crate::surface::Surface;

    fn point(x: f64, y: f64, z: f64) -> Point<World> {
        Point::from_vec3(Vec3::new(x, y, z))
    }

    /// A grid of `nu` by `nv` poles, regular but with nothing symmetric, a row of constant `v` at a time.
    fn poles(nu: u32, nv: u32) -> Vec<Point<World>> {
        let pole = |i: u32, j: u32| {
            let (x, y) = (f64::from(i), f64::from(j));
            let height = ((x * 7.0 + y * 3.0) % 5.0) * 0.3 - 0.6;
            point(1.7 * x + 0.1 * y * y + 10.0, 1.3 * y - 0.2 * x - 20.0, height + 5.0)
        };
        (0..nv).flat_map(|j| (0..nu).map(move |i| pole(i, j))).collect()
    }

    /// Weights from a half to two.
    fn weights(n: usize) -> Vec<f64> {
        [1.0, 0.5, 2.0, 1.25, 0.75].into_iter().cycle().take(n).collect()
    }

    fn form(degree: (u16, u16), knots: (Vec<f64>, Vec<f64>), rational: bool) -> BsplineSurface {
        let count = |knots: &[f64], d: u16| u32::try_from(knots.len().saturating_sub(usize::from(d) + 1)).unwrap();
        let count = (count(&knots.0, degree.0), count(&knots.1, degree.1));
        let poles = poles(count.0, count.1);
        let weights = rational.then(|| weights(poles.len()));
        BsplineSurface { degree, count, knots_u: knots.0, knots_v: knots.1, poles, weights }
    }

    /// Surfaces that between them have each degree's room, a rational form, a repeated knot inside and unclamped ends.
    fn surfaces() -> Vec<SplineSurface> {
        let forms = [
            form(
                (3, 3),
                (vec![0.0; 4].into_iter().chain([1.0; 4]).collect(), vec![0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 1.0, 1.0]),
                false,
            ),
            form((2, 2), (vec![0.0, 0.0, 0.0, 0.4, 0.4, 1.0, 1.0, 1.0], vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0]), true),
            form(
                (5, 2),
                (
                    vec![-1.0; 6].into_iter().chain([0.3, 0.6]).chain([2.0; 6]).collect(),
                    vec![0.0, 0.0, 0.0, 1.0, 2.0, 2.0, 2.0],
                ),
                true,
            ),
            form((9, 1), (vec![0.0; 10].into_iter().chain([1.0; 10]).collect(), vec![0.0, 0.0, 0.5, 1.0, 1.0]), false),
            form((2, 1), ((0..8).map(f64::from).collect(), vec![0.0, 0.0, 1.0, 1.0]), true),
        ];
        forms.iter().map(|f| SplineSurface::new(f).unwrap()).collect()
    }

    fn whole(s: &SplineSurface) -> UvBox {
        let d = s.domain;
        UvBox::new(UvPoint::new(d.u.lo, d.v.lo), UvPoint::new(d.u.hi, d.v.hi))
    }

    #[test]
    fn the_spline_surfaces_pass_the_conformance_suite() {
        for s in surfaces() {
            let window = whole(&s);
            let failures = conformance::surface(&s, window);
            assert!(failures.is_empty(), "{:?}: {failures:#?}", s.degree);
            let inner = UvBox::new(
                UvPoint::new(lerp(window, 0.2, 0), lerp(window, 0.1, 1)),
                UvPoint::new(lerp(window, 0.7, 0), lerp(window, 0.9, 1)),
            );
            let failures = conformance::surface(&s, inner);
            assert!(failures.is_empty(), "{:?} inside: {failures:#?}", s.degree);
        }
    }

    /// The parameter a fraction `f` across `window` in parameter `which`.
    fn lerp(window: UvBox, f: f64, which: u8) -> f64 {
        let (lo, hi) = if which == 0 { (window.lo.u(), window.hi.u()) } else { (window.lo.v(), window.hi.v()) };
        lo + (hi - lo) * f
    }

    #[test]
    fn the_suite_catches_a_spline_with_a_wrong_second_derivative() {
        let s = surfaces().swap_remove(1);
        let window = whole(&s);
        assert_eq!(
            conformance::checks(&conformance::surface(&wrong_second(s), window)),
            [Check::Orders, Check::Differences]
        );
    }

    #[test]
    fn a_polynomial_bezier_has_the_derivatives_of_its_formula() {
        // (u, v, u^2 v^2): x and y have Bernstein coefficients 0, 1/2, 1, and u^2 has 0, 0, 1.
        let pole =
            |i: u32, j: u32| point(f64::from(i) / 2.0, f64::from(j) / 2.0, if i == 2 && j == 2 { 1.0 } else { 0.0 });
        let s = SplineSurface::bezier((3, 3), (0..3).flat_map(|j| (0..3).map(move |i| pole(i, j))).collect(), None)
            .unwrap();
        let (u, v) = (0.3, 0.6);
        let z = |c: f64| Vec3::new(0.0, 0.0, c);
        let want = [
            Vec3::new(1.0, 0.0, 2.0 * u * v * v),
            Vec3::new(0.0, 1.0, 2.0 * u * u * v),
            z(2.0 * v * v),
            z(4.0 * u * v),
            z(2.0 * u * u),
            z(0.0),
            z(4.0 * v),
            z(4.0 * u),
            z(0.0),
            z(0.0),
            z(0.0),
            z(4.0),
            z(0.0),
            z(0.0),
        ];
        let mut out = vec![];
        Surface::derivatives(&s, UvPoint::new(u, v), 4, &mut out, &mut Scratch::default()).unwrap();
        assert_eq!(out.len(), want.len());
        for (k, (got, want)) in out.iter().zip(want).enumerate() {
            assert!((got.rate() - want).norm() < 1e-14, "{k}: {got:?} against {want:?}");
        }
        assert!((s.point(UvPoint::new(u, v)).to_vec3() - Vec3::new(u, v, u * u * v * v)).norm() < 1e-15);
        assert_eq!(Surface::shape(&s).form, "bezier");
    }

    #[test]
    fn a_rational_quarter_circle_swept_up_is_a_cylinder_exactly() {
        let w = 0.5f64.sqrt();
        let ring = [point(2.0, 0.0, 0.0), point(2.0, 2.0, 0.0), point(0.0, 2.0, 0.0)];
        let poles: Vec<Point<World>> =
            [0.0, 3.0].iter().flat_map(|z| ring.map(|p| point(p.to_vec3().x, p.to_vec3().y, *z))).collect();
        let s = SplineSurface::bezier((3, 2), poles, Some(vec![1.0, w, 1.0, 1.0, w, 1.0])).unwrap();
        for i in 0..=10 {
            for j in 0..=4 {
                let p = s.point(UvPoint::new(f64::from(i) / 10.0, f64::from(j) / 4.0)).to_vec3();
                assert!(
                    ((p.x * p.x + p.y * p.y).sqrt() - 2.0).abs() < 1e-15 && (p.z - 0.75 * f64::from(j)).abs() < 1e-15,
                    "{p:?}"
                );
            }
        }
    }

    #[test]
    fn lanes_across_a_knot_give_the_bits_of_single_calls() {
        // The second surface has a double knot at u = 0.4: these lanes fall either side of it, and then on one span.
        let s = surfaces().swap_remove(1);
        let single = |u: f64, v: f64| s.second(UvPoint::new(u, v));
        for us in [[0.35, 0.39, 0.4, 0.45], [0.41, 0.5, 0.6, 0.99]] {
            let jet: Jet2<Lanes<4>> = s.eval_second(UvPoint::new(Lanes::new(us), Lanes::splat(0.3)));
            for (lane, u) in us.into_iter().enumerate() {
                let want = single(u, 0.3);
                let got = [jet.first.point.to_vec3(), jet.first.du.rate(), jet.duv.rate()]
                    .map(|v| [v.x, v.y, v.z].map(|x| x.to_array()[lane].to_bits()));
                let want = [want.first.point.to_vec3(), want.first.du.rate(), want.duv.rate()]
                    .map(|v| [v.x, v.y, v.z].map(f64::to_bits));
                assert_eq!(got, want, "{u}");
            }
        }
    }

    #[test]
    fn a_part_keeps_the_points_and_parameters_it_had() {
        for s in surfaces() {
            let window = whole(&s);
            let patch = UvBox::new(
                UvPoint::new(lerp(window, 0.15, 0), lerp(window, 0.3, 1)),
                UvPoint::new(lerp(window, 0.8, 0), lerp(window, 0.55, 1)),
            );
            let part = SplineSurface::new(&Surface::to_bspline(&s, patch).unwrap()).unwrap();
            assert_eq!(whole(&part), patch);
            for (fu, fv) in [(0.0, 0.0), (0.3, 0.7), (0.5, 0.5), (1.0, 0.25), (1.0, 1.0)] {
                let uv = UvPoint::new(lerp(patch, fu, 0), lerp(patch, fv, 1));
                assert!((part.point(uv).to_vec3() - s.point(uv).to_vec3()).norm() < 1e-12, "{:?} at {uv:?}", s.degree);
            }
            assert_eq!(
                Surface::to_bspline(&s, UvBox::new(window.lo, UvPoint::new(window.hi.u() + 1.0, window.hi.v()))),
                None
            );
            assert_eq!(Surface::to_bspline(&s, UvBox::new(window.hi, window.lo)), None, "back to front");
        }
    }

    #[test]
    fn a_box_holds_its_patch_and_a_patch_outside_gives_all_of_space() {
        let s = surfaces().swap_remove(2);
        let window = whole(&s);
        assert!(conformance::slack(&s, window, 60) < 25.0, "the hull of the poles is near the surface");
        let outside = UvBox::new(window.lo, UvPoint::new(window.hi.u() + 0.5, window.hi.v()));
        assert!(Surface::bounds(&s, outside).contains(point(1e300, -1e300, 1e300)));
    }

    #[test]
    fn a_rational_box_is_its_poles_padded_and_a_patch_back_to_front_is_the_same() {
        // Poles from (0, 1, 2) to (3, 4, 5), weighted, so a pole read without its weight would move the box.
        let poles = vec![point(0.0, 1.0, 2.0), point(3.0, 1.5, 2.0), point(0.0, 4.0, 2.0), point(3.0, 4.0, 5.0)];
        let s = SplineSurface::bezier((2, 2), poles, Some(vec![1.0, 2.0, 1.0, 0.5])).unwrap();
        let b = Surface::bounds(&s, UvBox::new(UvPoint::new(0.0, 0.0), UvPoint::new(1.0, 1.0)));
        let margin = 64.0 * 4.0 * f64::EPSILON * 5.0;
        let (low, high) = (b.low().to_vec3(), b.high().to_vec3());
        for (got, want) in [(low.x, 0.0), (low.y, 1.0), (low.z, 2.0), (high.x, 3.0), (high.y, 4.0), (high.z, 5.0)] {
            let rounding = 4.0 * f64::EPSILON * want.abs();
            assert!(((got - want).abs() - margin).abs() <= 1e-3 * margin + rounding, "{got} against {want}: {b:?}");
        }
        let many = surfaces().swap_remove(1);
        let patch = UvBox::new(UvPoint::new(0.2, 0.1), UvPoint::new(0.9, 0.8));
        assert_eq!(
            Surface::bounds(&many, UvBox::new(patch.hi, patch.lo)),
            Surface::bounds(&many, patch),
            "back to front"
        );
    }

    #[test]
    fn the_stack_has_room_for_every_degree_at_its_edges() {
        // Degrees 4, 8 and 9 sit at the edges of the room the rungs make on the stack: their bits are `derivatives'`.
        for degree in [4_u32, 8, 9] {
            let n = degree + 1;
            let s = SplineSurface::bezier((n, n), poles(n, n), Some(weights((n * n) as usize))).unwrap();
            let (uv, mut out) = (UvPoint::new(0.3, 0.6), vec![]);
            Surface::derivatives(&s, uv, 2, &mut out, &mut Scratch::default()).unwrap();
            let jet = s.second(uv);
            assert_eq!(out, [jet.first.du, jet.first.dv, jet.duu, jet.duv, jet.dvv], "degree {degree}");
            assert!(jet.first.point.to_vec3().norm() > 1.0, "degree {degree}");
        }
    }

    #[test]
    fn a_surface_with_the_wrong_numbers_is_refused() {
        let good = form((2, 2), (vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0], vec![0.0, 0.0, 0.0, 1.0, 1.0, 1.0]), true);
        let code = |f: BsplineSurface| SplineSurface::new(&f).unwrap_err().code;
        assert_eq!(code(BsplineSurface { poles: poles(3, 2), ..good.clone() }), Code::WrongCount);
        assert_eq!(code(BsplineSurface { knots_v: vec![0.0, 0.0, 0.0, 1.0, 1.0], ..good.clone() }), Code::WrongCount);
        assert_eq!(
            code(BsplineSurface { knots_u: vec![0.0, 0.0, 1.0, 0.5, 1.0, 1.0], ..good.clone() }),
            Code::KnotsNotValid
        );
        assert_eq!(code(BsplineSurface { degree: (2, 0), ..good.clone() }), Code::OutOfRange);
        assert_eq!(code(BsplineSurface { weights: Some(vec![1.0; 8]), ..good.clone() }), Code::WrongCount);
        assert_eq!(code(BsplineSurface { weights: Some(vec![-1.0; 9]), ..good.clone() }), Code::WeightNotPositive);
        let mut far = good.clone();
        far.poles[4] = point(f64::NAN, 0.0, 0.0);
        assert_eq!(code(far), Code::NotFinite);
        assert!(SplineSurface::new(&good).is_ok());
        assert_eq!(SplineSurface::bezier((27, 2), poles(27, 2), None).unwrap_err().code, Code::OutOfRange);
        assert_eq!(SplineSurface::bezier((1, 2), poles(1, 2), None).unwrap_err().code, Code::OutOfRange);
    }

    #[test]
    fn its_shape_content_and_key_name_what_defines_it() {
        let s = surfaces().swap_remove(2);
        let shape = Surface::shape(&s);
        let numbers: Vec<Option<f64>> = shape.values.iter().map(NamedValue::number).collect();
        assert_eq!((shape.form, numbers), ("bspline", vec![Some(5.0), Some(2.0), Some(8.0), Some(4.0), Some(1.0)]));
        assert_eq!(Surface::batch_key(&s), BatchKey { family: FamilyName("bspline"), degree: (5, 2) });
        let mut content = Content::default();
        Surface::content(&s, &mut content);
        let (knots, poles) = (14 + 7, 8 * 4 * 4);
        assert_eq!(content.words().len(), 1 + 7 + 2 + 4 + knots + poles);
        let mut other = Content::default();
        Surface::content(&SplineSurface { rational: false, ..s.clone() }, &mut other);
        assert_ne!(content, other, "whether it is rational is part of what it is");
        assert_eq!((Surface::degeneracies(&s), Surface::analytic(&s, conformance::tolerance())), (&[][..], None));
    }
}
