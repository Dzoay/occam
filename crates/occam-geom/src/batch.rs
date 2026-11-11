//! Source: original
//! One surface at many points (P9): the shape of P1's sampling, P9's refinement and P12's dirty rectangles. The caller
//! owns the outputs and the workspace, so nothing allocates once they have grown (S4: degree 5 by 4 allocated on every
//! call), and the lanes run through `dispatch`, which cannot cross `dyn` (P4).

use occam_math::{LaneKernel, Lanes, Level, Real, dispatch};
use occam_units::{Point, Tangent, UvPoint, Vec3, World};

use crate::surface::Parametric;

/// How far an evaluation goes. Nothing beyond the order asked for is computed (P8 item 1: 33 to 34% of whole-run time
/// on the two workloads timed).
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Order {
    /// The point alone.
    Point,
    /// The point and the first derivatives.
    First,
    /// The point and the first and second derivatives.
    Second,
}

/// A point and its first derivatives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jet1<R: Real = f64> {
    /// The point.
    pub point: Point<World, R>,
    /// The derivative in `u`.
    pub du: Tangent<World, R>,
    /// The derivative in `v`.
    pub dv: Tangent<World, R>,
}

/// A point and its first and second derivatives.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Jet2<R: Real = f64> {
    /// The point and the first derivatives.
    pub first: Jet1<R>,
    /// The second derivative in `u`.
    pub duu: Tangent<World, R>,
    /// The mixed second derivative.
    pub duv: Tangent<World, R>,
    /// The second derivative in `v`.
    pub dvv: Tangent<World, R>,
}

/// A batch's outputs, owned by the caller. Each call clears them and writes only up to the order asked for, so the
/// others are empty.
#[derive(Clone, Debug, Default)]
pub struct Jets {
    /// The points.
    pub points: Vec<Point<World>>,
    /// The derivatives in `u`.
    pub du: Vec<Tangent<World>>,
    /// The derivatives in `v`.
    pub dv: Vec<Tangent<World>>,
    /// The second derivatives in `u`.
    pub duu: Vec<Tangent<World>>,
    /// The mixed second derivatives.
    pub duv: Vec<Tangent<World>>,
    /// The second derivatives in `v`.
    pub dvv: Vec<Tangent<World>>,
    /// Whether each point is defined: inside the domain, with every number written for it finite. L6's result per item,
    /// which S4 could reach only by scanning for NaN.
    pub defined: Vec<bool>,
}

impl Jets {
    /// The first point that is not defined.
    #[must_use]
    pub fn first_undefined(&self) -> Option<usize> {
        self.defined.iter().position(|defined| !defined)
    }

    fn clear(&mut self) {
        self.points.clear();
        for rates in [&mut self.du, &mut self.dv, &mut self.duu, &mut self.duv, &mut self.dvv] {
            rates.clear();
        }
        self.defined.clear();
    }

    /// Whether every number written for point `index` is finite.
    fn finite_at(&self, index: usize) -> bool {
        let finite = |v: Vec3| v.x.is_finite() && v.y.is_finite() && v.z.is_finite();
        let point = self.points.get(index).is_none_or(|p| finite(p.to_vec3()));
        let rates = [&self.du, &self.dv, &self.duu, &self.duv, &self.dvv];
        point && rates.into_iter().all(|rates| rates.get(index).is_none_or(|t| finite(t.rate())))
    }
}

/// The caller's workspace, so that no evaluation allocates once it has grown.
#[derive(Clone, Debug, Default)]
pub struct Scratch {
    reals: Vec<f64>,
}

impl Scratch {
    /// `len` numbers to work in, all zero. The space grows only when a call needs more than any before it.
    pub fn reals(&mut self, len: usize) -> &mut [f64] {
        self.reals.clear();
        self.reals.resize(len, 0.0);
        &mut self.reals
    }
}

/// Evaluates `surface` at every point of `uv` up to `order`, `W` points at a time and compiled for `level`. The bits
/// are those of single calls at every width and level. Every family's batch runs here, at width 1 until the family has
/// measured a gain at another (L29).
pub fn eval_in_lanes<const W: usize, S: Parametric>(
    surface: &S,
    uv: &[UvPoint],
    order: Order,
    out: &mut Jets,
    level: Level,
) {
    const { assert!(W > 0, "a batch runs at least one lane") };
    out.clear();
    dispatch(level, Batch::<W, S> { surface, uv, order, out: &mut *out });
    let domain = surface.domain();
    let mut defined = std::mem::take(&mut out.defined);
    defined.extend(uv.iter().zip(0..).map(|(at, index)| domain.contains(*at) && out.finite_at(index)));
    out.defined = defined;
}

/// The lane kernel of [`eval_in_lanes`].
struct Batch<'a, const W: usize, S> {
    surface: &'a S,
    uv: &'a [UvPoint],
    order: Order,
    out: &'a mut Jets,
}

impl<const W: usize, S: Parametric> LaneKernel for Batch<'_, W, S> {
    type Output = ();

    #[inline(always)]
    fn run(self) {
        let (surface, out) = (self.surface, self.out);
        for chunk in self.uv.chunks(W) {
            let at = UvPoint::new(gather::<W>(chunk, UvPoint::u), gather::<W>(chunk, UvPoint::v));
            let count = chunk.len();
            let point = |into: &mut Vec<Point<World>>, p: Point<World, Lanes<W>>| {
                scatter(into, count, p.to_vec3(), Point::from_vec3);
            };
            let rate = |into: &mut Vec<Tangent<World>>, t: Tangent<World, Lanes<W>>| {
                scatter(into, count, t.rate(), Tangent::new);
            };
            match self.order {
                Order::Point => point(&mut out.points, surface.eval_point(at)),
                Order::First => {
                    let jet = surface.eval_first(at);
                    point(&mut out.points, jet.point);
                    rate(&mut out.du, jet.du);
                    rate(&mut out.dv, jet.dv);
                }
                Order::Second => {
                    let jet = surface.eval_second(at);
                    point(&mut out.points, jet.first.point);
                    rate(&mut out.du, jet.first.du);
                    rate(&mut out.dv, jet.first.dv);
                    rate(&mut out.duu, jet.duu);
                    rate(&mut out.duv, jet.duv);
                    rate(&mut out.dvv, jet.dvv);
                }
            }
        }
    }
}

/// One parameter of up to `W` points in lanes, the last repeated into any lanes left over.
#[inline(always)]
fn gather<const W: usize>(chunk: &[UvPoint], part: fn(UvPoint) -> f64) -> Lanes<W> {
    let pad = chunk.last().map_or(0.0, |p| part(*p));
    let mut values = chunk.iter().map(|p| part(*p));
    Lanes::from_fn(|| values.next().unwrap_or(pad))
}

/// The first `count` lanes of `v`, each made into a `T` and added to `into`.
#[inline(always)]
fn scatter<const W: usize, T>(into: &mut Vec<T>, count: usize, v: Vec3<Lanes<W>>, make: fn(Vec3) -> T) {
    let (x, y, z) = (v.x.to_array(), v.y.to_array(), v.z.to_array());
    into.extend(x.into_iter().zip(y).zip(z).take(count).map(|((x, y), z)| make(Vec3::new(x, y, z))));
}

#[cfg(test)]
mod tests {
    use occam_units::{Local, Placement, Vector};

    use super::*;
    use crate::conformance::placement;
    use crate::plane::Plane;
    use crate::surface::Surface;

    fn plane() -> Plane {
        Plane::placed(&placement())
    }

    fn points() -> Vec<UvPoint> {
        (0..13u32).map(|i| UvPoint::new(f64::from(i) * 1.7 - 9.0, 4.0 - f64::from(i) * 0.9)).collect()
    }

    fn bits(rates: &[Tangent<World>]) -> Vec<[u64; 3]> {
        rates.iter().map(|t| [t.rate().x.to_bits(), t.rate().y.to_bits(), t.rate().z.to_bits()]).collect()
    }

    fn point_bits(points: &[Point<World>]) -> Vec<[u64; 3]> {
        let rates: Vec<Tangent<World>> = points.iter().map(|p| Tangent::new(p.to_vec3())).collect();
        bits(&rates)
    }

    fn batch<const W: usize>(order: Order) -> Jets {
        let mut out = Jets::default();
        eval_in_lanes::<W, Plane>(&plane(), &points(), order, &mut out, Level::baseline());
        out
    }

    #[test]
    fn every_width_gives_the_bits_of_single_calls() {
        let s = plane();
        let singles: Vec<_> = points().into_iter().map(|uv| s.second(uv)).collect();
        let want_points: Vec<_> = singles.iter().map(|j| j.first.point).collect();
        let want_duv: Vec<_> = singles.iter().map(|j| j.duv).collect();
        for out in [batch::<1>(Order::Second), batch::<2>(Order::Second), batch::<3>(Order::Second)] {
            assert_eq!(point_bits(&out.points), point_bits(&want_points));
            assert_eq!(bits(&out.duv), bits(&want_duv));
            assert_eq!(out.defined, vec![true; 13]);
        }
        for out in [batch::<4>(Order::Second), batch::<5>(Order::Second), batch::<8>(Order::Second)] {
            assert_eq!(point_bits(&out.points), point_bits(&want_points));
            assert_eq!(bits(&out.duv), bits(&want_duv));
        }
    }

    #[test]
    fn a_batch_writes_only_up_to_its_order() {
        let mut out = batch::<4>(Order::Second);
        eval_in_lanes::<4, Plane>(&plane(), &points(), Order::Point, &mut out, Level::baseline());
        assert_eq!((out.points.len(), out.du.len(), out.dvv.len(), out.defined.len()), (13, 0, 0, 13));
        eval_in_lanes::<4, Plane>(&plane(), &points(), Order::First, &mut out, Level::baseline());
        assert_eq!((out.points.len(), out.du.len(), out.dv.len(), out.duu.len()), (13, 13, 13, 0));
        assert_eq!(bits(&out.dv), bits(&vec![plane().first(UvPoint::new(0.0, 0.0)).dv; 13]));
    }

    #[test]
    fn a_point_outside_the_domain_or_not_finite_is_undefined_and_named() {
        let mut uv = points();
        uv.insert(6, UvPoint::new(f64::NAN, 0.0));
        uv.push(UvPoint::new(f64::MAX, 0.0));
        let shift = Placement::<World, World>::translation(Vector::from_vec3(Vec3::new(1e308, 0.0, 0.0)));
        let far = Plane::placed(&Placement::<Local, World>::identity().then(&shift));
        let mut out = Jets::default();
        eval_in_lanes::<4, Plane>(&far, &uv, Order::Point, &mut out, Level::baseline());
        let undefined: Vec<usize> = out.defined.iter().zip(0..).filter(|(d, _)| !**d).map(|(_, i)| i).collect();
        assert_eq!(undefined, [6, 14], "outside the domain, then inside it but past the largest double");
        assert_eq!(out.first_undefined(), Some(6));
        assert_eq!(batch::<4>(Order::Point).first_undefined(), None);
    }

    #[test]
    fn a_derivative_that_is_not_finite_makes_its_point_undefined() {
        let mut out = Jets::default();
        out.points.push(Point::from_vec3(Vec3::new(1.0, 2.0, 3.0)));
        out.du.push(Tangent::new(Vec3::new(0.0, f64::INFINITY, 0.0)));
        out.dvv.push(Tangent::new(Vec3::new(0.0, 0.0, 1.0)));
        out.points.push(Point::from_vec3(Vec3::new(1.0, 2.0, 3.0)));
        out.du.push(Tangent::new(Vec3::new(0.0, 1.0, 0.0)));
        out.dvv.push(Tangent::new(Vec3::new(0.0, 0.0, f64::NAN)));
        out.points.push(Point::from_vec3(Vec3::new(1.0, 2.0, 3.0)));
        assert_eq!([out.finite_at(0), out.finite_at(1), out.finite_at(2)], [false, false, true]);
        out.points.push(Point::from_vec3(Vec3::new(0.0, 0.0, f64::NEG_INFINITY)));
        assert!(!out.finite_at(3));
    }

    #[test]
    fn scratch_gives_zeroed_space_of_the_length_asked() {
        let mut scratch = Scratch::default();
        scratch.reals(3).copy_from_slice(&[1.0, 2.0, 3.0]);
        assert_eq!(scratch.reals(2), [0.0, 0.0]);
        assert_eq!(scratch.reals(4).len(), 4);
    }
}
