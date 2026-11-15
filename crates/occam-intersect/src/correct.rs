//! Source: barnhill-kersey-1990
//! The trace's corrector (Barnhill and Kersey): from a predicted point, Newton's method to the point where the two
//! surfaces meet in the plane through it normal to the curve, on the parameters of both surfaces, three equations of
//! their points and one of the plane. Where one side presents an analytic form, its implicit form takes the place of
//! its parameters, two equations on the other side's (`implicit`). Also the start's settling onto the curve, by the
//! least change of parameters, and each surface's normal.

use occam_geom::{Param, ParamRange, Scratch, Surface, Wrap};
use occam_units::{Direction, Point, Tolerance, UvPoint, Vec3};

use crate::implicit::Implicit;
use crate::trace::{Start, Station, Which};

/// The most Newton steps a correction takes.
const NEWTON: u32 = 12;

/// The most Gauss-Newton steps a foot takes.
const FOOT: u32 = 16;

/// A pivot this small against the largest entry makes a matrix singular to working precision.
const SINGULAR: f64 = 1e-13;

/// `x` with `m x = r`, by Gaussian elimination with partial pivoting, or `None` where `m` is singular to working
/// precision or not finite.
#[expect(clippy::indexing_slicing, reason = "every index is below `N`, the length of each array")]
pub(crate) fn solve<const N: usize>(mut m: [[f64; N]; N], mut r: [f64; N]) -> Option<[f64; N]> {
    let scale = m.iter().flatten().fold(0.0, |most: f64, x| if x.abs() > most { x.abs() } else { most });
    for col in 0..N {
        let pivot = (col..N).fold(col, |best, row| if m[row][col].abs() > m[best][col].abs() { row } else { best });
        let usable = m[pivot][col].abs() > SINGULAR * scale;
        if !usable {
            return None;
        }
        m.swap(col, pivot);
        r.swap(col, pivot);
        let (top, rest) = (m[col], r[col]);
        for row in (col..N).skip(1) {
            let factor = m[row][col] / top[col];
            for (entry, above) in m[row].iter_mut().zip(top).skip(col) {
                *entry -= factor * above;
            }
            r[row] -= factor * rest;
        }
    }
    let mut x = [0.0; N];
    for row in (0..N).rev() {
        let known = (row..N).skip(1).fold(r[row], |sum, k| sum - m[row][k] * x[k]);
        x[row] = known / m[row][row];
    }
    x.iter().all(|value| value.is_finite()).then_some(x)
}

/// A station in the trace's own order: `p` is the side Newton's method solves on, `q` the other.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Here {
    /// The point.
    pub at: Vec3,
    /// Its parameters on `p`.
    pub p: UvPoint,
    /// Its parameters on `q`.
    pub q: UvPoint,
}

/// One of the pair's sides, in the trace's own order.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Side {
    /// The side Newton's method solves on.
    P,
    /// The other.
    Q,
}

/// How the other side is held.
#[derive(Clone, Copy, Debug)]
pub(crate) enum Other {
    /// By its implicit form, its parameters found by its foot.
    Implicit(Implicit),
    /// By its parameters, which Newton's method solves for beside `p`'s.
    Parametric,
}

/// The two surfaces as the trace sees them.
pub(crate) struct Pair<'s> {
    /// The side Newton's method solves on.
    pub p: &'s dyn Surface,
    /// The other.
    pub q: &'s dyn Surface,
    /// How `q` is held.
    pub other: Other,
    /// Whether `p` is the second surface given.
    pub swapped: bool,
    /// The tolerance.
    pub tol: Tolerance,
}

/// `uv` less the parameter change `d`.
fn less(uv: UvPoint, d: (f64, f64)) -> UvPoint {
    UvPoint::new(uv.u() - d.0, uv.v() - d.1)
}

impl<'s> Pair<'s> {
    /// The pair `a` and `b`, the second held by its implicit form where it presents an analytic one, else the first
    /// where it does, else both by their parameters.
    pub(crate) fn new(a: &'s dyn Surface, b: &'s dyn Surface, tol: Tolerance) -> Self {
        let held = |surface: &dyn Surface| surface.analytic(tol).map(|form| Other::Implicit(Implicit::of(form)));
        match (held(b), held(a)) {
            (Some(other), _) => Self { p: a, q: b, other, swapped: false, tol },
            (None, Some(other)) => Self { p: b, q: a, other, swapped: true, tol },
            (None, None) => Self { p: a, q: b, other: Other::Parametric, swapped: false, tol },
        }
    }

    /// The band.
    pub(crate) fn band(&self) -> f64 {
        self.tol.band().millimetres()
    }

    /// The surface on `side`.
    pub(crate) fn surface(&self, side: Side) -> &'s dyn Surface {
        match side {
            Side::P => self.p,
            Side::Q => self.q,
        }
    }

    /// Which surface given `side` is.
    pub(crate) const fn which(&self, side: Side) -> Which {
        match (side, self.swapped) {
            (Side::P, false) | (Side::Q, true) => Which::A,
            (Side::P, true) | (Side::Q, false) => Which::B,
        }
    }

    /// The start's parameters in the trace's order, and the point between its two points.
    pub(crate) fn seed(&self, start: Start) -> Here {
        let (p, q) = if self.swapped { (start.on_b, start.on_a) } else { (start.on_a, start.on_b) };
        let at = (self.p.point(p).to_vec3() + self.q.point(q).to_vec3()) * 0.5;
        Here { at, p, q }
    }

    /// A station in the order the surfaces were given.
    pub(crate) fn station(&self, here: &Here) -> Station {
        let (on_a, on_b) = if self.swapped { (here.q, here.p) } else { (here.p, here.q) };
        Station { at: Point::from_vec3(here.at), on_a, on_b }
    }

    /// The unit normal of `surface` at `uv`: from its derivatives, or where they lose a direction, the normal declared
    /// for the line of the domain that collapses there, as at a sphere's pole.
    fn normal_of(surface: &dyn Surface, uv: UvPoint) -> Option<Vec3> {
        let jet = surface.first(uv);
        let (du, dv) = (jet.du.rate(), jet.dv.rate());
        let cross = du.cross(dv);
        if cross.norm() > SINGULAR * du.norm() * dv.norm() && cross.norm().is_finite() {
            return Some(cross * (1.0 / cross.norm()));
        }
        let on = |param: Param, at: f64| {
            let value = if param == Param::U { uv.u() } else { uv.v() };
            (value - at).abs() <= 1e-9 * (1.0 + at.abs())
        };
        surface
            .degeneracies()
            .iter()
            .find(|line| on(line.param, line.at))
            .and_then(|line| line.normal.map(Direction::get))
    }

    /// The unit normals of `p` and `q` at `here`, where both have one.
    pub(crate) fn normals(&self, here: &Here) -> Option<(Vec3, Vec3)> {
        let on_q = match self.other {
            Other::Implicit(form) => Some(form.at(here.at).1).filter(|n| n.norm().is_finite()),
            Other::Parametric => Self::normal_of(self.q, here.q),
        };
        Some((Self::normal_of(self.p, here.p)?, on_q?))
    }

    /// The gap between the two surfaces at a point `at` of `p`, held by `q`'s parameters `q` where it is parametric.
    fn gap(&self, at: Vec3, q: UvPoint) -> f64 {
        match self.other {
            Other::Implicit(form) => form.at(at).0.abs(),
            Other::Parametric => (at - self.q.point(q).to_vec3()).norm(),
        }
    }

    /// `here` with `q`'s parameters those of its point's foot, from `seed`, where it is within the band of `q`.
    fn footed(&self, at: Vec3, p: UvPoint, seed: UvPoint) -> Option<Here> {
        Some(Here { at, p, q: self.q_at(at, seed)? })
    }

    /// `q`'s parameters at a point `at` within the band of it: its foot from `seed`, or where that is not within the
    /// band, as from a seed on a cone's other nappe, its foot from those of `q`'s closest point, each parameter that
    /// comes round taken in the turn nearest the seed's, so that the parameters along a trace run on without a jump.
    pub(crate) fn q_at(&self, at: Vec3, seed: UvPoint) -> Option<UvPoint> {
        let held = |q: UvPoint| (self.q.point(q).to_vec3() - at).norm() <= self.band();
        let q = foot(self.q, at, seed);
        if held(q) {
            return Some(q);
        }
        let again = self.q.closest(Point::from_vec3(at), Some(seed), self.tol, &mut Scratch::default()).ok()?.at;
        let domain = self.q.domain();
        let near = |range: ParamRange, t: f64, to: f64| {
            let period = range.hi - range.lo;
            if range.wrap == Wrap::Periodic { t + period * ((to - t) / period).round() } else { t }
        };
        let again = UvPoint::new(near(domain.u, again.u(), seed.u()), near(domain.v, again.v(), seed.v()));
        let q = foot(self.q, at, again);
        held(q).then_some(q)
    }

    /// The point where the two surfaces meet in the plane through `aim` normal to `along`, by Newton's method from
    /// `seed`'s parameters, within a quarter of the band of both.
    pub(crate) fn correct(&self, seed: &Here, aim: Vec3, along: Vec3) -> Option<Here> {
        match self.other {
            Other::Implicit(form) => self.implicit(form, seed, aim, along),
            Other::Parametric => self.parametric(seed, aim, along),
        }
    }

    /// [`Pair::correct`] on `p`'s parameters alone, `q` held by `form`.
    fn implicit(&self, form: Implicit, seed: &Here, aim: Vec3, along: Vec3) -> Option<Here> {
        let mut p = seed.p;
        for _ in 0..NEWTON {
            let jet = self.p.first(p);
            let (at, du, dv) = (jet.point.to_vec3(), jet.du.rate(), jet.dv.rate());
            let (value, gradient) = form.at(at);
            let plane = along.dot(at - aim);
            if value.abs() <= self.band() * 0.25 && plane.abs() <= self.band() {
                return self.footed(at, p, seed.q);
            }
            let [step_u, step_v] =
                solve([[gradient.dot(du), gradient.dot(dv)], [along.dot(du), along.dot(dv)]], [value, plane])?;
            p = less(p, (step_u, step_v));
        }
        None
    }

    /// [`Pair::correct`] on the parameters of both.
    fn parametric(&self, seed: &Here, aim: Vec3, along: Vec3) -> Option<Here> {
        let mut here = *seed;
        for _ in 0..NEWTON {
            let (on_p, on_q) = (self.p.first(here.p), self.q.first(here.q));
            let at = on_p.point.to_vec3();
            let (gap, plane) = (at - on_q.point.to_vec3(), along.dot(at - aim));
            if gap.norm() <= self.band() * 0.25 && plane.abs() <= self.band() {
                return Some(Here { at, ..here });
            }
            let (pu, pv, qu, qv) = (on_p.du.rate(), on_p.dv.rate(), on_q.du.rate(), on_q.dv.rate());
            let row = |part: fn(Vec3) -> f64| [part(pu), part(pv), -part(qu), -part(qv)];
            let matrix = [row(|v| v.x), row(|v| v.y), row(|v| v.z), [along.dot(pu), along.dot(pv), 0.0, 0.0]];
            let step = solve(matrix, [gap.x, gap.y, gap.z, plane])?;
            here = Here { p: less(here.p, (step[0], step[1])), q: less(here.q, (step[2], step[3])), ..here };
        }
        None
    }

    /// The point where the two surfaces meet nearest `seed`, by Newton's method on the least change of parameters,
    /// with no plane to hold it: how a start comes onto the curve. `q`'s parameters come from its foot, for the side
    /// held implicitly, so that they do not depend on the start's guess at them.
    pub(crate) fn settle(&self, seed: &Here) -> Option<Here> {
        let mut here = *seed;
        for _ in 0..NEWTON {
            let on_p = self.p.first(here.p);
            let (at, pu, pv) = (on_p.point.to_vec3(), on_p.du.rate(), on_p.dv.rate());
            if self.gap(at, here.q) <= self.band() * 0.25 {
                return match self.other {
                    Other::Implicit(_) => {
                        let found = self.q.closest(on_p.point, None, self.tol, &mut Scratch::default()).ok()?.at;
                        self.footed(at, here.p, found)
                    }
                    Other::Parametric => Some(Here { at, ..here }),
                };
            }
            here = match self.other {
                Other::Implicit(form) => {
                    let (value, gradient) = form.at(at);
                    let (along_u, along_v) = (gradient.dot(pu), gradient.dot(pv));
                    let length = along_u * along_u + along_v * along_v;
                    Here { p: less(here.p, (value * along_u / length, value * along_v / length)), ..here }
                }
                Other::Parametric => {
                    let on_q = self.q.first(here.q);
                    let step = least_change([pu, pv, -on_q.du.rate(), -on_q.dv.rate()], at - on_q.point.to_vec3())?;
                    Here { p: less(here.p, (step[0], step[1])), q: less(here.q, (step[2], step[3])), ..here }
                }
            };
        }
        None
    }
}

/// The least change of four parameters that, to first order, moves a point by `gap`, where each parameter moves it
/// along its column: `Jt (J Jt)^-1 gap` for the matrix `J` of the columns.
fn least_change(columns: [Vec3; 4], gap: Vec3) -> Option<[f64; 4]> {
    let gram = |i: Vec3, j: Vec3| columns.iter().fold(0.0, |sum, column| sum + column.dot(i) * column.dot(j));
    let axes = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
    let [x, y, z] = solve(axes.map(|i| axes.map(|j| gram(i, j))), [gap.x, gap.y, gap.z])?;
    Some(columns.map(|column| column.dot(Vec3::new(x, y, z))))
}

/// The parameters of the foot on `surface` of a point `x` near it, by Gauss-Newton from `seed`, unclamped, so that
/// they run on past an edge of the domain as the surface's formula does. The step is damped, so that a parameter that
/// moves no point, as at a pole, stays where it is.
pub(crate) fn foot(surface: &dyn Surface, x: Vec3, seed: UvPoint) -> UvPoint {
    let mut uv = seed;
    for _ in 0..FOOT {
        let jet = surface.first(uv);
        let (r, su, sv) = (jet.point.to_vec3() - x, jet.du.rate(), jet.dv.rate());
        let damping = 1e-12 * (su.dot(su) + sv.dot(sv));
        let normal = [[su.dot(su) + damping, su.dot(sv)], [su.dot(sv), sv.dot(sv) + damping]];
        let Some([du, dv]) = solve(normal, [r.dot(su), r.dot(sv)]) else { break };
        uv = less(uv, (du, dv));
        if (su * du + sv * dv).norm() <= 1e-15 * (1.0 + x.norm()) {
            break;
        }
    }
    uv
}
