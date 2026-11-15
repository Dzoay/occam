//! Source: original
//! Measuring a body or a part of one (PARITY 1.8, 2.7): a solid's volume, area and centre of mass, by the divergence
//! theorem over its faces with adaptive quadrature ([`mass`]), and the tight box of anything ([`extent`]). A measure
//! changes nothing: the runner asks for its values with [`measure`] instead of making an edit.
//!
//! A step names its `target`, a label or an id, and may list `what` it wants of `volume`, `area`, `centre-of-mass` and
//! `bounds`; without a list it gets everything its target has. A solid body has all four; a sheet, a shell or a face
//! an area and bounds; a wire, an edge or a vertex bounds alone.
//!
//! **The tolerance.** The volume and the area are each found to [`RELATIVE`] of their own value, and the centre of
//! mass to [`RELATIVE`] of the body's size, the diagonal of a box holding it, as the quadrature estimates its error:
//! the sum, over the intervals it keeps, of how far each one's halves differ from it. A first pass to a millionth of
//! the body's size gives the values that the second is held to. The box holds the body, rounding included, and is
//! tight where its geometry is analytic ([`extent`]).

mod extent;
mod mass;
mod quadrature;
#[cfg(test)]
mod tests;

use occam_report::{Code, EntityKind, EntityRef, Fault, NamedValue, Quantity, Reason, Report};
use occam_topo::{Body, BodyKind, Edge, Face, Id, Shell, Snapshot, Vertex, View, Wire};
use occam_units::{Bounds, Point, Tolerance, Vec3, World};
use serde_json::{Map, Value};

use crate::args::Args;
use crate::operand::{Labels, Operand, resolve};
use crate::operation::Operation;
use crate::read::{At, missing, name, object, text, wrong_type};
use crate::request::Step;
use extent::Inside;
use mass::{Asked, Patch, Sums};
use quadrature::Budget;

/// How closely the volume and the area are found, as a part of their values, and the centre of mass, as a part of the
/// body's size.
pub const RELATIVE: f64 = 1e-9;

/// How closely the first pass finds them, as parts of the body's size, its cube and its fourth power.
const FIRST: f64 = 1e-6;

/// How many intervals one measure's quadrature may halve in all, every integral of every face together.
const BUDGET: u64 = 1 << 22;

/// Something a measure step can ask for.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub enum Measurement {
    /// A solid's volume.
    Volume,
    /// The area of the faces.
    Area,
    /// A solid's centre of mass, of a uniform density.
    CentreOfMass,
    /// The tight box, along the world's axes.
    Bounds,
}

impl Measurement {
    /// Every measurement, in the order a result gives them.
    pub const ALL: [Self; 4] = [Self::Volume, Self::Area, Self::CentreOfMass, Self::Bounds];

    /// The name a request gives it.
    #[must_use]
    pub const fn name(self) -> &'static str {
        match self {
            Self::Volume => "volume",
            Self::Area => "area",
            Self::CentreOfMass => "centre-of-mass",
            Self::Bounds => "bounds",
        }
    }
}

/// What a measure step asks for.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct MeasureArgs {
    /// What it measures.
    pub target: Operand,
    /// What it wants, in the order given; none means everything its target has.
    pub what: Vec<Measurement>,
}

/// What a measure step found: its target, and each value asked for, in [`Measurement::ALL`]'s order, in millimetres:
/// `volume`, `area`, `centre-of-mass`, and the box's corners `bounds-low` and `bounds-high`.
#[derive(Clone, Debug, PartialEq)]
pub struct Measured {
    /// What was measured.
    pub target: EntityRef,
    /// The values.
    pub values: Vec<NamedValue>,
}

/// The measure step whose fields other than `op` and `label` are `fields`, at `at`, with every bad input in `out`.
pub(crate) fn measure_step(at: &At, fields: &Map<String, Value>, out: &mut Vec<Report>) -> Option<MeasureArgs> {
    let all = Value::Object(fields.clone());
    let fields = object(at, &all, Operation::Measure.fields(), out)?;
    let target = fields.get("target").ok_or_else(|| missing(&at.field("target")));
    let target =
        target.and_then(|value| text(&at.field("target"), value)).map(Operand::read).map_err(|r| out.push(r)).ok();
    let what = match fields.get("what") {
        None => Some(Vec::new()),
        Some(Value::Array(items)) if items.is_empty() => {
            out.push(missing(&at.field("what").item(0)));
            None
        }
        Some(Value::Array(items)) => {
            let names = Measurement::ALL.map(|m| (m.name(), m));
            let read = items.iter().enumerate().map(|(k, item)| name(&at.field("what").item(k), item, &names));
            read.collect::<Result<Vec<_>, _>>().map_err(|r| out.push(r)).ok()
        }
        Some(other) => {
            out.push(wrong_type(&at.field("what"), other, "list"));
            None
        }
    };
    Some(MeasureArgs { target: target?, what: what? })
}

/// What a measure's target is.
#[derive(Clone, Copy, Debug, PartialEq, Eq)]
enum Target {
    Body(Id<Body>, BodyKind),
    Shell(Id<Shell>),
    Face(Id<Face>),
    Wire(Id<Wire>),
    Edge(Id<Edge>),
    Vertex(Id<Vertex>),
}

impl Target {
    /// What `entity` of `model` is, if it is something a measure takes.
    ///
    /// # Errors
    ///
    /// `topo.stale-id` where it names nothing live, as a label can once a later step has used up what it named.
    fn of(model: &Snapshot, entity: EntityRef) -> Result<Option<Self>, Report> {
        Ok(Some(match entity.kind {
            EntityKind::Body => {
                let id = model.resolve::<Body>(entity)?;
                Self::Body(id, model.view(id).map_or(BodyKind::Wire, occam_topo::View::kind))
            }
            EntityKind::Shell => Self::Shell(model.resolve(entity)?),
            EntityKind::Face => Self::Face(model.resolve(entity)?),
            EntityKind::Wire => Self::Wire(model.resolve(entity)?),
            EntityKind::Edge => Self::Edge(model.resolve(entity)?),
            EntityKind::Vertex => Self::Vertex(model.resolve(entity)?),
            _ => return Ok(None),
        }))
    }

    /// What it has, in [`Measurement::ALL`]'s order.
    fn has(self) -> &'static [Measurement] {
        match self {
            Self::Body(_, BodyKind::Solid) => &Measurement::ALL,
            Self::Body(_, BodyKind::Sheet) | Self::Shell(_) | Self::Face(_) => {
                &[Measurement::Area, Measurement::Bounds]
            }
            Self::Body(_, BodyKind::Wire) | Self::Wire(_) | Self::Edge(_) | Self::Vertex(_) => &[Measurement::Bounds],
        }
    }

    /// Its faces.
    fn faces(self, model: &Snapshot) -> Vec<Id<Face>> {
        let of_shell = |s: Id<Shell>| model.view(s).map(occam_topo::View::<Shell>::faces).unwrap_or_default();
        match self {
            Self::Body(id, _) => {
                model.view(id).map(|b| b.shells().into_iter().flat_map(of_shell).collect()).unwrap_or_default()
            }
            Self::Shell(id) => of_shell(id),
            Self::Face(id) => vec![id],
            Self::Wire(_) | Self::Edge(_) | Self::Vertex(_) => Vec::new(),
        }
    }

    /// Its edges, each once, and its points that lie on none: a vertex measured alone.
    fn edges(self, model: &Snapshot) -> (Vec<Id<Edge>>, Vec<Vec3>) {
        let of_uses = |uses: Vec<Id<occam_topo::EdgeUse>>| -> Vec<Id<Edge>> {
            uses.into_iter().filter_map(|u| model.view(u).map(occam_topo::View::edge)).collect()
        };
        let of_wire = |w: Id<Wire>| model.view(w).map(|w| of_uses(w.uses())).unwrap_or_default();
        let mut edges: Vec<Id<Edge>> = match self {
            Self::Body(id, _) => {
                let wires = model.view(id).map(occam_topo::View::wires).unwrap_or_default();
                let faces = self.faces(model).into_iter().filter_map(|f| model.view(f));
                faces.flat_map(occam_topo::View::<Face>::edges).chain(wires.into_iter().flat_map(of_wire)).collect()
            }
            Self::Shell(_) | Self::Face(_) => self
                .faces(model)
                .into_iter()
                .filter_map(|f| model.view(f))
                .flat_map(occam_topo::View::<Face>::edges)
                .collect(),
            Self::Wire(id) => of_wire(id),
            Self::Edge(id) => vec![id],
            Self::Vertex(id) => {
                let point = model.view(id).map(|v| v.point().to_vec3());
                return (Vec::new(), point.into_iter().collect());
            }
        };
        edges.sort_unstable();
        edges.dedup();
        (edges, Vec::new())
    }
}

/// A report that `what`, asked for at `at`, is not something `target` has.
fn not_defined(at: &At, what: Measurement, target: Target, entity: EntityRef) -> Report {
    let has: Vec<&str> = target.has().iter().map(|m| m.name()).collect();
    let input = at.input().given(what.name()).expected(&has.join(", "));
    Report::new(Code::MeasureNotDefined, Fault::InvalidInput).with_input(input).with_entity(entity)
}

/// The refusal of a measure that reached `reached` of the tolerance it was `asked`, which [`RELATIVE`] stands for.
fn not_converged(reached: Sums, asked: Sums) -> Report {
    let parts = reached.iter().zip(asked).filter(|(_, a)| a.is_finite()).map(|(r, a)| r / a);
    let worst = parts.map(|part| if part.is_nan() { f64::INFINITY } else { part }).fold(0.0, larger);
    Report::new(Code::NotConverged, Fault::NumericallySensitive)
        .with_reason(Reason::BudgetSpent)
        .with_value(NamedValue::new("reached", Quantity::Ratio, worst * RELATIVE))
        .with_value(NamedValue::new("asked", Quantity::Ratio, RELATIVE))
}

/// The box holding `edges` of `model`, from their curves' own boxes over their ranges, and `points`.
fn rough(model: &Snapshot, edges: &[Id<Edge>], points: &[Vec3]) -> Option<Bounds> {
    spanned(edges.iter().filter_map(|&e| model.view(e)), points)
}

/// The box holding `edges`, from their curves' own boxes over their ranges, and `points`.
fn spanned<'a>(edges: impl Iterator<Item = View<'a, Edge>>, points: &[Vec3]) -> Option<Bounds> {
    let boxes = edges.filter_map(|e| {
        let (lo, hi) = e.range();
        e.curve().map(|c| c.bounds(lo, hi))
    });
    let corners = points.iter().map(|&p| Point::<World>::from_vec3(p));
    let start = Bounds::of(corners);
    boxes.fold(start, |all, b| Some(all.map_or(b, |a| a.union(&b))))
}

/// The volume, area and first moments about `centre` of `patches`, `size` across, a solid's if `solid`: a first pass
/// to [`FIRST`] of the size, then a second to [`RELATIVE`] of what the first found.
fn mass_of(patches: &[Patch<'_>], (centre, size): (Vec3, f64), solid: bool, budget: &Budget) -> Result<Sums, Report> {
    let first = first(size, solid);
    let rough = mass::sums(patches, Asked { tolerance: first, centre }, budget).map_err(|r| not_converged(r, first))?;
    let second = second(rough.value, size, solid);
    let fine =
        mass::sums(patches, Asked { tolerance: second, centre }, budget).map_err(|r| not_converged(r, second))?;
    Ok(fine.value)
}

/// The sums' tolerances `parts`, all held for a solid, the area alone for anything else, and never the signed area in
/// the parameters, which only orients each loop.
fn held(parts: Sums, solid: bool) -> Sums {
    let [volume, area, x, y, z, _] = parts;
    let free = f64::INFINITY;
    if solid { [volume, area, x, y, z, free] } else { [free, area, free, free, free, free] }
}

/// The first pass's tolerances, for a body `size` across: [`FIRST`] of its cube, its square and its fourth power.
fn first(size: f64, solid: bool) -> Sums {
    let (square, cube) = (size * size, size * size * size);
    let fourth = FIRST * cube * size;
    held([FIRST * cube, FIRST * square, fourth, fourth, fourth, 0.0], solid)
}

/// The second pass's tolerances, from the first's `rough` sums: [`RELATIVE`] of the volume and the area, held to no
/// less than the first pass's, and of the body's size times the volume for the moments.
fn second(rough: Sums, size: f64, solid: bool) -> Sums {
    let [volume, area, ..] = rough;
    let volume = larger(volume.abs(), FIRST * size * size * size);
    let area = larger(area, FIRST * size * size);
    let moment = RELATIVE * size * volume;
    held([RELATIVE * volume, RELATIVE * area, moment, moment, moment, 0.0], solid)
}

/// The larger of two numbers, the first if they are equal.
fn larger(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// What the measure step `step` asks of `model`, with `labels` naming what earlier steps made.
///
/// # Errors
///
/// `input.missing` for a step built by hand without what a measure reads; `input.unknown-label` and `topo.stale-id`
/// for a target that names nothing; `input.wrong-type` for a target that is not a body, shell, face, wire, edge or
/// vertex; `measure.not-defined` for a measure the target does not have; `measure.no-parameter-curve` for an area of a
/// face whose edge has no curve in its parameters; and `measure.not-converged` where the quadrature runs out of budget.
pub fn measure(model: &Snapshot, step: &Step, labels: &Labels) -> Result<Measured, Report> {
    let (entity, target, asked) = asked(model, step, labels)?;
    let wants = |m: Measurement| asked.contains(&m);
    let (edges, points) = target.edges(model);
    let faces = target.faces(model);
    let mut values = Vec::new();
    if wants(Measurement::Volume) || wants(Measurement::Area) || wants(Measurement::CentreOfMass) {
        let patches = faces.iter().map(|&f| mass::patch(model, f)).collect::<Result<Vec<_>, _>>()?;
        let (low, high) = corners(rough(model, &edges, &points));
        let centre = (low + high) * 0.5;
        let solid = matches!(target, Target::Body(_, BodyKind::Solid));
        let sums = mass_of(&patches, (centre, (high - low).norm()), solid, &Budget::new(BUDGET))?;
        if wants(Measurement::Volume) {
            values.push(NamedValue::new("volume", Quantity::Volume, sums[0]));
        }
        if wants(Measurement::Area) {
            values.push(NamedValue::new("area", Quantity::Area, sums[1]));
        }
        if wants(Measurement::CentreOfMass) {
            let moment = Vec3::new(sums[2], sums[3], sums[4]);
            let at = centre + moment * (1.0 / sums[0]);
            values.push(NamedValue::point("centre-of-mass", [at.x, at.y, at.z]));
        }
    }
    if wants(Measurement::Bounds) {
        let (low, high) = bounds(model, (&faces, &edges), points)?;
        values.push(NamedValue::point("bounds-low", [low.x, low.y, low.z]));
        values.push(NamedValue::point("bounds-high", [high.x, high.y, high.z]));
    }
    Ok(Measured { target: entity, values })
}

/// What `step` measures in `model` and what it asks of it: everything the target has, if it lists nothing.
fn asked(model: &Snapshot, step: &Step, labels: &Labels) -> Result<(EntityRef, Target, Vec<Measurement>), Report> {
    let index = usize::try_from(step.index).unwrap_or(usize::MAX);
    let at = At::root().field("steps").item(index).in_step(step.index, Some(step.operation.name()));
    let Args::Measure(args) = &step.args else { return Err(missing(&at)) };
    let entity = resolve(&at.field("target"), &args.target, labels, model)?;
    let Some(target) = Target::of(model, entity).map_err(|r| r.with_input(at.field("target").input()))? else {
        let input =
            at.field("target").input().given(entity.kind.text()).expected("body, shell, face, wire, edge or vertex");
        return Err(Report::new(Code::InputWrongType, Fault::InvalidInput).with_input(input).with_entity(entity));
    };
    for (k, &what) in args.what.iter().enumerate() {
        if !target.has().contains(&what) {
            return Err(not_defined(&at.field("what").item(k), what, target, entity));
        }
    }
    let asked = if args.what.is_empty() { target.has().to_vec() } else { args.what.clone() };
    Ok((entity, target, asked))
}

/// The tight box of `faces` and `edges` of `model`, and `points`, as its low and high corners.
fn bounds(
    model: &Snapshot,
    (faces, edges): (&[Id<Face>], &[Id<Edge>]),
    points: Vec<Vec3>,
) -> Result<(Vec3, Vec3), Report> {
    let tolerance = model.tolerance();
    let mut points = points;
    let mut boxes = Vec::new();
    for view in faces.iter().filter_map(|&face| model.view(face)) {
        match inside(view, tolerance)? {
            Inside::Points(found) => points.extend(found),
            Inside::Boxed(b) => boxes.push(b),
        }
    }
    Ok(corners(boxes.into_iter().fold(rough(model, edges, &points), |all, b| Some(all.map_or(b, |a| a.union(&b))))))
}

/// What inside the face `face` reaches further than its edges, to `tolerance`. A plane, a cylinder and a cone reach no
/// further inside than their edges, so their faces need no loops.
fn inside(face: View<'_, Face>, tolerance: Tolerance) -> Result<Inside, Report> {
    let Some(surface) = face.surface() else { return Ok(Inside::Points(Vec::new())) };
    match extent::turning(surface, tolerance) {
        Some(found) if found.is_empty() => Ok(Inside::Points(found)),
        turning => Ok(extent::interior(&mass::of(face)?, turning, tolerance)),
    }
}

/// The tight box of the face `face`, to `tolerance`, as [`Measurement::Bounds`] finds a body's: its edges' own boxes,
/// and where its surface reaches further inside it. Read from the model or edit the view is in, so a refused step's
/// faces have one too. `None` for a face with no edge.
///
/// # Errors
///
/// `measure.no-parameter-curve` for a face on a surface that may reach further inside than its edges, a use of whose
/// loops has no curve in its parameters.
pub fn face_bounds(face: View<'_, Face>, tolerance: Tolerance) -> Result<Option<Bounds>, Report> {
    let edges = face.edges();
    let (points, boxed) = match inside(face, tolerance)? {
        Inside::Points(found) => (found, None),
        Inside::Boxed(b) => (Vec::new(), Some(b)),
    };
    let rough = spanned(edges.iter().filter_map(|&e| face.of(e)), &points);
    Ok(boxed.into_iter().fold(rough, |all, b| Some(all.map_or(b, |a| a.union(&b)))))
}

/// The low and high corners of `bounds`, or of the origin where there are none.
fn corners(bounds: Option<Bounds>) -> (Vec3, Vec3) {
    let origin = Vec3::new(0.0, 0.0, 0.0);
    bounds.map_or((origin, origin), |b| (b.low().to_vec3(), b.high().to_vec3()))
}
