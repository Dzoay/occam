//! Source: levin-1976
//! The pair's quadrics (Levin 1976). Each of a sphere, a cylinder and a cone is the zero set of `Xᵀ M X + 2 q · X + c`,
//! its 4×4 matrix, written in a frame local to the pair so that its numbers keep their digits. A cylinder or a cone is a
//! ruled member of the pencil the two span, a root of its determinant, and is parametrized by its rulings: the ruling
//! at the angle `θ` is `P(θ) + v G(θ)`. Along it the other quadric is `α v² + 2β v + γ`, with `α`, `β` and `γ`
//! polynomials in `cos θ` and `sin θ` of degree at most two, and `Δ = β² − αγ`.
//!
//! The decisions are made on certified intervals of each surface's own numbers: a point's distance from a surface and
//! the surface's normal there, in closed form.

use occam_geom::Analytic;
use occam_math::{Interval, Predicate, Ring, Side, sin_cos};
use occam_units::Vec3;

use super::trig::Trig;
#[cfg(test)]
use crate::closed::{along, cross3};
use crate::closed::{exact3, minus, norm3, square_to, unit};
use crate::decide::{Decisions, Undecided};

/// A sphere, a cylinder or a cone, in world coordinates.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Natural {
    /// About `centre`.
    Sphere { centre: Vec3, radius: f64 },
    /// About the axis through `at` along the unit `axis`.
    Cylinder { at: Vec3, axis: Vec3, radius: f64 },
    /// Both nappes, with its apex at `apex`, opening along the unit `axis` by the half-angle whose sine and cosine are
    /// `sin` and `cos`.
    Cone { apex: Vec3, axis: Vec3, sin: f64, cos: f64 },
}

/// An interval holding `x`, a value the owned `sin_cos` computed within an ulp.
#[cfg(test)]
fn computed(x: f64) -> Interval {
    Interval::new(x, 4.0 * f64::EPSILON * x.abs())
}

/// The 4×4 matrix of a quadric, as its 3×3 part `m`, its linear part `q` and its constant `c`, and a cone's apex,
/// the one point of the surface where its gradient vanishes.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Quadric {
    pub(super) m: [Vec3; 3],
    pub(super) q: Vec3,
    pub(super) c: f64,
    pub(super) apex: Option<Vec3>,
}

impl Quadric {
    /// `M v`.
    pub(super) fn times(&self, v: Vec3) -> Vec3 {
        let [a, b, c] = self.m;
        Vec3::new(a.dot(v), b.dot(v), c.dot(v))
    }

    /// `uᵀ M v`.
    pub(super) fn form(&self, u: Vec3, v: Vec3) -> f64 {
        u.dot(self.times(v))
    }

    /// Its value at `p`.
    pub(super) fn value(&self, p: Vec3) -> f64 {
        self.form(p, p) + 2.0 * self.q.dot(p) + self.c
    }

    /// Half its gradient at `p`, `M p + q`.
    pub(super) fn half_gradient(&self, p: Vec3) -> Vec3 {
        self.times(p) + self.q
    }

    /// Its 4×4 matrix, `[[M, q], [qᵀ, c]]`.
    pub(super) fn homogeneous(&self) -> [[f64; 4]; 4] {
        let ([a, b, c], q) = (self.m, self.q);
        [[a.x, a.y, a.z, q.x], [b.x, b.y, b.z, q.y], [c.x, c.y, c.z, q.z], [q.x, q.y, q.z, self.c]]
    }

    /// Its ten numbers as the predicates read them: the 3×3 part's six, the linear part's three and the constant.
    fn entries(&self) -> [f64; 10] {
        let [a, b, c] = self.m;
        [a.x, b.y, c.z, a.y, a.z, b.z, self.q.x, self.q.y, self.q.z, self.c]
    }

    /// Whether `p` lies on it within the length band: within the band of a cone's apex, where its gradient vanishes
    /// and its value is no distance, or else its value there, a polynomial in its numbers and `p`'s, decided against
    /// the band times the size of its gradient.
    ///
    /// # Errors
    ///
    /// [`Undecided`] where the band cannot tell.
    pub(super) fn holds(&self, p: Vec3, decide: &mut Decisions) -> Result<Side, Undecided> {
        if let Some(apex) = self.apex
            && decide.length(norm3(minus(exact3(p), exact3(apex))))? == Side::Within
        {
            return Ok(Side::Within);
        }
        let inputs = numbers(self.entries(), [p.x, p.y, p.z]);
        decide.length_of(&AtPoint, inputs, 2.0 * self.half_gradient(p).norm())
    }

    /// Whether the unit direction `g` lies on its asymptotic cone within the angle band: `gᵀ M g`, decided against the
    /// band times twice the size of `M g`, how fast it changes as `g` turns near the cone, where `M g` is across `g`;
    /// or a thousandth of `M`'s size where that is smaller, as along a cylinder's axis, where it is zero to rounding.
    ///
    /// # Errors
    ///
    /// [`Undecided`] where the band cannot tell.
    pub(super) fn toward(&self, g: Vec3, decide: &mut Decisions) -> Result<Side, Undecided> {
        let turning = 2.0 * self.times(g).norm();
        let size = self.m.iter().map(|row| row.dot(*row)).sum::<f64>().sqrt();
        let inputs = numbers(self.entries(), [g.x, g.y, g.z]);
        decide.angle_of(&Toward, inputs, if turning > 1e-3 * size { turning } else { 1e-3 * size })
    }

    /// The quadric whose 3×3 part is `m` with `q` and `c` placed for the point `at`: `(X − at)ᵀ m (X − at) + k`.
    fn about(m: [Vec3; 3], at: Vec3, k: f64) -> Self {
        let mut quadric = Self { m, q: Vec3::new(0.0, 0.0, 0.0), c: 0.0, apex: None };
        let moved = quadric.times(at);
        quadric.q = -moved;
        quadric.c = at.dot(moved) + k;
        quadric
    }
}

/// `first` then `then`, each exactly, as a predicate's inputs.
fn numbers<const N: usize, const M: usize, const K: usize>(first: [f64; N], then: [f64; M]) -> [Interval; K] {
    std::array::from_fn(|i| {
        Interval::exact(first.get(i).copied().or_else(|| then.get(i.wrapping_sub(N)).copied()).unwrap_or(0.0))
    })
}

/// `xᵀ M x + 2 q · x + c` for a symmetric `M`, from its ten numbers and `x`'s three.
fn quadratic<T: Ring>(n: &[T], x: &[T; 3], linear: bool) -> T {
    let at = |i: usize| n.get(i).cloned().unwrap_or_else(|| T::of(0.0));
    let [x0, x1, x2] = x;
    let square = at(0).mul(x0).mul(x0).add(&at(1).mul(x1).mul(x1)).add(&at(2).mul(x2).mul(x2));
    let cross = at(3).mul(x0).mul(x1).add(&at(4).mul(x0).mul(x2)).add(&at(5).mul(x1).mul(x2));
    let form = square.add(&cross.add(&cross));
    if !linear {
        return form;
    }
    let line = at(6).mul(x0).add(&at(7).mul(x1)).add(&at(8).mul(x2));
    form.add(&line).add(&line).add(&at(9))
}

/// A quadric's value at a point.
struct AtPoint;

impl Predicate<13> for AtPoint {
    const DEGREE: u32 = 3;

    fn value<T: Ring>(&self, inputs: &[T; 13]) -> T {
        let (n, x) = inputs.split_at(10);
        let x: [T; 3] = std::array::from_fn(|i| x.get(i).cloned().unwrap_or_else(|| T::of(0.0)));
        quadratic(n, &x, true)
    }
}

/// A quadric's 3×3 part at a direction, `gᵀ M g`.
struct Toward;

impl Predicate<13> for Toward {
    const DEGREE: u32 = 3;

    fn value<T: Ring>(&self, inputs: &[T; 13]) -> T {
        let (n, g) = inputs.split_at(10);
        let g: [T; 3] = std::array::from_fn(|i| g.get(i).cloned().unwrap_or_else(|| T::of(0.0)));
        quadratic(n, &g, false)
    }
}

/// `scale I − d dᵀ`, row by row.
fn minus_outer(scale: f64, d: Vec3) -> [Vec3; 3] {
    let unit_rows = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
    unit_rows.map(|e| e * scale - d * e.dot(d))
}

impl Natural {
    /// The surface `form` is, if it is a sphere, a cylinder or a cone.
    pub(super) fn of(form: Analytic) -> Option<Self> {
        Some(match form {
            Analytic::Sphere { centre, radius } => {
                Self::Sphere { centre: centre.to_vec3(), radius: radius.millimetres() }
            }
            Analytic::Cylinder { on_axis, axis, radius } => {
                Self::Cylinder { at: on_axis.to_vec3(), axis: axis.get(), radius: radius.millimetres() }
            }
            Analytic::Cone { apex, axis, half_angle } => {
                let (sin, cos) = sin_cos(half_angle.radians());
                Self::Cone { apex: apex.to_vec3(), axis: axis.get(), sin, cos }
            }
            Analytic::Plane { .. } | Analytic::Torus { .. } => return None,
        })
    }

    /// The point it is placed by: a centre, a point of the axis or the apex.
    pub(super) const fn at(&self) -> Vec3 {
        match *self {
            Self::Sphere { centre, .. } => centre,
            Self::Cylinder { at, .. } => at,
            Self::Cone { apex, .. } => apex,
        }
    }

    /// Its quadric, in the frame whose origin is `origin`.
    pub(super) fn quadric(&self, origin: Vec3) -> Quadric {
        match *self {
            Self::Sphere { centre, radius } => {
                Quadric::about(minus_outer(1.0, Vec3::new(0.0, 0.0, 0.0)), centre - origin, -radius * radius)
            }
            Self::Cylinder { at, axis, radius } => {
                Quadric::about(minus_outer(1.0, axis), at - origin, -radius * radius)
            }
            Self::Cone { apex, axis, cos, .. } => Quadric {
                apex: Some(apex - origin),
                ..Quadric::about(minus_outer(cos * cos, axis), apex - origin, 0.0)
            },
        }
    }

    /// Its distance from `p`, signed, positive outside, certified: exact for a sphere and a cylinder, and for a cone the
    /// distance from the line of the nearer ruling, which is the distance wherever that ruling's foot is on its nappe.
    #[cfg(test)]
    pub(super) fn gap(&self, p: Vec3) -> Interval {
        let p = exact3(p);
        match *self {
            Self::Sphere { centre, radius } => norm3(minus(p, exact3(centre))) - Interval::exact(radius),
            Self::Cylinder { at, axis, radius } => {
                norm3(cross3(minus(p, exact3(at)), exact3(axis))) - Interval::exact(radius)
            }
            Self::Cone { apex, axis, sin, cos } => {
                let q = minus(p, exact3(apex));
                let rho = norm3(cross3(q, exact3(axis)));
                let h = along(p, exact3(apex), exact3(axis));
                let h = if h.mid() < 0.0 { -h } else { h };
                rho * computed(cos) - h * computed(sin)
            }
        }
    }

    /// Its unit normal at `p`, a point on or near it, certified.
    #[cfg(test)]
    pub(super) fn normal(&self, p: Vec3) -> [Interval; 3] {
        let p = exact3(p);
        let scaled = |v: [Interval; 3], by: Interval| v.map(|x| x * by);
        let plus = |a: [Interval; 3], b: [Interval; 3]| [a[0] + b[0], a[1] + b[1], a[2] + b[2]];
        let radial = |at: Vec3, axis: Vec3| {
            let q = minus(p, exact3(at));
            let h = along(p, exact3(at), exact3(axis));
            let out = minus(q, scaled(exact3(axis), h));
            (scaled(out, Interval::exact(1.0) / norm3(out)), h)
        };
        match *self {
            Self::Sphere { centre, .. } => {
                let q = minus(p, exact3(centre));
                scaled(q, Interval::exact(1.0) / norm3(q))
            }
            Self::Cylinder { at, axis, .. } => radial(at, axis).0,
            Self::Cone { apex, axis, sin, cos } => {
                let (out, h) = radial(apex, axis);
                let down = if h.mid() < 0.0 { computed(sin) } else { -computed(sin) };
                plus(scaled(out, computed(cos)), scaled(exact3(axis), down))
            }
        }
    }
}

/// How a ruled quadric's rulings run.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) enum Kind {
    /// A cylinder's: `P(θ) = o + r u(θ)`, `G = z`, with `u(θ) = x cos θ + y sin θ`.
    Upright { radius: f64 },
    /// A cone's: `P = o`, the apex, and `G(θ) = sin a u(θ) + cos a z`.
    Slanted { sin: f64, cos: f64 },
}

/// A cylinder or a cone by its rulings, in the pair's frame.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(super) struct Ruling {
    pub(super) kind: Kind,
    pub(super) origin: Vec3,
    pub(super) x: Vec3,
    pub(super) y: Vec3,
    pub(super) z: Vec3,
}

/// What the other quadric leaves along the rulings: `α v² + 2β v + γ`, and `Δ = β² − αγ`.
#[derive(Clone, Debug, PartialEq)]
pub(super) struct Along {
    pub(super) alpha: Trig,
    pub(super) beta: Trig,
    pub(super) gamma: Trig,
    pub(super) delta: Trig,
}

impl Ruling {
    /// The rulings of `surface`, a cylinder or a cone, in the frame whose origin is `origin`.
    pub(super) fn of(surface: &Natural, origin: Vec3) -> Option<Self> {
        let (at, z, kind) = match *surface {
            Natural::Cylinder { at, axis, radius } => (at, axis, Kind::Upright { radius }),
            Natural::Cone { apex, axis, sin, cos } => (apex, axis, Kind::Slanted { sin, cos }),
            Natural::Sphere { .. } => return None,
        };
        let x = square_to(z);
        Some(Self { kind, origin: at - origin, x, y: unit(z.cross(x)), z })
    }

    /// `u(θ)`.
    fn out(&self, theta: f64) -> Vec3 {
        let (sin, cos) = sin_cos(theta);
        self.x * cos + self.y * sin
    }

    /// `G(θ)`.
    pub(super) fn direction(&self, theta: f64) -> Vec3 {
        match self.kind {
            Kind::Upright { .. } => self.z,
            Kind::Slanted { sin, cos } => self.out(theta) * sin + self.z * cos,
        }
    }

    /// The point `P(θ) + v G(θ)`.
    #[cfg(test)]
    pub(super) fn point(&self, theta: f64, v: f64) -> Vec3 {
        let base = match self.kind {
            Kind::Upright { radius } => self.origin + self.out(theta) * radius,
            Kind::Slanted { .. } => self.origin,
        };
        base + self.direction(theta) * v
    }

    /// What `other` leaves along the rulings.
    pub(super) fn along(&self, other: &Quadric) -> Along {
        let (x, y, up) = (self.x, self.y, self.z);
        let form = |u: Vec3, w: Vec3| other.form(u, w);
        // uᵀ M u = (Mxx + Myy)/2 + (Mxx − Myy)/2 cos 2θ + Mxy sin 2θ.
        let (xx, yy, xy) = (form(x, x), form(y, y), form(x, y));
        let round =
            |scale: f64| Trig(vec![scale * f64::midpoint(xx, yy), 0.0, 0.0, scale * (xx - yy) / 2.0, scale * xy]);
        let (alpha, beta, gamma) = match self.kind {
            Kind::Upright { radius } => {
                let g = other.half_gradient(self.origin);
                let gamma = round(radius * radius).plus(&Trig(vec![
                    other.value(self.origin),
                    2.0 * radius * g.dot(x),
                    2.0 * radius * g.dot(y),
                ]));
                let beta = Trig(vec![up.dot(g), radius * form(up, x), radius * form(up, y)]);
                (Trig(vec![form(up, up)]), beta, gamma)
            }
            Kind::Slanted { sin, cos } => {
                let g = other.half_gradient(self.origin);
                let mixed =
                    Trig(vec![cos * cos * form(up, up), 2.0 * sin * cos * form(up, x), 2.0 * sin * cos * form(up, y)]);
                let beta = Trig(vec![cos * g.dot(up), sin * g.dot(x), sin * g.dot(y)]);
                (round(sin * sin).plus(&mixed), beta, Trig(vec![other.value(self.origin)]))
            }
        };
        let delta = beta.times(&beta).plus(&alpha.times(&gamma).scaled(-1.0));
        Along { alpha, beta, gamma, delta }
    }
}

impl Along {
    /// The function whose roots are the critical points of the other quadric on the rulings, where it is least or
    /// most along them: `Δ / α` has a critical point there, so `Δ' α − Δ α'` vanishes.
    #[cfg(test)]
    pub(super) fn critical(&self) -> Trig {
        let (rate, slope) = (self.delta.rate(), self.alpha.rate());
        rate.times(&self.alpha).plus(&self.delta.times(&slope).scaled(-1.0))
    }

    /// The nearer root of the quadratic on the ruling at `theta`, `v = −β / α` where the two roots meet.
    #[cfg(test)]
    pub(super) fn middle(&self, theta: f64) -> f64 {
        -self.beta.at(theta) / self.alpha.at(theta)
    }
}

/// The sine of the angle between the unit vectors `a` and `b`, either way round, certified.
#[cfg(test)]
pub(super) fn sine_between(a: [Interval; 3], b: [Interval; 3]) -> Interval {
    norm3(cross3(a, b))
}

#[cfg(test)]
mod tests {
    use occam_units::{Angle, Length, Tolerance};

    use super::*;

    /// Decisions at bands of 1e-9 of a length and of an angle.
    fn decide() -> Decisions {
        Decisions::new(Tolerance::new(Length::from_millimetres(1e-9), Angle::from_radians(1e-9)).unwrap())
    }

    fn v(x: f64, y: f64, z: f64) -> Vec3 {
        Vec3::new(x, y, z)
    }

    /// The cone of half-angle 0.5 about Z with its apex at `(5, 0, 0)`, in a frame whose origin is the world's.
    fn cone() -> (Quadric, f64, f64) {
        let (sin, cos) = sin_cos(0.5);
        (Natural::Cone { apex: v(5.0, 0.0, 0.0), axis: v(0.0, 0.0, 1.0), sin, cos }.quadric(v(0.0, 0.0, 0.0)), sin, cos)
    }

    #[test]
    fn a_point_is_on_a_cone_at_its_apex_and_along_it_and_off_it_by_more_than_the_band() {
        let (q, sin, cos) = cone();
        let mut d = decide();
        assert_eq!(q.holds(v(5.0, 0.0, 0.0), &mut d), Ok(Side::Within), "the apex");
        assert_eq!(q.holds(v(5.0 + 0.5e-9, 0.0, 0.0), &mut d), Ok(Side::Within), "within the band of the apex");
        assert_eq!(q.holds(v(5.0 + 1e-3, 0.0, 0.0), &mut d), Ok(Side::Above), "off the apex across the axis");
        let moved =
            Natural::Cone { apex: v(5.0, 0.0, 0.0), axis: v(0.0, 0.0, 1.0), sin, cos }.quadric(v(1.0, 2.0, 3.0));
        assert_eq!(moved.holds(v(4.0, -2.0, -3.0), &mut d), Ok(Side::Within), "its apex, in a frame about (1, 2, 3)");
        let on = v(5.0 + 2.0 * sin, 0.0, 2.0 * cos);
        let out = v(cos, 0.0, -sin);
        assert_eq!(q.holds(on, &mut d), Ok(Side::Within));
        assert_eq!(q.holds(on + out * 0.5e-9, &mut d), Ok(Side::Within));
        assert_eq!(q.holds(on + out * 3e-9, &mut d), Ok(Side::Above));
        assert_eq!(q.holds(on - out * 3e-9, &mut d), Ok(Side::Below));
    }

    #[test]
    fn a_spheres_centre_is_not_on_it_and_a_cylinders_points_are_decided_by_their_distance() {
        let mut d = decide();
        let sphere = Natural::Sphere { centre: v(1.0, 2.0, 3.0), radius: 2.0 }.quadric(v(0.0, 0.0, 0.0));
        assert_eq!(sphere.holds(v(1.0, 2.0, 3.0), &mut d), Ok(Side::Below), "its centre is inside it");
        assert_eq!(sphere.holds(v(3.0 + 3e-9, 2.0, 3.0), &mut d), Ok(Side::Above));
        let axis = v(1.0, 2.0, 2.0) * (1.0 / 3.0);
        let cylinder = Natural::Cylinder { at: v(0.0, 1.0, 0.0), axis, radius: 0.5 }.quadric(v(0.5, 0.0, 0.0));
        let across = unit(axis.cross(v(1.0, 0.0, 0.0)));
        let on = v(0.0, 1.0, 0.0) + axis * 7.0 + across * 0.5 - v(0.5, 0.0, 0.0);
        assert_eq!(cylinder.holds(on, &mut d), Ok(Side::Within));
        assert_eq!(cylinder.holds(on + across * 3e-9, &mut d), Ok(Side::Above));
        assert_eq!(cylinder.holds(on - across * 0.5e-9, &mut d), Ok(Side::Within));
    }

    #[test]
    fn a_direction_is_on_a_cones_asymptotic_cone_within_the_angle_band_and_off_it_beyond() {
        let (q, _, _) = cone();
        let mut d = decide();
        let at = |angle: f64| {
            let (s, c) = sin_cos(angle);
            v(s * 0.6, s * 0.8, c)
        };
        assert_eq!(q.toward(at(0.5), &mut d), Ok(Side::Within));
        assert_eq!(q.toward(at(0.5 + 0.3e-9), &mut d), Ok(Side::Within));
        assert_eq!(q.toward(at(0.5 + 3e-9), &mut d), Ok(Side::Above), "outside its opening");
        assert_eq!(q.toward(at(0.5 + 1.5e-9), &mut d), Ok(Side::Above), "half a band beyond it");
        assert_eq!(q.toward(at(0.5 - 1.5e-9), &mut d), Ok(Side::Below));
        assert_eq!(q.toward(at(0.5 - 3e-9), &mut d), Ok(Side::Below), "inside it");
        let sphere = Natural::Sphere { centre: v(0.0, 0.0, 0.0), radius: 1.0 }.quadric(v(0.0, 0.0, 0.0));
        assert_eq!(sphere.toward(v(0.0, 0.0, 1.0), &mut d), Ok(Side::Above), "a sphere has no asymptotic cone");
        let cylinder = Natural::Cylinder { at: v(0.0, 0.0, 0.0), axis: v(0.0, 0.0, 1.0), radius: 1.0 };
        let cylinder = cylinder.quadric(v(0.0, 0.0, 0.0));
        assert_eq!(cylinder.toward(v(0.0, 0.0, 1.0), &mut d), Ok(Side::Within), "its axis");
        // Off its axis by δ, `gᵀ M g = sin² δ`, decided against a thousandth of its size, √2, where `2 sin δ` is less.
        let off_axis = |angle: f64| {
            let (s, c) = sin_cos(angle);
            v(s, 0.0, c)
        };
        assert_eq!(cylinder.toward(off_axis(1e-6), &mut d), Ok(Side::Within), "1e-12 against 1.41e-12");
        assert_eq!(cylinder.toward(off_axis(3e-6), &mut d), Ok(Side::Above), "9e-12 against 1.41e-12");
        // Where `2 sin δ`, 1e-3, is less than the floor, the floor is what the band is taken of.
        let mut wide =
            Decisions::new(Tolerance::new(Length::from_millimetres(1e-9), Angle::from_radians(2e-4)).unwrap());
        assert_eq!(cylinder.toward(off_axis(5e-4), &mut wide), Ok(Side::Within), "2.5e-7 against 2.83e-7");
    }
}
