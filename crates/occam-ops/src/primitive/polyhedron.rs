//! Source: original
//! The flat primitives: a box, and a wedge, a box whose top is narrowed to a strip, or to an edge. Each is a polyhedron
//! given by its corners and its faces, each face its corners in turn, anticlockwise seen from outside, so each face's
//! normal, by Newell's sum of cross products, points out. Each face lies on its own plane through its centre, which is
//! inside it, as every face here is convex, so the centre is its loop's witness.

use std::collections::BTreeMap;
use std::sync::Arc;

use occam_geom::Plane;
use occam_report::Report;
use occam_topo::EdgeRole;
use occam_units::{Local, Placement, UvPoint, Vec3};

use super::parts::{Parts, uv_line, within};
use super::pose::axes;
use super::solid::{EdgeSpec, FaceSpec, LoopSpec, Solid, UseSpec};

/// The faces of a box, `0` to `3` its bottom anticlockwise from its origin seen from above and `4` to `7` its top above
/// them.
const BOX: [&[usize]; 6] = [&[0, 3, 2, 1], &[4, 5, 6, 7], &[0, 1, 5, 4], &[2, 3, 7, 6], &[0, 4, 7, 3], &[1, 2, 6, 5]];

/// The faces of a wedge whose top is an edge, `4` from the top's start at the front and `5` at the back.
const SHARP: [&[usize]; 5] = [&[0, 3, 2, 1], &[0, 1, 4], &[2, 3, 5], &[0, 4, 5, 3], &[1, 2, 5, 4]];

/// A box `size` long along the frame's `x`, `y` and `z`, from its origin.
pub(crate) fn cuboid(parts: Parts, [dx, dy, dz]: [f64; 3]) -> Result<Solid, Report> {
    wedge(parts, [dx, dy, dz], (0.0, dx))
}

/// A wedge: a base `dx` by `dy` from the frame's origin, and a top `dz` above it whose strip runs along `x` from
/// `start` for `width`, the whole depth `dy`; a box where the strip is the base's, and a prism where its width is zero.
pub(crate) fn wedge(parts: Parts, [dx, dy, dz]: [f64; 3], (start, width): (f64, f64)) -> Result<Solid, Report> {
    let mut corners = vec![
        Vec3::new(0.0, 0.0, 0.0),
        Vec3::new(dx, 0.0, 0.0),
        Vec3::new(dx, dy, 0.0),
        Vec3::new(0.0, dy, 0.0),
        Vec3::new(start, 0.0, dz),
    ];
    let end = start + width;
    if width > 0.0 {
        corners.extend([Vec3::new(end, 0.0, dz), Vec3::new(end, dy, dz), Vec3::new(start, dy, dz)]);
        polyhedron(parts, &corners, &BOX)
    } else {
        corners.push(Vec3::new(start, dy, dz));
        polyhedron(parts, &corners, &SHARP)
    }
}

/// Each corner of `ring` with the one after it, round to the first.
fn sides(ring: &[usize]) -> impl Iterator<Item = (usize, usize)> + '_ {
    ring.iter().copied().zip(ring.iter().copied().cycle().skip(1))
}

/// The plane of the face whose corners are `ring` of `corners`, as a frame in the primitive's coordinates: its origin
/// at their centre, its `z` along their outward normal and its `x` along their first side.
fn face_frame(corners: &[Vec3], ring: &[usize]) -> Placement<Local, Local> {
    let count = f64::from(u32::try_from(ring.len()).unwrap_or(1));
    let (mut normal, mut centre) = (Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, 0.0));
    for (a, b) in sides(ring) {
        normal = normal + corner(corners, a).cross(corner(corners, b));
        centre = centre + corner(corners, a) * (1.0 / count);
    }
    let side = sides(ring).next().map_or(Vec3::new(1.0, 0.0, 0.0), |(a, b)| corner(corners, b) - corner(corners, a));
    axes(centre, normal * (1.0 / normal.norm()), side * (1.0 / side.norm()))
}

/// The polyhedron with `corners` and `faces`, each edge made where two faces first meet it, along the first face's
/// turn.
fn polyhedron(parts: Parts, corners: &[Vec3], faces: &[&[usize]]) -> Result<Solid, Report> {
    let mut edges: Vec<EdgeSpec> = Vec::new();
    let mut made: BTreeMap<(usize, usize), usize> = BTreeMap::new();
    let mut specs = Vec::new();
    for &ring in faces {
        let frame = face_frame(corners, ring);
        let uv = |i: usize| within(frame, corner(corners, i));
        let mut uses = Vec::new();
        for (a, b) in sides(ring) {
            let (edge, reversed, (from, to)) = if let Some(&edge) = made.get(&(b, a)) {
                (edge, true, (b, a))
            } else {
                let edge = edges.len();
                made.insert((a, b), edge);
                let (curve, range) = parts.line(corner(corners, a), corner(corners, b));
                edges.push(EdgeSpec { curve, range, ends: (a, b), role: EdgeRole::Ordinary });
                (edge, false, (a, b))
            };
            let (p, q) = (uv(from), uv(to));
            uses.push(UseSpec { edge, reversed, pcurve: uv_line((p.x, p.y), (q.x - p.x, q.y - p.y))? });
        }
        let surface = Arc::new(Plane::placed(&frame.then(&parts.pose)));
        specs.push(FaceSpec { surface, loops: vec![LoopSpec { uses, witness: UvPoint::new(0.0, 0.0) }] });
    }
    let points = corners.iter().map(|&c| parts.point(c)).collect();
    Ok(Solid { points, edges, faces: specs })
}

/// Corner `i`, which the faces' own indices always name.
fn corner(corners: &[Vec3], i: usize) -> Vec3 {
    corners.get(i).copied().unwrap_or(Vec3::new(0.0, 0.0, 0.0))
}
