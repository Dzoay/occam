//! Source: original
//! What `gates perf` counts and `examples/scale.rs` times of the topology: plates of unit squares built through
//! [`Edit`], and edits that split their top squares by a diagonal into two triangles, each a face of the plate's shell.
//! Built only for the tests and under the `workloads` feature, which occam's benchmarks and the scale example turn on;
//! nothing that ships does. The plate is genus 0 and all squares, where S3-P3's synthetic solids were genus 1 and
//! mostly quads.

use occam_math::Exec;
use occam_units::{Angle, Length, Tolerance};

use crate::edit::Edit;
use crate::models::{Built, build, plate, split, top_square};
use crate::snapshot::Snapshot;

/// A plate `k` by `k` by 1 of unit squares, committed: its bottom and its top each `k²` squares, and each side `k`, so
/// `2k² + 4k` faces.
pub struct Plate {
    snapshot: Snapshot,
    built: Built,
    across: usize,
}

impl Plate {
    /// The plate `across` squares across, built in one edit and committed on `exec`: `None` only if the commit refuses
    /// it, which no clean plate makes it do.
    #[must_use]
    pub fn new(across: usize, exec: &Exec) -> Option<Self> {
        let tolerance = Tolerance::new(Length::from_millimetres(1e-7), Angle::from_radians(1e-9)).ok()?;
        let mut edit = Snapshot::empty(tolerance).edit();
        let built = build(&mut edit, &plate(across));
        let (snapshot, _) = edit.commit(exec).ok()?;
        Some(Self { snapshot, built, across })
    }

    /// The plate as committed.
    #[must_use]
    pub const fn snapshot(&self) -> &Snapshot {
        &self.snapshot
    }

    /// How many squares its top has across, and down.
    #[must_use]
    pub const fn across(&self) -> usize {
        self.across
    }

    /// An edit of `base` splitting each of the top squares `(column, row)` by its diagonal into two triangles. `base` is
    /// the plate or a snapshot after it in which those squares are still whole; a square split twice, or one outside
    /// the plate, makes the workload panic.
    #[must_use]
    pub fn split(&self, base: &Snapshot, squares: &[(usize, usize)]) -> Edit {
        let mut edit = base.edit();
        for &square in squares {
            split(&mut edit, top_square(&self.built, self.across, square));
        }
        edit
    }
}

#[cfg(test)]
mod tests {
    use super::*;
    use crate::kind::Face;

    #[test]
    fn a_plate_has_two_k_squared_and_four_k_faces_and_splitting_squares_adds_one_face_each() {
        let plate = Plate::new(4, &Exec::serial()).unwrap();
        assert_eq!((plate.across(), plate.snapshot().ids::<Face>().len()), (4, 2 * 16 + 16));
        let edit = plate.split(plate.snapshot(), &[(0, 0), (3, 3), (1, 2)]);
        let (after, changes) = edit.commit(&Exec::serial()).unwrap();
        assert_eq!(after.ids::<Face>().len(), 48 + 3);
        assert_eq!(changes.created.len(), 3 * 6, "a face, its loop, an edge, its two uses and its line, each time");
        let again = plate.split(&after, &[(2, 0)]).commit(&Exec::serial()).unwrap().0;
        assert_eq!(again.ids::<Face>().len(), 52);
    }
}
