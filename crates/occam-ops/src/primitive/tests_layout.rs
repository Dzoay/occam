//! Source: original
//! The primitives' layouts, read back from the model at a frame off the world's axes and at sizes away from 1: no face
//! is reversed, so each lies on its surface facing out; a flat face's plane runs from the face's centre along its
//! first side; a round face's witness is the middle of its patch; and a torus's seams are its circles where `u` and
//! where `v` are 0.

use occam_math::Exec;
use occam_topo::{Edge, EdgeRole, EdgeUse, Face, Id, Snapshot, Vertex};
use occam_units::{UvPoint, Vec3};

use super::{Pose, PrimitiveArgs, Shape, make};
use crate::request::default_tolerance;

/// A frame off the world's axes, its `z` and `x` unit and square to each other.
fn pose() -> Pose {
    let z = Vec3::new(2.0, -1.0, 2.0) * (1.0 / 3.0);
    let x = Vec3::new(1.0, 2.0, 0.0) * (1.0 / 5.0_f64.sqrt());
    Pose { origin: Vec3::new(12.5, -3.0, 40.0), z, x }
}

/// Where the frame puts `(x, y, z)` of the primitive's own coordinates.
fn placed((x, y, z): (f64, f64, f64)) -> Vec3 {
    let pose = pose();
    pose.origin + pose.x * x + pose.z.cross(pose.x) * y + pose.z * z
}

/// `shape` made at the frame and committed.
fn built(shape: Shape) -> Snapshot {
    let mut edit = Snapshot::empty(default_tolerance().unwrap()).edit();
    make(&mut edit, &PrimitiveArgs { shape, pose: pose() }).unwrap();
    edit.commit(&Exec::serial()).unwrap().0
}

/// Where vertex `v` of `s` is.
fn at(s: &Snapshot, v: Id<Vertex>) -> Vec3 {
    s.view(v).unwrap().point().to_vec3()
}

/// Where use `u` of `s` runs from and to.
fn ends(s: &Snapshot, u: Id<EdgeUse>) -> (Vec3, Vec3) {
    let (from, to) = s.view(u).unwrap().vertices().unwrap();
    (at(s, from), at(s, to))
}

/// The middle of `points`.
fn mean(points: &[Vec3]) -> Vec3 {
    let share = 1.0 / f64::from(u32::try_from(points.len()).unwrap());
    points.iter().fold(Vec3::new(0.0, 0.0, 0.0), |sum, &p| sum + p * share)
}

/// The flat primitives: a box with sides exactly 1 long, as long as the unit direction along each, and wedges with
/// sides 1 long and a top that slants, so that faces' first sides run every way from corners away from the origin.
const FLAT: [Shape; 3] = [
    Shape::Box([1.0, 2.5, 4.0]),
    Shape::Wedge { size: [3.0, 1.0, 2.0], start: 0.5, width: 1.0 },
    Shape::Wedge { size: [3.0, 1.0, 2.0], start: -0.75, width: 0.0 },
];

/// The round primitives, each cone both ways up and coming to a point at either end.
const ROUND: [Shape; 7] = [
    Shape::Cylinder { radius: 2.5, height: 7.0 },
    Shape::Cone { radius1: 1.5, radius2: 4.0, height: 3.0 },
    Shape::Cone { radius1: 4.0, radius2: 1.5, height: 3.0 },
    Shape::Cone { radius1: 0.0, radius2: 2.5, height: 6.0 },
    Shape::Cone { radius1: 2.5, radius2: 0.0, height: 6.0 },
    Shape::Sphere { radius: 3.5 },
    Shape::Torus { major: 7.5, minor: 2.25 },
];

#[test]
fn no_face_of_any_primitive_is_reversed_so_each_surface_faces_out_of_its_solid() {
    // `every_face_of_every_primitive_faces_out` shows each face's outward normal points out; this, that it is the
    // surface's own, as each is laid out: a flat face's plane, a cap's, a side's, a sphere's and a torus's.
    for shape in FLAT.into_iter().chain(ROUND) {
        let s = built(shape);
        let reversed: Vec<Id<Face>> = s.ids::<Face>().into_iter().filter(|&f| s.view(f).unwrap().reversed()).collect();
        assert_eq!(reversed, [], "{shape:?}");
    }
}

#[test]
fn a_flat_face_lies_on_a_plane_from_its_centre_along_its_first_side_facing_out() {
    for shape in FLAT {
        let s = built(shape);
        let inside = mean(&s.ids::<Vertex>().into_iter().map(|v| at(&s, v)).collect::<Vec<_>>());
        for f in s.ids::<Face>() {
            let face = s.view(f).unwrap();
            let uses = s.view(face.outer()).unwrap().uses();
            let centre = mean(&uses.iter().map(|&u| ends(&s, u).0).collect::<Vec<_>>());
            let (from, to) = ends(&s, uses[0]);
            let side = (to - from) * (1.0 / (to - from).norm());
            let jet = face.surface().unwrap().first(UvPoint::new(0.0, 0.0));
            assert!((jet.point.to_vec3() - centre).norm() < 1e-12, "{shape:?}: {:?} is not {centre:?}", jet.point);
            assert!((jet.du.rate() - side).norm() < 1e-12, "{shape:?}: u runs along {:?}, not {side:?}", jet.du);
            let normal = jet.du.rate().cross(jet.dv.rate());
            assert!((centre - inside).dot(normal) > 0.0, "{shape:?}: at {centre:?} the plane faces in");
        }
    }
}

#[test]
fn a_round_face_s_witness_is_the_middle_of_its_patch() {
    // A side's two patches run a half turn each from `u` = 0 and a half turn, and its witnesses are a quarter turn on,
    // halfway along the side; a cap's is its centre. A sphere's patches' are on its equator, a torus's halfway round
    // the tube too, above and below.
    let cases = [
        (ROUND[0], vec![(0.0, 2.5, 3.5), (0.0, -2.5, 3.5), (0.0, 0.0, 7.0), (0.0, 0.0, 0.0)]),
        (ROUND[1], vec![(0.0, 2.75, 1.5), (0.0, -2.75, 1.5), (0.0, 0.0, 3.0), (0.0, 0.0, 0.0)]),
        (ROUND[4], vec![(0.0, 1.25, 3.0), (0.0, -1.25, 3.0), (0.0, 0.0, 0.0)]),
        (ROUND[5], vec![(0.0, 3.5, 0.0), (0.0, -3.5, 0.0)]),
        (ROUND[6], vec![(0.0, 7.5, 2.25), (0.0, -7.5, 2.25), (0.0, 7.5, -2.25), (0.0, -7.5, -2.25)]),
    ];
    for (shape, middles) in cases {
        let s = built(shape);
        let mut wanted: Vec<Vec3> = middles.into_iter().map(placed).collect();
        for f in s.ids::<Face>() {
            let face = s.view(f).unwrap();
            let p = face.surface().unwrap().point(s.view(face.outer()).unwrap().witness()).to_vec3();
            let found = wanted.iter().position(|&w| (w - p).norm() < 1e-12);
            let k = found.unwrap_or_else(|| panic!("{shape:?}: a witness at {p:?}, none of {wanted:?}"));
            wanted.swap_remove(k);
        }
        assert_eq!(wanted, [], "{shape:?}: no witness at these");
    }
}

#[test]
fn a_torus_s_seams_are_its_circles_where_u_and_where_v_are_0() {
    // Each edge is a half circle, named by its middle: round the axis outermost, where `v` is 0, and innermost, where
    // it is a half turn; round the tube on the frame's `x`, where `u` is 0, and opposite, where it is a half turn.
    let s = built(ROUND[6]);
    let middles: Vec<(EdgeRole, Vec3)> = s
        .ids::<Edge>()
        .into_iter()
        .map(|e| {
            let edge = s.view(e).unwrap();
            let (lo, hi) = edge.range();
            (edge.role(), edge.curve().unwrap().point(f64::midpoint(lo, hi)).to_vec3())
        })
        .collect();
    let (major, minor, outer, inner) = (7.5, 2.25, 9.75, 5.25);
    let seams = [(0.0, outer, 0.0), (0.0, -outer, 0.0), (major, 0.0, minor), (major, 0.0, -minor)];
    let ordinary = [(0.0, inner, 0.0), (0.0, -inner, 0.0), (-major, 0.0, minor), (-major, 0.0, -minor)];
    let each = seams.map(|p| (EdgeRole::Seam, p)).into_iter().chain(ordinary.map(|p| (EdgeRole::Ordinary, p)));
    for (role, middle) in each {
        let p = placed(middle);
        let roles: Vec<EdgeRole> = middles.iter().filter(|(_, m)| (*m - p).norm() < 1e-12).map(|&(r, _)| r).collect();
        assert_eq!(roles, [role], "the edge through {p:?}");
    }
}
