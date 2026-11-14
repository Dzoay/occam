//! Source: original
//! The `wire` operation (PARITY 1.6): straight edges through points, or lines, arcs and whole circles one by one, joined
//! end to end within the band, as a wire body. A wire that lies in a plane may not touch or cross itself there.

use occam_report::{Code, EntityRef, Input, Report};
use occam_topo::Edit;
use occam_units::Vec3;

use super::build::{self, Chain, Made, refused, shown};
use super::given::{Piece, WireArgs};
use super::outline::{Run, off_plane, plane_through};
use super::space::Flatland;
use super::touch::{Touch, first_touch};
use crate::read::count;

/// The band of the model `edit` changes, in millimetres.
pub(crate) fn band(edit: &Edit) -> f64 {
    edit.base().tolerance().band().millimetres()
}

/// Makes the wire `args` asks for in `edit`, and gives its body.
///
/// # Errors
///
/// What [`made`], [`build::chain`] and [`crossing`] report.
pub(crate) fn wire(edit: &mut Edit, args: &WireArgs) -> Result<Option<EntityRef>, Report> {
    let band = band(edit);
    let chain = build::chain(edit, made(args, band)?, band)?;
    crossing(edit, &chain, args.at(), band)?;
    Ok(Some(build::wire_body(edit, &chain)?.entity()))
}

/// The edges `args` asks for, in order, each with where it is in the request. Through points that are `closed`, a last
/// point within `band` of the first is that point again, not another edge; edges that are `closed` are joined as
/// [`close`] joins them.
///
/// # Errors
///
/// What [`build::made`] reports of an edge.
pub(crate) fn made(args: &WireArgs, band: f64) -> Result<Vec<(Made, Input)>, Report> {
    let pieces: Vec<(Piece, &Input)> = match args {
        WireArgs::Edges { edges, .. } => edges.iter().map(|(piece, at)| (*piece, at)).collect(),
        WireArgs::Points { points, closed, .. } => {
            let mut corners: Vec<&(Vec3, Input)> = points.iter().collect();
            let comes_back =
                matches!((corners.first(), corners.last()), (Some(a), Some(b)) if (b.0 - a.0).norm() <= band);
            if *closed && comes_back && corners.len() > 2 {
                corners.pop();
            }
            let mut lines: Vec<(Piece, &Input)> =
                corners.windows(2).filter_map(|pair| Some(line(pair.first()?, pair.get(1)?))).collect();
            if let (true, Some(last), Some(first)) = (*closed, corners.last(), corners.first()) {
                lines.push(line(last, first));
            }
            lines
        }
    };
    let mut made: Vec<(Made, Input)> = pieces
        .into_iter()
        .map(|(piece, at)| Ok((build::made(piece, at, band)?, at.clone())))
        .collect::<Result<_, _>>()?;
    if let WireArgs::Edges { closed: true, at, .. } = args {
        close(&mut made, at, band)?;
    }
    Ok(made)
}

/// Joins the last of `made` to the first, as points that are `closed` are joined: by a straight edge, placed at `at`, the
/// wire's place in the request, where the last ends further than `band` from where the first begins.
///
/// # Errors
///
/// None: an edge longer than the band is made.
fn close(made: &mut Vec<(Made, Input)>, at: &Input, band: f64) -> Result<(), Report> {
    let ends = made.first().map(|(m, _)| m.start).zip(made.last().map(|(m, _)| m.end));
    if let Some((start, end)) = ends.filter(|&(start, end)| (end - start).norm() > band) {
        made.push((build::made(Piece::Line { from: end, to: start }, at, band)?, at.clone()));
    }
    Ok(())
}

/// The straight edge from `from` to `to`, placed where `to` is in the request.
fn line<'a>(from: &(Vec3, Input), to: &'a (Vec3, Input)) -> (Piece, &'a Input) {
    (Piece::Line { from: from.0, to: to.0 }, &to.1)
}

/// A report that the run at `at` touches or crosses itself where `touch` says, in `plane`.
pub(crate) fn crosses_itself(plane: &Flatland, touch: Touch, at: &Input, band: f64) -> Report {
    refused(Code::WireCrossesItself, at, band)
        .with_value(shown("point", plane.point(touch.at)))
        .with_value(count("first", touch.first.1))
        .with_value(count("second", touch.second.1))
}

/// That `chain`, if it lies in a plane, neither touches nor crosses itself there.
///
/// # Errors
///
/// [`Code::WireCrossesItself`] where two of its edges come within `band` of each other other than where consecutive
/// edges meet.
fn crossing(edit: &Edit, chain: &Chain, at: &Input, band: f64) -> Result<(), Report> {
    let run = Run::read(edit, chain.edges.iter().map(|&e| (e, false)).collect(), at)?;
    let points = run.samples();
    let Some(plane) = plane_through(&points, band) else { return Ok(()) };
    if off_plane(&plane, &points, band).is_some() {
        return Ok(());
    }
    let flats = run.flats(&plane);
    first_touch(&[(&flats, chain.closed)], band).map_or(Ok(()), |touch| Err(crosses_itself(&plane, touch, at, band)))
}
