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
use crate::laps::{Laps, across};
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
    /// The curve goes round the axis any number of times, but not through it: its angle is counted in whole turns
    /// from where it crosses the plane through the axis at `at`.
    Laps(Laps),
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
pub(crate) fn angle_from<R: Real>(from: R, x: R, y: R) -> R {
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

    /// The image through `chart` of `curve`, whose range is finite, which may go round a turned chart's axis any
    /// number of times but does not pass through it, as a loop of a quartic or an arc of one ending at a sphere's
    /// pole: its angle is kept continuous by counting where it crosses the plane through the axis at the angle of its
    /// middle. `None` where its range is not finite or it crosses that plane more than eight times.
    #[must_use]
    pub fn along(curve: C, chart: Chart) -> Option<Self> {
        let range = curve.range();
        if !(range.lo.is_finite() && range.hi.is_finite()) {
            return None;
        }
        let middle = curve.eval_point(0.5 * range.lo + 0.5 * range.hi);
        let Form::Turned { origin, x, y, .. } = chart.0 else { return Some(Self::beside(curve, chart, middle)) };
        let low = below(&chart, middle.to_vec3());
        let side = if low { -1.0 } else { 1.0 };
        let at = angle_of(&chart, middle.to_vec3(), low);
        let point = |t: f64| across((curve.eval_point(t).to_vec3() - origin) * side, (x, y));
        let laps = Laps::counted(point, (range.lo, range.hi), at)?;
        Some(Self::made(curve, chart, Turn::Laps(laps), middle.to_vec3()))
    }

    fn made(curve: C, chart: Chart, turn: Turn, reference: Vec3) -> Self {
        let (low, tube) = (below(&chart, reference), tube(&chart, reference));
        let range = curve.range();
        let mut image = Self { curve, chart, turn, below: low, tube, after: UvMap::IDENTITY, range };
        // Beside the axis the image is the point's parameters alone, so a whole periodic curve's ends meet there too,
        // and so they do for one that goes round the axis and back as often, its turns counted to none.
        let comes_round = range.wrap == Wrap::Periodic
            && match turn {
                Turn::Beside { .. } => true,
                Turn::Laps(laps) => laps.winding() == 0.0,
                Turn::Round { .. } => false,
            };
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
            Turn::Laps(laps) => laps.angle(t, east, north),
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
            Turn::Laps(laps) => laps.span,
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
            Turn::Beside { at } => vec![0.0, at, 0.0, 0.0, 0.0],
            Turn::Round { at, rate, centre } => vec![1.0, at, rate, centre.0, centre.1],
            Turn::Laps(laps) => std::iter::once(2.0).chain(laps.numbers()).collect(),
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

    /// [`placement`] with its Z turned onto its X.
    fn across_x() -> occam_units::Placement<occam_units::Local, World> {
        let quarter = Angle::from_radians(std::f64::consts::FRAC_PI_2);
        occam_units::Placement::rotation(Direction::normalised(Vec3::new(0.0, 1.0, 0.0)), quarter).then(&placement())
    }

    /// A quartic loop on a cylinder about [`placement`]'s Z of `radius`, from its numbers after the axes: `round` with
    /// the cylinder of radius 1 about X round the whole turn, else between the roots of `Δ` where the cylinder of
    /// radius 0.6 about X meets one of radius 1.
    fn quartic(round: bool) -> crate::Quartic {
        let frame = [at(0.0, 0.0, 0.0), along(1.0, 0.0, 0.0), along(0.0, 1.0, 0.0), along(0.0, 0.0, 1.0)];
        let mut numbers: Vec<f64> = frame.iter().flat_map(|v| [v.x, v.y, v.z]).collect();
        let k = 0.1_f64.sqrt();
        numbers.extend(if round {
            vec![0.0, 0.6, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.82, 0.0, 0.0, 0.18, 0.0, 1.0]
        } else {
            vec![0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 1.0, 1.0, 0.0, k, 0.8, 1.0, 0.0]
        });
        numbers.extend([0.0, TAU, 2.0]);
        crate::Quartic::restore(&numbers).unwrap()
    }

    /// Images that count the turns: a circle round a sphere's axis and an ellipse round a cylinder's, and two quartic
    /// loops, each round one cylinder's axis and beside the other's.
    fn counted() -> Vec<Case> {
        let along_case =
            |name: &'static str, c: Box<dyn Curve>, image: Option<Box<dyn ParamCurve>>, s: Box<dyn Surface>| {
                (name, image.unwrap_or_else(|| panic!("{name} has an image")), c, s, (0.0, TAU))
            };
        let thin = || -> Box<dyn Surface> { Box::new(Cylinder::placed(&placement(), mm(0.6)).unwrap()) };
        let wide = || -> Box<dyn Surface> { Box::new(Cylinder::placed(&placement(), mm(1.0)).unwrap()) };
        let crossing = |r: f64| -> Box<dyn Surface> { Box::new(Cylinder::placed(&across_x(), mm(r)).unwrap()) };
        let image = |c: crate::Quartic, s: &dyn Surface| -> Option<Box<dyn ParamCurve>> {
            Some(Box::new(Image::along(c, chart(s))?))
        };
        let (round, between) = (quartic(true), quartic(false));
        let mut cases = vec![
            along_case("round_on_its_own", Box::new(round), image(round, thin().as_ref()), thin()),
            along_case("round_beside", Box::new(round), image(round, crossing(1.0).as_ref()), crossing(1.0)),
            along_case("between_beside", Box::new(between), image(between, wide().as_ref()), wide()),
            along_case("between_round", Box::new(between), image(between, crossing(0.6).as_ref()), crossing(0.6)),
        ];
        let facing = Vec3::new(0.3, 0.0, 1.0) * (1.0 / 1.09_f64.sqrt());
        let drop = 0.3 * facing.z;
        let centre = at(facing.x * drop, 0.0, facing.z * drop);
        let tilted = circle(centre, along(0.0, 1.0, 0.0), along(facing.z, 0.0, -facing.x), (1.0 - drop * drop).sqrt());
        let sphere: Box<dyn Surface> = Box::new(Sphere::placed(&placement(), mm(1.0)).unwrap());
        let counted: Option<Box<dyn ParamCurve>> =
            Image::along(tilted, chart(sphere.as_ref())).map(|i| Box::new(i) as _);
        cases.push(along_case("sphere_round", Box::new(tilted), counted, sphere));
        cases
    }

    #[test]
    fn an_image_that_counts_its_turns_maps_onto_its_curve_and_passes_the_suite() {
        for (name, image, curve, surface, (lo, hi)) in counted() {
            for i in 0..=400 {
                let t = lo + (hi - lo) * f64::from(i) / 400.0;
                let miss = (surface.point(image.uv(t)).to_vec3() - curve.point(t).to_vec3()).norm();
                assert!(miss <= 1e-12 * 4.0, "{name} at {t} maps {miss} from its curve");
                if i > 0 {
                    let before = image.uv(t - (hi - lo) / 400.0);
                    let jump = (image.uv(t).u() - before.u()).abs();
                    assert!(jump < 0.2, "{name} jumps by {jump} at {t}");
                }
            }
            let failures = conformance::param_curve(image.as_ref(), lo, hi);
            assert!(failures.is_empty(), "{name}: {failures:#?}");
        }
    }

    #[test]
    fn an_image_that_counts_its_turns_comes_round_only_where_its_angle_does() {
        let cases = counted();
        let wrap = |name: &str| cases.iter().find(|c| c.0 == name).map(|c| c.1.range().wrap);
        assert_eq!(wrap("round_on_its_own"), Some(Wrap::Open), "it goes once round the axis");
        assert_eq!(wrap("between_round"), Some(Wrap::Open));
        assert_eq!(wrap("round_beside"), Some(Wrap::Periodic), "beside the axis, its ends meet");
        assert_eq!(wrap("between_beside"), Some(Wrap::Periodic));
        let image = &cases.iter().find(|c| c.0 == "round_on_its_own").map(|c| &c.1).unwrap();
        assert!((image.uv(TAU).u() - image.uv(0.0).u()).abs() > 6.0, "its angle turns once");
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

    /// `centre + a cos t + b sin t + wobble sin 3t`: a conic in any two axes, square or not, off its form by the wobble.
    #[derive(Clone, Copy, Debug, PartialEq)]
    struct Sketch {
        centre: Vec3,
        a: Vec3,
        b: Vec3,
        wobble: Vec3,
    }

    impl Sketch {
        /// Its rate of order `k`: `a cos(t + kπ/2) + b sin(t + kπ/2) + 3^k wobble sin(3t + kπ/2)`.
        #[expect(
            clippy::arithmetic_side_effects,
            reason = "IEEE arithmetic over `Real`, which cannot panic or overflow"
        )]
        fn rate<R: Real>(&self, t: R, k: u8) -> Vec3<R> {
            let quarter = R::splat(f64::from(k) * FRAC_PI_2);
            let ((sin, cos), (sin3, _)) = (sin_cos(t + quarter), sin_cos(R::splat(3.0) * t + quarter));
            let scale = (0..k).fold(1.0, |s, _| s * 3.0);
            splat(self.a) * cos + splat(self.b) * sin + splat(self.wobble) * (sin3 * R::splat(scale))
        }
    }

    impl ParametricCurve for Sketch {
        fn eval_point<R: Real>(&self, t: R) -> Point<World, R> {
            Point::from_vec3(splat(self.centre) + self.rate(t, 0))
        }

        fn eval_first<R: Real>(&self, t: R) -> (Point<World, R>, occam_units::Tangent<World, R>) {
            (self.eval_point(t), occam_units::Tangent::new(self.rate(t, 1)))
        }

        fn eval_second<R: Real>(
            &self,
            t: R,
        ) -> (Point<World, R>, occam_units::Tangent<World, R>, occam_units::Tangent<World, R>) {
            let (point, first) = self.eval_first(t);
            (point, first, occam_units::Tangent::new(self.rate(t, 2)))
        }

        fn derivatives(
            &self,
            t: f64,
            order: u8,
            out: &mut Vec<occam_units::Tangent<World>>,
            _: &mut Scratch,
        ) -> Result<(), Report> {
            out.extend((1..=order).map(|k| occam_units::Tangent::new(self.rate(t, k))));
            Ok(())
        }

        fn range(&self) -> ParamRange {
            ParamRange { lo: 0.0, hi: TAU, wrap: Wrap::Periodic, kind: occam_units::ParamKind::Angle }
        }

        fn bounds(&self, _: f64, _: f64) -> occam_units::Bounds {
            let reach = self.a.norm() + self.b.norm() + self.wobble.norm();
            occam_units::Bounds::around(Point::from_vec3(self.centre), Length::from_millimetres(reach * (1.0 + 1e-9)))
        }

        fn family(&self) -> FamilyName {
            FamilyName("sketch")
        }

        fn content(&self, out: &mut Content) {
            for v in [self.centre, self.a, self.b, self.wobble] {
                out.push_vec3(v);
            }
        }

        fn shape(&self) -> Shape {
            Shape { form: "sketch", values: Vec::new() }
        }
    }

    #[test]
    fn a_curve_is_a_conic_within_a_part_in_a_trillion_of_its_centre_and_axes() {
        // About a centre 100 from the origin, on axes 20 and 2 long, wobbled across by `w sin 3t`: read at 0, π/2 and π,
        // its form misses it by `w (sin 3t + sin t)` at 1, 2.5 and 4, most at 2.5, held to 1e-12 of 100 + 20 + 2.
        let (centre, a, b) = (Vec3::new(60.0, -80.0, 0.0), Vec3::new(0.0, 0.0, 20.0), Vec3::new(1.2, 1.6, 0.0));
        let most = [1.0, 2.5, 4.0_f64].into_iter().fold(0.0_f64, |m, t| {
            let ((s1, _), (s3, _)) = (sin_cos(t), sin_cos(3.0 * t));
            if (s1 + s3).abs() > m { (s1 + s3).abs() } else { m }
        });
        let band = 1e-12 * (100.0 + 20.0 + 2.0);
        let plane = Plane::placed(&placement());
        for (share, kept) in [(0.99, true), (1.01, false)] {
            let wobble = Vec3::new(0.6, -0.8, 0.0) * (share * band / most);
            let image = Image::conic(Sketch { centre, a, b, wobble }, chart(&plane));
            assert_eq!(image.is_some(), kept, "off by {share} of its band");
        }
    }

    #[test]
    fn an_images_nearest_point_says_how_far_it_is_and_is_found_however_far_along_it() {
        let plane = Plane::placed(&placement());
        let start = at(0.5, 0.5, 0.0);
        let line = Line::new(Point::from_vec3(start), Direction::normalised(along(0.6, 0.8, 0.0)));
        let image = Image::beside(line, chart(&plane), Point::from_vec3(start));
        // With no seed, and with one ten along from the nearest point, whose window must stretch back to it.
        for (far, seed) in [0.4, 37.0, 1000.0].into_iter().flat_map(|far| [(far, None), (far, Some(far + 10.0))]) {
            let on = image.eval_uv(far);
            let off = UvPoint::new(on.u() - 0.8 * 0.3, on.v() + 0.6 * 0.3);
            let found = ParametricParamCurve::closest(&image, off, seed, &mut Scratch::default()).unwrap();
            let gap = (flat(found.uv) - flat(off)).norm();
            assert!((found.at - far).abs() <= 1e-9 * (1.0 + far), "{far} from {seed:?}: found at {}", found.at);
            assert!((found.distance - 0.3).abs() <= 1e-9 && (gap - found.distance).abs() <= 1e-12, "{found:?}");
        }
    }

    /// Images of random curves on random surfaces, placed at random: each image's reference, side of a cone's apex and
    /// tube angle are the ones the test works out itself, and each lies on its curve, keeps continuous, has the rates
    /// its differences have, and is held by its box, which is no wider than its reference allows.
    mod battery {
        use std::f64::consts::{PI, TAU};

        use occam_units::{Local, Placement, Vector};

        use super::super::*;
        use super::{Sketch, mm, numbers};
        use crate::{Circle, Cone, Cylinder, Ellipse, Hyperbola, Line, Plane, Sphere, Surface, Torus};

        /// A linear congruential generator, so that the cases are the same on every run.
        pub(super) struct Dice(pub(super) u64);

        impl Dice {
            pub(super) fn next(&mut self) -> f64 {
                self.0 = self.0.wrapping_mul(6_364_136_223_846_793_005).wrapping_add(1_442_695_040_888_963_407);
                f64::from(u32::try_from(self.0.wrapping_shr(40)).unwrap()) / 16_777_216.0
            }

            pub(super) fn within(&mut self, lo: f64, hi: f64) -> f64 {
                lo + (hi - lo) * self.next()
            }

            pub(super) fn unit(&mut self) -> Vec3 {
                let v = Vec3::new(self.within(-1.0, 1.0), self.within(-1.0, 1.0), self.within(-1.0, 1.0));
                v * (1.0 / v.norm())
            }

            pub(super) fn placement(&mut self) -> Placement<Local, World> {
                let axis = occam_units::Direction::normalised(self.unit());
                let turn = Placement::rotation(axis, occam_units::Angle::from_radians(self.within(0.0, 3.0)));
                let shift = Placement::translation(Vector::from_vec3(self.unit() * self.within(0.5, 6.0)));
                Placement::identity().then(&turn).then(&shift)
            }
        }

        /// A unit axis square to the unit `n`.
        fn square(n: Vec3, dice: &mut Dice) -> Vec3 {
            let a = n.cross(dice.unit());
            a * (1.0 / a.norm())
        }

        /// What an image must hold besides its points.
        struct Expect {
            turn: Turn,
            below: bool,
            tube: f64,
        }

        fn frame(chart: &Chart) -> Option<(Vec3, Vec3, Vec3, Vec3, Meridian)> {
            match chart.0 {
                Form::Turned { origin, x, y, z, meridian } => Some((origin, x, y, z, meridian)),
                Form::Flat { .. } => None,
            }
        }

        /// What an image beside the axis toward `toward`, or a conic's whose start is `toward`, holds as its side of a
        /// cone's apex and its tube angle.
        fn sides(chart: &Chart, toward: Vec3) -> (bool, f64) {
            let Some((origin, x, y, z, meridian)) = frame(chart) else { return (false, 0.0) };
            let q = toward - origin;
            match meridian {
                Meridian::Slanted { .. } => (q.dot(z) < 0.0, 0.0),
                Meridian::Tube { major, .. } => (false, atan2(q.dot(z), q.dot(x).hypot_free(q.dot(y)) - major)),
                Meridian::Upright { .. } | Meridian::Centred { .. } => (false, 0.0),
            }
        }

        trait Hypot {
            fn hypot_free(self, other: f64) -> f64;
        }

        impl Hypot for f64 {
            fn hypot_free(self, other: f64) -> f64 {
                (self * self + other * other).sqrt()
            }
        }

        /// The angle of `q` about a turned chart's axis, turned to its opposite below a cone's apex.
        fn angle(chart: &Chart, q: Vec3, below: bool) -> f64 {
            let Some((_, x, y, ..)) = frame(chart) else { return 0.0 };
            let side = if below { -1.0 } else { 1.0 };
            atan2(side * q.dot(y), side * q.dot(x))
        }

        fn beside_of(chart: &Chart, toward: Vec3) -> Expect {
            let (below, tube) = sides(chart, toward);
            let origin = frame(chart).map_or(Vec3::new(0.0, 0.0, 0.0), |f| f.0);
            Expect { turn: Turn::Beside { at: angle(chart, toward - origin, below) }, below, tube }
        }

        /// What the image of `middle + a cos t + b sin t` holds, round the axis where `inside` says the axis passes
        /// inside.
        fn conic_of(chart: &Chart, (middle, a, b): (Vec3, Vec3, Vec3), inside: bool) -> Expect {
            let (below, tube) = sides(chart, middle + a);
            let Some((origin, x, y, ..)) = frame(chart) else {
                return Expect { turn: Turn::Beside { at: 0.0 }, below, tube };
            };
            if !inside {
                return Expect { turn: Turn::Beside { at: angle(chart, middle - origin, below) }, below, tube };
            }
            let side = if below { -1.0 } else { 1.0 };
            let rate = if a.cross(b).dot(x.cross(y)) > 0.0 { 1.0 } else { -1.0 };
            let centre = (side * (middle - origin).dot(x), side * (middle - origin).dot(y));
            Expect { turn: Turn::Round { at: angle(chart, a, below), rate, centre }, below, tube }
        }

        fn same(a: f64, b: f64) -> bool {
            (a - b).abs() <= 1e-11 * (1.0 + a.abs())
        }

        fn same_turn(got: Turn, want: Turn) -> bool {
            match (got, want) {
                (Turn::Beside { at }, Turn::Beside { at: want }) => same(at, want),
                (Turn::Round { at, rate, centre }, Turn::Round { at: a, rate: r, centre: c }) => {
                    same(at, a) && rate.to_bits() == r.to_bits() && same(centre.0, c.0) && same(centre.1, c.1)
                }
                _ => false,
            }
        }

        fn flat(d: UvVector) -> Vec3 {
            Vec3::new(d.u(), d.v(), 0.0)
        }

        /// The image's rates at `t` against central differences of the order below.
        fn rates<C: ParametricCurve + Clone>(name: &str, image: &Image<C>, t: f64) {
            let h = 1e-5;
            let (_, d1, d2) = image.eval_second(t);
            let point = |s: f64| super::super::flat(image.eval_uv(s));
            let first = |s: f64| flat(image.eval_first(s).1);
            let mut out = Vec::new();
            image.derivatives(t, 3, &mut out, &mut Scratch::default()).unwrap();
            let third = out.get(2).map_or(Vec3::new(f64::NAN, 0.0, 0.0), |d| flat(*d));
            let second = |s: f64| flat(image.eval_second(s).2);
            let pairs = [
                (flat(d1), (point(t + h) - point(t - h)) * (0.5 / h)),
                (flat(d2), (first(t + h) - first(t - h)) * (0.5 / h)),
                (third, (second(t + h) - second(t - h)) * (0.5 / h)),
            ];
            for (k, (got, want)) in pairs.into_iter().enumerate() {
                assert!(
                    (got - want).norm() <= 1e-4 * (1.0 + want.norm()),
                    "{name}: rate {k} at {t}: {got:?} not {want:?}"
                );
            }
        }

        /// The image's box over windows of `(lo, hi)` holds its points there, and is no wider than its reference allows.
        fn boxes<C: ParametricCurve + Clone>(name: &str, image: &Image<C>, (lo, hi): (f64, f64)) {
            let at = |f: f64| lo + (hi - lo) * f;
            for (from, to) in
                [(at(0.0), at(0.3)), (at(0.2), at(0.55)), (at(0.45), at(1.0)), (lo, hi), (at(0.6), at(0.6))]
            {
                let b = image.bounds(from, to);
                for t in (0..=16).map(|i| from + (to - from) * f64::from(i) / 16.0) {
                    let uv = image.eval_uv(t);
                    let inside = b.lo.u() <= uv.u() && uv.u() <= b.hi.u() && b.lo.v() <= uv.v() && uv.v() <= b.hi.v();
                    assert!(inside, "{name}: {uv:?} at {t} is outside {b:?}");
                }
                let Form::Turned { origin, z, meridian, .. } = image.chart.0 else { continue };
                let reach = match image.turn {
                    Turn::Beside { .. } => TAU,
                    Turn::Round { .. } => (to - from).abs() + 2.0 * TAU,
                    Turn::Laps(laps) => laps.span.1 - laps.span.0,
                };
                let wide = b.hi.u() - b.lo.u();
                assert!(wide <= reach + 1e-9 * (1.0 + b.hi.u().abs()), "{name}: {from} to {to} is {wide} wide in u");
                // A height or a latitude is no further from its curve's than the curve's box is from the axis.
                let space = image.curve.bounds(from, to);
                let far = (space.high().to_vec3() - origin).norm() + (space.low().to_vec3() - origin).norm();
                let tall = b.hi.v() - b.lo.v();
                assert!(tall <= TAU + 4.0 * far, "{name}: {from} to {to} is {tall} tall in v");
                // A height, or a distance along a cone's side, is as tall as the curve's box is along the axis allows.
                let (l, h) = (space.low().to_vec3(), space.high().to_vec3());
                let corners: Vec<f64> = (0..8_u8)
                    .map(|i| {
                        let pick = |bit: u8, a: f64, b: f64| if i & bit == 0 { a } else { b };
                        (Vec3::new(pick(1, l.x, h.x), pick(2, l.y, h.y), pick(4, l.z, h.z)) - origin).dot(z)
                    })
                    .collect();
                let reach = corners.iter().fold((f64::INFINITY, f64::NEG_INFINITY), |(lo, hi), c| {
                    (if *c < lo { *c } else { lo }, if *c > hi { *c } else { hi })
                });
                let along = match meridian {
                    Meridian::Upright { .. } => Some(1.0),
                    Meridian::Slanted { slope, .. } => Some(1.0 / slope.1),
                    Meridian::Centred { .. } | Meridian::Tube { .. } => None,
                };
                if let Some(per) = along {
                    let most = (reach.1 - reach.0) * per;
                    assert!(tall <= most * (1.0 + 1e-9) + 1e-9, "{name}: {from} to {to} is {tall} tall, over {most}");
                }
            }
        }

        fn check<C: ParametricCurve + Clone>(
            name: &str,
            image: &Image<C>,
            want: &Expect,
            s: &dyn Surface,
            span: (f64, f64),
        ) {
            assert!(same_turn(image.turn, want.turn), "{name}: {:?} against {:?}", image.turn, want.turn);
            assert_eq!(image.below, want.below, "{name}: below the apex");
            assert!(same(image.tube, want.tube), "{name}: tube {} against {}", image.tube, want.tube);
            let (lo, hi) = span;
            let mut last: Option<UvPoint> = None;
            for i in 0..=240_u32 {
                let t = lo + (hi - lo) * f64::from(i) / 240.0;
                let (uv, p) = (image.eval_uv(t), image.curve.eval_point(t).to_vec3());
                let miss = (s.point(uv).to_vec3() - p).norm();
                assert!(miss <= 1e-11 * (1.0 + p.norm()), "{name}: at {t} it maps {miss} from its curve");
                if let Some(before) = last {
                    // Only an angle can jump: the angle about the axis, and a torus's tube angle.
                    let tube = matches!(image.chart.0, Form::Turned { meridian: Meridian::Tube { .. }, .. });
                    let jump = (uv.u() - before.u()).abs() + if tube { (uv.v() - before.v()).abs() } else { 0.0 };
                    assert!(jump < 1.5, "{name}: it jumps by {jump} at {t}");
                }
                last = Some(uv);
                if i % 30 == 15 {
                    rates(name, image, t);
                }
            }
            boxes(name, image, span);
        }

        /// The placed point and direction of local ones.
        fn put(place: &Placement<Local, World>, p: Vec3) -> Vec3 {
            place.point(Point::from_vec3(p)).to_vec3()
        }

        fn turn(place: &Placement<Local, World>, v: Vec3) -> Vec3 {
            place.direction(occam_units::Direction::normalised(v)).get()
        }

        /// A circle and an ellipse placed from local axes.
        fn ring(place: &Placement<Local, World>, c: Vec3, (x, y): (Vec3, Vec3), r: f64) -> Circle {
            Circle::restore(&numbers(put(place, c), turn(place, x), turn(place, y), &[r], true)).unwrap()
        }

        fn oval(place: &Placement<Local, World>, c: Vec3, (x, y): (Vec3, Vec3), radii: (f64, f64)) -> Ellipse {
            Ellipse::restore(&numbers(put(place, c), turn(place, x), turn(place, y), &[radii.0, radii.1], true))
                .unwrap()
        }

        fn conic<C: ParametricCurve + Clone>(name: &str, c: C, s: &dyn Surface, inside: bool) {
            let chart = Chart::of(s, s.analytic(crate::conformance::tolerance()).unwrap()).unwrap();
            let at = |t: f64| c.eval_point(t).to_vec3();
            let (centre, a, b) = (
                (at(0.0) + at(PI)) * 0.5,
                at(0.0) - (at(0.0) + at(PI)) * 0.5,
                at(FRAC_PI_2) - (at(0.0) + at(PI)) * 0.5,
            );
            let want = conic_of(&chart, (centre, a, b), inside);
            let image = Image::conic(c, chart).unwrap_or_else(|| panic!("{name} is a conic"));
            check(name, &image, &want, s, (0.0, TAU));
        }

        fn beside<C: ParametricCurve + Clone>(name: &str, c: C, s: &dyn Surface, toward: Vec3, span: (f64, f64)) {
            let chart = Chart::of(s, s.analytic(crate::conformance::tolerance()).unwrap()).unwrap();
            let want = beside_of(&chart, toward);
            check(name, &Image::beside(c, chart, Point::from_vec3(toward)), &want, s, span);
        }

        /// A circle a plane cuts from a sphere of `r`, and whether the sphere's axis passes inside it, or `None` where a
        /// pole is too near the plane for the image to keep from jumping between samples.
        fn sphere_case(place: &Placement<Local, World>, r: f64, dice: &mut Dice) -> Option<(Circle, bool)> {
            let n = dice.unit();
            let d = r * dice.within(-0.85, 0.85);
            if (r * n.z - d).abs() < 0.2 * r || (r * n.z + d).abs() < 0.2 * r {
                return None;
            }
            let (centre, rho) = (n * d, (r * r - d * d).sqrt());
            let meet = Vec3::new(0.0, 0.0, d / n.z);
            let x = square(n, dice);
            Some((ring(place, centre, (x, n.cross(x)), rho), (meet - centre).norm() < rho))
        }

        #[test]
        fn images_on_spheres_and_cylinders_hold_their_references() {
            let mut dice = Dice(7);
            for seed in 0..24 {
                let place = dice.placement();
                let r = dice.within(0.5, 2.0);
                let sphere = Sphere::placed(&place, mm(r)).unwrap();
                if let Some((circle, inside)) = sphere_case(&place, r, &mut dice) {
                    conic(&format!("sphere[{seed}]"), circle, &sphere, inside);
                    if !inside {
                        let toward = circle.eval_point(1.0).to_vec3();
                        beside(&format!("sphere_beside[{seed}]"), circle, &sphere, toward, (0.0, TAU));
                    }
                }
                let cylinder = Cylinder::placed(&place, mm(r)).unwrap();
                let n = Vec3::new(
                    dice.within(-1.0, 1.0),
                    dice.within(-1.0, 1.0),
                    dice.within(0.25, 1.0) * if seed % 2 == 0 { 1.0 } else { -1.0 },
                );
                let n = n * (1.0 / n.norm());
                let minor = n.cross(Vec3::new(0.0, 0.0, 1.0)) * (1.0 / n.cross(Vec3::new(0.0, 0.0, 1.0)).norm());
                let centre = Vec3::new(0.0, 0.0, dice.within(-2.0, 2.0));
                let axes = if seed % 3 == 0 { (minor, n.cross(minor)) } else { (n.cross(minor) * -1.0, minor) };
                let radii = if seed % 3 == 0 { (r, r / n.z.abs()) } else { (r / n.z.abs(), r) };
                conic(&format!("cylinder[{seed}]"), oval(&place, centre, axes, radii), &cylinder, true);
                let (sin, cos) = sin_cos(dice.within(-PI, PI));
                let foot = put(&place, Vec3::new(r * cos, r * sin, centre.z));
                let rising = Line::new(
                    Point::from_vec3(foot),
                    occam_units::Direction::normalised(turn(&place, Vec3::new(0.0, 0.0, 1.0))),
                );
                beside(&format!("cylinder_line[{seed}]"), rising, &cylinder, foot, (-3.0, 3.0));
            }
        }

        #[test]
        fn an_image_round_a_sphere_keeps_its_angle_on_a_circle_its_axis_sees_almost_edge_on() {
            // The circle's plane all but holds the axis, and passes a tenth of the radius from a pole: seen down the
            // axis it is a thin ellipse, on which the angle from its centre runs far from its parameter.
            let sphere = Sphere::placed(&Placement::identity(), mm(1.0)).unwrap();
            let n = Vec3::new(0.73, 0.675, 0.111);
            let n = n * (1.0 / n.norm());
            let (d, e) = (0.019, Vec3::new(-0.533, 0.46, 0.71));
            let x = e - n * e.dot(n);
            let x = x * (1.0 / x.norm());
            let circle = ring(&Placement::identity(), n * d, (x, n.cross(x)), (1.0 - d * d).sqrt());
            conic("sphere_edge_on", circle, &sphere, true);
        }

        /// The ellipse a plane through the axis `h` from a cone's apex, facing `n`, cuts from the cone of `half` about
        /// the local Z axis with its apex at `apex`: its centre, axes and radii.
        fn cone_ellipse(apex: Vec3, h: f64, n: Vec3, half: f64) -> (Vec3, (Vec3, Vec3), (f64, f64)) {
            let up = Vec3::new(0.0, 0.0, 1.0);
            let (sin, cos) = sin_cos(half);
            let (slant, gap) = (n.dot(up), -h * n.dot(up));
            let e1 = (up - n * slant) * (1.0 / (up - n * slant).norm());
            let (rise, k, foot) = (e1.dot(up), slant * slant - sin * sin, apex - n * gap);
            let centre = foot + e1 * (-rise * gap * slant / k);
            (centre, (e1, n.cross(e1)), (gap.abs() * sin * cos / k, gap.abs() * sin / k.sqrt()))
        }

        #[test]
        fn images_on_cones_hold_their_references_on_both_nappes() {
            let mut dice = Dice(11);
            for seed in 0..24 {
                let place = dice.placement();
                let (half, radius) = (dice.within(0.3, 1.1), dice.within(0.0, 1.0));
                let (sin, cos) = sin_cos(half);
                let cone = Cone::placed(&place, mm(radius), occam_units::Angle::from_radians(half)).unwrap();
                let apex = Vec3::new(0.0, 0.0, -radius * cos / sin);
                let height = dice.within(0.5, 2.0) * if seed % 2 == 0 { 1.0 } else { -1.0 };
                let level = (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
                let flat = ring(&place, apex + Vec3::new(0.0, 0.0, height), level, height.abs() * sin / cos);
                conic(&format!("cone_ring[{seed}]"), flat, &cone, true);
                // From nearly as steep as the side, a long ellipse whose centre is far from the axis inside it, to steep.
                let steep =
                    sin + if seed % 4 == 0 { dice.within(0.002, 0.02) } else { dice.within(0.1, 0.9) } * (1.0 - sin);
                let side = (1.0 - steep * steep).sqrt();
                let (s2, c2) = sin_cos(dice.within(-PI, PI));
                let facing = Vec3::new(side * c2, side * s2, steep);
                let (centre, axes, radii) = cone_ellipse(apex, height, facing, half);
                conic(&format!("cone_ellipse[{seed}]"), oval(&place, centre, axes, radii), &cone, true);
                let phase = dice.within(-PI, PI);
                let (ps, pc) = sin_cos(phase);
                let first = turn(&place, axes.0) * (radii.0 * pc) + turn(&place, axes.1) * (radii.1 * ps);
                let second = turn(&place, axes.0) * (-radii.0 * ps) + turn(&place, axes.1) * (radii.1 * pc);
                let still = Vec3::new(0.0, 0.0, 0.0);
                let skewed = Sketch { centre: put(&place, centre), a: first, b: second, wobble: still };
                conic(&format!("cone_skewed[{seed}]"), skewed, &cone, true);
                let (gap, angle) = (dice.within(0.2, 1.0), dice.within(-PI, PI));
                let (s3, c3) = sin_cos(angle);
                let (out, across) = (Vec3::new(c3, s3, 0.0), Vec3::new(-s3, c3, 0.0));
                for (way, name) in [(1.0, "cone_upper"), (-1.0, "cone_lower")] {
                    let (x, y) = (turn(&place, Vec3::new(0.0, 0.0, way)), turn(&place, across * way));
                    let centre = put(&place, apex + out * gap);
                    let branch = Hyperbola::restore(&numbers(centre, x, y, &[gap * cos / sin, gap], false)).unwrap();
                    let vertex = branch.eval_point(0.0).to_vec3();
                    beside(&format!("{name}[{seed}]"), branch, &cone, vertex, (-1.5, 1.5));
                }
            }
        }

        #[test]
        fn images_on_tori_and_planes_hold_their_references() {
            let mut dice = Dice(13);
            for seed in 0..24 {
                let place = dice.placement();
                let (major, minor) = (dice.within(2.0, 3.0), dice.within(0.3, 0.8));
                let torus = Torus::placed(&place, mm(major), mm(minor)).unwrap();
                let tube = dice.within(-PI + 0.05, PI - 0.05);
                let (sv, cv) = sin_cos(tube);
                let (x, y) = (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0));
                let flat = ring(&place, Vec3::new(0.0, 0.0, minor * sv), (x, y), major + minor * cv);
                conic(&format!("torus_ring[{seed}]"), flat, &torus, true);
                let (su, cu) = sin_cos(dice.within(-PI, PI));
                let out = Vec3::new(cu, su, 0.0);
                let middle = dice.within(-PI, PI);
                let arc = ring(&place, out * major, (out, Vec3::new(0.0, 0.0, 1.0)), minor)
                    .trimmed(middle - 2.5, middle + 2.5)
                    .unwrap();
                let toward = arc.eval_point(middle).to_vec3();
                beside(&format!("torus_meridian[{seed}]"), arc, &torus, toward, (middle - 2.5, middle + 2.5));
                let sheet = Plane::placed(&place);
                let up = Vec3::new(0.0, 0.0, 1.0);
                let x = square(up, &mut dice);
                let radius = dice.within(0.5, 3.0);
                let centre = Vec3::new(dice.within(-2.0, 2.0), dice.within(-2.0, 2.0), 0.0);
                let circle = ring(&place, centre, (x, up.cross(x)), radius);
                conic(&format!("plane[{seed}]"), circle, &sheet, false);
            }
        }
    }
}
