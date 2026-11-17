//! Source: original
//! What `gates determinism` hashes of the mesher (docs/DETERMINISM.md): the [`Mesh::digest`] of the tests' models
//! meshed whole, edges, faces, merge and watertight check, one with a face that will not mesh, and the check's reports on
//! a mesh cracked by one ulp. occam-mesh's `examples/determinism.rs` prints them.
//!
//! The record is `ci/determinism-mesh.txt`. There is no second mesher to take it from, as the maths record is taken from
//! `tools/determinism/stream.py`, so it is what the x86-64 build prints on one thread: every other build and thread
//! count is held to that one, and the mesher to its answers, but an answer every build gets wrong alike would pass. The
//! mesher has no lane kernel yet, so each build runs it once, as scalar.

use occam_math::{Digest, Exec};
use occam_mesh::{Limits, Mesh, mesh, watertight};
use occam_report::DisplayUnits;
use occam_topo::Snapshot;
use occam_units::{Angle, Length, Point, Vec3};

use super::layouts::{Layout, collapsed_patch, disc_drawn, model, plane_and_cylinder, sphere_half, undrawn};

/// How many operations the probe hashes.
pub const OPERATION_COUNT: usize = 7;

/// The operations, in the order [`hashes`] gives them and the record lists them.
pub const OPERATIONS: [&str; OPERATION_COUNT] = [
    "mesh-plane-and-cylinder",
    "mesh-collapsed-patch",
    "mesh-sphere-half",
    "mesh-disc",
    "mesh-hole",
    "mesh-crack",
    "mesh-many",
];

/// How many times `mesh-many` holds each model: enough faces that every thread of a pool has some.
const COPIES: usize = 3;

/// The limits a deflection of `deflection` and an angle of `angle` give, or `None` if those are refused.
fn limits(deflection: f64, angle: f64) -> Option<Limits> {
    Limits::new(Length::from_millimetres(deflection), Angle::from_radians(angle)).ok()
}

/// The mesh of the model of `layouts` at `deflection`, on `exec`.
fn meshed(layouts: &[Layout], deflection: f64, exec: &Exec) -> Option<(Snapshot, Mesh)> {
    let (snapshot, _) = model(layouts);
    let meshed = mesh(exec, &snapshot, limits(deflection, 0.35)?);
    Some((snapshot, meshed))
}

/// The digest of the mesh of the model of `layouts` at `deflection`, on `exec`.
fn digest(layouts: &[Layout], deflection: f64, exec: &Exec) -> u64 {
    meshed(layouts, deflection, exec).map_or(0, |(_, mesh)| mesh.digest())
}

/// The digest of a plane and a cylinder's mesh with a vertex of the plane's block, in the middle of the edge they
/// share, moved one ulp along `x`, and of each report the watertight check then gives, as its JSON.
fn crack(exec: &Exec) -> u64 {
    let Some((snapshot, mut cracked)) = meshed(&[plane_and_cylinder(2.0)], 0.01, exec) else { return 0 };
    let side = cracked.blocks.first().and_then(|block| block.sides.get(3));
    let at = side.and_then(|side| side.vertices.get(side.vertices.len() / 2)).copied();
    if let Some(p) = at.and_then(|k| cracked.points.get_mut(usize::try_from(k).ok()?)) {
        let v = p.to_vec3();
        *p = Point::from_vec3(Vec3::new(v.x.next_up(), v.y, v.z));
    }
    let mut digest = Digest::EMPTY;
    digest.write_u64(cracked.digest());
    for report in watertight(&snapshot, &cracked) {
        let json = report.to_json(DisplayUnits::default()).unwrap_or_default();
        digest.write_bytes(json.as_bytes());
    }
    digest.finish()
}

/// Every model of the probe, [`COPIES`] times, as one model.
fn many() -> Vec<Layout> {
    (0..COPIES).flat_map(|_| [plane_and_cylinder(2.0), collapsed_patch(), sphere_half(2.0), disc_drawn(1.0)]).collect()
}

/// Every operation's hash on `exec`, in the order of [`OPERATIONS`].
#[must_use]
pub fn hashes(exec: &Exec) -> [u64; OPERATION_COUNT] {
    [
        digest(&[plane_and_cylinder(2.0)], 0.01, exec),
        digest(&[collapsed_patch()], 0.05, exec),
        digest(&[sphere_half(2.0)], 0.05, exec),
        digest(&[disc_drawn(1.0)], 1e-3, exec),
        digest(&[undrawn(plane_and_cylinder(2.0), 0), disc_drawn(1.0)], 0.01, exec),
        crack(exec),
        digest(&many(), 0.1, exec),
    ]
}

#[cfg(test)]
mod tests {
    use super::*;

    /// What the probe must give. The file is the one home of these numbers, and `gates determinism` reads it too.
    const RECORDED: &str = include_str!("../../../../ci/determinism-mesh.txt");

    fn want() -> Vec<u64> {
        let mut lines = RECORDED.lines().filter(|line| !line.starts_with('#'));
        OPERATIONS
            .iter()
            .map(|name| {
                let (operation, hash) = lines.next().unwrap_or("- 0").split_once(' ').unwrap();
                assert_eq!(operation, *name, "the record is in the order of the operations");
                u64::from_str_radix(hash, 16).unwrap()
            })
            .collect()
    }

    #[test]
    fn the_probe_gives_the_recorded_hashes_on_one_thread_and_on_four() {
        assert_eq!(hashes(&Exec::serial()).to_vec(), want());
        #[cfg(not(target_family = "wasm"))]
        assert_eq!(hashes(&Exec::threads(core::num::NonZeroUsize::new(4).unwrap()).unwrap()).to_vec(), want());
    }

    #[test]
    fn every_operation_is_recorded_apart() {
        let mut recorded = want();
        recorded.sort_unstable();
        recorded.dedup();
        assert_eq!(recorded.len(), OPERATION_COUNT, "{RECORDED}");
    }
}
