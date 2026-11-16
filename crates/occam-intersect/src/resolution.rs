//! Source: original
//! What the tolerance cannot tell apart (O24). Where two surfaces touch, or nearly, every method answers by one rule: a
//! contact is a [`Contact::Point`] only where the decision certifies that nothing larger than the band can hide there,
//! and is otherwise a [`Contact::BelowResolution`] with a bound that contains whatever does.
//!
//! The derivation. About a point `c` where the two surfaces' unit normals are parallel, write either surface as a graph
//! over the first's tangent plane, along its normal: the gap from the first to the second at `x` in that plane is
//! `g(x) = g₀ + ½ xᵀ K x` to second order, where `K`, the relative curvature, is the second surface's second
//! fundamental form less the first's, both taken with the first's normal. Where the surfaces cross near `c`, they do so
//! where `g` is zero, a conic about `c` whose half-axes are `√(2 |g₀| / |λ|)` for the eigenvalues `λ` of `K`: a loop where
//! `K` is definite, two branches crossing at `c`, a node, where it is not and `g₀` is zero. The band cannot tell a gap
//! within it from no gap, so a loop whose `g₀` is within the band is a tangency to it. The largest such loop has the
//! radius `d = √(2 band / κ)`, `κ` the least `|λ|`: a loop smaller than `d` cannot be told from a contact at `c`. Two unit
//! spheres a gap `ε` into each other have `κ = 2` and a loop of radius about `√ε` (P5), which is `d` where `ε` is the band.
//!
//! The bound is twice what the second-order model gives, [`MARGIN`]: the model's error is of third order, a part
//! `O(d / R)` of it for the surfaces' least radius of curvature `R`, which is far below one wherever `d` is a loop the
//! band hides. A method that has the contact at `c` itself, as the closed forms and the pencil method do, bounds it by
//! `2 d`. The hybrid has a station `p` of a trace that ended in a touch instead, a point within the band of both
//! surfaces where its trace could no longer tell a crossing from a touch, and reads the model there: the gap `g(p)`, its
//! slope `∇g`, the part of the second surface's unit normal along the first's tangent plane, and `K`. Then
//! `c − p = −K⁻¹ ∇g`, `g₀ = g(p) + ½ ∇g · (c − p)`, and every point of what hides there is within
//! `2 (|c − p| + √(2 (|g₀| + band) / κ))` of `p`. Where `κ` is nought to the precision of `K`, the surfaces are tangent
//! along a curve through the contact, and no finite bound follows from the model: the bound is infinite.
//!
//! A contact is certified, a [`Contact::Point`], where its bound is decided within the band through occam-math's
//! predicates: where it is not a tangency at all, as at a cone's apex on a plane, whose bound is nought, or where the
//! relative curvature is so great that the loop the band hides is within the band of its middle. Every other tangency is
//! [`Contact::BelowResolution`], and P5's silent wrong shapes, a loop come back as a point, are not silent.

use occam_geom::Surface;
use occam_units::{Length, UvPoint, Vec3};

use crate::section::Contact;

/// How many times the second-order estimate a bound is: room for the model's third-order error, and for a point found
/// on a loop rather than at its middle.
pub(crate) const MARGIN: f64 = 2.0;

/// An eigenvalue this small against the largest is nought to the precision of a matrix of exact numbers.
pub(crate) const NULL: f64 = 1e-9;

/// How many times the sine of the angle between two surfaces' normals the error of their relative curvature read from
/// their derivatives is, as a part of its greatest eigenvalue.
const LEAN: f64 = 4.0;

/// Two surfaces near a point of each, as the gap between them over the first's tangent plane, in an orthonormal basis
/// of that plane: `g(x) = gap + slope · x + ½ xᵀ bend x`.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Near {
    /// The gap at the first point, along the first surface's unit normal.
    pub gap: f64,
    /// Its slope: how fast the second surface leaves the first's tangent plane along each axis.
    pub slope: [f64; 2],
    /// The relative curvature `K`, symmetric.
    pub bend: [[f64; 2]; 2],
    /// The orthonormal basis of the first's tangent plane.
    pub basis: [Vec3; 2],
    /// How large, as a part of the greatest, an eigenvalue of `bend` may be and still be nought to its precision.
    pub null: f64,
}

/// The eigenvalues of the symmetric `[[top, side], [side, bottom]]`, the one on `top`'s side of their middle first, and
/// a unit eigenvector of that one.
fn eigen([[top, side], [_, bottom]]: [[f64; 2]; 2]) -> ([f64; 2], [f64; 2]) {
    let (mean, half) = (f64::midpoint(top, bottom), 0.5 * (top - bottom));
    let reach = (half * half + side * side).sqrt();
    let (first, second) = if half < 0.0 { (mean - reach, mean + reach) } else { (mean + reach, mean - reach) };
    let (along, across) = (first - bottom, side);
    let length = (along * along + across * across).sqrt();
    let way = if length > 0.0 { [along / length, across / length] } else { [1.0, 0.0] };
    ([first, second], way)
}

/// The size of `x`.
fn size(x: f64) -> f64 {
    x.abs()
}

/// `u · v` for two vectors of the plane.
fn dot2([a, b]: [f64; 2], [c, d]: [f64; 2]) -> f64 {
    a * c + b * d
}

impl Near {
    /// The least and the greatest size of the relative curvature's eigenvalues.
    pub(crate) fn sizes(&self) -> (f64, f64) {
        let ([first, second], _) = eigen(self.bend);
        if size(first) < size(second) { (size(first), size(second)) } else { (size(second), size(first)) }
    }

    /// The relative curvature's eigenvalues.
    pub(crate) fn eigenvalues(&self) -> [f64; 2] {
        eigen(self.bend).0
    }

    /// Whether an eigenvalue of the relative curvature is nought to its precision, as both are where it is nought.
    pub(crate) fn flat(&self) -> bool {
        let (least, most) = self.sizes();
        least <= self.null * most
    }

    /// Where the gap is least or most, `c − p` in the plane's basis: `−K⁻¹ slope`, leaving out any eigenvalue that is
    /// nought to the precision of `K`, along whose direction the gap is flat to second order.
    pub(crate) fn critical(&self) -> [f64; 2] {
        let ([first, second], [x, y]) = eigen(self.bend);
        let (_, most) = self.sizes();
        let mut out = [0.0; 2];
        for (lambda, way) in [(first, [x, y]), (second, [-y, x])] {
            if size(lambda) <= self.null * most {
                continue;
            }
            let along = -dot2(self.slope, way) / lambda;
            out = [out[0] + along * way[0], out[1] + along * way[1]];
        }
        out
    }

    /// The gap where it is least or most, `g₀ = g(p) + ½ slope · (c − p)`.
    pub(crate) fn extremal(&self) -> f64 {
        self.gap + 0.5 * dot2(self.slope, self.critical())
    }

    /// `x` of the plane, in space.
    pub(crate) fn lifted(&self, [x, y]: [f64; 2]) -> Vec3 {
        let [e1, e2] = self.basis;
        e1 * x + e2 * y
    }

    /// How far from the first point every point of what the band hides there lies: `2 (|c − p| + d)`, where `d` is the
    /// radius of the largest loop a gap of `|g₀|` and the band gives, or infinite where `K` has an eigenvalue that is
    /// nought to its precision.
    pub(crate) fn bound(&self, band: f64) -> f64 {
        if self.flat() {
            return f64::INFINITY;
        }
        let offset = self.lifted(self.critical()).norm();
        MARGIN * (offset + hidden(size(self.extremal()) + band, self.sizes().0))
    }

    /// The gap between `b` at `on_b` and `a` at `on_a`, from their derivatives through the seam, or `None` where
    /// either's derivatives span no plane there, as at a cone's apex, where its numbers are not finite. The second's
    /// curvature is read along its own tangent
    /// plane, which leans from the first's by the angle between their normals, so the relative curvature is known
    /// only to a part of the greatest about four times that angle's sine, besides its rounding: an eigenvalue no
    /// larger is nought to its precision.
    pub(crate) fn between(a: &dyn Surface, on_a: UvPoint, b: &dyn Surface, on_b: UvPoint) -> Option<Self> {
        let (first, second) = (Form::at(a, on_a), Form::at(b, on_b));
        let n = first.normal;
        let e1 = unit(first.du);
        let (e2, m) = (n.cross(e1), if second.normal.dot(n) < 0.0 { -second.normal } else { second.normal });
        let rise = m.dot(n);
        let slope = [-m.dot(e1) / rise, -m.dot(e2) / rise];
        let relative = |s: Vec3, t: Vec3| second.second(s, t, m) - first.second(s, t, n);
        let across = relative(e1, e2);
        let bend = [[relative(e1, e1), across], [across, relative(e2, e2)]];
        let gap = (second.point - first.point).dot(n);
        let out = Self { gap, slope, bend, basis: [e1, e2], null: NULL + LEAN * m.cross(n).norm() };
        let numbers = [gap, slope[0], slope[1], bend[0][0], bend[0][1], bend[1][1]];
        numbers.iter().all(|x| x.is_finite()).then_some(out)
    }
}

/// `v` scaled to unit length.
fn unit(v: Vec3) -> Vec3 {
    v * (1.0 / v.norm())
}

/// A surface's point, derivatives and unit normal at one parameter.
struct Form {
    point: Vec3,
    du: Vec3,
    dv: Vec3,
    second: [Vec3; 3],
    normal: Vec3,
    /// The inverse of the first fundamental form, as its three numbers.
    inverse: [f64; 3],
}

impl Form {
    /// The form of `surface` at `uv`: where its derivatives span no plane, its normal and inverse are not finite.
    fn at(surface: &dyn Surface, uv: UvPoint) -> Self {
        let jet = surface.second(uv);
        let (du, dv) = (jet.first.du.rate(), jet.first.dv.rate());
        let (e, f, g) = (du.dot(du), du.dot(dv), dv.dot(dv));
        let det = e * g - f * f;
        Self {
            point: jet.first.point.to_vec3(),
            du,
            dv,
            second: [jet.duu.rate(), jet.duv.rate(), jet.dvv.rate()],
            normal: unit(du.cross(dv)),
            inverse: [g / det, -f / det, e / det],
        }
    }

    /// The second fundamental form, along `normal`, of the tangent vectors nearest `s` and `t`.
    fn second(&self, s: Vec3, t: Vec3, normal: Vec3) -> f64 {
        let [uu, uv, vv] = self.inverse;
        let params = |w: Vec3| {
            let (along_u, along_v) = (self.du.dot(w), self.dv.dot(w));
            (uu * along_u + uv * along_v, uv * along_u + vv * along_v)
        };
        let ((su, sv), (tu, tv)) = (params(s), params(t));
        let [duu, duv, dvv] = self.second;
        (duu * (su * tu) + duv * (su * tv + sv * tu) + dvv * (sv * tv)).dot(normal)
    }
}

/// The radius of the largest loop a gap of `gap` gives where the relative curvature's least eigenvalue has the size
/// `least`: `√(2 gap / least)`, infinite where `least` is nought.
pub(crate) fn hidden(gap: f64, least: f64) -> f64 {
    if least > 0.0 { (2.0 * gap / least).sqrt() } else { f64::INFINITY }
}

/// The contact at `at` whose bound is `bound`: a point, with its parameters on both surfaces, where `certified`, which
/// a method sets where it decided the bound within the band, and otherwise below the resolution.
pub(crate) fn contact(at: Vec3, on: (UvPoint, UvPoint), bound: f64, certified: bool) -> Contact {
    let at = occam_units::Point::from_vec3(at);
    if certified {
        Contact::Point { at, on_a: on.0, on_b: on.1 }
    } else {
        Contact::BelowResolution { at, bound: Length::from_millimetres(bound) }
    }
}

#[cfg(test)]
mod tests {
    use occam_units::{Point, UvPoint, Vec3};

    use super::*;
    use crate::trace_tests::{BAND, ball, level, mm, moved, tol, upright};

    /// The model with relative curvature `bend`, the gap `gap` and the slope `slope`, over X and Y, of a matrix of
    /// exact numbers.
    fn near(gap: f64, slope: [f64; 2], bend: [[f64; 2]; 2]) -> Near {
        Near { gap, slope, bend, basis: [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0)], null: NULL }
    }

    fn close(a: f64, b: f64) -> bool {
        (a - b).abs() <= 1e-12 * (1.0 + b.abs())
    }

    #[test]
    fn a_symmetric_matrix_has_its_eigenvalues_and_a_unit_eigenvector_of_the_first() {
        // [[2, 1], [1, 2]] has 3 along (1, 1) and 1 along (1, -1); [[1, 1], [1, -1]] has ±√2.
        let ([first, second], [x, y]) = eigen([[2.0, 1.0], [1.0, 2.0]]);
        assert!(close(first, 3.0) && close(second, 1.0) && close(x, y) && close(x * x + y * y, 1.0), "{first} {x} {y}");
        let ([first, second], [x, y]) = eigen([[1.0, 2.0], [2.0, -2.0]]);
        assert!(close(first, 2.0) && close(second, -3.0), "{first} {second}");
        assert!(close(x + 2.0 * y, first * x) && close(2.0 * x - 2.0 * y, first * y), "{x} {y}");
        let ([first, second], way) = eigen([[-1.0, 0.0], [0.0, 4.0]]);
        assert_eq!((first, second, way), (-1.0, 4.0, [-1.0, 0.0]));
        assert_eq!(eigen([[0.0; 2]; 2]), ([0.0, 0.0], [1.0, 0.0]), "nought has every way");
        assert_eq!(eigen([[2.0, 0.0], [0.0, 2.0]]).0, [2.0, 2.0]);
    }

    #[test]
    fn the_sizes_are_the_least_and_greatest_eigenvalues_sizes_and_one_nought_is_flat() {
        assert_eq!(near(0.0, [0.0; 2], [[-3.0, 0.0], [0.0, 1.0]]).sizes(), (1.0, 3.0));
        assert_eq!(near(0.0, [0.0; 2], [[1.0, 0.0], [0.0, -3.0]]).sizes(), (1.0, 3.0));
        assert!(!near(0.0, [0.0; 2], [[1.0, 0.0], [0.0, 2e-9]]).flat());
        assert!(near(0.0, [0.0; 2], [[1.0, 0.0], [0.0, 0.5e-9]]).flat(), "within the precision, nought");
        assert!(near(0.0, [0.0; 2], [[0.0; 2]; 2]).flat());
        let leaning = Near { null: 0.25, ..near(0.0, [0.0; 2], [[1.0, 0.0], [0.0, 0.2]]) };
        assert!(leaning.flat(), "a lean makes a larger one nought");
    }

    #[test]
    fn where_the_gap_is_least_is_less_the_slope_through_the_curvature_and_flat_ways_are_left_out() {
        // g = 0.1 + 0.4 x - 0.6 y + ½ (2 x² + 3 y²): least at (-0.2, 0.2), where it is 0.1 - 0.04 - 0.06 = 0.
        let bowl = near(0.1, [0.4, -0.6], [[2.0, 0.0], [0.0, 3.0]]);
        let [x, y] = bowl.critical();
        assert!(close(x, -0.2) && close(y, 0.2), "{x} {y}");
        assert!(bowl.extremal().abs() <= 1e-15, "{}", bowl.extremal());
        assert_eq!(bowl.lifted([x, y]), Vec3::new(x, y, 0.0));
        // Flat along Y: only the X part moves.
        let trough = near(0.0, [0.4, -0.6], [[2.0, 0.0], [0.0, 0.0]]);
        let [x, y] = trough.critical();
        assert!(close(x, -0.2) && y == 0.0, "{x} {y}");
        // Nought to the precision of the greater, however small against it, whichever comes first.
        for bend in [[[2.0, 0.0], [0.0, 1e-12]], [[1e-12, 0.0], [0.0, 2.0]]] {
            let [x, y] = near(0.0, [0.4, 0.4], bend).critical();
            assert!(close(x + y, -0.2) && (x == 0.0 || y == 0.0), "{bend:?}: {x} {y}");
        }
        // Neither axis the plane's: K = [[2, 1], [1, 2]], its inverse a third of [[2, -1], [-1, 2]].
        let [x, y] = near(0.0, [3.0, 0.0], [[2.0, 1.0], [1.0, 2.0]]).critical();
        assert!(close(x, -2.0) && close(y, 1.0), "{x} {y}");
        let turned = near(0.0, [1.0, 1.0], [[1.0, 1.0], [1.0, 1.0]]);
        let [x, y] = turned.critical();
        assert!(close(x, -0.5) && close(y, -0.5), "a turned trough: {x} {y}");
    }

    #[test]
    fn the_bound_is_twice_the_offset_and_the_loop_the_gap_and_the_band_hide() {
        // At its middle with no gap, a bowl of curvature 2 hides a loop of √(2 band / 2) = √band.
        let bowl = near(0.0, [0.0; 2], [[2.0, 0.0], [0.0, 5.0]]);
        assert!(close(bowl.bound(1e-8), MARGIN * 1e-4), "{}", bowl.bound(1e-8));
        // On a loop of radius 0.1 of curvature 2: 0.1 from its middle, whose gap is 0.01.
        let on = near(0.0, [0.2, 0.0], [[2.0, 0.0], [0.0, 2.0]]);
        let want = MARGIN * (0.1 + (2.0 * (0.01 + 1e-8) / 2.0_f64).sqrt());
        assert!(close(on.bound(1e-8), want), "{} against {want}", on.bound(1e-8));
        let saddle = near(0.0, [0.0; 2], [[-1.0, 0.0], [0.0, 4.0]]);
        assert!(close(saddle.bound(2e-8), MARGIN * 2e-4), "the least size sets it: {}", saddle.bound(2e-8));
        assert_eq!(near(0.0, [0.0; 2], [[1.0, 0.0], [0.0, 0.0]]).bound(1e-8), f64::INFINITY);
        assert_eq!((hidden(2.0, 1.0), hidden(1.0, 0.0), hidden(0.0, 0.0)), (2.0, f64::INFINITY, f64::INFINITY));
    }

    #[test]
    fn the_gap_between_a_sphere_and_a_plane_it_nearly_touches_is_read_from_their_derivatives() {
        // The plane z = 0 and the sphere of radius 2 about (0, 0, 2 + 1e-3): the gap is 1e-3 and the sphere's
        // curvature, a half, is the whole of the relative curvature.
        let (plane, sphere) = (level(0.0), ball(2.0, [0.0, 0.0, 2.0 + 1e-3]));
        let mut scratch = occam_geom::Scratch::default();
        let low = Point::from_vec3(Vec3::new(0.0, 0.0, 1e-3));
        let on_b = sphere.invert(low, None, tol(), &mut scratch).unwrap();
        let model = Near::between(&plane, UvPoint::new(0.0, 0.0), &sphere, on_b).unwrap();
        assert!(close(model.gap.abs(), 1e-3) && model.slope.iter().all(|s| s.abs() <= 1e-12), "{model:?}");
        let (least, most) = model.sizes();
        assert!(close(least, 0.5) && close(most, 0.5), "{model:?}");
        assert!(close(model.extremal().abs(), 1e-3));
        assert!(model.null - NULL <= 1e-15, "normals parallel: no lean, {}", model.null);
        // Off its foot, the slope is the sphere's lean, and the null grows with it.
        let aside = Point::from_vec3(Vec3::new(0.2, 0.0, 2.0 + 1e-3 - (4.0_f64 - 0.04).sqrt()));
        let on_b = sphere.invert(aside, None, tol(), &mut scratch).unwrap();
        let model = Near::between(&plane, UvPoint::new(0.2, 0.0), &sphere, on_b).unwrap();
        let lean = (model.slope[0] * model.slope[0] + model.slope[1] * model.slope[1]).sqrt();
        assert!(close(lean, 0.1 / (1.0_f64 - 0.01).sqrt()), "{model:?}");
        assert!(model.null > NULL + 0.39 && model.null < NULL + 0.41, "{model:?}");
        let [x, y] = model.critical();
        let back = (x * x + y * y).sqrt();
        assert!((back - 0.2).abs() <= 0.01 && x < 0.0, "the foot is the middle, to the model's lean: {x} {y}");
        // And off it across, the lean is the slope's other part.
        let across = Point::from_vec3(Vec3::new(0.0, 0.2, 2.0 + 1e-3 - (4.0_f64 - 0.04).sqrt()));
        let on_b = sphere.invert(across, None, tol(), &mut scratch).unwrap();
        let model = Near::between(&plane, UvPoint::new(0.0, 0.2), &sphere, on_b).unwrap();
        assert!(
            model.slope[0].abs() <= 1e-12 && close(model.slope[1].abs(), 0.1 / (1.0_f64 - 0.01).sqrt()),
            "{model:?}"
        );
    }

    #[test]
    fn a_surface_of_slanting_parameters_has_its_own_curvatures_against_its_tangent_plane() {
        // The paraboloid z = (x² + y²) / 4, a graph whose parameters are not square to each other off its axis. At
        // radius r its curvatures are a half over (1 + r²/4) to the powers 3/2 along the meridian and 1/2 across it.
        let bowl = crate::hybrid_tests::graph((0.25, 0.0), (0.25, 0.0));
        let uv = UvPoint::new(0.6, 0.4);
        let jet = bowl.first(uv);
        let normal = jet.du.rate().cross(jet.dv.rate());
        let at = bowl.point(uv).to_vec3();
        let unmoved = occam_units::Placement::identity();
        let plane = occam_geom::Plane::placed(&crate::closed::tests::frame_by(at, normal, &unmoved));
        let model = Near::between(&plane, UvPoint::new(0.0, 0.0), &bowl, uv).unwrap();
        let lift: f64 = 1.0 + 0.52 / 4.0;
        let (least, most) = model.sizes();
        let (meridian, across) = (0.5 / (lift * lift.sqrt()), 0.5 / lift.sqrt());
        assert!(close(least, meridian) && close(most, across), "{model:?} against {meridian} and {across}");
        assert!(model.gap.abs() <= 1e-15 && model.slope.iter().all(|s| s.abs() <= 1e-12), "{model:?}");
    }

    #[test]
    fn a_saddles_twist_turns_the_model_back_to_its_middle() {
        // The bilinear saddle z = 0.2 x y over the plane z = 0: the gap is exactly 0.2 x y, so from (0.3, 0.1) the place
        // where it is least or most is the origin, where it is nought; its relative curvature is all twist.
        // Its parameters turned by `turn` from the plane's, the twist is read along axes that are not its own.
        for turn in [0.0, 0.5] {
            let (sine, cosine) = occam_math::sin_cos(turn);
            let spun = |x: f64, y: f64| (cosine * x - sine * y, sine * x + cosine * y);
            let corner = |x: f64, y: f64| {
                let (u, v) = spun(x, y);
                Point::from_vec3(Vec3::new(u, v, 0.2 * x * y))
            };
            let form = occam_geom::BsplineSurface {
                degree: (1, 1),
                count: (2, 2),
                knots_u: vec![-1.0, -1.0, 1.0, 1.0],
                knots_v: vec![-1.0, -1.0, 1.0, 1.0],
                poles: vec![corner(-1.0, -1.0), corner(1.0, -1.0), corner(-1.0, 1.0), corner(1.0, 1.0)],
                weights: None,
            };
            let saddle = occam_geom::SplineSurface::new(&form).unwrap();
            let (u, v) = spun(0.3, 0.1);
            let model = Near::between(&level(0.0), UvPoint::new(u, v), &saddle, UvPoint::new(0.3, 0.1)).unwrap();
            let [x, y] = model.critical();
            assert!((x + u).abs() <= 0.01 && (y + v).abs() <= 0.01, "{turn}: {x} {y}: {model:?}");
            assert!(model.extremal().abs() <= 1e-3 && close(model.gap, 0.006), "{turn}: {model:?}");
        }
    }

    #[test]
    fn a_surface_whose_derivatives_span_no_plane_there_gives_no_model() {
        // A cone's apex, where its derivative round the axis is nought.
        let cone = occam_geom::Cone::placed(&moved([0.0; 3]), mm(0.0), occam_units::Angle::from_radians(0.5)).unwrap();
        let (plane, apex) = (level(0.0), UvPoint::new(0.3, 0.0));
        assert_eq!(Near::between(&plane, UvPoint::new(0.0, 0.0), &cone, apex), None);
        assert_eq!(Near::between(&cone, apex, &plane, UvPoint::new(0.0, 0.0)), None);
        assert!(Near::between(&plane, UvPoint::new(0.0, 0.0), &upright(0.0), UvPoint::new(0.0, 0.0)).is_some());
    }

    #[test]
    fn a_contact_is_a_point_with_its_parameters_where_certified_and_else_below_the_resolution_with_its_bound() {
        let (at, on) = (Vec3::new(1.0, 2.0, 3.0), (UvPoint::new(0.1, 0.2), UvPoint::new(0.3, 0.4)));
        let point = contact(at, on, 0.0, true);
        assert_eq!(point, Contact::Point { at: Point::from_vec3(at), on_a: on.0, on_b: on.1 });
        let below = contact(at, on, BAND, false);
        assert_eq!(below, Contact::BelowResolution { at: Point::from_vec3(at), bound: Length::from_millimetres(BAND) });
    }
}
