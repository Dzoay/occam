//! Source: original
//! P9's synthetic models on the kernel, for the mesher's timings and quality (the `view` milestone's goals): the same
//! parts P9's generator drew from the same seed, a deterministic mix of plates with holes, solids of revolution, B-spline
//! panels with a hole and quilts of Bézier patches on tori, each closed. Plates and solids of revolution are made by a
//! request, as an agent would make them; panels and quilts, which no request makes yet, are written through `Edit`.
//!
//! The kernel lays a part out as it lays out any other, where P9 laid out its own: a periodic surface is cut into
//! patches, so that no face crosses its own seam, so the same parts have more faces than P9 counted. The parts come in
//! P9's order, each step committed and so checked.

mod parts;
mod quality;
mod request;
mod spline;
mod splitmix;
mod written;

use occam_math::{Digest, Exec};
use occam_mesh::{Limits, Mesh, Picker, Ray};
use occam_report::Report;
use occam_topo::{Face, Snapshot};
use occam_units::{Angle, Length, Tolerance};

pub use parts::P9_SEED;
pub use quality::{Quality, quality};

use parts::{Part, Shape, parts};

/// The faces P9 counted in its models, by the target each was drawn for: 1,002 for 1,000 and 10,001 for 10,000.
pub const P9_TARGETS: [usize; 2] = [1000, 10_000];

/// The tolerance every part is built to: a request's own.
fn tolerance() -> Result<Tolerance, Report> {
    Tolerance::new(Length::from_millimetres(1e-7), Angle::from_radians(1e-9))
}

/// Runs the steps gathered so far on `model`, if there are any.
fn flush(model: Snapshot, steps: &mut Vec<serde_json::Value>) -> Result<Snapshot, Vec<Report>> {
    if steps.is_empty() {
        return Ok(model);
    }
    let ran = crate::run(Some(&model), &request::request(steps));
    steps.clear();
    if let Some(failed) = ran.failed {
        return Err(failed.reports);
    }
    ran.model.ok_or_else(Vec::new)
}

/// Writes `parts`, each a panel or a quilt, in one edit of `model`, and commits it.
fn written(exec: &Exec, model: &Snapshot, parts: &[&Part]) -> Result<Snapshot, Vec<Report>> {
    let mut edit = model.edit();
    for part in parts {
        let solid = match part.shape {
            Shape::Panel { size, phase } => spline::panel(&part.frame, size, phase),
            Shape::Quilt { across, radius, degree, seed } => spline::quilt(&part.frame, across, radius, degree, seed),
            _ => continue,
        };
        written::write(&mut edit, &solid.map_err(|report| vec![report])?).map_err(|report| vec![report])?;
    }
    edit.commit(exec).map(|(snapshot, _)| snapshot)
}

/// The model of P9's parts for `target` faces from `seed`: [`P9_TARGETS`] and [`P9_SEED`] give P9's two models. Runs
/// of plates and solids of revolution go in one request each, and runs of panels and quilts in one edit each, so the
/// parts keep P9's order. The edits are committed, and so checked, on `exec`.
///
/// # Errors
///
/// What a request or a commit refuses, which no part of P9's makes either do.
pub fn synthetic(exec: &Exec, target: usize, seed: u64) -> Result<Snapshot, Vec<Report>> {
    let mut model = Snapshot::empty(tolerance().map_err(|report| vec![report])?);
    let (mut steps, mut spline) = (Vec::new(), Vec::new());
    let drawn = parts(target, seed);
    for (k, part) in drawn.iter().enumerate() {
        let made = request::steps(&part.frame, &format!("part{k}"), &part.shape);
        if made.is_empty() {
            model = flush(model, &mut steps)?;
            spline.push(part);
        } else {
            if !spline.is_empty() {
                model = written(exec, &model, &core::mem::take(&mut spline))?;
            }
            steps.extend(made);
        }
    }
    model = flush(model, &mut steps)?;
    if spline.is_empty() { Ok(model) } else { written(exec, &model, &spline) }
}

/// The target of the model `gates perf` meshes: P9's first parts, two plates and a panel between them.
pub const PERF_TARGET: usize = 20;

/// `model` meshed on `exec` as `gates perf` meshes it, natively and in the browser: to P9's angle and a tenth of a
/// millimetre, the page's own deflection.
///
/// # Errors
///
/// None: the limits are valid. `Limits::new`'s report, were they not.
pub fn perf_mesh(exec: &Exec, model: &Snapshot) -> Result<Mesh, Report> {
    let limits = Limits::new(Length::from_millimetres(0.1), Angle::from_radians(0.35))?;
    Ok(occam_mesh::mesh(exec, model, limits))
}

/// A ray onto every sixteenth triangle of `mesh`, down its normal from a millimetre off its middle: what `gates perf`
/// picks.
#[must_use]
pub fn pick_rays(mesh: &Mesh) -> Vec<Ray> {
    let corner = |k: u32| usize::try_from(k).ok().and_then(|k| mesh.points.get(k)).map(|p| p.to_vec3());
    let ray = |triangle: &[u32; 3]| {
        let [a, b, c] = triangle.map(corner);
        let (a, b, c) = (a?, b?, c?);
        let normal = (b - a).cross(c - a);
        let unit = normal * (1.0 / normal.norm());
        let from = (a + b + c) * (1.0 / 3.0) + unit;
        Ray::new([from.x, from.y, from.z], [-unit.x, -unit.y, -unit.z]).ok()
    };
    mesh.triangles.iter().step_by(16).filter_map(ray).collect()
}

/// What `gates perf` meshes and picks in the browser, made once: P9's first parts, and a picker of their mesh with its
/// rays.
pub struct PerfParts {
    model: Snapshot,
    picker: Picker,
    rays: Vec<Ray>,
}

impl PerfParts {
    /// P9's first parts, meshed on one thread to make the picker.
    ///
    /// # Errors
    ///
    /// What [`synthetic`] refuses, which it does not for these parts.
    pub fn new() -> Result<Self, Vec<Report>> {
        Self::of(synthetic(&Exec::serial(), PERF_TARGET, P9_SEED)?).map_err(|report| vec![report])
    }

    /// `model`, meshed on one thread to make the picker.
    fn of(model: Snapshot) -> Result<Self, Report> {
        let meshed = perf_mesh(&Exec::serial(), &model)?;
        Ok(Self { picker: Picker::new(&meshed), rays: pick_rays(&meshed), model })
    }

    /// The parts meshed whole on one thread, as [`perf_mesh`]: the mesh's digest, which holds what it refused and
    /// left open too.
    #[must_use]
    pub fn mesh(&self) -> u64 {
        perf_mesh(&Exec::serial(), &self.model).map_or(0, |meshed| meshed.digest())
    }

    /// Each ray picked with edges within a hundredth of a millimetre: a digest of what each hit and how far along.
    #[must_use]
    pub fn pick(&self) -> u64 {
        let mut digest = Digest::EMPTY;
        for ray in &self.rays {
            if let Ok(hit) = self.picker.pick(ray, Length::from_millimetres(0.01)) {
                digest.write_bytes(hit.entity.text().as_bytes());
                digest.write_value(hit.distance.millimetres());
            }
        }
        digest.finish()
    }
}

/// How many faces `model` has.
#[must_use]
pub fn face_count(model: &Snapshot) -> usize {
    model.ids::<Face>().len()
}

#[cfg(test)]
mod tests;
