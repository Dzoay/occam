//! Source: original
//! Transforms and copies (PARITY 2.11): a body moved, turned or scaled in place, every entity keeping its id, or a copy
//! of it, moved likewise, every entity new with the lineage `copied` to the one it came from. A step gives a `scale`,
//! one factor or three along the axes of an `about` frame, and then a rigid `frame` where the world's frame goes
//! (`docs/decisions/2026-11-12-non-uniform-scale.md`). Every curve and surface moves through the seam
//! (`docs/decisions/2026-11-12-moving-geometry.md`), and each face's parameter curves, each loop's witness and each
//! edge's range follow, so that `commit` checks the moved body as it checks any edit.

mod body;
mod given;
mod moved;
mod write;

use occam_report::{EntityRef, Report};
use occam_topo::Edit;

pub use given::TransformArgs;

use crate::args::Args;
use crate::operand::Labels;
use crate::operation::{Does, Made};

/// A `transform` step: its reader and its runner.
pub(crate) const TRANSFORM: Does = Does { read: given::transform_step, run: run_transform };

/// A `copy` step: a transform's reader, and its own runner.
pub(crate) const COPY: Does = Does { read: given::transform_step, run: run_copy };

/// Moves the body `args` names in place, if it holds a transform's.
fn run_transform(edit: &mut Edit, args: &Args, labels: &Labels) -> Option<Made> {
    let Args::Transform(args) = args else { return None };
    Some(transform(edit, args, labels))
}

/// Copies the body `args` names, if it holds a transform's.
fn run_copy(edit: &mut Edit, args: &Args, labels: &Labels) -> Option<Made> {
    let Args::Transform(args) = args else { return None };
    Some(copy(edit, args, labels))
}

/// Moves the body `args` names in place, and gives it.
///
/// # Errors
///
/// What naming the body reports, [`occam_report::Code::NoExactForm`] for a curve or surface with no exact form under
/// the map, naming it, and what the edit reports.
fn transform(edit: &mut Edit, args: &TransformArgs, labels: &Labels) -> Result<Option<EntityRef>, Report> {
    let body = body::target(edit.base(), &args.target, labels)?;
    let parts = body::parts(edit.base(), body);
    let moves = moved::Moves::of(edit, &parts, &args.map, &args.scale_at)?;
    write::in_place(edit, &parts, &moves)?;
    Ok(Some(body.entity()))
}

/// Copies the body `args` names, moved, and gives the copy.
///
/// # Errors
///
/// As [`transform`].
fn copy(edit: &mut Edit, args: &TransformArgs, labels: &Labels) -> Result<Option<EntityRef>, Report> {
    let body = body::target(edit.base(), &args.target, labels)?;
    let parts = body::parts(edit.base(), body);
    let moves = moved::Moves::of(edit, &parts, &args.map, &args.scale_at)?;
    Ok(Some(write::copied(edit, body, &parts, &moves)?.entity()))
}

#[cfg(test)]
mod tests;
