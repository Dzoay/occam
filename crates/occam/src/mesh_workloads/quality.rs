//! Source: original
//! P9's quality table, read off a mesh: how far each triangle strays from its surface over the deflection, measured to
//! the triangle by the mesher's own [`occam_mesh::deviation`]; how far its normal turns from the surface's, at its
//! corners, the middles of its sides and its middle; and its smallest angle. Every figure is per triangle, gathered for
//! the whole mesh and for each family of surface.
#![expect(clippy::arithmetic_side_effects, reason = "counts of triangles, far below usize::MAX")]

use core::f64::consts::PI;
use std::collections::BTreeMap;
use std::sync::Arc;

use occam_geom::Surface;
use occam_math::{Exec, Grain, atan2};
use occam_mesh::{Block, FaceMesh, Mesh, deviation};
use occam_topo::Snapshot;
use occam_units::{Point, UvPoint, Vec3, World};

/// What a mesh, or the part of it on one family of surface, measured.
#[derive(Clone, Debug, Default, PartialEq)]
pub struct Quality {
    /// `all`, or the family of surface, as the model names it.
    pub family: String,
    /// How many faces.
    pub faces: usize,
    /// How many triangles.
    pub triangles: usize,
    /// The furthest any triangle strays from its surface, over the deflection.
    pub deviation: f64,
    /// How many triangles stray further than the deflection.
    pub over_deflection: usize,
    /// The most a triangle's normal turns from its surface's, in radians.
    pub turn: f64,
    /// How many triangles turn further than the angular limit.
    pub over_angle: usize,
    /// Of those, how many are slivers, whose smallest angle is under a degree.
    pub over_angle_slivers: usize,
    /// The area, in square millimetres.
    pub area: f64,
    /// The area of the triangles that turn further than the angular limit.
    pub area_over_angle: f64,
    /// The smallest angle of any triangle, in degrees.
    pub smallest: f64,
    /// The mean of each triangle's smallest angle, in degrees.
    pub mean_smallest: f64,
    /// How many triangles have an angle under 20 degrees.
    pub under_20: usize,
}

/// One triangle's measures.
#[derive(Clone, Copy, Debug)]
struct Triangle {
    deviation: f64,
    turn: f64,
    area: f64,
    smallest: f64,
}

/// The angle between `a` and `b`, in radians: π where either has no length to speak of.
fn between(a: Vec3, b: Vec3) -> f64 {
    let angle = atan2(a.cross(b).norm(), a.dot(b));
    if angle.is_finite() && a.norm() > 0.0 && b.norm() > 0.0 { angle } else { PI }
}

/// The surface's normal at `uv`, out of the face.
fn outward(surface: &dyn Surface, uv: UvPoint, reversed: bool) -> Vec3 {
    let jet = surface.first(uv);
    let normal = jet.du.rate().cross(jet.dv.rate());
    if reversed { normal * -1.0 } else { normal }
}

/// The triangle `corners`, at `uvs` on `surface`, measured.
fn measured(surface: &dyn Surface, reversed: bool, corners: [Point<World>; 3], uvs: [UvPoint; 3]) -> Triangle {
    let [a, b, c] = corners.map(Point::to_vec3);
    let normal = (b - a).cross(c - a);
    let mesh = FaceMesh {
        points: corners.to_vec(),
        normals: Vec::new(),
        uvs: uvs.to_vec(),
        triangles: vec![[0, 1, 2]],
        sides: Vec::new(),
        capped: false,
    };
    let [one, two, three] = uvs;
    let mid = |s: UvPoint, t: UvPoint| UvPoint::new(f64::midpoint(s.u(), t.u()), f64::midpoint(s.v(), t.v()));
    let centre = UvPoint::new((one.u() + two.u() + three.u()) / 3.0, (one.v() + two.v() + three.v()) / 3.0);
    let samples = [one, two, three, mid(one, two), mid(two, three), mid(three, one), centre];
    let turn = samples.iter().map(|&uv| between(normal, outward(surface, uv, reversed))).fold(0.0, f64::max_finite);
    let angles = [between(b - a, c - a), between(c - b, a - b), between(a - c, b - c)];
    let smallest = angles.into_iter().fold(PI, f64::min_finite) * 180.0 / PI;
    Triangle { deviation: deviation(&mesh, surface), turn, area: 0.5 * normal.norm(), smallest }
}

/// The higher and the lower of two numbers, the first where they tie.
trait Finite {
    fn max_finite(self, other: Self) -> Self;
    fn min_finite(self, other: Self) -> Self;
}

impl Finite for f64 {
    fn max_finite(self, other: Self) -> Self {
        if other > self { other } else { self }
    }

    fn min_finite(self, other: Self) -> Self {
        if other < self { other } else { self }
    }
}

/// Each triangle of `block`, measured, with its face's surface's family.
fn face(model: &Snapshot, mesh: &Mesh, block: &Block) -> (String, Vec<Triangle>) {
    let Some(view) = model.view(block.face) else { return (String::new(), Vec::new()) };
    let (Some(surface), reversed) = (view.surface(), view.reversed()) else { return (String::new(), Vec::new()) };
    let corner = |k: u32| {
        let k = usize::try_from(k).ok()?;
        Some((*mesh.points.get(k)?, *mesh.uvs.get(k)?))
    };
    let span = usize::try_from(block.triangles.start).unwrap_or(0)..usize::try_from(block.triangles.end).unwrap_or(0);
    let triangles = mesh.triangles.get(span).unwrap_or_default().iter().filter_map(|t| {
        let [Some(a), Some(b), Some(c)] = t.map(corner) else { return None };
        Some(measured(surface, reversed, [a.0, b.0, c.0], [a.1, b.1, c.1]))
    });
    (surface.family().0.to_string(), triangles.collect())
}

/// Adds `triangles`, of one face, to `quality`, against `deflection` and `angle`.
fn gather(quality: &mut Quality, triangles: &[Triangle], (deflection, angle): (f64, f64)) {
    quality.faces += 1;
    for t in triangles {
        let (ratio, over) = (t.deviation / deflection, t.turn > angle);
        quality.triangles += 1;
        quality.deviation = quality.deviation.max_finite(ratio);
        quality.over_deflection += usize::from(ratio > 1.0);
        quality.turn = quality.turn.max_finite(t.turn);
        quality.over_angle += usize::from(over);
        quality.over_angle_slivers += usize::from(over && t.smallest < 1.0);
        quality.area += t.area;
        quality.area_over_angle += if over { t.area } else { 0.0 };
        quality.smallest = quality.smallest.min_finite(t.smallest);
        quality.mean_smallest += t.smallest;
        quality.under_20 += usize::from(t.smallest < 20.0);
    }
}

/// P9's quality table for `mesh`, the mesh of `model`, measured on `exec`, a face a task: the whole mesh first, then
/// each family of surface in the order of its name.
#[must_use]
pub fn quality(exec: &Exec, model: &Arc<Snapshot>, mesh: &Arc<Mesh>) -> Vec<Quality> {
    let limits = mesh.edges.limits();
    let against = (limits.deflection().millimetres(), limits.angle().radians());
    let (shared, meshed) = (Arc::clone(model), Arc::clone(mesh));
    let faces = exec.map(mesh.blocks.len(), Grain::Each, move |k| {
        meshed.blocks.get(k).map(|block| face(&shared, &meshed, block)).unwrap_or_default()
    });
    let fresh = |family: &str| Quality { family: family.to_string(), smallest: 180.0, ..Quality::default() };
    let mut all = fresh("all");
    let mut families: BTreeMap<String, Quality> = BTreeMap::new();
    for (family, triangles) in &faces {
        gather(&mut all, triangles, against);
        gather(families.entry(family.clone()).or_insert_with(|| fresh(family)), triangles, against);
    }
    let mut table: Vec<Quality> = core::iter::once(all).chain(families.into_values()).collect();
    for row in &mut table {
        let count = f64::from(u32::try_from(row.triangles).unwrap_or(u32::MAX));
        row.mean_smallest = if row.triangles > 0 { row.mean_smallest / count } else { 0.0 };
    }
    table
}

#[cfg(test)]
mod tests {
    use occam_geom::Plane;
    use occam_mesh::Limits;
    use occam_units::{Angle, Length, Placement};

    use super::*;

    fn corner(x: f64, y: f64) -> (Point<World>, UvPoint) {
        (Point::from_vec3(Vec3::new(x, y, 0.0)), UvPoint::new(x, y))
    }

    #[test]
    fn a_triangle_on_its_plane_strays_nowhere_and_turns_nowhere_and_one_on_the_far_side_turns_round() {
        let plane = Plane::placed(&Placement::identity());
        let [a, b, c] = [corner(0.0, 0.0), corner(2.0, 0.0), corner(0.0, 2.0)];
        let flat = measured(&plane, false, [a.0, b.0, c.0], [a.1, b.1, c.1]);
        assert_eq!((flat.deviation, flat.turn, flat.area), (0.0, 0.0, 2.0));
        assert!((flat.smallest - 45.0).abs() < 1e-12, "{}", flat.smallest);
        assert_eq!(measured(&plane, true, [a.0, b.0, c.0], [a.1, b.1, c.1]).turn, PI);
        let sliver = measured(&plane, false, [a.0, b.0, b.0], [a.1, b.1, b.1]);
        assert_eq!((sliver.turn, sliver.area, sliver.smallest), (PI, 0.0, 0.0));
    }

    #[test]
    fn the_angle_between_two_directions_is_pi_where_either_has_no_length() {
        let (x, y) = (Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 3.0, 0.0));
        assert!((between(x, y) - PI / 2.0).abs() < 1e-15);
        assert_eq!(between(x, x * 2.0), 0.0);
        assert_eq!(between(x, Vec3::new(0.0, 0.0, 0.0)), PI);
    }

    #[test]
    fn a_meshed_box_is_all_planes_within_its_deflection_its_area_its_six_sides() {
        let ran = crate::run(
            None,
            r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [{"op": "box", "size": [1, 2, 3]}]}"#,
        );
        let model = Arc::new(ran.model.unwrap());
        let limits = Limits::new(Length::from_millimetres(0.1), Angle::from_radians(0.35)).unwrap();
        let mesh = Arc::new(occam_mesh::mesh(&Exec::serial(), &model, limits));
        let table = quality(&Exec::serial(), &model, &mesh);
        let families: Vec<&str> = table.iter().map(|row| row.family.as_str()).collect();
        assert_eq!(families, ["all", "plane"]);
        let all = &table[0];
        assert_eq!((all.faces, all.triangles), (6, mesh.triangles.len()));
        assert!((all.area - 22.0).abs() < 1e-12, "{}", all.area);
        assert!(all.deviation < 1e-12 && all.turn < 1e-12, "{all:?}");
        assert_eq!((all.over_deflection, all.over_angle, all.over_angle_slivers), (0, 0, 0));
        assert!(all.smallest > 0.0 && all.smallest <= all.mean_smallest && all.mean_smallest <= 60.0, "{all:?}");
        assert_eq!(table[1], Quality { family: "plane".to_string(), ..all.clone() });
    }
}
