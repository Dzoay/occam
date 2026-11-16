//! Source: original
//! A pick, as the commands give it (docs/decisions/2026-11-13-picking-in-the-first-release.md): a ray from a point
//! along a direction, in millimetres in the model's frame, tested against the model's CPU mesh made to the limits
//! asked, edges within a tolerance. The answer is the model's own id of the face or edge hit first, the point hit and
//! how far along the ray it is; or, for a ray that hits nothing, `mesh.ray-missed` with a point to aim at. Its one
//! JSON form names the model's digest, so the page posts what it picked with the model it picked it on. A session
//! keeps the mesh and its box trees between picks at the same limits, as a mesh worker picks again and again.

mod wire;

use occam_mesh::{Mesh, Picker, Ray};
use occam_report::{Code, EntityRef, Fault, Input, NamedValue, Report};
use occam_topo::Snapshot;
use occam_units::Length;

use crate::describe::{self, Source};
use crate::mesh::Limits;

/// What a pick asks: a ray from `from` along `direction`, in millimetres in the model's frame, with edges hit within
/// `tolerance` of it, in millimetres.
#[derive(Clone, Copy, Debug, PartialEq)]
pub struct Pick {
    /// Where the ray starts.
    pub from: [f64; 3],
    /// Its direction, of any length but zero.
    pub direction: [f64; 3],
    /// How near an edge must come to the ray to be hit, at least zero.
    pub tolerance: f64,
}

/// What a ray hit first: a face or an edge of the model, where, and how far along the ray.
#[derive(Clone, Debug, PartialEq)]
pub struct Hit {
    /// The face or edge.
    pub entity: EntityRef,
    /// The point hit, in millimetres: where the ray crosses a face, or an edge's point nearest the ray.
    pub point: [f64; 3],
    /// How far along the ray it is, in millimetres.
    pub distance: f64,
}

/// What a pick answered.
#[derive(Clone, Debug, PartialEq)]
pub struct Picked {
    /// The digest of the model picked.
    pub digest: Option<u64>,
    /// The ray, its direction of length one, and the tolerance.
    pub asked: Pick,
    /// What it hit first, or `mesh.ray-missed`.
    pub hit: Result<Hit, Report>,
    /// The mesh's reports: what refused faces or edges, which no ray can hit, and what the watertight check found.
    pub reports: Vec<Report>,
}

impl Picked {
    /// Whether the ray hit something.
    #[must_use]
    pub const fn ok(&self) -> bool {
        self.hit.is_ok()
    }
}

/// A model's mesh at some limits, made ready to pick, with the model's digest.
#[derive(Clone, Debug)]
pub(crate) struct Picking {
    pub limits: Limits,
    pub digest: Option<u64>,
    pub mesh: Mesh,
    pub picker: Picker,
}

impl Picking {
    /// `mesh`, made to `limits` of the model whose digest is `digest`, made ready to pick.
    pub(crate) fn new(limits: Limits, digest: Option<u64>, mesh: Mesh) -> Self {
        let picker = Picker::new(&mesh);
        Self { limits, digest, mesh, picker }
    }

    /// What `asked` hits in the mesh of `model`.
    pub(crate) fn pick(&self, model: &Snapshot, asked: Pick) -> Result<Picked, Report> {
        let ray = Ray::new(asked.from, asked.direction)?;
        let tolerance = tolerance(asked.tolerance)?;
        let described = |report: &Report| describe::report(report.clone(), &[Source::Model(model)]);
        let found = self.picker.pick(&ray, tolerance).map_err(|report| described(&report));
        let hit = found.map(|hit| {
            let p = hit.point.to_vec3();
            Hit { entity: hit.entity, point: [p.x, p.y, p.z], distance: hit.distance.millimetres() }
        });
        let reports = self.mesh.refused.iter().chain(self.mesh.edges.refused()).chain(&self.mesh.open);
        let way = ray.direction;
        let asked = Pick { direction: [way.x, way.y, way.z], ..asked };
        Ok(Picked { digest: self.digest, asked, hit, reports: reports.map(described).collect() })
    }

    /// The samples of `edge`, if the mesh sampled it: what the page draws a selected or highlighted edge through.
    pub(crate) fn samples(&self, edge: EntityRef) -> Option<Vec<[f64; 3]>> {
        let sampled = self.mesh.edges.iter().find_map(|(id, sampled)| (id.entity() == edge).then_some(sampled))?;
        let points = sampled.ok()?.points.iter().map(|p| p.to_vec3()).map(|p| [p.x, p.y, p.z]);
        Some(points.collect())
    }
}

/// `millimetres` as a pick's tolerance.
///
/// # Errors
///
/// `units.not-finite` for a number that is not finite, and `input.negative` for one below zero.
fn tolerance(millimetres: f64) -> Result<Length, Report> {
    let at = Input::at("tolerance");
    if !millimetres.is_finite() {
        let report = Report::new(Code::NotFinite, Fault::InvalidInput).with_input(at);
        return Err(report.with_value(NamedValue::length("value", millimetres)));
    }
    if millimetres < 0.0 {
        let report = Report::new(Code::InputNegative, Fault::InvalidInput).with_input(at);
        return Err(report.with_value(NamedValue::length("value", millimetres)));
    }
    Ok(Length::from_millimetres(millimetres))
}

#[cfg(test)]
mod tests;
