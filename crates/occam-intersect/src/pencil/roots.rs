//! Source: dupont-lazard-lazard-petitjean-2008
//! The pencil's classification, by the real roots of its determinant, their multiplicities and the ranks there
//! (Dupont, Lazard, Lazard and Petitjean 2008, part II; Wang, Goldman and Tu 2003). The members are
//! `cos φ A + sin φ B`, each quadric's 4×4 matrix scaled to unit size, so the determinant is a trigonometric
//! polynomial of degree four in `φ`, read from nine of its values, and a root at `φ = π/2` is `B` itself.
//!
//! A multiple root is a root of the determinant's derivative too, and a triple one of its second, so the candidates are
//! the roots of its first three derivatives where the determinant is near zero. There the member's eigenvalues give its rank. Of rank two it is a pair of planes, real where its other
//! two eigenvalues differ in sign, and the root is multiple when the planes' conics on one surface lie on the other.
//! Of rank three it is a cone, and the root is multiple when the cone's vertex lies on the surfaces: a node, an isolated
//! point, or, where the cone meets their common tangent plane in one ruling, a cusp, a triple root. A vertex at
//! infinity is a direction on both asymptotic cones. Each of these is decided on certified intervals of polynomials in
//! the matrices' entries, through occam-math's predicates, against the band.

use std::f64::consts::{PI, TAU};

use occam_math::{Interval, Predicate, Ring, Side, sin_cos};
use occam_units::Vec3;

use super::Pair;
use super::eigen::eigen;
use super::quadric::Quadric;
use super::split::holds;
use super::trig::Trig;
use crate::closed::{square_to, unit};
use crate::decide::{Decisions, Undecided};

/// What singular point a vertex on both surfaces is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
pub(crate) enum Singular {
    /// Two real branches cross there.
    Crossing,
    /// No real branch passes: a point where the surfaces touch.
    Isolated,
    /// The two branches have one tangent: a cusp, a triple root.
    Cusp,
}

/// A multiple root of the pencil's determinant at `φ`, and what its member is.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) enum Multiple {
    /// Rank three, its vertex at `at`, in the pair's frame, on both surfaces.
    Vertex { phi: f64, at: Vec3, kind: Singular },
    /// Rank three, its vertex at infinity along a direction on both asymptotic cones.
    AtInfinity { phi: f64 },
    /// Rank two: two real planes, each a unit normal and a point, in the pair's frame.
    Planes { planes: [(Vec3, Vec3); 2] },
    /// Rank two, two planes that are complex conjugates, meeting in a real line.
    Conjugate,
    /// Rank one: a plane counted twice, along whose conic the surfaces are tangent.
    Double,
}

/// The pencil's members, `cos φ A + sin φ B`.
pub(crate) struct Members {
    a: [[f64; 4]; 4],
    b: [[f64; 4]; 4],
}

/// `m` scaled to unit size.
fn unit_size(m: [[f64; 4]; 4]) -> [[f64; 4]; 4] {
    let size = m.iter().flatten().map(|x| x * x).sum::<f64>().sqrt();
    m.map(|row| row.map(|x| x / size))
}

/// The determinant of a 4×4 matrix, by Laplace's expansion in the 2×2 minors of its first two rows and its last two.
fn determinant(m: &[[f64; 4]; 4]) -> f64 {
    let minor = |r: usize, (c, d): (usize, usize)| {
        let s = r.saturating_add(1);
        entry(m, r, c) * entry(m, s, d) - entry(m, r, d) * entry(m, s, c)
    };
    let terms = [((0, 1), (2, 3), 1.0), ((0, 2), (1, 3), -1.0), ((0, 3), (1, 2), 1.0)];
    terms.iter().map(|&(c, d, sign)| sign * (minor(0, c) * minor(2, d) + minor(0, d) * minor(2, c))).sum()
}

impl Members {
    /// The members of the pencil of `pair`'s two quadrics.
    pub(crate) fn of(pair: &Pair) -> Self {
        Self { a: unit_size(pair.quadric(0).homogeneous()), b: unit_size(pair.quadric(1).homogeneous()) }
    }

    /// The member at `phi`.
    pub(crate) fn at(&self, phi: f64) -> [[f64; 4]; 4] {
        let (sin, cos) = sin_cos(phi);
        std::array::from_fn(|i| std::array::from_fn(|j| cos * entry(&self.a, i, j) + sin * entry(&self.b, i, j)))
    }

    /// The member at `phi`'s 3×3 part, row by row.
    pub(crate) fn part(&self, phi: f64) -> [Vec3; 3] {
        let m = self.at(phi);
        std::array::from_fn(|i| Vec3::new(entry(&m, i, 0), entry(&m, i, 1), entry(&m, i, 2)))
    }

    /// The determinant of the member at `φ`, from its values at nine angles a ninth of a turn apart.
    fn determinant(&self) -> Trig {
        self.read(4, determinant)
    }

    /// The sum of the member at `φ`'s principal 3×3 minors, from its values at seven angles: where every member is
    /// singular, the product of its three other eigenvalues, zero where its rank is two.
    fn minors(&self) -> Trig {
        self.read(3, |m| (0..4).map(|k| principal(m, k)).sum())
    }

    /// The trigonometric polynomial of degree `degree` that `f` of the member at `φ` is, from its values at
    /// `2 degree + 1` angles spread round the turn.
    fn read(&self, degree: u32, f: impl Fn(&[[f64; 4]; 4]) -> f64) -> Trig {
        let count = degree.saturating_mul(2).saturating_add(1);
        let n = f64::from(count);
        let values: Vec<(f64, f64)> =
            (0..count).map(|k| TAU * f64::from(k) / n).map(|phi| (phi, f(&self.at(phi)))).collect();
        let mut out = vec![values.iter().map(|v| v.1).sum::<f64>() / n];
        for j in 1..=degree {
            let (mut c, mut s) = (0.0, 0.0);
            for (phi, value) in &values {
                let (sin, cos) = sin_cos(f64::from(j) * phi);
                c += value * cos;
                s += value * sin;
            }
            out.extend([2.0 * c / n, 2.0 * s / n]);
        }
        Trig(out)
    }
}

/// The determinant of `m` without its row and column `k`.
fn principal(m: &[[f64; 4]; 4], k: usize) -> f64 {
    let keep: Vec<usize> = (0..4).filter(|i| *i != k).collect();
    let at = |i: usize, j: usize| entry(m, keep.get(i).copied().unwrap_or(0), keep.get(j).copied().unwrap_or(0));
    at(0, 0) * (at(1, 1) * at(2, 2) - at(1, 2) * at(2, 1)) - at(0, 1) * (at(1, 0) * at(2, 2) - at(1, 2) * at(2, 0))
        + at(0, 2) * (at(1, 0) * at(2, 1) - at(1, 1) * at(2, 0))
}

fn entry(m: &[[f64; 4]; 4], i: usize, j: usize) -> f64 {
    m.get(i).and_then(|r| r.get(j)).copied().unwrap_or(0.0)
}

/// The tangent cone at a vertex, `b² − ac` for the cone's form `a x² + 2b xy + c y²` on the tangent plane: positive
/// where two real branches cross, negative where none passes, and zero at a cusp.
struct Tangents;

impl Predicate<12> for Tangents {
    const DEGREE: u32 = 6;

    fn value<T: Ring>(&self, inputs: &[T; 12]) -> T {
        let at = |i: usize| inputs.get(i).cloned().unwrap_or_else(|| T::of(0.0));
        let rows = [[0, 3, 4], [3, 1, 5], [4, 5, 2]];
        let form = |u: [usize; 3], v: [usize; 3]| {
            let row = |r: [usize; 3]| r.iter().zip(v).fold(T::of(0.0), |sum, (m, x)| sum.add(&at(*m).mul(&at(x))));
            u.iter().zip(rows).fold(T::of(0.0), |sum, (x, r)| sum.add(&at(*x).mul(&row(r))))
        };
        let (first, second) = ([6, 7, 8], [9, 10, 11]);
        let (along, mixed, across) = (form(first, first), form(first, second), form(second, second));
        mixed.mul(&mixed).sub(&along.mul(&across))
    }
}

/// What singular point the vertex `at` of the cone with 3×3 part `m` is, on the surface `on`, whose tangent plane it
/// shares with the other. The cone's form on that plane is `a x² + 2b xy + c y²`, and its discriminant `b² − ac` over
/// `(a + c)` and the cone's slope out of the plane along the direction where the form is least, `ν · M g`, is the
/// angle between the plane and the cone's tangent plane there, to first order: zero at a cusp, where the plane is
/// tangent to the cone along one ruling, above it where two real branches cross, and below where none passes.
fn singular(m: [Vec3; 3], on: &Quadric, at: Vec3, decide: &mut Decisions) -> Result<Singular, Undecided> {
    let normal = unit(on.half_gradient(at));
    let first = unit(square_to(normal));
    let second = normal.cross(first);
    let times = |v: Vec3| Vec3::new(m[0].dot(v), m[1].dot(v), m[2].dot(v));
    let (a, b, c) = (first.dot(times(first)), first.dot(times(second)), second.dot(times(second)));
    let (_, ways) = eigen([[a, b], [b, c]]);
    let least = first * ways[0][0] + second * ways[0][1];
    let slope = (a + c).abs() * normal.dot(times(least)).abs();
    let numbers =
        [m[0].x, m[1].y, m[2].z, m[0].y, m[0].z, m[1].z, first.x, first.y, first.z, second.x, second.y, second.z];
    Ok(match decide.angle_of(&Tangents, numbers.map(Interval::exact), slope)? {
        Side::Above => Singular::Crossing,
        Side::Below => Singular::Isolated,
        Side::Within => Singular::Cusp,
    })
}

/// Where the vertex of a member of rank three is.
enum Place {
    /// A point of the pair's frame on both surfaces, with the surface whose gradient is the larger there.
    On(Vec3, usize),
    /// A direction on both asymptotic cones.
    Infinite,
    /// Off them.
    Off,
}

/// Whether `vertex`, a member's, lies on both surfaces, within the band: a point on the surface whose gradient is the
/// larger there, since a point of the member on one is on the other, or a direction on its asymptotic cone.
fn place(pair: &Pair, vertex: [f64; 4], decide: &mut Decisions) -> Result<Place, Undecided> {
    let (point, weight) = (Vec3::new(vertex[0], vertex[1], vertex[2]), vertex[3]);
    let quadrics = [pair.quadric(0), pair.quadric(1)];
    if weight.abs() <= 1e-9 * point.norm() {
        let way = unit(point);
        let steep = |q: &Quadric| (q.times(way) - way * way.dot(q.times(way))).norm();
        let on = usize::from(steep(&quadrics[1]) > steep(&quadrics[0]));
        let side = quadrics.get(on).map_or(Ok(Side::Above), |q| q.toward(way, decide))?;
        return Ok(if side == Side::Within { Place::Infinite } else { Place::Off });
    }
    let at = point * (1.0 / weight);
    let gradient = |q: &Quadric| q.half_gradient(at).norm();
    let on = usize::from(gradient(&quadrics[1]) > gradient(&quadrics[0]));
    let side = quadrics.get(on).map_or(Ok(Side::Above), |q| q.holds(at, decide))?;
    Ok(if side == Side::Within { Place::On(at, on) } else { Place::Off })
}

/// The multiple root a member of rank three at `phi` makes, if its vertex lies on both surfaces.
fn vertex(
    pair: &Pair,
    (phi, m): (f64, [[f64; 4]; 4]),
    vertex: [f64; 4],
    decide: &mut Decisions,
) -> Result<Option<Multiple>, Undecided> {
    Ok(match place(pair, vertex, decide)? {
        Place::Off => None,
        Place::Infinite => Some(Multiple::AtInfinity { phi }),
        Place::On(at, on) => {
            let part = std::array::from_fn(|i| Vec3::new(entry(&m, i, 0), entry(&m, i, 1), entry(&m, i, 2)));
            Some(Multiple::Vertex { phi, at, kind: singular(part, &pair.quadric(on), at, decide)? })
        }
    })
}

/// The planes `√|μ₃| e₃ ± √|μ₄| e₄` of a member of rank two, or none where they are conjugate or one is at infinity.
fn planes(values: [f64; 4], vectors: [[f64; 4]; 4]) -> Option<[(Vec3, Vec3); 2]> {
    let ([_, _, mu3, mu4], [_, _, e3, e4]) = (values, vectors);
    if mu3 * mu4 >= 0.0 {
        return None;
    }
    let (k3, k4) = (mu3.abs().sqrt(), mu4.abs().sqrt());
    let plane = |sign: f64| {
        let l: [f64; 4] = std::array::from_fn(|i| {
            k3 * e3.get(i).copied().unwrap_or(0.0) + sign * k4 * e4.get(i).copied().unwrap_or(0.0)
        });
        let normal = Vec3::new(l[0], l[1], l[2]);
        let size = normal.norm();
        (size > 1e-9).then(|| (normal * (1.0 / size), normal * (-l[3] / (size * size))))
    };
    Some([plane(1.0)?, plane(-1.0)?])
}

/// Whether every member of the pencil of `pair` is singular, its determinant zero throughout: two members chosen
/// apart from any special one are singular, and their vertices, finite and not far, lie on both surfaces within the
/// band.
pub(crate) fn every_member_singular(pair: &Pair, decide: &mut Decisions) -> Result<bool, Undecided> {
    let members = Members::of(pair);
    let reach = 1e3 * (1.0 + (pair.surfaces[0].at() - pair.surfaces[1].at()).norm());
    let mut probed = 0_u32;
    for phi in [0.7, 2.1, 1.3, 2.7] {
        let (values, vectors) = eigen(members.at(phi));
        if values[0].abs() > 1e-9 * values[3].abs() {
            return Ok(false);
        }
        let [x, y, z, w] = vectors[0];
        if w.abs() * reach <= Vec3::new(x, y, z).norm() {
            continue;
        }
        if matches!(place(pair, vectors[0], decide)?, Place::Off) {
            return Ok(false);
        }
        probed = probed.saturating_add(1);
        if probed == 2 {
            return Ok(true);
        }
    }
    Ok(false)
}

/// In a pencil whose every member is singular, the member of rank two, where the sum of the principal 3×3 minors
/// vanishes, as its two planes, where they hold.
pub(crate) fn singular_planes(pair: &Pair, decide: &mut Decisions) -> Result<Option<[(Vec3, Vec3); 2]>, Undecided> {
    let members = Members::of(pair);
    let minors = members.minors();
    for phi in minors.roots(1e-14 * minors.size()) {
        let (values, vectors) = eigen(members.at(phi));
        if let Some(two) = planes(values, vectors)
            && holds(pair, two, decide)?
        {
            return Ok(Some(two));
        }
    }
    Ok(None)
}

/// The multiple roots of the pencil of `pair`, each with what its member is.
pub(crate) fn multiple(pair: &Pair, decide: &mut Decisions) -> Result<Vec<Multiple>, Undecided> {
    let members = Members::of(pair);
    let det = members.determinant();
    let size = det.size();
    let turn = |phi: f64| {
        let half = phi.rem_euclid(PI);
        if half >= PI - 1e-9 { half - PI } else { half }
    };
    let (first, second) = (det.rate(), det.rate().rate());
    let third = second.rate();
    let mut critical: Vec<f64> =
        [first, second, third].iter().flat_map(|d| d.roots(1e-14 * d.size())).map(turn).collect();
    critical.sort_by(f64::total_cmp);
    critical.dedup_by(|a, b| (*a - *b).abs() <= 1e-9);
    let mut found = Vec::new();
    for phi in critical {
        if det.at(phi).abs() > 1e-3 * size {
            continue;
        }
        let m = members.at(phi);
        let (values, vectors) = eigen(m);
        let largest = values[3].abs();
        let multiple = if values[1].abs() > 1e-6 * largest {
            vertex(pair, (phi, m), vectors[0], decide)?
        } else if values[2].abs() <= 1e-6 * largest {
            Some(Multiple::Double)
        } else {
            match planes(values, vectors) {
                None => Some(Multiple::Conjugate),
                Some(two) => holds(pair, two, decide)?.then_some(Multiple::Planes { planes: two }),
            }
        };
        if let Some(one) = multiple
            && !found.iter().any(|f| same(f, &one))
        {
            found.push(one);
        }
    }
    Ok(found)
}

/// Whether two multiple roots are one, found from two candidates near it: one vertex, one direction's member, one
/// pair of planes, or one of each kind with no place.
fn same(a: &Multiple, b: &Multiple) -> bool {
    match (a, b) {
        (Multiple::Vertex { at, .. }, Multiple::Vertex { at: other, .. }) => {
            (*at - *other).norm() <= 1e-7 * (1.0 + at.norm())
        }
        (Multiple::AtInfinity { phi }, Multiple::AtInfinity { phi: other }) => sin_cos(phi - other).0.abs() <= 1e-7,
        (Multiple::Planes { planes }, Multiple::Planes { planes: other }) => {
            let alike = |p: &(Vec3, Vec3), q: &(Vec3, Vec3)| p.0.cross(q.0).norm() <= 1e-7;
            (alike(&planes[0], &other[0]) && alike(&planes[1], &other[1]))
                || (alike(&planes[0], &other[1]) && alike(&planes[1], &other[0]))
        }
        (Multiple::Conjugate, Multiple::Conjugate) | (Multiple::Double, Multiple::Double) => true,
        _ => false,
    }
}
