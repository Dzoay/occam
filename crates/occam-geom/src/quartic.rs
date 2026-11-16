//! Source: levin-1979
//! [`Quartic`]: a loop of the smooth quartic where two quadrics meet, as Levin's pencil method gives it (Levin 1976,
//! 1979). A ruled quadric of the pair, a cylinder or a cone, carries the loop along its rulings: the ruling at the angle
//! `θ` about its axis is `P(θ) + v G(θ)`, and the other quadric leaves on it the quadratic `α v² + 2β v + γ`, so the
//! loop is `v = (−β ± √Δ) / α`, where `Δ = β² − αγ` and `α`, `β` and `Δ` are polynomials in `cos θ` and `sin θ`.
//!
//! The two signs join smoothly where `Δ` vanishes, because the loop is followed by an angle `s` between two roots of
//! `Δ`. With `m` and `h` the middle and half width of the arc of `θ` where `Δ` is positive, `θ = m + φ`, where
//! `sin(φ/2) = sin(h/2) cos s`, and `Δ = F E` with `F = cos(θ − m) − cos h = 2 sin²(h/2) sin² s`, so that
//! `±√Δ = √2 sin(h/2) sin s √E`, smooth through both roots. A loop round the whole turn, where `Δ` never vanishes,
//! keeps one sign, and its angle is `θ` itself. Either way the loop is periodic and smooth, with rates of every order,
//! and exact, a closed form evaluated in floating point. A smooth quartic has no rational form, so `to_bspline` gives
//! none.

use std::f64::consts::SQRT_2;

use occam_math::{Interval, Real};
use occam_report::{NamedValue, Quantity, Report};
use occam_units::{Affine, Bounds, Point, Tangent, Vec3, World};

use crate::axes::Axes;
use crate::batch::Scratch;
use crate::curve::ParametricCurve;
use crate::domain::{ParamMap, ParamRange};
use crate::field::{Field, Jet, Taylor};
use crate::identity::{Content, FamilyName, Shape};
use crate::restore::{Numbers, out_of_range, wrap_code};
use crate::series;
use crate::support::{enclose, everything, span};
use crate::turn::turn;

/// The ruled quadric that carries the loop, on [`Quartic::axes`].
#[derive(Clone, Copy, Debug, PartialEq)]
enum Ruled {
    /// `P(θ) = origin + r (x cos θ + y sin θ)`, `G = z`.
    Upright { radius: f64 },
    /// `P = origin`, the apex, and `G(θ) = sin a (x cos θ + y sin θ) + cos a z`, over both nappes.
    Slanted { sin: f64, cos: f64 },
}

/// How the root's sign is taken.
#[derive(Clone, Copy, Debug, PartialEq)]
enum Root {
    /// Round the whole turn, `θ = s`, with `sign √Δ`.
    Whole { delta: [f64; 5], sign: f64 },
    /// Between two roots of `Δ`, about the middle `(cos m, sin m)`, with `k = sin(h/2)` and `E` of degree one.
    Between { middle: (f64, f64), k: f64, e: [f64; 3] },
}

/// A loop of a smooth quartic on a cylinder or a cone.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Quartic {
    axes: Axes,
    ruled: Ruled,
    alpha: [f64; 5],
    beta: [f64; 3],
    root: Root,
    range: ParamRange,
}

/// `c₀ + c₁ cos θ + c₂ sin θ + c₃ cos 2θ + c₄ sin 2θ`.
fn second<F: Field>(c: &[f64; 5], (cos, sin): (&F, &F)) -> F {
    let [c0, c1, c2, c3, c4] = *c;
    let double_cos = cos.times(cos).minus(&sin.times(sin));
    let double_sin = cos.times(sin).scaled(2.0);
    let first = cos.scaled(c1).plus(&sin.scaled(c2)).plus(&cos.constant(c0));
    first.plus(&double_cos.scaled(c3)).plus(&double_sin.scaled(c4))
}

/// `c₀ + c₁ cos θ + c₂ sin θ`.
fn first<F: Field>(c: &[f64; 3], (cos, sin): (&F, &F)) -> F {
    let [c0, c1, c2] = *c;
    cos.scaled(c1).plus(&sin.scaled(c2)).plus(&cos.constant(c0))
}

/// The error for a number a loop cannot have: `geom.out-of-range`, against the highest it could be.
fn refused(value: f64, high: f64) -> Report {
    out_of_range(value, high)
}

impl Quartic {
    /// A loop saved as its content: the numbers after the family's name, in the order written. They are the axes, the
    /// ruled quadric (0 and a radius for a cylinder, 1 and the sine and cosine of its half-angle for a cone), `α`'s
    /// five coefficients and `β`'s three, the way the root is taken (0 with `Δ`'s five and the sign, or 1 with
    /// `cos m`, `sin m`, `sin(h/2)` and `E`'s three), and the range.
    ///
    /// # Errors
    ///
    /// `geom.wrong-count` for too few or too many numbers, `geom.degenerate-direction` for axes no placement makes,
    /// `geom.out-of-range` for a size, a kind, a sign or `sin(h/2)` no loop has, and as [`ParamRange::trimmed`] for the
    /// range.
    pub fn restore(numbers: &[f64]) -> Result<Self, Report> {
        let mut read = Numbers::new(numbers);
        let axes = Axes::restore(&mut read)?;
        let kind = read.flag()?;
        let (a, b) = (read.next()?, read.next()?);
        let ruled = if kind {
            if !(a > 0.0 && b > 0.0 && ((a * a + b * b) - 1.0).abs() <= 1e-12) {
                return Err(refused(a, 1.0));
            }
            Ruled::Slanted { sin: a, cos: b }
        } else {
            if !(a > 0.0 && a.is_finite() && b == 0.0) {
                return Err(refused(a, f64::MAX));
            }
            Ruled::Upright { radius: a }
        };
        let take = |read: &mut Numbers<'_>, n: usize| read.take(n).map(<[f64]>::to_vec);
        let alpha: [f64; 5] = take(&mut read, 5)?.try_into().map_err(|_| refused(0.0, 5.0))?;
        let beta: [f64; 3] = take(&mut read, 3)?.try_into().map_err(|_| refused(0.0, 3.0))?;
        let root = if read.flag()? {
            let [cm, sm, k, e0, e1, e2]: [f64; 6] = take(&mut read, 6)?.try_into().map_err(|_| refused(0.0, 6.0))?;
            if !(k > 0.0 && k < 1.0 && ((cm * cm + sm * sm) - 1.0).abs() <= 1e-12) {
                return Err(refused(k, 1.0));
            }
            Root::Between { middle: (cm, sm), k, e: [e0, e1, e2] }
        } else {
            let all = take(&mut read, 6)?;
            let (delta, sign) = all.split_at(5);
            let delta: [f64; 5] = delta.try_into().map_err(|_| refused(0.0, 5.0))?;
            let sign = sign.first().copied().unwrap_or(0.0);
            if sign.abs().to_bits() != 1.0_f64.to_bits() {
                return Err(refused(sign, 1.0));
            }
            Root::Whole { delta, sign }
        };
        let finite = alpha.iter().chain(&beta).all(|x| x.is_finite());
        if !finite {
            return Err(refused(f64::NAN, f64::MAX));
        }
        let range = read.range(turn(), true)?;
        read.done()?;
        Ok(Self { axes, ruled, alpha, beta, root, range })
    }

    /// The part of the loop from `lo` to `hi`.
    ///
    /// # Errors
    ///
    /// As [`ParamRange::trimmed`].
    pub fn trimmed(self, lo: f64, hi: f64) -> Result<Self, Report> {
        Ok(Self { range: self.range.trimmed(lo, hi)?, ..self })
    }

    /// `(cos θ, sin θ)` and the signed root at the loop's parameter `s`.
    fn angle<F: Field>(&self, s: &F) -> ((F, F), F) {
        let (sin_s, cos_s) = s.sin_cos();
        match self.root {
            Root::Whole { delta, sign } => {
                let delta = second(&delta, (&cos_s, &sin_s));
                ((cos_s, sin_s), delta.root().scaled(sign))
            }
            Root::Between { middle: (cm, sm), k, e } => {
                let one = s.constant(1.0);
                let half = cos_s.scaled(k);
                let square = half.times(&half);
                let (cos_phi, sin_phi) =
                    (one.minus(&square.scaled(2.0)), half.times(&one.minus(&square).root()).scaled(2.0));
                let cos = cos_phi.scaled(cm).minus(&sin_phi.scaled(sm));
                let sin = cos_phi.scaled(sm).plus(&sin_phi.scaled(cm));
                let e = first(&e, (&cos, &sin));
                let root = sin_s.times(&e.root()).scaled(SQRT_2 * k);
                ((cos, sin), root)
            }
        }
    }

    /// The loop's point at `s`, in any field.
    fn at<F: Field>(&self, s: &F) -> [F; 3] {
        let ((cos, sin), root) = self.angle(s);
        let alpha = second(&self.alpha, (&cos, &sin));
        let v = root.minus(&first(&self.beta, (&cos, &sin))).over(&alpha);
        let Axes { origin, x, y, normal } = self.axes;
        let z = normal.get();
        let coordinate = |o: f64, x: f64, y: f64, z: f64| -> F {
            let out = cos.scaled(x).plus(&sin.scaled(y));
            match self.ruled {
                Ruled::Upright { radius } => out.scaled(radius).plus(&v.scaled(z)).plus(&s.constant(o)),
                Ruled::Slanted { sin: sa, cos: ca } => {
                    v.times(&out.scaled(sa).plus(&s.constant(ca * z))).plus(&s.constant(o))
                }
            }
        };
        [coordinate(origin.x, x.x, y.x, z.x), coordinate(origin.y, x.y, y.y, z.y), coordinate(origin.z, x.z, y.z, z.z)]
    }

    /// The numbers its content holds after the family's name, as [`Quartic::restore`] reads them.
    fn numbers(&self) -> Vec<f64> {
        let mut out = Vec::with_capacity(33);
        let Axes { origin, x, y, normal } = self.axes;
        for v in [origin, x, y, normal.get()] {
            out.extend([v.x, v.y, v.z]);
        }
        out.extend(match self.ruled {
            Ruled::Upright { radius } => [0.0, radius, 0.0],
            Ruled::Slanted { sin, cos } => [1.0, sin, cos],
        });
        out.extend(self.alpha.iter().chain(&self.beta));
        match self.root {
            Root::Whole { delta, sign } => {
                out.push(0.0);
                out.extend(delta.iter().chain(&[sign]));
            }
            Root::Between { middle: (cm, sm), k, e } => {
                out.extend([1.0, cm, sm, k]);
                out.extend(e);
            }
        }
        out.extend([self.range.lo, self.range.hi, wrap_code(self.range.wrap)]);
        out
    }
}

/// A jet's three coordinates as a point and its two rates.
fn rungs<R: Real>([x, y, z]: [Jet<R>; 3]) -> (Point<World, R>, Tangent<World, R>, Tangent<World, R>) {
    (
        Point::from_vec3(Vec3::new(x.value, y.value, z.value)),
        Tangent::new(Vec3::new(x.first, y.first, z.first)),
        Tangent::new(Vec3::new(x.second, y.second, z.second)),
    )
}

impl ParametricCurve for Quartic {
    #[inline(always)]
    fn eval_point<R: Real>(&self, t: R) -> Point<World, R> {
        rungs(self.at(&Jet::variable(t))).0
    }

    #[inline(always)]
    fn eval_first<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>) {
        let (point, first, _) = rungs(self.at(&Jet::variable(t)));
        (point, first)
    }

    #[inline(always)]
    fn eval_second<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>, Tangent<World, R>) {
        rungs(self.at(&Jet::variable(t)))
    }

    /// The first two rates as the rungs give them, and every higher one from Taylor series of the same closed form.
    fn derivatives(&self, t: f64, order: u8, out: &mut Vec<Tangent<World>>, _: &mut Scratch) -> Result<(), Report> {
        let (_, first, second) = self.eval_second(t);
        out.extend([first, second].into_iter().take(usize::from(order)));
        if order > 2 {
            let [x, y, z] = self.at(&Taylor::variable(t, order)).map(|c| series::derivatives(&c.0));
            let rates = x.into_iter().zip(y).zip(z).map(|((x, y), z)| Tangent::new(Vec3::new(x, y, z)));
            out.extend(rates.skip(2));
        }
        Ok(())
    }

    fn range(&self) -> ParamRange {
        self.range
    }

    /// The union of boxes the closed form gives in intervals over sixteen pieces from `lo` to `hi`.
    fn bounds(&self, lo: f64, hi: f64) -> Bounds {
        if !(lo.is_finite() && hi.is_finite()) {
            return everything();
        }
        let pieces: u32 = if hi > lo { 16 } else { 1 };
        let step = (hi - lo) / f64::from(pieces);
        let boxes = (0..pieces).map(|i| {
            let from = lo + step * f64::from(i);
            let to = if i.saturating_add(1) == pieces { hi } else { from + step };
            self.at::<Interval>(&span(from, to))
        });
        enclose(boxes)
    }

    fn family(&self) -> FamilyName {
        FamilyName("quartic")
    }

    fn content(&self, out: &mut Content) {
        for x in self.numbers() {
            out.push(x);
        }
    }

    fn shape(&self) -> Shape {
        let size = match self.ruled {
            Ruled::Upright { radius } => NamedValue::length("radius", radius),
            Ruled::Slanted { sin, .. } => NamedValue::new("half-angle-sine", Quantity::Ratio, sin),
        };
        Shape { form: "quartic", values: self.axes.curve_values("origin", &[size], self.range) }
    }

    /// A similarity keeps a loop: the axes move, lengths scale, and the angle stays.
    fn moved(&self, map: &Affine) -> Option<(Self, ParamMap)> {
        let similar = map.similar()?;
        let k = similar.scale;
        let ruled = match self.ruled {
            Ruled::Upright { radius } => Ruled::Upright { radius: radius * k },
            cone @ Ruled::Slanted { .. } => cone,
        };
        let root = match self.root {
            Root::Whole { delta, sign } => Root::Whole { delta: delta.map(|d| d * k * k), sign },
            Root::Between { middle, k: half, e } => Root::Between { middle, k: half, e: e.map(|c| c * k * k) },
        };
        let moved = Self {
            axes: self.axes.moved(&similar),
            ruled,
            alpha: self.alpha,
            beta: self.beta.map(|b| b * k),
            root,
            ..*self
        };
        Some((moved, ParamMap::IDENTITY))
    }
}

#[cfg(test)]
mod tests {
    use std::f64::consts::TAU;

    use occam_math::sin_cos;
    use occam_units::{Direction, Point};

    use super::*;
    use crate::conformance::{self, placement};
    use crate::curve::Curve;

    /// [`placement`]'s origin and axes, as a loop's content writes them.
    fn axes() -> Vec<f64> {
        let p = placement();
        let o = p.point(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0))).to_vec3();
        let axis = |x, y, z| p.direction(Direction::normalised(Vec3::new(x, y, z))).get();
        [o, axis(1.0, 0.0, 0.0), axis(0.0, 1.0, 0.0), axis(0.0, 0.0, 1.0)]
            .iter()
            .flat_map(|v| [v.x, v.y, v.z])
            .collect()
    }

    fn made(parts: &[&[f64]]) -> Quartic {
        let numbers: Vec<f64> = axes().into_iter().chain(parts.iter().flat_map(|p| p.iter().copied())).collect();
        Quartic::restore(&numbers).unwrap_or_else(|r| panic!("{r:?}"))
    }

    const WHOLE: [f64; 3] = [0.0, TAU, 2.0];

    /// On the cylinder of radius 0.6 about the placement's Z, where the cylinder of radius 1 about its X meets it:
    /// `0.36 sin²θ + v² = 1`, round the whole turn, with the sign `sign`.
    fn round(sign: f64) -> Quartic {
        made(&[
            &[0.0, 0.6, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0],
            &[0.82, 0.0, 0.0, 0.18, 0.0, sign],
            &WHOLE,
        ])
    }

    /// On the cylinder of radius 1 about Z, where the cylinder of radius 0.6 about X meets it: `sin²θ + v² = 0.36`,
    /// so `Δ = (cos θ − 0.8)(cos θ + 0.8)`, between the roots about `θ = 0`, where `cos h = 0.8`.
    fn between() -> Quartic {
        let k = 0.1_f64.sqrt();
        made(&[&[0.0, 1.0, 0.0, 1.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0, 0.0], &[1.0, 1.0, 0.0, k, 0.8, 1.0, 0.0], &WHOLE])
    }

    /// On the double cone of half-angle 0.5 with its apex at the placement's origin, where the sphere of radius 0.8
    /// about `(0.3, 0, 1)` meets it: `v² + 2β v + 0.45 = 0`, with `β = −(0.3 sin a cos θ + cos a)`.
    fn on_cone(sign: f64) -> Quartic {
        let (sa, ca) = sin_cos(0.5);
        let delta = [ca * ca - 0.45 + 0.045 * sa * sa, 0.6 * sa * ca, 0.0, 0.045 * sa * sa, 0.0];
        made(&[&[1.0, sa, ca, 1.0, 0.0, 0.0, 0.0, 0.0, -ca, -0.3 * sa, 0.0, 0.0], &delta, &[sign], &WHOLE])
    }

    /// `p` in the placement's frame.
    fn local(p: Vec3) -> Vec3 {
        let frame: Vec<Vec3> = axes().chunks(3).map(|c| Vec3::new(c[0], c[1], c[2])).collect();
        let d = p - frame[0];
        Vec3::new(d.dot(frame[1]), d.dot(frame[2]), d.dot(frame[3]))
    }

    /// How far a point is off the two quadrics a loop was made of, as their implicit functions measure it.
    type Off = fn(Vec3) -> f64;

    #[test]
    fn each_loop_lies_on_both_quadrics_that_made_it() {
        let cases: [(Quartic, Off); 5] = [
            (round(1.0), |p| (p.x * p.x + p.y * p.y - 0.36).abs() + (p.y * p.y + p.z * p.z - 1.0).abs()),
            (round(-1.0), |p| (p.x * p.x + p.y * p.y - 0.36).abs() + (p.y * p.y + p.z * p.z - 1.0).abs()),
            (between(), |p| (p.x * p.x + p.y * p.y - 1.0).abs() + (p.y * p.y + p.z * p.z - 0.36).abs()),
            (on_cone(1.0), |p| {
                let (sa, ca) = sin_cos(0.5);
                let cone = (p.x * p.x + p.y * p.y) * ca * ca - p.z * p.z * sa * sa;
                cone.abs() + ((p - Vec3::new(0.3, 0.0, 1.0)).norm() - 0.8).abs()
            }),
            (on_cone(-1.0), |p| ((p - Vec3::new(0.3, 0.0, 1.0)).norm() - 0.8).abs()),
        ];
        for (i, (q, off)) in cases.iter().enumerate() {
            for j in 0..=400 {
                let s = TAU * f64::from(j) / 400.0;
                let p = local(q.point(s).to_vec3());
                assert!(off(p).abs() <= 1e-13, "{i} at {s}: {} off, at {p:?}", off(p));
            }
        }
    }

    #[test]
    fn a_loop_between_two_roots_passes_through_both_and_joins_its_signs_there() {
        let q = between();
        let (start, half) = (local(q.point(0.0).to_vec3()), local(q.point(std::f64::consts::PI).to_vec3()));
        assert!((start.y - 0.6).abs() < 1e-14 && start.z.abs() < 1e-14, "{start:?}");
        assert!((half.y + 0.6).abs() < 1e-14 && half.z.abs() < 1e-14, "{half:?}");
        let rising = local(q.point(0.5).to_vec3()).z;
        let falling = local(q.point(3.6).to_vec3()).z;
        assert!(rising > 0.1 && falling < -0.1, "{rising} {falling}");
    }

    #[test]
    fn loops_and_their_parts_pass_the_conformance_suite() {
        for (name, q) in [("round", round(1.0)), ("between", between()), ("cone", on_cone(-1.0))] {
            let failures = conformance::curve(&q, 0.0, TAU);
            assert!(failures.is_empty(), "{name}: {failures:#?}");
            let part = q.trimmed(0.4, 2.9).unwrap();
            let failures = conformance::curve(&part, 0.4, 2.9);
            assert!(failures.is_empty(), "{name}, trimmed: {failures:#?}");
            assert!(Curve::to_bspline(&q, 0.0, 1.0).is_none(), "{name} has no rational form");
        }
    }

    #[test]
    fn a_loop_comes_back_from_its_content_and_refuses_what_no_loop_has() {
        for q in [round(1.0), between(), on_cone(1.0), between().trimmed(1.0, 2.0).unwrap()] {
            let back = Quartic::restore(&q.numbers()).unwrap();
            assert_eq!(back, q);
        }
        let mut numbers = between().numbers();
        numbers[25] = 1.5;
        assert_eq!(Quartic::restore(&numbers).err().map(|r| r.code), Some(occam_report::Code::OutOfRange));
        let mut numbers = round(1.0).numbers();
        numbers[29] = 0.5;
        assert_eq!(Quartic::restore(&numbers).err().map(|r| r.code), Some(occam_report::Code::OutOfRange));
        assert_eq!(Curve::family(&round(1.0)), FamilyName("quartic"));
    }
}
