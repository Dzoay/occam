//! Source: original
//! Volume, area and first moments of a body's faces by the divergence theorem (PARITY 1.8). Each is an integral over the
//! faces of a field's flux, `(p - c) / 3` for the volume and `(p - c)_x^2 / 2` along `x` for the moment about `c` in `x`,
//! or of the area's element alone, so over a face's region of its parameters, `g(u, v)` times `Su x Sv`. Green's theorem
//! turns that into an integral round the region's loops of `G(u, v) dv`, where `G` is `g`'s integral along `u` from a
//! fixed `u` the face's region does not go below; so each loop is integrated by its uses' curves in the face's
//! parameters, and at each point there along `u`, both adaptively ([`super::quadrature`]). Where a loop's uses do not
//! meet in the parameters, as across a pole or an apex, where the surface collapses, the straight step between them
//! is integrated too.
//!
//! Whichever way a loop runs, its own signed area in the parameters says: an outer loop counts with the sign that
//! makes its region's area positive, and a hole with the other. The face's outward normal is the surface's turned
//! over where the face is reversed; the area takes none of that sign. About a centre near the body, the sums keep small.

use core::cell::Cell;

use occam_geom::{ParamCurve, Surface};
use occam_report::{Code, EntityRef, Fault, Report};
use occam_topo::{Face, Id, Snapshot};
use occam_units::{UvPoint, Vec3};

use super::quadrature::{Budget, Integral, integral};

/// The sums a body's faces give: the volume, the area, the three first moments about the centre, and, carried along
/// to orient each loop, the signed area of the loop's region in its face's parameters.
pub(super) const SUMS: usize = 6;

/// Each of the sums.
pub(super) type Sums = [f64; SUMS];

/// How many steps each piece of a loop is sampled at, to find how far `v` runs along it and where `u` is least.
const SAMPLES: u32 = 16;

/// A piece of a loop in its face's parameters: a use's curve there over its edge's range, in the use's direction, or
/// the straight step from where one use ends to where the next begins.
#[derive(Clone, Copy)]
pub(super) enum Piece<'a> {
    /// The curve, from and to these values of the edge's parameter.
    Curve(&'a dyn ParamCurve, f64, f64),
    /// From the first point to the second, over 0 to 1.
    Step(UvPoint, UvPoint),
}

impl Piece<'_> {
    /// The parameter's range, in the piece's direction.
    pub(super) const fn range(self) -> (f64, f64) {
        match self {
            Self::Curve(_, from, to) => (from, to),
            Self::Step(..) => (0.0, 1.0),
        }
    }

    /// The point at `t`, and how fast `v` runs there.
    pub(super) fn at(self, t: f64) -> (UvPoint, f64) {
        match self {
            Self::Curve(curve, ..) => {
                let (point, rate) = curve.first(t);
                (point, rate.v())
            }
            Self::Step(a, b) => {
                let point = UvPoint::new(a.u() + t * (b.u() - a.u()), a.v() + t * (b.v() - a.v()));
                (point, b.v() - a.v())
            }
        }
    }

    /// Its points at the samples' steps, both ends included.
    pub(super) fn samples(self) -> impl Iterator<Item = UvPoint> {
        let (from, to) = self.range();
        let steps = f64::from(SAMPLES);
        (0..=SAMPLES).map(move |k| self.at(from + (to - from) * f64::from(k) / steps).0)
    }
}

/// A face to integrate over: the surface it lies on, whether its outward normal is the surface's turned over, and its
/// loops, the outer first, each as the pieces round it.
pub(super) struct Patch<'a> {
    pub surface: &'a dyn Surface,
    pub reversed: bool,
    pub loops: Vec<Vec<Piece<'a>>>,
}

/// A report that `face`'s loop has a use, of `edge`, with no curve in the face's parameters.
fn no_curve(face: Id<Face>, edge: EntityRef) -> Report {
    Report::new(Code::NoParameterCurve, Fault::InvalidInput).with_entity(edge).with_entity(face.entity())
}

/// A report that `entity` names something the model does not hold, which a committed model never does.
fn corrupt(entity: EntityRef) -> Report {
    Report::new(Code::InvariantFailed, Fault::Internal).with_entity(entity)
}

/// The pieces of the loop whose uses are `uses`, each its curve in `face`'s parameters.
fn pieces<'a>(model: &'a Snapshot, face: Id<Face>, uses: &[Id<occam_topo::EdgeUse>]) -> Result<Vec<Piece<'a>>, Report> {
    let mut curves = Vec::new();
    for &id in uses {
        let view = model.view(id).ok_or_else(|| corrupt(id.entity()))?;
        let edge = model.view(view.edge()).ok_or_else(|| corrupt(view.edge().entity()))?;
        let curve = view.pcurve().ok_or_else(|| no_curve(face, view.edge().entity()))?;
        let (lo, hi) = edge.range();
        curves.push(if view.reversed() { Piece::Curve(curve, hi, lo) } else { Piece::Curve(curve, lo, hi) });
    }
    Ok(joined(&curves))
}

/// `curves` in turn, with a step wherever one ends away from where the next begins in `v`, round to the first; a step
/// along `u` alone adds nothing.
fn joined<'a>(curves: &[Piece<'a>]) -> Vec<Piece<'a>> {
    let mut out = Vec::new();
    for (k, &piece) in curves.iter().enumerate() {
        out.push(piece);
        let next = curves.get(k.saturating_add(1)).or_else(|| curves.first()).copied().unwrap_or(piece);
        let (end, start) = (piece.at(piece.range().1).0, next.at(next.range().0).0);
        if (start.v() - end.v()).abs() > 0.0 {
            out.push(Piece::Step(end, start));
        }
    }
    out
}

/// The face `face` of `model` to integrate over.
///
/// # Errors
///
/// `measure.no-parameter-curve` where one of its uses has no curve in its parameters.
pub(super) fn patch(model: &Snapshot, face: Id<Face>) -> Result<Patch<'_>, Report> {
    let view = model.view(face).ok_or_else(|| corrupt(face.entity()))?;
    let surface = view.surface().ok_or_else(|| corrupt(view.surface_id().entity()))?;
    let mut loops = Vec::new();
    for id in view.loops() {
        let uses = model.view(id).map(occam_topo::View::<occam_topo::Loop>::uses).unwrap_or_default();
        loops.push(pieces(model, face, &uses)?);
    }
    Ok(Patch { surface, reversed: view.reversed(), loops })
}

/// The sums' integrands at `(u, v)` of `surface`, about `centre`, the flux taken outward where `sign` is 1.
fn integrands(surface: &dyn Surface, centre: Vec3, sign: f64, (u, v): (f64, f64)) -> Sums {
    let jet = surface.first(UvPoint::new(u, v));
    let p = jet.point.to_vec3() - centre;
    let normal = jet.du.rate().cross(jet.dv.rate());
    let out = normal * sign;
    [p.dot(out) / 3.0, normal.norm(), 0.5 * p.x * p.x * out.x, 0.5 * p.y * p.y * out.y, 0.5 * p.z * p.z * out.z, 1.0]
}

/// `sums` times `by`.
fn scaled(sums: Sums, by: f64) -> Sums {
    sums.map(|s| s * by)
}

/// What the integrals of one body are asked for: the tolerance of each sum over the whole body, and the centre the
/// moments are taken about.
#[derive(Clone, Copy, Debug)]
pub(super) struct Asked {
    pub tolerance: Sums,
    pub centre: Vec3,
}

/// The integral round `piece` of `G dv`, `G` from `start` along `u`, within `outer`, each `G` within `inner`.
fn round(
    patch: &Patch<'_>,
    piece: Piece<'_>,
    (start, centre): (f64, Vec3),
    (outer, inner): (Sums, Sums),
    budget: &Budget,
) -> Result<Integral<SUMS>, Sums> {
    let sign = if patch.reversed { -1.0 } else { 1.0 };
    let failed: Cell<Option<Sums>> = Cell::new(None);
    let along = |t: f64| {
        let (uv, rate) = piece.at(t);
        // Along `u` alone, `dv` is nothing, and so is what `G` would add.
        if rate.abs() <= 0.0 {
            return [0.0; SUMS];
        }
        let g = |s: f64| integrands(patch.surface, centre, sign, (s, uv.v()));
        match integral(&g, (start, uv.u()), inner, budget) {
            Ok(within) => scaled(within.value, rate),
            Err(reached) => {
                failed.set(Some(reached));
                [f64::NAN; SUMS]
            }
        }
    };
    integral(&along, piece.range(), outer, budget).map_err(|reached| failed.get().unwrap_or(reached))
}

/// The sums over `patches`, each to its share of what is `asked`, within `budget`.
///
/// # Errors
///
/// The error reached so far where the budget is spent, or a value is not finite, first.
pub(super) fn sums(patches: &[Patch<'_>], asked: Asked, budget: &Budget) -> Result<Integral<SUMS>, Sums> {
    let outer = shared(asked.tolerance, patches.iter().flat_map(|p| &p.loops).map(Vec::len).sum());
    let mut total = Integral::ZERO;
    for patch in patches {
        let start = patch.loops.iter().flatten().flat_map(|p| p.samples()).map(UvPoint::u).fold(f64::INFINITY, least);
        let mut face = Integral::ZERO;
        for (k, pieces) in patch.loops.iter().enumerate() {
            let mut whole = Integral::ZERO;
            for &piece in pieces {
                let points: Vec<UvPoint> = piece.samples().collect();
                let inner = along(outer, run(&points));
                whole = whole.plus(round(patch, piece, (start, asked.centre), (outer, inner), budget)?, 1.0);
            }
            let turn = if whole.value[SUMS - 1] < 0.0 { -1.0 } else { 1.0 };
            face = face.plus(whole, if k == 0 { turn } else { -turn });
        }
        total = total.plus(face, 1.0);
    }
    Ok(total)
}

/// Each of `count` pieces' share of `tolerance`, the body's: half of it, split evenly, so that what each `G` along them
/// may be off by can take the other half.
fn shared(tolerance: Sums, count: usize) -> Sums {
    scaled(tolerance, 0.5 / f64::from(u32::try_from(count).unwrap_or(u32::MAX).max(1)))
}

/// How far `v` runs along `points` in turn.
fn run(points: &[UvPoint]) -> f64 {
    // Folded, each pair's `v` less nothing, then the second's less the first's.
    points.windows(2).map(|w| w.iter().fold(0.0, |d, p| p.v() - d).abs()).sum()
}

/// The tolerance of each `G` along a piece whose share is `share` and that `v` runs `run` along: a quarter of the
/// share over the run, so that `G`'s error times how far `v` runs is a quarter of it; the share where `v` does not run.
fn along(share: Sums, run: f64) -> Sums {
    scaled(share, if run > 0.0 { 0.25 / run } else { 1.0 })
}

/// The lesser of two numbers, the first if they are equal.
fn least(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

#[cfg(test)]
mod tests {
    use occam_geom::{Line, Plane, UvCurve};
    use occam_units::{Direction, Local, Placement, Point, World};

    use super::*;

    /// The straight curve in the parameters from `a` to `b`, over its length.
    fn segment(a: (f64, f64), b: (f64, f64)) -> (UvCurve<Line>, f64) {
        let along = Vec3::new(b.0 - a.0, b.1 - a.1, 0.0);
        let start = Point::<World>::from_vec3(Vec3::new(a.0, a.1, 0.0));
        (UvCurve::new(Line::new(start, Direction::normalised(along))).unwrap(), along.norm())
    }

    /// The pieces through `corners`, each from one to the next, leaving out the last back to the first.
    fn open(corners: &[(f64, f64)]) -> Vec<(UvCurve<Line>, f64)> {
        corners.windows(2).map(|w| segment(w[0], w[1])).collect()
    }

    fn pieces(curves: &[(UvCurve<Line>, f64)]) -> Vec<Piece<'_>> {
        curves.iter().map(|(c, length)| Piece::Curve(c as &dyn ParamCurve, 0.0, *length)).collect()
    }

    fn plane() -> Plane {
        Plane::placed(&Placement::<Local, World>::identity())
    }

    /// The sums over the plane's region `loops`, about `(1, 0.5, 1)`, the face reversed if `reversed`.
    fn over(plane: &Plane, loops: Vec<Vec<Piece<'_>>>, reversed: bool) -> Integral<SUMS> {
        let patch = Patch { surface: plane, reversed, loops };
        let tolerance = [1e-12, 1e-12, 1e-12, 1e-12, 1e-12, f64::INFINITY];
        let asked = Asked { tolerance, centre: Vec3::new(1.0, 0.5, 1.0) };
        sums(&[patch], asked, &Budget::new(1 << 16)).unwrap()
    }

    #[test]
    fn a_loop_is_closed_by_a_step_where_its_curves_do_not_meet_and_bounds_its_region_either_way_round() {
        // A rectangle 2 by 1 in the plane z = 0, its left side left out, so the step from (0, 1) to (0, 0) closes it.
        let plane = plane();
        let corners = [(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (0.0, 1.0)];
        let curves = open(&corners);
        let loop_ = joined(&pieces(&curves));
        assert_eq!(loop_.len(), 4, "three curves and a step");
        assert!(matches!(loop_[3], Piece::Step(a, b) if (a.u(), a.v(), b.u(), b.v()) == (0.0, 1.0, 0.0, 0.0)));
        let sums = over(&plane, vec![loop_], false);
        // About (1, 0.5, 1), the plane is 1 below: its flux of (p - c) / 3 up is -1/3 of its area. The moments: of
        // (x - 1)^2 / 2 along x, nothing, as the normal is along z; of (z - 1)^2 / 2 along z, 1/2 of its area.
        let want = [-2.0 / 3.0, 2.0, 0.0, 0.0, 1.0, 2.0];
        assert!(sums.value.iter().zip(want).all(|(g, w)| (g - w).abs() < 1e-12), "{sums:?}");
        // Turned over, the flux changes sign and the area does not; run the other way round, nothing changes.
        let turned = over(&plane, vec![joined(&pieces(&curves))], true);
        assert!((turned.value[0] - 2.0 / 3.0).abs() < 1e-12 && (turned.value[1] - 2.0).abs() < 1e-12, "{turned:?}");
        let back = open(&[(0.0, 1.0), (2.0, 1.0), (2.0, 0.0), (0.0, 0.0)]);
        let other = over(&plane, vec![joined(&pieces(&back))], false);
        assert!((other.value[0] + 2.0 / 3.0).abs() < 1e-12 && (other.value[1] - 2.0).abs() < 1e-12, "{other:?}");
        // Pieces that meet, or meet but in `u`, need no step.
        let along = open(&[(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (0.0, 1.0), (0.5, 0.0)]);
        assert_eq!(joined(&pieces(&along)).len(), 4);
    }

    #[test]
    fn a_hole_takes_its_region_away_whichever_way_it_runs() {
        let plane = plane();
        let outer = open(&[(0.0, 0.0), (2.0, 0.0), (2.0, 1.0), (0.0, 1.0), (0.0, 0.0)]);
        let square = [(0.5, 0.25), (1.0, 0.25), (1.0, 0.75), (0.5, 0.75), (0.5, 0.25)];
        for hole in [open(&square), open(&square.iter().rev().copied().collect::<Vec<_>>())] {
            let sums = over(&plane, vec![joined(&pieces(&outer)), joined(&pieces(&hole))], false);
            assert!((sums.value[1] - 1.75).abs() < 1e-12 && (sums.value[0] + 1.75 / 3.0).abs() < 1e-12, "{sums:?}");
        }
    }

    #[test]
    fn a_step_that_crosses_the_parameters_is_integrated_along_its_line() {
        // A unit sphere's region from latitude 0 to 0.5 and longitude v to 1, its left side a step along u = v: the
        // area is the integral of cos v (1 - v), which is 1 + sin(0.5) / 2 - cos(0.5).
        let sphere = occam_geom::Sphere::placed(
            &Placement::<Local, World>::identity(),
            occam_units::Length::from_millimetres(1.0),
        );
        let sphere = sphere.unwrap();
        let curves = open(&[(0.0, 0.0), (1.0, 0.0), (1.0, 0.5), (0.5, 0.5)]);
        let loop_ = joined(&pieces(&curves));
        assert!(matches!(loop_.last(), Some(Piece::Step(..))));
        let patch = Patch { surface: &sphere, reversed: false, loops: vec![loop_] };
        let tolerance = [1e-12, 1e-12, 1e-12, 1e-12, 1e-12, f64::INFINITY];
        let asked = Asked { tolerance, centre: Vec3::new(0.0, 0.0, 0.0) };
        let got = sums(&[patch], asked, &Budget::new(1 << 16)).unwrap();
        let (sin, cos) = occam_math::sin_cos(0.5);
        let want = 1.0 + 0.5 * sin - cos;
        assert!((got.value[1] - want).abs() < 1e-11, "{} against {want}", got.value[1]);
    }

    #[test]
    fn each_piece_takes_an_even_share_of_half_the_tolerance_and_each_g_a_quarter_of_that_over_how_far_v_runs() {
        let tolerance = [4.0, 8.0, 12.0, 16.0, 20.0, f64::INFINITY];
        assert_eq!(shared(tolerance, 4), [0.5, 1.0, 1.5, 2.0, 2.5, f64::INFINITY]);
        assert_eq!(shared(tolerance, 0), [2.0, 4.0, 6.0, 8.0, 10.0, f64::INFINITY], "no pieces, as one");
        let points = [UvPoint::new(0.0, 0.0), UvPoint::new(5.0, 2.0), UvPoint::new(1.0, -1.0), UvPoint::new(1.0, -1.0)];
        assert_eq!(run(&points), 5.0);
        assert_eq!(run(&points[..1]), 0.0);
        assert_eq!(along(tolerance, 2.0), [0.5, 1.0, 1.5, 2.0, 2.5, f64::INFINITY]);
        assert_eq!(along(tolerance, 0.0), tolerance, "where v does not run, its whole share");
    }

    #[test]
    fn the_integrands_are_the_fluxes_and_the_area_at_a_point() {
        let plane = plane();
        let at = integrands(&plane, Vec3::new(1.0, 2.0, 3.0), -1.0, (4.0, 6.0));
        // p - c = (3, 4, -3), the normal (0, 0, 1), turned over.
        assert_eq!(at, [1.0, 1.0, 0.0, 0.0, -4.5, 1.0]);
    }
}
