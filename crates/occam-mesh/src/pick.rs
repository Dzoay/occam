//! Source: original
//! Picking (docs/decisions/2026-11-13-picking-in-the-first-release.md): a ray, in `f64` in the model's frame, tested
//! against the CPU mesh and never against GPU output. A [`Picker`] is built with a [`Mesh`]: a box tree over every
//! face's triangles, and one over every edge's samples as segments. A pick walks both on one thread, so the same ray
//! gives the same answer at any thread count the mesh was made at, and on every target.
//!
//! A ray hits a triangle where the line through its start and a point one millimetre along it crosses the triangle,
//! decided exactly: each side of the triangle is tested by the sign of the volume it makes with that line, so a ray
//! through a side shared by two triangles hits both, never neither. An edge is hit where its samples come within the
//! tolerance of the ray, at or after the ray's start. The answer is the nearest face hit, by distance along the ray
//! and then by the smaller id, unless an edge wins: one within the tolerance that lies no further along the ray than
//! that face plus the tolerance, or one of that face's own edges within the tolerance of where the ray hits it. Of
//! the edges that win, the one nearest the ray wins, then the one nearest along it, then the smaller id. A ray that
//! hits nothing is refused with [`Code::RayMissed`], and its remedy aims at the centre of the triangle that lies
//! nearest the ray's way from its start.

mod tree;

use std::collections::BTreeMap;

use occam_report::{Code, EntityRef, Fault, Input, NamedValue, Remedy, RemedyKind, Report};
use occam_topo::{Edge, Face, Id};
use occam_units::{Length, Point, Vec3, World};

use crate::limits::{higher, lower};
use crate::merge::{Mesh, span};
use crate::predicates::{P3, orient3d};
use tree::{Bounds, Tree, ZERO, around};

/// A ray: where it starts and its unit direction, in the model's frame, in millimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Ray {
    /// Where it starts.
    pub origin: Vec3,
    /// Its direction, of length one.
    pub direction: Vec3,
}

impl Ray {
    /// The ray from `origin` along `direction`, which is made of length one.
    ///
    /// # Errors
    ///
    /// [`Code::NotFinite`] for an origin with a coordinate that is not finite, and [`Code::InputNoDirection`] for a
    /// direction that is zero or not finite.
    pub fn new(origin: [f64; 3], direction: [f64; 3]) -> Result<Self, Report> {
        if let Some(&bad) = origin.iter().find(|x| !x.is_finite()) {
            return Err(Report::new(Code::NotFinite, Fault::InvalidInput)
                .with_input(Input::at("from"))
                .with_value(NamedValue::length("value", bad)));
        }
        let [x, y, z] = direction;
        let way = Vec3::new(x, y, z);
        let length = way.norm();
        if !(length.is_finite() && length > 0.0) {
            return Err(Report::new(Code::InputNoDirection, Fault::InvalidInput).with_input(Input::at("direction")));
        }
        let unit = Vec3::new(x / length, y / length, z / length);
        let [x, y, z] = origin;
        Ok(Self { origin: Vec3::new(x, y, z), direction: unit })
    }

    /// The point `t` along it.
    #[must_use]
    pub fn at(&self, t: f64) -> Vec3 {
        self.origin + self.direction * t
    }
}

/// What a ray hit first: a face or an edge, the point hit, and how far along the ray it is.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    /// The face or the edge.
    pub entity: EntityRef,
    /// Where: on the face, where the ray crosses it; on the edge, its point nearest the ray.
    pub point: Point<World>,
    /// How far along the ray from its start, to where the ray crosses the face or comes nearest the edge.
    pub distance: Length,
}

/// A triangle of a face: its corners, and the face's place among the faces.
#[derive(Clone, Debug, PartialEq)]
struct Triangle {
    corners: [Vec3; 3],
    face: u32,
}

/// An edge's samples, and the places of the faces that use it.
#[derive(Clone, Debug, PartialEq)]
struct Samples {
    edge: Id<Edge>,
    points: Vec<Vec3>,
    faces: Vec<u32>,
}

/// A mesh made ready to pick: its faces' triangles and its edges' samples, each under a box tree.
#[derive(Clone, Debug, PartialEq, Default)]
pub struct Picker {
    faces: Vec<Id<Face>>,
    triangles: Vec<Triangle>,
    edges: Vec<Samples>,
    /// Each segment, as its edge's place and the place of its first sample.
    segments: Vec<(u32, u32)>,
    triangle_tree: Tree,
    segment_tree: Tree,
}

/// `k` as a place in a list.
fn place(k: u32) -> usize {
    usize::try_from(k).unwrap_or(usize::MAX)
}

/// `k` as a number.
fn number(k: usize) -> u32 {
    u32::try_from(k).unwrap_or(u32::MAX)
}

impl Picker {
    /// `mesh` made ready to pick: every face that meshed, in id order, and every edge that was sampled.
    #[must_use]
    pub fn new(mesh: &Mesh) -> Self {
        let mut picker = Self::default();
        let mut users: BTreeMap<Id<Edge>, Vec<u32>> = BTreeMap::new();
        for (k, block) in mesh.blocks.iter().enumerate() {
            picker.faces.push(block.face);
            block.sides.iter().for_each(|side| users.entry(side.edge).or_default().push(number(k)));
            let triangles = span(&block.triangles).and_then(|range| mesh.triangles.get(range)).unwrap_or_default();
            for triangle in triangles {
                let corner = |k: u32| mesh.points.get(place(k)).map(|p| p.to_vec3());
                let [a, b, c] = triangle.map(corner);
                if let (Some(a), Some(b), Some(c)) = (a, b, c) {
                    picker.triangles.push(Triangle { corners: [a, b, c], face: number(k) });
                }
            }
        }
        for (edge, sampled) in mesh.edges.iter() {
            let Ok(sampled) = sampled else { continue };
            let points: Vec<Vec3> = sampled.points.iter().map(|p| p.to_vec3()).collect();
            let at = number(picker.edges.len());
            picker.segments.extend((1..points.len()).map(|k| (at, number(k.saturating_sub(1)))));
            let faces = users.get(&edge).cloned().unwrap_or_default();
            picker.edges.push(Samples { edge, points, faces });
        }
        let boxes: Vec<Bounds> = picker.triangles.iter().map(|t| around(&t.corners)).collect();
        picker.triangle_tree = Tree::new(&boxes);
        let boxes: Vec<Bounds> = picker.segments.iter().map(|&s| around(&picker.segment(s))).collect();
        picker.segment_tree = Tree::new(&boxes);
        picker
    }

    /// The ends of the segment `(edge, first)`.
    fn segment(&self, (edge, first): (u32, u32)) -> [Vec3; 2] {
        let points = self.edges.get(place(edge)).map_or_default(|samples| samples.points.as_slice());
        let at = |k: usize| points.get(k).copied().unwrap_or(ZERO);
        [at(place(first)), at(place(first).saturating_add(1))]
    }

    /// What `ray` hits first, with edges within `tolerance` of it: see the module's documentation.
    ///
    /// # Errors
    ///
    /// [`Code::RayMissed`] for a ray that hits no face and passes no edge within `tolerance`, with the remedy
    /// [`RemedyKind::AimAt`] where the mesh has a triangle to aim at.
    pub fn pick(&self, ray: &Ray, tolerance: Length) -> Result<Hit, Report> {
        let reach = tolerance.millimetres();
        let face = self.first_face(ray);
        if let Some(near) = self.nearest_edge(ray, reach, face)
            && let Some(samples) = self.edges.get(place(near.edge))
        {
            let (entity, point) = (samples.edge.entity(), Point::from_vec3(near.point));
            return Ok(Hit { entity, point, distance: Length::from_millimetres(near.along) });
        }
        let hit = face.and_then(|(along, k)| Some((along, self.faces.get(place(k))?.entity())));
        let Some((along, entity)) = hit else { return Err(self.missed(ray, reach)) };
        Ok(Hit { entity, point: Point::from_vec3(ray.at(along)), distance: Length::from_millimetres(along) })
    }

    /// The nearest face the ray crosses, as how far along it and the face's place: the smaller place where two tie.
    fn first_face(&self, ray: &Ray) -> Option<(f64, u32)> {
        let line = Line::of(ray);
        let mut best: Option<(f64, u32)> = None;
        self.triangle_tree.visit(ray, 0.0, |k| {
            let Some(triangle) = self.triangles.get(place(k)) else { return };
            let Some(along) = line.crossing(ray, &triangle.corners) else { return };
            if best.is_none_or(|held| (along, triangle.face) < held) {
                best = Some((along, triangle.face));
            }
        });
        best
    }

    /// The edge that wins over `face`, the nearest face hit, if any does, with where it comes nearest the ray.
    fn nearest_edge(&self, ray: &Ray, reach: f64, face: Option<(f64, u32)>) -> Option<Near> {
        let mut nearest: BTreeMap<u32, Near> = BTreeMap::new();
        self.segment_tree.visit(ray, reach, |k| {
            let Some(&segment) = self.segments.get(place(k)) else { return };
            let near = approach(ray, self.segment(segment), segment);
            if near.apart <= reach && nearest.get(&segment.0).is_none_or(|held| near.before(held)) {
                nearest.insert(segment.0, near);
            }
        });
        let wins = |near: &&Near| match face {
            None => true,
            Some((along, k)) => near.along <= along + reach || self.borders(near.edge, k, ray.at(along), reach),
        };
        nearest.values().filter(wins).copied().reduce(|best, near| if near.before(&best) { near } else { best })
    }

    /// Whether the edge at `edge` is one of the face at `face`'s own, within `reach` of `hit`, a point on that face.
    fn borders(&self, edge: u32, face: u32, hit: Vec3, reach: f64) -> bool {
        let Some(samples) = self.edges.get(place(edge)) else { return false };
        samples.faces.contains(&face)
            && samples.points.windows(2).any(|pair| matches!(pair, [a, b] if apart(hit, *a, *b) <= reach))
    }

    /// The report for `ray`, which hits nothing within `reach`, with the remedy of aiming at the centre of the
    /// triangle that lies nearest its way, if a ray towards one crosses it.
    fn missed(&self, ray: &Ray, reach: f64) -> Report {
        let [x, y, z] = [ray.origin.x, ray.origin.y, ray.origin.z];
        let report = Report::new(Code::RayMissed, Fault::InvalidInput)
            .with_value(NamedValue::point("from", [x, y, z]))
            .with_value(NamedValue::length("tolerance", reach));
        let Some(centre) = self.aim(ray) else { return report };
        let remedy =
            Remedy::new(RemedyKind::AimAt).with_value(NamedValue::point("point", [centre.x, centre.y, centre.z]));
        report.with_remedy(remedy)
    }

    /// The centre of the triangle whose way from the ray's start is nearest the ray's, of those that the ray from the
    /// start towards their centre crosses, built as `occam pick --towards` builds it, so that following the remedy is
    /// sure to hit: the first such where two tie. A start in a triangle's plane, or at its centre, crosses none.
    fn aim(&self, ray: &Ray) -> Option<Vec3> {
        let mut best: Option<(f64, Vec3)> = None;
        for triangle in &self.triangles {
            let [a, b, c] = triangle.corners;
            let centre = (a + b + c) * (1.0 / 3.0);
            let way = centre - ray.origin;
            let Ok(towards) = Ray::new(coordinates(ray.origin), coordinates(way)) else { continue };
            let crossed = Line::of(&towards).crossing(&towards, &triangle.corners).is_some();
            let cosine = way.dot(ray.direction) / way.norm();
            if crossed && best.is_none_or(|(most, _)| cosine > most) {
                best = Some((cosine, centre));
            }
        }
        best.map(|(_, centre)| centre)
    }
}

/// The line a pick tests triangles against: the ray's start, and the point one millimetre along it, each rounded once,
/// so that every triangle is tested against the same two points.
struct Line {
    start: P3,
    on: P3,
}

/// The coordinates of `v`.
const fn coordinates(v: Vec3) -> P3 {
    [v.x, v.y, v.z]
}

impl Line {
    fn of(ray: &Ray) -> Self {
        Self { start: coordinates(ray.origin), on: coordinates(ray.at(1.0)) }
    }

    /// How far along `ray` it crosses the triangle `corners`, if it does at or after the ray's start: decided exactly,
    /// a side or corner included, and never for a triangle in the line's plane.
    fn crossing(&self, ray: &Ray, corners: &[Vec3; 3]) -> Option<f64> {
        let [a, b, c] = corners.map(coordinates);
        let side = |p: P3, q: P3| orient3d(self.start, self.on, p, q);
        let sides = [side(a, b)?, side(b, c)?, side(c, a)?];
        let above = sides.contains(&occam_math::Side::Above);
        if above == sides.contains(&occam_math::Side::Below) {
            return None;
        }
        let [a, b, c] = *corners;
        let normal = (b - a).cross(c - a);
        let towards = ray.direction.dot(normal);
        let along = (a - ray.origin).dot(normal) / towards;
        (along.is_finite() && along >= 0.0).then_some(along)
    }
}

/// Where an edge comes nearest a ray: the edge's place and that of the segment's first sample, how far apart, how far
/// along the ray, and the point on the edge.
#[derive(Clone, Copy, Debug, PartialEq)]
struct Near {
    edge: u32,
    sample: u32,
    apart: f64,
    along: f64,
    point: Vec3,
}

impl Near {
    /// Whether this is nearer the ray than `other`, then nearer along it, then of the edge with the smaller place, then
    /// of its segment with the smaller place: two segments that tie, meeting at a sample, are told apart by it.
    fn before(&self, other: &Self) -> bool {
        (self.apart, self.along, self.edge, self.sample) < (other.apart, other.along, other.edge, other.sample)
    }
}

/// `s` held to the segment, from 0 to 1, NaN as 0: what a quotient over a segment of no length gives.
fn unit(s: f64) -> f64 {
    lower(1.0, higher(0.0, s))
}

/// Where the segment from `a` to `b`, from sample `sample` of the edge at `edge`, comes nearest the ray, at or after
/// its start. Where the ray runs exactly along the segment, every point of it is as near, and its start's is taken;
/// a segment of no length is its start.
fn approach(ray: &Ray, [a, b]: [Vec3; 2], (edge, sample): (u32, u32)) -> Near {
    let (along_edge, from) = (b - a, ray.origin - a);
    let (square, cosine) = (along_edge.dot(along_edge), along_edge.dot(ray.direction));
    let (ahead, behind) = (along_edge.dot(from), ray.direction.dot(from));
    let gap = square - cosine * cosine;
    let s = if gap > 0.0 { unit((ahead - behind * cosine) / gap) } else { 0.0 };
    let t = higher(0.0, s * cosine - behind);
    let s = unit((ahead + t * cosine) / square);
    let point = a + along_edge * s;
    Near { edge, sample, apart: (ray.at(t) - point).norm(), along: t, point }
}

/// How far `p` is from the segment from `a` to `b`.
fn apart(p: Vec3, a: Vec3, b: Vec3) -> f64 {
    let along = b - a;
    let s = unit((p - a).dot(along) / along.dot(along));
    (p - (a + along * s)).norm()
}

#[cfg(test)]
mod tests;
