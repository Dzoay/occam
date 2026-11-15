//! Source: original
//! [`Image`]: a curve in space read in a surface's parameters through the surface's [`Chart`], the closed form of its
//! parameters, for the images of section curves (P5). It is exact, a closed form evaluated in floating point, and shares
//! the curve's parameter: the image at `t` is the parameters of the curve's point at `t`. Its first two rates are closed
//! forms of the curve's; every higher one comes from Taylor series of the same forms.
//!
//! An angle about a chart's axis is kept continuous, so that the image does not jump where the surface's parameter
//! comes round. A curve beside the axis keeps within a half turn of the direction of a point given. A conic round the
//! axis is followed in two steps that each stay inside a half turn: the direction from its centre, within a quarter turn
//! of an angle turning with its parameter, and the direction from the axis, short of a half turn from that. Such a
//! curve comes round in space and not in the parameters, so its image's range does not come round: only a whole curve
//! beside the axis, whose ends meet in the parameters, keeps a range that comes round. An arc whose ends meet in space
//! meets itself only where the parameters are undefined, at a pole, so its image's range is open. A curve through a
//! sphere's pole or a cone's apex jumps there in the parameters, so it is cut there first, into arcs that end there.

use std::f64::consts::{FRAC_PI_2, PI, TAU};

use occam_math::{Real, atan2, sin_cos};
use occam_report::Report;
use occam_units::{Point, UvPoint, UvVector, Vec3, World};

use crate::batch::Scratch;
use crate::chart::{Chart, Form, Meridian};
use crate::curve::ParametricCurve;
use crate::domain::{ParamMap, ParamRange, UvBox, UvMap, Wrap};
use crate::identity::{Content, FamilyName, Shape};
use crate::nearest::nearest_by;
use crate::param_curve::{ParametricParamCurve, UvClosest};
use crate::restore::wrap_code;
use crate::series;

/// How the angle about a turned chart's axis is kept continuous.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Turn {
    /// The curve keeps to one side of the axis: its angle is within a half turn of `at`.
    Beside { at: f64 },
    /// The curve goes once round the axis: the direction to it from `centre`, its centre in the chart's frame across
    /// the axis, is within a quarter turn of `at + rate t`, and `rate` is 1 or −1.
    Round { at: f64, rate: f64, centre: (f64, f64) },
}

/// A curve in space as a curve in a surface's parameters, through the surface's chart.
#[derive(Clone, Debug, PartialEq)]
pub struct Image<C> {
    curve: C,
    chart: Chart,
    turn: Turn,
    /// On a cone, whether the curve is on the nappe below the apex, where the angle is that of the point's opposite.
    below: bool,
    /// On a torus, the tube angle the image's `v` is within a half turn of.
    tube: f64,
    /// How the parameters have moved since: the image is this map of the chart's parameters.
    after: UvMap,
    range: ParamRange,
}

/// `v` in every lane.
#[inline(always)]
fn splat<R: Real>(v: Vec3) -> Vec3<R> {
    Vec3::new(R::splat(v.x), R::splat(v.y), R::splat(v.z))
}

/// The angle of `(x, y)` measured from the direction at the angle `from`, from minus to plus a half turn.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
fn angle_from<R: Real>(from: R, x: R, y: R) -> R {
    let (sin, cos) = sin_cos(from);
    atan2(y * cos - x * sin, x * cos + y * sin)
}

/// The first and second rates of the angle of `(x, y)`, from theirs.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
fn angle_rates<R: Real>([x, x1, x2]: [R; 3], [y, y1, y2]: [R; 3]) -> (R, R) {
    let square = x * x + y * y;
    let turning = x * y1 - y * x1;
    let growing = R::splat(2.0) * (x * x1 + y * y1);
    (turning / square, (x * y2 - y * x2) / square - turning * growing / (square * square))
}

/// The distance of `(x, y)` from the origin and its first and second rates, from theirs.
#[inline(always)]
#[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
fn radial_rates<R: Real>([x, x1, x2]: [R; 3], [y, y1, y2]: [R; 3]) -> [R; 3] {
    let rho = (x * x + y * y).sqrt();
    let rate = (x * x1 + y * y1) / rho;
    [rho, rate, (x1 * x1 + y1 * y1 + x * x2 + y * y2 - rate * rate) / rho]
}

/// Whether `p` is on the nappe below a cone's apex.
fn below(chart: &Chart, p: Vec3) -> bool {
    matches!(chart.0, Form::Turned { origin, z, meridian: Meridian::Slanted { .. }, .. } if (p - origin).dot(z) < 0.0)
}

/// `p`'s angle about a turned chart's axis, turned to its opposite below a cone's apex; 0 on a flat chart.
fn angle_of(chart: &Chart, p: Vec3, below: bool) -> f64 {
    let Form::Turned { origin, x, y, .. } = chart.0 else { return 0.0 };
    let side = if below { -1.0 } else { 1.0 };
    atan2(side * (p - origin).dot(y), side * (p - origin).dot(x))
}

/// A torus's tube angle at `p`; 0 on any other chart.
fn tube(chart: &Chart, p: Vec3) -> f64 {
    let Form::Turned { origin, x, y, z, meridian: Meridian::Tube { major, .. } } = chart.0 else { return 0.0 };
    let q = p - origin;
    atan2(q.dot(z), (q.dot(x) * q.dot(x) + q.dot(y) * q.dot(y)).sqrt() - major)
}

/// How the conic `centre + a cos t + b sin t` goes round a turned chart's axis, where the axis passes inside it: where
/// the axis meets the conic's plane is `α a + β b` from its centre, inside where `α² + β² < 1`. An axis along the plane
/// meets it nowhere, or at infinity, which is not inside.
fn round(chart: &Chart, (centre, first, second): (Vec3, Vec3, Vec3), below: bool) -> Option<Turn> {
    let Form::Turned { origin, x, y, z, .. } = chart.0 else { return None };
    let normal = first.cross(second);
    let meet = origin + z * ((centre - origin).dot(normal) / z.dot(normal)) - centre;
    let square = normal.dot(normal);
    let (along_a, along_b) = (meet.cross(second).dot(normal) / square, first.cross(meet).dot(normal) / square);
    let inside = along_a * along_a + along_b * along_b < 1.0;
    if !inside {
        return None;
    }
    let side = if below { -1.0 } else { 1.0 };
    let offset = centre - origin;
    let rate = if normal.dot(x.cross(y)) < 0.0 { -1.0 } else { 1.0 };
    let at = atan2(side * first.dot(y), side * first.dot(x));
    Some(Turn::Round { at, rate, centre: (side * offset.dot(x), side * offset.dot(y)) })
}

/// `outer` after `inner`.
fn then(inner: UvMap, outer: UvMap) -> UvMap {
    let row = |[by_u, by_v, shift]: [f64; 3]| {
        let mix =
            |i: usize| by_u * inner.u.get(i).copied().unwrap_or(0.0) + by_v * inner.v.get(i).copied().unwrap_or(0.0);
        [mix(0), mix(1), mix(2) + shift]
    };
    UvMap { u: row(outer.u), v: row(outer.v) }
}

/// The smaller and larger of `values`.
fn extremes(values: impl Iterator<Item = f64>) -> (f64, f64) {
    let pick = |(lo, hi): (f64, f64), x: f64| (if x < lo { x } else { lo }, if x > hi { x } else { hi });
    values.fold((f64::INFINITY, f64::NEG_INFINITY), pick)
}

/// `(lo, hi)` widened by a part in a trillion and a little more, which holds any rounding of the values between.
fn padded((lo, hi): (f64, f64)) -> (f64, f64) {
    let pad = |x: f64| 1e-12 * (1.0 + x.abs());
    (lo - pad(lo), hi + pad(hi))
}

fn flat(uv: UvPoint) -> Vec3 {
    Vec3::new(uv.u(), uv.v(), 0.0)
}

impl<C: ParametricCurve + Clone> Image<C> {
    /// The image through `chart` of `curve`, which keeps to one side of a turned chart's axis: within a half turn of
    /// the angle of `toward` about it. An open curve keeps to one side, a closed one the axis does not pass inside, and an
    /// arc that ends at a sphere's pole.
    #[must_use]
    pub fn beside(curve: C, chart: Chart, toward: Point<World>) -> Self {
        let at = angle_of(&chart, toward.to_vec3(), below(&chart, toward.to_vec3()));
        Self::made(curve, chart, Turn::Beside { at }, toward.to_vec3())
    }

    /// The image through `chart` of `curve`, a conic `c + a cos t + b sin t` as a circle or an ellipse is: round a
    /// turned chart's axis where the axis passes inside it, and else beside it, within a half turn of its centre's
    /// angle. `None` where `curve` is not of that form, to a part in 10^12 of its size.
    #[must_use]
    pub fn conic(curve: C, chart: Chart) -> Option<Self> {
        let at = |t: f64| curve.eval_point(t).to_vec3();
        let (start, centre) = (at(0.0), (at(0.0) + at(PI)) * 0.5);
        let (a, b) = (start - centre, at(FRAC_PI_2) - centre);
        let size = centre.norm() + a.norm() + b.norm();
        let fits = [1.0, 2.5, 4.0].into_iter().all(|t: f64| {
            let (sin, cos) = sin_cos(t);
            (at(t) - (centre + a * cos + b * sin)).norm() <= 1e-12 * size
        });
        if !fits {
            return None;
        }
        let low = below(&chart, start);
        let turn = round(&chart, (centre, a, b), low).unwrap_or(Turn::Beside { at: angle_of(&chart, centre, low) });
        Some(Self::made(curve, chart, turn, start))
    }

    fn made(curve: C, chart: Chart, turn: Turn, reference: Vec3) -> Self {
        let (low, tube) = (below(&chart, reference), tube(&chart, reference));
        let range = curve.range();
        let mut image = Self { curve, chart, turn, below: low, tube, after: UvMap::IDENTITY, range };
        // Beside the axis the image is the point's parameters alone, so a whole periodic curve's ends meet there too.
        let comes_round = matches!(turn, Turn::Beside { .. }) && range.wrap == Wrap::Periodic;
        if range.wrap != Wrap::Open && !comes_round {
            image.range = ParamRange { wrap: Wrap::Open, ..range };
        }
        image
    }

    /// `q`, a point less the chart's origin or a rate of the curve, in the frame `(x, y, z)`: across the axis and
    /// along it, turned to the point's opposite below a cone's apex.
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn local<R: Real>(&self, q: Vec3<R>, (x, y, z): (Vec3, Vec3, Vec3)) -> [R; 3] {
        let side = R::splat(if self.below { -1.0 } else { 1.0 });
        [q.dot(splat(x)) * side, q.dot(splat(y)) * side, q.dot(splat(z))]
    }

    /// The chart's parameters of the curve's `point` at `t`, before the image has moved.
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn place<R: Real>(&self, t: R, point: Vec3<R>) -> UvPoint<R> {
        let (origin, frame, meridian) = match self.chart.0 {
            Form::Flat { origin, at, dual, .. } => {
                let offset = point - splat(origin);
                let (u, v) = (offset.dot(splat(dual.0)), offset.dot(splat(dual.1)));
                return UvPoint::new(R::splat(at.0) + u, R::splat(at.1) + v);
            }
            Form::Turned { origin, x, y, z, meridian } => (origin, (x, y, z), meridian),
        };
        let [east, north, height] = self.local(point - splat(origin), frame);
        let angle = match self.turn {
            Turn::Beside { at } => R::splat(at) + angle_from(R::splat(at), east, north),
            Turn::Round { at, rate, centre } => {
                let ahead = R::splat(at) + R::splat(rate) * t;
                let toward = ahead + angle_from(ahead, east - R::splat(centre.0), north - R::splat(centre.1));
                toward + angle_from(toward, east, north)
            }
        };
        let rho = (east * east + north * north).sqrt();
        let along = match meridian {
            Meridian::Upright { .. } => height,
            Meridian::Slanted { apex, slope } => {
                let side = R::splat(if self.below { -1.0 } else { 1.0 });
                R::splat(apex) + side * rho * R::splat(slope.0) + height * R::splat(slope.1)
            }
            Meridian::Centred { .. } => atan2(height, rho),
            Meridian::Tube { major, .. } => {
                R::splat(self.tube) + angle_from(R::splat(self.tube), rho - R::splat(major), height)
            }
        };
        UvPoint::new(angle, along)
    }

    /// The chart's first and second rates of the image, from the curve's point and first two rates, before it moved.
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn rates<R: Real>(&self, [point, first, second]: [Vec3<R>; 3]) -> (UvVector<R>, UvVector<R>) {
        let (origin, frame, meridian) = match self.chart.0 {
            Form::Flat { dual, .. } => {
                let along = |q: Vec3<R>| UvVector::new(q.dot(splat(dual.0)), q.dot(splat(dual.1)));
                return (along(first), along(second));
            }
            Form::Turned { origin, x, y, z, meridian } => (origin, (x, y, z), meridian),
        };
        let [[x0, y0, h0], [x1, y1, h1], [x2, y2, h2]] =
            [point - splat(origin), first, second].map(|rate| self.local(rate, frame));
        let (east, north, height) = ([x0, x1, x2], [y0, y1, y2], [h0, h1, h2]);
        let turning = angle_rates(east, north);
        let rho = radial_rates(east, north);
        let rising = match meridian {
            Meridian::Upright { .. } => (h1, h2),
            Meridian::Slanted { slope: (sin, cos), .. } => {
                let side = R::splat(if self.below { -1.0 } else { 1.0 }) * R::splat(sin);
                let [_, rho1, rho2] = rho;
                (side * rho1 + h1 * R::splat(cos), side * rho2 + h2 * R::splat(cos))
            }
            Meridian::Centred { .. } => angle_rates(rho, height),
            Meridian::Tube { major, .. } => {
                let [rho0, rho1, rho2] = rho;
                angle_rates([rho0 - R::splat(major), rho1, rho2], height)
            }
        };
        (UvVector::new(turning.0, rising.0), UvVector::new(turning.1, rising.1))
    }

    /// `uv` as the image has moved it.
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn moved_point<R: Real>(&self, uv: UvPoint<R>) -> UvPoint<R> {
        let row = |[by_u, by_v, shift]: [f64; 3]| R::splat(by_u) * uv.u() + R::splat(by_v) * uv.v() + R::splat(shift);
        UvPoint::new(row(self.after.u), row(self.after.v))
    }

    /// The rate `d` as the image has moved it.
    #[inline(always)]
    #[expect(clippy::arithmetic_side_effects, reason = "IEEE arithmetic over `Real`, which cannot panic or overflow")]
    fn moved_rate<R: Real>(&self, rate: UvVector<R>) -> UvVector<R> {
        let row = |[by_u, by_v, _]: [f64; 3]| R::splat(by_u) * rate.u() + R::splat(by_v) * rate.v();
        UvVector::new(row(self.after.u), row(self.after.v))
    }

    /// The rates of orders 1 to `order` at `t` from Taylor series of the image's closed form, moved.
    fn by_series(&self, t: f64, order: u8, scratch: &mut Scratch) -> Result<Vec<UvVector>, Report> {
        let mut rates = Vec::with_capacity(usize::from(order));
        self.curve.derivatives(t, order, &mut rates, scratch)?;
        let point = self.curve.eval_point(t).to_vec3();
        let mut factorial = 1.0;
        let scaled = rates.iter().zip(1_u32..).map(|(r, k)| {
            factorial *= f64::from(k);
            r.rate() * (1.0 / factorial)
        });
        let p: Vec<Vec3> = std::iter::once(point).chain(scaled).collect();
        let here = self.place(t, point);
        let along = |axis: Vec3, origin: Vec3, side: f64| -> Vec<f64> {
            p.iter().zip(0_u8..).map(|(q, k)| (if k == 0 { *q - origin } else { *q }).dot(axis) * side).collect()
        };
        let (u, v) = match self.chart.0 {
            Form::Flat { origin, at, dual, .. } => (
                series::scaled(&along(dual.0, origin, 1.0), 1.0, at.0),
                series::scaled(&along(dual.1, origin, 1.0), 1.0, at.1),
            ),
            Form::Turned { origin, x, y, z, meridian } => {
                let side = if self.below { -1.0 } else { 1.0 };
                let (east, north, height) = (along(x, origin, side), along(y, origin, side), along(z, origin, 1.0));
                let rho = series::root(&series::plus(&series::product(&east, &east), &series::product(&north, &north)));
                let v = match meridian {
                    Meridian::Upright { .. } => height,
                    Meridian::Slanted { apex, slope: (sin, cos) } => {
                        series::plus(&series::scaled(&rho, side * sin, apex), &series::scaled(&height, cos, 0.0))
                    }
                    Meridian::Centred { .. } => series::angle(here.v(), &rho, &height),
                    Meridian::Tube { major, .. } => {
                        series::angle(here.v(), &series::scaled(&rho, 1.0, -major), &height)
                    }
                };
                (series::angle(here.u(), &east, &north), v)
            }
        };
        let (du, dv) = (series::derivatives(&u), series::derivatives(&v));
        Ok(du.into_iter().zip(dv).map(|(a, b)| self.moved_rate(UvVector::new(a, b))).collect())
    }

    /// The parameters within which the point of the image nearest `uv` lies: its range where that is finite, and else
    /// a stretch about `seed`, or 0, doubled until both its ends are more than twice as far as its nearest sample.
    fn window(&self, uv: UvPoint, seed: Option<f64>) -> (f64, f64) {
        if self.range.lo.is_finite() && self.range.hi.is_finite() {
            return (self.range.lo, self.range.hi);
        }
        let far = |t: f64| (flat(self.eval_uv(t)) - flat(uv)).norm();
        let centre = seed.filter(|t| t.is_finite()).unwrap_or(0.0);
        let mut reach = 1.0;
        for _ in 0..64 {
            let nearest = (0..=32_u32)
                .map(|i| far(centre + reach * (f64::from(i) / 16.0 - 1.0)))
                .fold(f64::INFINITY, |a, d| if d < a { d } else { a });
            if far(centre - reach) > 2.0 * nearest && far(centre + reach) > 2.0 * nearest {
                break;
            }
            reach *= 2.0;
        }
        (centre - reach, centre + reach)
    }

    /// The angles the image can reach over `lo` to `hi`, before it moved: within a half turn of a curve beside the
    /// axis's reference, and within a quarter and a half turn of a curve round it's.
    fn turns(&self, lo: f64, hi: f64) -> (f64, f64) {
        match self.turn {
            Turn::Beside { at } => (at - PI, at + PI),
            Turn::Round { at, rate, .. } => {
                let (a, b) = extremes([at + rate * lo, at + rate * hi].into_iter());
                (a - TAU, b + TAU)
            }
        }
    }

    /// The second parameter's reach over the curve's box, by the heights of its `corners` along the axis, before the
    /// image moved: a height itself, on a cone the height over the cosine of its half-angle, which is how far a point of
    /// either nappe is along it from the apex, and a latitude.
    fn heights(&self, corners: &[Vec3], origin: Vec3, z: Vec3, meridian: Meridian) -> (f64, f64) {
        let (low, high) = extremes(corners.iter().map(|c| (*c - origin).dot(z)));
        match meridian {
            Meridian::Upright { .. } => (low, high),
            Meridian::Slanted { apex, slope } => (apex + low / slope.1, apex + high / slope.1),
            Meridian::Centred { radius } => {
                let latitude = |h: f64| {
                    let h = if h > radius {
                        radius
                    } else if h < -radius {
                        -radius
                    } else {
                        h
                    };
                    atan2(h, ((radius - h) * (radius + h)).sqrt())
                };
                (latitude(low), latitude(high))
            }
            Meridian::Tube { .. } => (self.tube - PI, self.tube + PI),
        }
    }
}

impl<C: ParametricCurve + Clone> ParametricParamCurve for Image<C> {
    #[inline(always)]
    fn eval_uv<R: Real>(&self, t: R) -> UvPoint<R> {
        self.moved_point(self.place(t, self.curve.eval_point(t).to_vec3()))
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, t: R) -> (UvPoint<R>, UvVector<R>) {
        let (point, first, second) = self.curve.eval_second(t);
        let p = [point.to_vec3(), first.rate(), second.rate()];
        (self.moved_point(self.place(t, p[0])), self.moved_rate(self.rates(p).0))
    }

    #[inline(always)]
    fn eval_second<R: Real>(&self, t: R) -> (UvPoint<R>, UvVector<R>, UvVector<R>) {
        let (point, first, second) = self.curve.eval_second(t);
        let p = [point.to_vec3(), first.rate(), second.rate()];
        let (d1, d2) = self.rates(p);
        (self.moved_point(self.place(t, p[0])), self.moved_rate(d1), self.moved_rate(d2))
    }

    /// The first two rates as the rungs give them, and every higher one from Taylor series of the same closed form.
    fn derivatives(&self, t: f64, order: u8, out: &mut Vec<UvVector>, scratch: &mut Scratch) -> Result<(), Report> {
        let (_, first, second) = self.eval_second(t);
        out.extend([first, second].into_iter().take(usize::from(order)));
        if order > 2 {
            out.extend(self.by_series(t, order, scratch)?.into_iter().skip(2));
        }
        Ok(())
    }

    fn range(&self) -> ParamRange {
        self.range
    }

    /// The box of the image over `lo` to `hi`, from the box of the curve there: a parameter that is linear in the point
    /// is bounded at the box's corners, an angle by how far it can turn from its reference, and the box then moved.
    fn bounds(&self, lo: f64, hi: f64) -> UvBox {
        let b = self.curve.bounds(lo, hi);
        let (low, high) = (b.low().to_vec3(), b.high().to_vec3());
        let corners: Vec<Vec3> = (0..8_u8)
            .map(|i| {
                let pick = |bit: u8, l: f64, h: f64| if i & bit == 0 { l } else { h };
                Vec3::new(pick(1, low.x, high.x), pick(2, low.y, high.y), pick(4, low.z, high.z))
            })
            .collect();
        let all =
            UvBox::new(UvPoint::new(f64::NEG_INFINITY, f64::NEG_INFINITY), UvPoint::new(f64::INFINITY, f64::INFINITY));
        if !corners.iter().all(|c| c.x.is_finite() && c.y.is_finite() && c.z.is_finite()) {
            return all;
        }
        let (us, vs) = match self.chart.0 {
            Form::Flat { origin, at, dual, .. } => {
                let along = |axis: Vec3, at: f64| extremes(corners.iter().map(|c| at + (*c - origin).dot(axis)));
                (along(dual.0, at.0), along(dual.1, at.1))
            }
            Form::Turned { origin, z, meridian, .. } => {
                (self.turns(lo, hi), self.heights(&corners, origin, z, meridian))
            }
        };
        let moved: Vec<UvPoint> = [(us.0, vs.0), (us.0, vs.1), (us.1, vs.0), (us.1, vs.1)]
            .into_iter()
            .map(|(u, v)| self.moved_point(UvPoint::new(u, v)))
            .collect();
        let (ul, uh) = padded(extremes(moved.iter().map(|p| p.u())));
        let (vl, vh) = padded(extremes(moved.iter().map(|p| p.v())));
        UvBox::new(UvPoint::new(ul, vl), UvPoint::new(uh, vh))
    }

    /// The nearest point by the shared search over the window that holds it, on the image read as a curve in the plane
    /// `z = 0`.
    fn closest(&self, uv: UvPoint, seed: Option<f64>, _scratch: &mut Scratch) -> Result<UvClosest, Report> {
        let jet = |t: f64| {
            let (p, d1, d2) = self.eval_second(t);
            [flat(p), Vec3::new(d1.u(), d1.v(), 0.0), Vec3::new(d2.u(), d2.v(), 0.0)]
        };
        let at = nearest_by(jet, flat(uv), self.window(uv, seed));
        let found = self.eval_uv(at);
        Ok(UvClosest { at, uv: found, distance: (flat(found) - flat(uv)).norm() })
    }

    fn family(&self) -> FamilyName {
        FamilyName("image")
    }

    fn content(&self, out: &mut Content) {
        out.push_family(self.curve.family());
        self.curve.content(out);
        self.chart.push(out);
        let turn = match self.turn {
            Turn::Beside { at } => [0.0, at, 0.0, 0.0, 0.0],
            Turn::Round { at, rate, centre } => [1.0, at, rate, centre.0, centre.1],
        };
        let rest =
            [f64::from(u8::from(self.below)), self.tube, self.range.lo, self.range.hi, wrap_code(self.range.wrap)];
        for x in turn.into_iter().chain(rest).chain(self.after.u).chain(self.after.v) {
            out.push(x);
        }
    }

    fn shape(&self) -> Shape {
        Shape { form: "image", values: self.curve.shape().values }
    }

    /// Any map of the parameters keeps an image: it is that map of the chart's parameters after the image's own, and
    /// its parameter stays.
    fn moved(&self, map: &UvMap) -> Option<(Self, ParamMap)> {
        Some((Self { after: then(self.after, *map), ..self.clone() }, ParamMap::IDENTITY))
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::{FRAC_PI_6, TAU};

    use occam_units::{Angle, Direction, Length};

    use super::*;
    use crate::conformance::{self, placement};
    use crate::{
        BsplineSurface, Circle, Cone, Curve, Cylinder, Ellipse, Hyperbola, Line, Parabola, ParamCurve, Plane, Sphere,
        SplineSurface, Surface, Torus,
    };

    fn mm(x: f64) -> Length {
        Length::from_millimetres(x)
    }

    /// The point at `(x, y, z)` in [`placement`]'s frame.
    fn at(x: f64, y: f64, z: f64) -> Vec3 {
        placement().point(Point::from_vec3(Vec3::new(x, y, z))).to_vec3()
    }

    /// The unit direction along `(x, y, z)` in [`placement`]'s frame.
    fn along(x: f64, y: f64, z: f64) -> Vec3 {
        placement().direction(Direction::normalised(Vec3::new(x, y, z))).get()
    }

    /// A conic's numbers after its family's name: its axes about `centre`, `sizes`, and its whole range.
    fn numbers(centre: Vec3, x: Vec3, y: Vec3, sizes: &[f64], closed: bool) -> Vec<f64> {
        let n = x.cross(y);
        let mut out = vec![centre.x, centre.y, centre.z, x.x, x.y, x.z, y.x, y.y, y.z, n.x, n.y, n.z];
        out.extend_from_slice(sizes);
        out.extend(if closed { [0.0, TAU, 2.0] } else { [f64::NEG_INFINITY, f64::INFINITY, 0.0] });
        out
    }

    fn circle(centre: Vec3, x: Vec3, y: Vec3, radius: f64) -> Circle {
        Circle::restore(&numbers(centre, x, y, &[radius], true)).unwrap()
    }

    fn chart(s: &dyn Surface) -> Chart {
        Chart::of(s, s.analytic(conformance::tolerance()).unwrap()).unwrap()
    }

    /// A case: its name, the image, its curve, the surface both are on, and the stretch the suite runs it over.
    type Case = (&'static str, Box<dyn ParamCurve>, Box<dyn Curve>, Box<dyn Surface>, (f64, f64));

    fn beside<C: ParametricCurve + Clone>(
        name: &'static str,
        c: C,
        s: Box<dyn Surface>,
        toward: Vec3,
        stretch: (f64, f64),
    ) -> Case {
        let image = Image::beside(c.clone(), chart(s.as_ref()), Point::from_vec3(toward));
        (name, Box::new(image), Box::new(c), s, stretch)
    }

    fn conic<C: ParametricCurve + Clone>(name: &'static str, c: C, s: Box<dyn Surface>) -> Case {
        let image = Image::conic(c.clone(), chart(s.as_ref())).unwrap_or_else(|| panic!("{name} is a conic"));
        (name, Box::new(image), Box::new(c), s, (0.0, TAU))
    }

    /// Images on a plane, as itself and as a B-spline, and on a cylinder, the one round its axis.
    fn flat_and_round() -> Vec<Case> {
        let plane = || -> Box<dyn Surface> { Box::new(Plane::placed(&placement())) };
        let (x, y, z) = (along(1.0, 0.0, 0.0), along(0.0, 1.0, 0.0), along(0.0, 0.0, 1.0));
        let ring = circle(at(0.3, -0.2, 0.0), x, y, 1.1);
        let line = Line::new(Point::from_vec3(at(0.5, 0.5, 0.0)), Direction::normalised(along(0.6, 0.8, 0.0)));
        let branch = Hyperbola::restore(&numbers(at(0.0, 0.0, 0.0), x, y, &[1.2, 0.7], false)).unwrap();
        let bow = Parabola::restore(&numbers(at(0.2, 0.0, 0.0), y, -x, &[0.6], false)).unwrap();
        let corner = |u: f64, v: f64| Point::from_vec3(at(6.0 * u - 3.0, 6.0 * v - 3.0, 0.0));
        let knots = vec![0.0, 0.0, 1.0, 1.0];
        let poles = vec![corner(0.0, 0.0), corner(1.0, 0.0), corner(0.0, 1.0), corner(1.0, 1.0)];
        let form = BsplineSurface {
            degree: (1, 1),
            count: (2, 2),
            knots_u: knots.clone(),
            knots_v: knots,
            poles,
            weights: None,
        };
        let patch: Box<dyn Surface> = Box::new(SplineSurface::new(&form).unwrap());
        let slant = Vec3::new(1.0, 0.0, 2.0) * (1.0 / 5.0_f64.sqrt());
        let minor = along(0.0, -1.0, 0.0);
        let major = placement().direction(Direction::normalised(slant)).get().cross(minor);
        let oval =
            Ellipse::restore(&numbers(at(0.0, 0.0, 0.0), minor, major, &[0.8, 0.8 * 5.0_f64.sqrt() / 2.0], true))
                .unwrap();
        let cylinder = || -> Box<dyn Surface> { Box::new(Cylinder::placed(&placement(), mm(0.8)).unwrap()) };
        let (sin, cos) = sin_cos(1.0);
        let rising = Line::new(Point::from_vec3(at(0.8 * cos, 0.8 * sin, 0.0)), Direction::normalised(z));
        vec![
            conic("plane_circle", ring, plane()),
            beside("plane_line", line, plane(), at(0.5, 0.5, 0.0), (-2.0, 3.0)),
            beside("plane_hyperbola", branch, plane(), at(1.2, 0.0, 0.0), (-1.5, 1.2)),
            beside("plane_parabola", bow, plane(), at(0.2, 0.0, 0.0), (-2.0, 2.0)),
            conic("bspline_circle", ring, patch),
            conic("cylinder_ellipse", oval, cylinder()),
            beside("cylinder_line", rising, cylinder(), at(0.8 * cos, 0.8 * sin, 0.0), (-3.0, 2.0)),
        ]
    }

    /// Images on a sphere beside and round its axis and an arc ending at its pole, on a cone's two nappes, and on a
    /// torus.
    fn turned() -> Vec<Case> {
        let sphere = || -> Box<dyn Surface> { Box::new(Sphere::placed(&placement(), mm(1.0)).unwrap()) };
        let cone = || -> Box<dyn Surface> {
            Box::new(Cone::placed(&placement(), mm(0.0), Angle::from_radians(FRAC_PI_6)).unwrap())
        };
        let (x, y, z) = (along(1.0, 0.0, 0.0), along(0.0, 1.0, 0.0), along(0.0, 0.0, 1.0));
        let beside_axis = circle(at(0.5, 0.0, 0.0), y, z, 0.75_f64.sqrt());
        let facing = Vec3::new(0.3, 0.0, 1.0) * (1.0 / 1.09_f64.sqrt());
        let drop = 0.3 * facing.z;
        let centre = at(facing.x * drop, 0.0, facing.z * drop);
        let tilted = circle(centre, along(0.0, 1.0, 0.0), along(facing.z, 0.0, -facing.x), (1.0 - drop * drop).sqrt());
        let through = circle(at(0.5, 0.0, 0.5), along(-1.0, 0.0, 1.0), along(0.0, -1.0, 0.0), 0.5_f64.sqrt())
            .trimmed(0.0, TAU)
            .unwrap();
        let (sin, cos) = sin_cos(FRAC_PI_6);
        let upper = Hyperbola::restore(&numbers(at(0.4, 0.0, 0.0), z, -y, &[1.6 * sin * cos, 0.4], false)).unwrap();
        let lower = Hyperbola::restore(&numbers(at(0.4, 0.0, 0.0), -z, y, &[1.6 * sin * cos, 0.4], false)).unwrap();
        let ring_below = circle(at(0.0, 0.0, -1.0), x, y, sin / cos);
        let torus: Box<dyn Surface> = Box::new(Torus::placed(&placement(), mm(2.0), mm(0.5)).unwrap());
        let tube_ring = circle(at(0.0, 0.0, 0.3), x, y, 2.4);
        let pole_arc = beside(
            "sphere_arc_at_pole",
            through,
            sphere(),
            through.eval_point(std::f64::consts::PI).to_vec3(),
            (0.1, TAU - 0.1),
        );
        vec![
            conic("sphere_beside", beside_axis, sphere()),
            conic("sphere_round", tilted, sphere()),
            pole_arc,
            beside("cone_upper", upper, cone(), at(0.4, 0.0, 1.6 * sin * cos), (-1.2, 1.4)),
            beside("cone_lower", lower, cone(), at(0.4, 0.0, -1.6 * sin * cos), (-1.2, 1.4)),
            conic("cone_ring_below", ring_below, cone()),
            conic("torus_ring", tube_ring, torus),
        ]
    }

    #[test]
    fn an_image_maps_onto_its_curve_through_its_surface() {
        for (name, image, curve, surface, (lo, hi)) in flat_and_round().into_iter().chain(turned()) {
            for i in 0..=40 {
                let t = lo + (hi - lo) * f64::from(i) / 40.0;
                let miss = (surface.point(image.uv(t)).to_vec3() - curve.point(t).to_vec3()).norm();
                assert!(miss <= 1e-12 * 4.0, "{name} at {t} maps {miss} from its curve");
            }
        }
    }

    #[test]
    fn images_on_a_plane_and_a_cylinder_pass_the_parameter_curve_suite() {
        for (name, image, _, _, (lo, hi)) in flat_and_round() {
            let failures = conformance::param_curve(image.as_ref(), lo, hi);
            assert!(failures.is_empty(), "{name}: {failures:#?}");
        }
    }

    #[test]
    fn images_on_a_sphere_a_cone_and_a_torus_pass_the_parameter_curve_suite() {
        for (name, image, _, _, (lo, hi)) in turned() {
            let failures = conformance::param_curve(image.as_ref(), lo, hi);
            assert!(failures.is_empty(), "{name}: {failures:#?}");
        }
    }

    #[test]
    fn an_image_round_an_axis_does_not_come_round_and_one_beside_it_does() {
        let (cases, more) = (flat_and_round(), turned());
        let wrap = |name: &str| cases.iter().chain(&more).find(|c| c.0 == name).map(|c| c.1.range().wrap);
        assert_eq!(wrap("cylinder_ellipse"), Some(Wrap::Open), "it comes round in space, not in the parameters");
        assert_eq!(wrap("cone_ring_below"), Some(Wrap::Open));
        assert_eq!(wrap("sphere_beside"), Some(Wrap::Periodic));
        assert_eq!(wrap("plane_circle"), Some(Wrap::Periodic));
        assert_eq!(wrap("sphere_arc_at_pole"), Some(Wrap::Open), "its ends are one point at different angles");
        let image = &cases.iter().find(|c| c.0 == "cylinder_ellipse").map(|c| &c.1).unwrap();
        let (start, end) = (image.uv(0.0), image.uv(TAU));
        assert!((end.u() - start.u()).abs() > 6.0, "its angle turns once: {start:?} to {end:?}");
    }

    #[test]
    fn a_curve_that_is_not_a_conic_has_no_conic_image() {
        let plane = Plane::placed(&placement());
        let branch = Hyperbola::restore(&numbers(
            at(0.0, 0.0, 0.0),
            along(1.0, 0.0, 0.0),
            along(0.0, 1.0, 0.0),
            &[1.0, 1.0],
            false,
        ))
        .unwrap();
        assert!(Image::conic(branch, chart(&plane)).is_none());
    }
}
