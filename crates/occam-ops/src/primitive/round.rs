//! Source: original
//! The round primitives: cylinder, cone, sphere and torus. Each periodic surface is split into patches of half a turn,
//! with an edge at each split, so no face crosses its surface's own seam (S1 item 5); the edge on the seam itself has
//! that role. A sphere's poles and a cone's apex are vertices, where a face's loop crosses from one side of its patch
//! to the other along the line its surface collapses on. Every patch's loop runs anticlockwise in its parameters, whose
//! surface normal points out, so each edge's two uses run opposite ways.

use core::f64::consts::{FRAC_PI_2, PI, TAU};
use std::sync::Arc;

use occam_geom::{Cone, Cylinder, Plane, Sphere, Surface, Torus};
use occam_math::{atan2, sin_cos};
use occam_report::Report;
use occam_topo::EdgeRole;
use occam_units::{Angle, Length, Local, Placement, UvPoint, Vec3};

use super::parts::{Held, Parts, turned, uv_circle, uv_held, within};
use super::pose::axes;
use super::solid::{EdgeSpec, FaceSpec, LoopSpec, Solid, UseSpec};

/// The frame in the primitive's coordinates at `origin` with its `z` and `x` along the given unit directions.
fn frame(origin: Vec3, z: Vec3, x: Vec3) -> Placement<Local, Local> {
    axes(origin, z, x)
}

/// The circle's frame at height `z` about the primitive's axis.
fn level(z: f64) -> Placement<Local, Local> {
    frame(Vec3::new(0.0, 0.0, z), Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0))
}

/// The point at `radius` from the axis at angle `angle` and height `z`.
fn rim(radius: f64, angle: f64, z: f64) -> Vec3 {
    let (sin, cos) = sin_cos(angle);
    Vec3::new(radius * cos, radius * sin, z)
}

/// A use with its curve in the face's parameters, holding `fixed` at `at`.
fn held(edge: usize, reversed: bool, fixed: Held, at: f64) -> Result<UseSpec, Report> {
    Ok(UseSpec { edge, reversed, pcurve: uv_held(fixed, at)? })
}

/// One loop with `uses` and the witness `(u, v)`.
fn ring(uses: Vec<UseSpec>, (u, v): (f64, f64)) -> Vec<LoopSpec> {
    vec![LoopSpec { uses, witness: UvPoint::new(u, v) }]
}

/// The flat end of a round solid on `face`, a frame whose `z` points out of the solid, bounded by the two half circles
/// `arcs` of `radius` about the axis at height `z`, each used reversed if `reversed`.
fn cap(
    parts: Parts,
    face: Placement<Local, Local>,
    (z, radius): (f64, f64),
    arcs: [(usize, bool); 2],
) -> Result<FaceSpec, Report> {
    let centre = within(face, Vec3::new(0.0, 0.0, z));
    let (x, y) = (turned(face, Vec3::new(1.0, 0.0, 0.0)), turned(face, Vec3::new(0.0, 1.0, 0.0)));
    let pcurve = uv_circle(centre, x, y, radius)?;
    let uses = arcs.map(|(edge, reversed)| UseSpec { edge, reversed, pcurve: Arc::clone(&pcurve) }).into();
    Ok(FaceSpec { surface: Arc::new(Plane::placed(&face.then(&parts.pose))), loops: ring(uses, (0.0, 0.0)) })
}

/// A solid of revolution about the primitive's `z` axis from a circle of radius `low` at height 0, or the apex if `low`
/// is zero, to a circle of radius `high` at `height`, its side `side`, whose `v` runs `slant` from the low circle to
/// the high one: a cylinder or a cone.
fn frustum(
    parts: Parts,
    (low, high, height): (f64, f64, f64),
    side: Arc<dyn Surface>,
    slant: f64,
) -> Result<Solid, Report> {
    let apex = low <= 0.0;
    let (top0, top1) = (rim(high, 0.0, height), rim(high, PI, height));
    let (low0, low1) = (rim(low, 0.0, 0.0), rim(low, PI, 0.0));
    let mut points = vec![parts.point(top0), parts.point(top1), parts.point(low0)];
    let (seam, line) = (parts.line(low0, top0), parts.line(if apex { low0 } else { low1 }, top1));
    let mut edges = vec![
        EdgeSpec { curve: seam.0, range: seam.1, ends: (2, 0), role: EdgeRole::Seam },
        EdgeSpec { curve: line.0, range: line.1, ends: (if apex { 2 } else { 3 }, 1), role: EdgeRole::Ordinary },
        EdgeSpec {
            curve: parts.arc(level(height), high, (0.0, PI))?,
            range: (0.0, PI),
            ends: (0, 1),
            role: EdgeRole::Ordinary,
        },
        EdgeSpec {
            curve: parts.arc(level(height), high, (PI, TAU))?,
            range: (PI, TAU),
            ends: (1, 0),
            role: EdgeRole::Ordinary,
        },
    ];
    let mut a = vec![held(1, false, Held::U, PI)?, held(2, true, Held::V, slant)?, held(0, true, Held::U, 0.0)?];
    let mut b = vec![held(0, false, Held::U, TAU)?, held(3, true, Held::V, slant)?, held(1, true, Held::U, PI)?];
    let top = frame(Vec3::new(0.0, 0.0, height), Vec3::new(0.0, 0.0, 1.0), Vec3::new(1.0, 0.0, 0.0));
    let mut faces = vec![cap(parts, top, (height, high), [(2, false), (3, false)])?];
    if !apex {
        points.push(parts.point(low1));
        edges.push(EdgeSpec {
            curve: parts.arc(level(0.0), low, (0.0, PI))?,
            range: (0.0, PI),
            ends: (2, 3),
            role: EdgeRole::Ordinary,
        });
        edges.push(EdgeSpec {
            curve: parts.arc(level(0.0), low, (PI, TAU))?,
            range: (PI, TAU),
            ends: (3, 2),
            role: EdgeRole::Ordinary,
        });
        a.insert(0, held(4, false, Held::V, 0.0)?);
        b.insert(0, held(5, false, Held::V, 0.0)?);
        let bottom = frame(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 0.0));
        faces.push(cap(parts, bottom, (0.0, low), [(5, true), (4, true)])?);
    }
    faces.insert(0, FaceSpec { surface: Arc::clone(&side), loops: ring(b, (PI + FRAC_PI_2, slant / 2.0)) });
    faces.insert(0, FaceSpec { surface: side, loops: ring(a, (FRAC_PI_2, slant / 2.0)) });
    Ok(Solid { points, edges, faces })
}

/// A cylinder of `radius` about the frame's `z` axis, from its origin up `height`.
pub(crate) fn cylinder(parts: Parts, radius: f64, height: f64) -> Result<Solid, Report> {
    let side = Arc::new(Cylinder::placed(&parts.pose, Length::from_millimetres(radius))?);
    frustum(parts, (radius, radius, height), side, height)
}

/// A cone about the frame's `z` axis, of radius `low` at its origin and `high` at `height` above it, which differ.
/// The cone is laid out from its narrower end, turned over where that is the top, so its half-angle is between 0 and
/// a right angle.
pub(crate) fn cone(parts: Parts, (low, high): (f64, f64), height: f64) -> Result<Solid, Report> {
    let (narrow, wide, parts) = if low < high {
        (low, high, parts)
    } else {
        let over = frame(Vec3::new(0.0, 0.0, height), Vec3::new(0.0, 0.0, -1.0), Vec3::new(1.0, 0.0, 0.0));
        (high, low, Parts { pose: over.then(&parts.pose) })
    };
    let spread = wide - narrow;
    let half = atan2(spread, height);
    let side = Arc::new(Cone::placed(&parts.pose, Length::from_millimetres(narrow), Angle::from_radians(half))?);
    frustum(parts, (narrow, wide, height), side, (spread * spread + height * height).sqrt())
}

/// A sphere of `radius` about the frame's origin: two patches of half a turn, from pole to pole, meeting along two
/// half circles, the first on the sphere's seam.
pub(crate) fn sphere(parts: Parts, radius: f64) -> Result<Solid, Report> {
    let poles = [Vec3::new(0.0, 0.0, -radius), Vec3::new(0.0, 0.0, radius)];
    let meridian = |x: f64, z: f64| frame(Vec3::new(0.0, 0.0, 0.0), Vec3::new(0.0, z, 0.0), Vec3::new(x, 0.0, 0.0));
    let range = (-FRAC_PI_2, FRAC_PI_2);
    let edges = vec![
        EdgeSpec { curve: parts.arc(meridian(1.0, -1.0), radius, range)?, range, ends: (0, 1), role: EdgeRole::Seam },
        EdgeSpec {
            curve: parts.arc(meridian(-1.0, 1.0), radius, range)?,
            range,
            ends: (0, 1),
            role: EdgeRole::Ordinary,
        },
    ];
    let surface: Arc<dyn Surface> = Arc::new(Sphere::placed(&parts.pose, Length::from_millimetres(radius))?);
    let a = vec![held(1, false, Held::U, PI)?, held(0, true, Held::U, 0.0)?];
    let b = vec![held(0, false, Held::U, TAU)?, held(1, true, Held::U, PI)?];
    let faces = vec![
        FaceSpec { surface: Arc::clone(&surface), loops: ring(a, (FRAC_PI_2, 0.0)) },
        FaceSpec { surface, loops: ring(b, (PI + FRAC_PI_2, 0.0)) },
    ];
    Ok(Solid { points: poles.map(|p| parts.point(p)).into(), edges, faces })
}

/// The range of angles of half-turn `k`: the first or the second.
fn half(k: usize) -> (f64, f64) {
    if k == 0 { (0.0, PI) } else { (PI, TAU) }
}

/// A torus's corners, where `u` and `v` are each 0 or a half turn, as `(u, v)` in half turns.
const CORNERS: [(usize, usize); 4] = [(0, 0), (1, 0), (0, 1), (1, 1)];

/// A torus's half circles round its axis: at `v` of `held` half turns, over half-turn `k` of `u`, from corner to
/// corner.
const ROUND: [(usize, usize, usize, usize); 4] = [(0, 0, 0, 1), (0, 1, 1, 0), (1, 0, 2, 3), (1, 1, 3, 2)];

/// A torus's half circles round its tube: at `u` of `held` half turns, over half-turn `k` of `v`, from corner to
/// corner.
const TUBE: [(usize, usize, usize, usize); 4] = [(0, 0, 0, 2), (0, 1, 2, 0), (1, 0, 1, 3), (1, 1, 3, 1)];

/// A torus's patches, each its uses in turn, anticlockwise in the parameters from its low `u` and `v`: the edge,
/// whether reversed, the parameter it holds and at how many half turns; and its middle in half turns, plus a quarter.
const PATCHES: [[(usize, bool, Held, f64); 4]; 4] = [
    [(0, false, Held::V, 0.0), (6, false, Held::U, 1.0), (2, true, Held::V, 1.0), (4, true, Held::U, 0.0)],
    [(1, false, Held::V, 0.0), (4, false, Held::U, 2.0), (3, true, Held::V, 1.0), (6, true, Held::U, 1.0)],
    [(2, false, Held::V, 1.0), (7, false, Held::U, 1.0), (0, true, Held::V, 2.0), (5, true, Held::U, 0.0)],
    [(3, false, Held::V, 1.0), (5, false, Held::U, 2.0), (1, true, Held::V, 2.0), (7, true, Held::U, 1.0)],
];

/// The middle of each of [`PATCHES`], in half turns.
const MIDDLES: [(f64, f64); 4] = [(0.5, 0.5), (1.5, 0.5), (0.5, 1.5), (1.5, 1.5)];

/// The angle of `turns` half turns.
fn turns(turns: f64) -> f64 {
    turns * PI
}

/// A torus about the frame's `z` axis, its tube of radius `minor` about a circle of radius `major` in the frame's `xy`
/// plane: four patches of half a turn each way. The circles where `u` and where `v` are 0 lie on the torus's seams.
pub(crate) fn torus(parts: Parts, major: f64, minor: f64) -> Result<Solid, Report> {
    let half_turns = |k: usize| if k == 0 { 0.0 } else { PI };
    let at = |(iu, iv): (usize, usize)| {
        let (sv, cv) = sin_cos(half_turns(iv));
        rim(major + minor * cv, half_turns(iu), minor * sv)
    };
    let points = CORNERS.map(|corner| parts.point(at(corner))).into();
    let role = |held: usize| if held == 0 { EdgeRole::Seam } else { EdgeRole::Ordinary };
    let mut edges = Vec::new();
    for (iv, k, from, to) in ROUND {
        let (sv, cv) = sin_cos(half_turns(iv));
        let curve = parts.arc(level(minor * sv), major + minor * cv, half(k))?;
        edges.push(EdgeSpec { curve, range: half(k), ends: (from, to), role: role(iv) });
    }
    for (iu, k, from, to) in TUBE {
        let (su, cu) = sin_cos(half_turns(iu));
        let tube = frame(Vec3::new(major * cu, major * su, 0.0), Vec3::new(su, -cu, 0.0), Vec3::new(cu, su, 0.0));
        edges.push(EdgeSpec {
            curve: parts.arc(tube, minor, half(k))?,
            range: half(k),
            ends: (from, to),
            role: role(iu),
        });
    }
    let (major, minor) = (Length::from_millimetres(major), Length::from_millimetres(minor));
    let surface: Arc<dyn Surface> = Arc::new(Torus::placed(&parts.pose, major, minor)?);
    let mut faces = Vec::new();
    for (patch, (u, v)) in PATCHES.iter().zip(MIDDLES) {
        let uses = patch.iter().map(|&(edge, reversed, fixed, at)| held(edge, reversed, fixed, turns(at)));
        let uses = uses.collect::<Result<Vec<_>, Report>>()?;
        faces.push(FaceSpec { surface: Arc::clone(&surface), loops: ring(uses, (turns(u), turns(v))) });
    }
    Ok(Solid { points, edges, faces })
}

#[cfg(test)]
mod tests {
    use core::f64::consts::FRAC_PI_3;

    use super::*;

    #[test]
    fn a_rim_point_is_its_radius_from_the_axis_at_its_angle_and_height() {
        // A third of a half turn on, whose cosine is a half and sine root 3 over 2, and a half turn beyond, where both
        // turn over: any angle, not only the 0 and the half turn the primitives ask for.
        let (x, y) = (1.25, 2.5 * 0.75_f64.sqrt());
        for (angle, (x, y)) in [(FRAC_PI_3, (x, y)), (PI + FRAC_PI_3, (-x, -y))] {
            let p = rim(2.5, angle, -4.0);
            assert!((p - Vec3::new(x, y, -4.0)).norm() < 1e-14, "at {angle}: {p:?}");
        }
    }
}
