//! Source: original
//! A model meshed for display, as the commands give it (docs/architecture/mesh-and-renderer.md): the buffers as the
//! renderer contract wants them, and one answer per face, its place in the buffers or the report refusing it, with the
//! reports refusing edges and what the watertight check found. A session keeps its last mesh, so after a run only the
//! faces the run touched, and those along an edge whose samples changed, are meshed again; the rest are kept.

mod wire;

use std::collections::{BTreeMap, BTreeSet};
use std::ops::Range;

use occam_mesh::Mesh;
use occam_report::{EntityRef, Report};
use occam_topo::{Face, Id, Snapshot};
use occam_units::{Angle, Length};

pub use occam_mesh::{Buffers, FaceRange, Limits};

use crate::PerId;
use crate::describe::{self, Source};

/// One face's place in the buffers.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Placed {
    /// Its vertex block.
    pub vertices: Range<u32>,
    /// Its triangles, whose indices count from the start of its block.
    pub triangles: Range<u32>,
    /// Whether refinement stopped at the most triangles a face may have, short of the deflection.
    pub capped: bool,
    /// Whether its block was kept, bit for bit, from the session's last mesh, rather than meshed again.
    pub kept: bool,
}

/// A model meshed: its buffers, and how each face meshed.
#[derive(Clone, Debug, PartialEq)]
pub struct Meshed {
    /// What it was meshed to.
    pub limits: Limits,
    /// The buffers.
    pub buffers: Buffers,
    /// Each face, in id order, with its place in the buffers or the report refusing it.
    pub faces: Vec<PerId<Placed>>,
    /// The reports refusing edges, then what the watertight check found: none for a mesh that is whole.
    pub reports: Vec<Report>,
}

impl Meshed {
    /// `mesh`, the mesh of `model` made to `limits`, of which `meshed` are the faces meshed by this call, the rest
    /// kept; each report with what the entities it names are.
    pub(crate) fn new(limits: Limits, model: &Snapshot, mesh: &Mesh, meshed: &[Id<Face>]) -> Self {
        let again: BTreeSet<Id<Face>> = meshed.iter().copied().collect();
        let described = |report: &Report| describe::report(report.clone(), &[Source::Model(model)]);
        let mut faces: BTreeMap<EntityRef, Result<Placed, Report>> = BTreeMap::new();
        for block in &mesh.blocks {
            let (vertices, triangles) = (block.vertices.clone(), block.triangles.clone());
            let kept = !again.contains(&block.face);
            faces.insert(block.face.entity(), Ok(Placed { vertices, triangles, capped: block.capped, kept }));
        }
        for report in &mesh.refused {
            if let Some(&face) = report.entities.first() {
                faces.entry(face).or_insert_with(|| Err(described(report)));
            }
        }
        let reports = mesh.edges.refused().chain(&mesh.open).map(described).collect();
        Self { limits, buffers: Buffers::of(mesh), faces: faces.into_iter().collect(), reports }
    }

    /// Whether every face meshed and nothing was found wrong.
    #[must_use]
    pub fn ok(&self) -> bool {
        self.faces.iter().all(|(_, placed)| placed.is_ok()) && self.reports.is_empty()
    }
}

/// The limits of a mesh from its deflection in millimetres and its angle in radians.
///
/// # Errors
///
/// `mesh.limit-not-positive` for a deflection or an angle that is not a finite number greater than zero.
pub(crate) fn limits(deflection: f64, angle: f64) -> Result<Limits, Report> {
    Limits::new(Length::from_millimetres(deflection), Angle::from_radians(angle))
}

#[cfg(test)]
mod tests;
