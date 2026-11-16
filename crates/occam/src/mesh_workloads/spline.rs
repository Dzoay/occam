//! Source: original
//! P9's spline parts on the kernel's own geometry: the panel, a bicubic B-spline slab with a hole, and the quilt, a
//! torus of Bézier patches. No request makes them yet, so they are written through `Edit`. Every surface is one the
//! native format writes: a panel's walls are its edges' curves swept straight down, which are B-splines of degree one
//! across, where P9 had a surface of extrusion.
//!
//! A panel's hole is a circle in its top's parameters. The kernel has no curve that is a surface's image of another,
//! so the hole's edge is a cubic B-spline through the top at [`HOLE_POINTS`] angles round the circle, as an imported
//! model's would be, and it records how far it strays from the top between them.
#![expect(
    clippy::arithmetic_side_effects,
    reason = "indices into a panel's 7 by 7 poles and a quilt's lattice of at most 36 by 36"
)]

use core::f64::consts::{PI, TAU};
use std::sync::Arc;

use occam_geom::{BsplineCurve, BsplineSurface, Circle, Curve, Line, ParamCurve, SplineCurve, SplineSurface, Surface};
use occam_geom::{UvCurve, interpolate};
use occam_math::sin_cos;
use occam_report::Report;
use occam_units::{Direction, Length, Local, Placement, Point, UvPoint, Vec3, Vector, World};

use super::parts::{Frame, SplitMix};
use super::written::{Piece, Ring, Sided, Solid, Use, missing, quad};

/// How many intervals the hole's edge is fitted over: enough that it strays well inside the model's tolerance.
pub(crate) const HOLE_POINTS: u32 = 512;

/// The hole in a panel's top, in its parameters: the centre and the radius, as P9 drew it.
const HOLE: ((f64, f64), f64) = ((0.45, 0.55), 0.17);

/// The panel's knots, each way: clamped cubic over four spans.
const KNOTS: [f64; 11] = [0.0, 0.0, 0.0, 0.0, 0.25, 0.5, 0.75, 1.0, 1.0, 1.0, 1.0];

/// How many poles the panel has each way.
const PANEL_POLES: u32 = 7;

/// `local`, a point in `frame`'s axes, in the world.
pub(crate) fn world(frame: &Frame, [x, y, z]: [f64; 3]) -> Point<World> {
    let (sin, cos) = sin_cos(frame.turn);
    let [ox, oy, oz] = frame.origin;
    Point::from_vec3(Vec3::new(ox + x * cos - y * sin, oy + x * sin + y * cos, oz + z))
}

/// `p` moved `down` millimetres along -Z.
fn lowered(p: Point<World>, down: f64) -> Point<World> {
    p - Vector::from_vec3(Vec3::new(0.0, 0.0, down))
}

/// `form` with every pole moved `down` along -Z.
fn lowered_curve(form: &BsplineCurve, down: f64) -> BsplineCurve {
    BsplineCurve { poles: form.poles.iter().map(|&p| lowered(p, down)).collect(), ..form.clone() }
}

/// The surface `form` sweeps moving `down` along -Z: its parameter across is the length moved.
fn swept(form: &BsplineCurve, down: f64) -> Result<Arc<dyn Surface>, Report> {
    let count = u32::try_from(form.poles.len()).unwrap_or(u32::MAX);
    let poles = form.poles.iter().copied().chain(form.poles.iter().map(|&p| lowered(p, down))).collect();
    let knots_v = vec![0.0, 0.0, down, down];
    let surface = BsplineSurface {
        degree: (form.degree, 1),
        count: (count, 2),
        knots_u: form.knots.clone(),
        knots_v,
        poles,
        weights: None,
    };
    Ok(Arc::new(SplineSurface::new(&surface)?))
}

/// The straight edge from `top` down `down` millimetres, its parameter the length.
fn plumb(top: Point<World>) -> Arc<dyn Curve> {
    Arc::new(Line::new(top, Direction::normalised(Vec3::new(0.0, 0.0, -1.0))))
}

fn curve(form: &BsplineCurve) -> Result<Arc<dyn Curve>, Report> {
    Ok(Arc::new(SplineCurve::new(form)?))
}

/// A panel's top's poles, a row of constant `v` at a time: P9's bump over `w` by `d`, `height` up.
fn panel_poles(frame: &Frame, (w, d, height): (f64, f64, f64), phase: f64) -> Vec<Point<World>> {
    let last = f64::from(PANEL_POLES - 1);
    let mut poles = Vec::new();
    for j in 0..PANEL_POLES {
        for i in 0..PANEL_POLES {
            let (x, y) = (w * f64::from(i) / last, d * f64::from(j) / last);
            let (along, _) = sin_cos(2.0 * x / w * PI + phase);
            let (_, across) = sin_cos(1.5 * y / d * PI + 0.7 * phase);
            poles.push(world(frame, [x, y, height + 0.08 * (w + d) * along * across]));
        }
    }
    poles
}

/// The B-spline of the panel's poles `at` gives, in order.
fn panel_curve(poles: &[Point<World>], at: impl Iterator<Item = u32>) -> BsplineCurve {
    let picked = at.filter_map(|k| poles.get(usize::try_from(k).ok()?).copied()).collect();
    BsplineCurve { degree: 3, knots: KNOTS.to_vec(), poles: picked, weights: None }
}

/// The hole's circle in the top's parameters, its parameter the angle.
fn hole_circle() -> Result<Arc<dyn ParamCurve>, Report> {
    let ((u, v), radius) = HOLE;
    let centre =
        Placement::<Local, World>::identity().then(&Placement::translation(Vector::from_vec3(Vec3::new(u, v, 0.0))));
    let circle = Circle::placed(&centre, Length::from_millimetres(radius))?;
    Ok(Arc::new(UvCurve::new(circle)?))
}

/// The point of `top` at `angle` round the hole.
fn on_hole(top: &dyn Surface, angle: f64) -> Point<World> {
    let ((u, v), radius) = HOLE;
    let (sin, cos) = sin_cos(angle);
    top.point(UvPoint::new(u + radius * cos, v + radius * sin))
}

/// The hole's edge fitted through `top` at [`HOLE_POINTS`] angles, its parameter the angle, and how far it strays
/// from the top half way between them.
fn hole_edge(top: &dyn Surface) -> Result<(BsplineCurve, f64), Report> {
    let step = TAU / f64::from(HOLE_POINTS);
    let angles: Vec<f64> = (0..=HOLE_POINTS).map(|k| step * f64::from(k)).collect();
    let points: Vec<Point<World>> = angles.iter().map(|&angle| on_hole(top, angle)).collect();
    let form = interpolate(3, &angles, &points)?;
    let fitted = SplineCurve::new(&form)?;
    let mut strays = 0.0;
    for &angle in angles.iter().skip(1) {
        let half = angle - 0.5 * step;
        let off = (Curve::point(&fitted, half).to_vec3() - on_hole(top, half).to_vec3()).norm();
        strays = if off > strays { off } else { strays };
    }
    Ok((form, strays))
}

/// The panel's twelve points: the top's corners, the bottom's, and where the hole's edge begins and is half way round,
/// at the top and then at the bottom.
fn panel_points(poles: &[Point<World>], hole: [Point<World>; 2], height: f64) -> Result<[Point<World>; 12], Report> {
    let last = PANEL_POLES - 1;
    let corners = [0, last, PANEL_POLES * PANEL_POLES - 1, PANEL_POLES * last];
    let top: Vec<Point<World>> = corners.iter().filter_map(|&k| poles.get(usize::try_from(k).ok()?).copied()).collect();
    let bottom = top.iter().map(|&p| lowered(p, height));
    let holes = hole.into_iter().chain(hole.map(|p| lowered(p, height)));
    let all: Vec<Point<World>> = top.iter().copied().chain(bottom).chain(holes).collect();
    all.try_into().map_err(|_| missing())
}

/// The ends of each edge of the panel's top and of its bottom.
const SIDE_ENDS: [(usize, usize); 8] = [(0, 1), (1, 2), (3, 2), (0, 3), (4, 5), (5, 6), (7, 6), (4, 7)];
/// The ends of each plumb line: the four corners', then the hole's two.
const PLUMB_ENDS: [(usize, usize); 6] = [(0, 4), (1, 5), (2, 6), (3, 7), (8, 10), (9, 11)];
/// The hole's halves at the top and at the bottom: their ends.
const HOLE_ENDS: [(usize, usize); 4] = [(8, 9), (9, 8), (10, 11), (11, 10)];

/// The plumb line whose ends are `ends`, `height` long.
fn plumb_piece(points: &[Point<World>; 12], ends: (usize, usize), height: f64) -> Result<Piece, Report> {
    let top = points.get(ends.0).copied().ok_or_else(missing)?;
    Ok(Piece { curve: plumb(top), range: (0.0, height), ends, deviation: 0.0 })
}

/// The panel's eighteen edges: the top's four sides, the bottom's, the four corners' plumb lines, the hole's halves at
/// the top and at the bottom, and its two plumb lines.
fn panel_edges(
    sides: &[BsplineCurve; 4],
    (hole, strays): (&BsplineCurve, f64),
    points: &[Point<World>; 12],
    height: f64,
) -> Result<Vec<Piece>, Report> {
    let lowered_sides = sides.iter().map(|form| lowered_curve(form, height));
    let forms: Vec<BsplineCurve> = sides.iter().cloned().chain(lowered_sides).collect();
    let mut edges = Vec::new();
    for (form, &ends) in forms.iter().zip(&SIDE_ENDS) {
        edges.push(Piece { curve: curve(form)?, range: (0.0, 1.0), ends, deviation: 0.0 });
    }
    for &ends in PLUMB_ENDS.iter().take(4) {
        edges.push(plumb_piece(points, ends, height)?);
    }
    let lower = lowered_curve(hole, height);
    let halves = [(hole, (0.0, PI)), (hole, (PI, TAU)), (&lower, (0.0, PI)), (&lower, (PI, TAU))];
    for ((form, range), &ends) in halves.into_iter().zip(&HOLE_ENDS) {
        edges.push(Piece { curve: curve(form)?, range, ends, deviation: strays });
    }
    for &ends in PLUMB_ENDS.iter().skip(4) {
        edges.push(plumb_piece(points, ends, height)?);
    }
    Ok(edges)
}

/// The hole's loop on the top, clockwise, or on the bottom, anticlockwise: its two halves, each drawn on the circle.
fn hole_ring(halves: [usize; 2], clockwise: bool) -> Result<Ring, Report> {
    let circle = hole_circle()?;
    let half = |edge| Use { edge, reversed: clockwise, pcurve: Arc::clone(&circle) };
    let uses = if clockwise { vec![half(halves[1]), half(halves[0])] } else { vec![half(halves[0]), half(halves[1])] };
    let ((u, v), _) = HOLE;
    Ok(Ring { uses, witness: UvPoint::new(u, v) })
}

/// A face of the top or the bottom: its four sides, then the hole.
fn slab(surface: Arc<dyn Surface>, reversed: bool, sides: [usize; 4], halves: [usize; 2]) -> Result<Sided, Report> {
    let mut face = quad(surface, reversed, ((0.0, 1.0), (0.0, 1.0)), sides)?;
    // The middle of the top lies in the hole, so the outer loop's witness is near a corner.
    if let Some(outer) = face.loops.first_mut() {
        outer.witness = UvPoint::new(0.1, 0.1);
    }
    face.loops.push(hole_ring(halves, !reversed)?);
    Ok(face)
}

/// Each wall of the panel's sides: the top's side it hangs from, whether its surface faces in, and its edges as
/// [`quad`] takes them.
const WALLS: [(usize, bool, [usize; 4]); 4] =
    [(0, true, [0, 9, 4, 8]), (1, true, [1, 10, 5, 9]), (2, false, [2, 10, 6, 11]), (3, false, [3, 11, 7, 8])];

/// The panel's faces: the top, the bottom, the four walls and the hole's wall in two halves.
fn panel_faces(
    top: Arc<dyn Surface>,
    bottom: Arc<dyn Surface>,
    sides: &[BsplineCurve; 4],
    hole: &BsplineCurve,
    height: f64,
) -> Result<Vec<Sided>, Report> {
    let mut faces = vec![slab(top, false, [0, 1, 2, 3], [12, 13])?, slab(bottom, true, [4, 5, 6, 7], [14, 15])?];
    for &(side, reversed, edges) in &WALLS {
        let form = sides.get(side).ok_or_else(missing)?;
        faces.push(quad(swept(form, height)?, reversed, ((0.0, 1.0), (0.0, height)), edges)?);
    }
    let wall = swept(hole, height)?;
    faces.push(quad(Arc::clone(&wall), false, ((0.0, PI), (0.0, height)), [12, 17, 14, 16])?);
    faces.push(quad(wall, false, ((PI, TAU), (0.0, height)), [13, 16, 15, 17])?);
    Ok(faces)
}

/// P9's panel in `frame`: a bicubic B-spline top over `size.0` by `size.1`, bumped by `phase`, the same `size.2` lower
/// for the bottom, walls hanging from the top's sides, and a hole through it.
pub(crate) fn panel(frame: &Frame, size: (f64, f64, f64), phase: f64) -> Result<Solid, Report> {
    let height = size.2;
    let poles = panel_poles(frame, size, phase);
    let form = |poles: Vec<Point<World>>| BsplineSurface {
        degree: (3, 3),
        count: (PANEL_POLES, PANEL_POLES),
        knots_u: KNOTS.to_vec(),
        knots_v: KNOTS.to_vec(),
        poles,
        weights: None,
    };
    let top = SplineSurface::new(&form(poles.clone()))?;
    let bottom: Arc<dyn Surface> =
        Arc::new(SplineSurface::new(&form(poles.iter().map(|&p| lowered(p, height)).collect()))?);
    let (n, last) = (PANEL_POLES, PANEL_POLES - 1);
    let sides = [
        panel_curve(&poles, 0..n),
        panel_curve(&poles, (0..n).map(|j| j * n + last)),
        panel_curve(&poles, (0..n).map(|i| last * n + i)),
        panel_curve(&poles, (0..n).map(|j| j * n)),
    ];
    let (hole, strays) = hole_edge(&top)?;
    let points = panel_points(&poles, [on_hole(&top, 0.0), on_hole(&top, PI)], height)?;
    let edges = panel_edges(&sides, (&hole, strays), &points, height)?;
    let faces = panel_faces(Arc::new(top), bottom, &sides, &hole, height)?;
    Ok(Solid { points: points.to_vec(), edges, faces })
}

/// A quilt's lattice of poles, `size.0` round the torus's axis by `size.1` round its tube, each pushed in or out by up
/// to 15% of the tube's radius, as P9 drew it from `seed`: a torus of `4 radius` and `radius`.
struct Lattice {
    points: Vec<Point<World>>,
    size: (usize, usize),
}

impl Lattice {
    fn new(frame: &Frame, size: (usize, usize), radius: f64, seed: u64) -> Self {
        let mut rng = SplitMix(seed);
        let mut points = Vec::new();
        let count = |n: usize| f64::from(u32::try_from(n).unwrap_or(u32::MAX));
        for j in 0..size.1 {
            for i in 0..size.0 {
                let (sin_a, cos_a) = sin_cos(TAU * count(i) / count(size.0));
                let (sin_b, cos_b) = sin_cos(TAU * count(j) / count(size.1));
                let noise = 1.0 + 0.3 * (rng.next_f64() - 0.5);
                let rho = 4.0 * radius + radius * noise * cos_b;
                points.push(world(frame, [rho * cos_a, rho * sin_a, radius * noise * sin_b]));
            }
        }
        Self { points, size }
    }

    /// The pole `(i, j)`, round the torus either way.
    fn at(&self, i: usize, j: usize) -> Result<Point<World>, Report> {
        let (su, sv) = self.size;
        let k = i.checked_rem(su).zip(j.checked_rem(sv)).map(|(i, j)| i + su * j);
        k.and_then(|k| self.points.get(k).copied()).ok_or_else(missing)
    }
}

/// The weight of the pole `k` along a rational quilt's patch, as P9 gave it: less at the middle pole.
fn weight(degree: usize, k: usize) -> f64 {
    if degree == 2 && k == 1 { 0.85 } else { 1.0 }
}

/// The Bézier curve through the lattice's poles `at` gives, in order, rational at degree 2.
fn quilt_curve(
    lattice: &Lattice,
    degree: usize,
    at: impl Fn(usize) -> (usize, usize),
) -> Result<Arc<dyn Curve>, Report> {
    let poles = (0..=degree).map(|k| lattice.at(at(k).0, at(k).1)).collect::<Result<Vec<_>, _>>()?;
    let weights = (degree == 2).then(|| (0..=degree).map(|k| weight(degree, k)).collect());
    Ok(Arc::new(SplineCurve::bezier(poles, weights)?))
}

/// The patch `(i, j)` of a quilt of `degree`.
fn quilt_patch(lattice: &Lattice, degree: usize, (i, j): (usize, usize)) -> Result<Arc<dyn Surface>, Report> {
    let (mut poles, mut weights) = (Vec::new(), Vec::new());
    for jj in 0..=degree {
        for ii in 0..=degree {
            poles.push(lattice.at(degree * i + ii, degree * j + jj)?);
            weights.push(if weight(degree, ii) < 1.0 { weight(degree, ii) } else { weight(degree, jj) });
        }
    }
    let side = u32::try_from(degree + 1).map_err(|_| missing())?;
    let weights = (degree == 2).then_some(weights);
    Ok(Arc::new(SplineSurface::bezier((side, side), poles, weights)?))
}

/// P9's quilt in `frame`: a torus of `across.0` by `across.1` Bézier patches of `degree`, rational at degree 2, its
/// lattice jittered from `seed`. Each patch's sides are the curves of its lattice's rows and columns, which the
/// patches either side share.
pub(crate) fn quilt(frame: &Frame, across: (u32, u32), radius: f64, degree: u32, seed: u64) -> Result<Solid, Report> {
    let size = |n: u32| usize::try_from(n).map_err(|_| missing());
    let ((nu, nv), d) = ((size(across.0)?, size(across.1)?), size(degree)?);
    let lattice = Lattice::new(frame, (d * nu, d * nv), radius, seed);
    let vertex = |i: usize, j: usize| (i % nu) + nu * (j % nv);
    let mut solid = Solid::default();
    for j in 0..nv {
        for i in 0..nu {
            solid.points.push(lattice.at(d * i, d * j)?);
        }
    }
    for j in 0..nv {
        for i in 0..nu {
            let along = quilt_curve(&lattice, d, |k| (d * i + k, d * j))?;
            let round = quilt_curve(&lattice, d, |k| (d * i, d * j + k))?;
            solid.edges.push(Piece {
                curve: along,
                range: (0.0, 1.0),
                ends: (vertex(i, j), vertex(i + 1, j)),
                deviation: 0.0,
            });
            solid.edges.push(Piece {
                curve: round,
                range: (0.0, 1.0),
                ends: (vertex(i, j), vertex(i, j + 1)),
                deviation: 0.0,
            });
        }
    }
    for j in 0..nv {
        for i in 0..nu {
            let sides = [2 * vertex(i, j), 2 * vertex(i + 1, j) + 1, 2 * vertex(i, j + 1), 2 * vertex(i, j) + 1];
            solid.faces.push(quad(quilt_patch(&lattice, d, (i, j))?, false, ((0.0, 1.0), (0.0, 1.0)), sides)?);
        }
    }
    Ok(solid)
}
