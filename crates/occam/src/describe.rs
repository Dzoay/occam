//! Source: original
//! The one resolver (O26, docs/architecture/reports.md): what each id in a result is, in the caller's terms, described
//! in one batch as the result leaves the kernel, so that a failure site names ids and nothing more and none can forget
//! to describe them. Each id is read from the first place it is live: the model the run left, which is what the caller
//! works on next; the model the step that named it left, and the one it began from, for what was gone by the end; and a
//! refused step's edit as it stood, for what exists only there. An id live in none is described as `none`.
//!
//! A description gives the entity's form: its geometry's for a face or an edge (`plane`, `line`), what a body is
//! (`solid`, `sheet`, `wire`), whether a loop is a face's outer one or a hole, and the kind otherwise. Its values are
//! its geometry's parameters, where it is (a vertex's point, an edge's start and finish), the corners `low` and `high`
//! of a box that holds it, rounded outward, and how many of what it is made of. Its origin is the step that made it,
//! for the bodies, faces and edges that keep one. Its neighbours are what an agent walks by (PARITY 1.1): a body's
//! shells and wires; a shell's body and faces; a face's shell, surface, loops and edges; a loop's face and edges; a
//! wire's body and edges; a use's loop or wire, edge and curve in the face's parameters; an edge's faces, wires,
//! vertices and curve; a vertex's edges. Geometry names none: what lies on it names it.

use std::collections::{BTreeMap, BTreeSet};

use occam_geom::{Curve, Shape, Surface, UvBox};
use occam_ops::Operation;
use occam_report::{Description, EntityKind, EntityRef, NamedValue, Origin, Report};
use occam_topo::{
    Body, BodyKind, CurveGeom, Edge, EdgeUse, Edit, Face, Id, Kind, Loop, ParamCurveGeom, Provenance, Shell, Snapshot,
    SurfaceGeom, UseOwner, Vertex, View, Wire,
};
use occam_units::{Bounds, Point, UvPoint, World};

use crate::run::{Done, Failed, Refused};

/// Where the describer reads an entity.
#[derive(Clone, Copy)]
pub(crate) enum Source<'a> {
    /// A model.
    Model(&'a Snapshot),
    /// A refused step's edit, as it stood.
    Edit(&'a Edit),
}

impl<'a> Source<'a> {
    /// The entity `id` names, if it is live here.
    fn of<K: Kind>(self, id: Id<K>) -> Option<View<'a, K>> {
        match self {
            Self::Model(model) => model.view(id),
            Self::Edit(edit) => edit.view(id),
        }
    }

    /// The entity `entity` names, as one of kind `K`, if it is live here.
    fn view<K: Kind>(self, entity: EntityRef) -> Option<View<'a, K>> {
        self.of(Id::<K>::from_entity(entity)?)
    }
}

/// The steps of the request being answered, by the model's step each one was: its index and what it asked for.
pub(crate) type Steps = BTreeMap<u64, (u32, Operation)>;

/// The one place a run's result leaves the kernel: every id each step in `done` names is described, from the model the
/// run left, which is what the caller works on next, then from the one the step left and the one it began from, for
/// what was gone by the end; and every id the reports of a step that was refused name, from its edit as it stood, then
/// from the model the run left. `models` holds each model the run passed through, in order. Gives the failure, if
/// there is one, its reports described.
pub(crate) fn describe_all(
    models: &[Snapshot],
    steps: &Steps,
    done: &mut [Done],
    refused: Option<Refused>,
) -> Option<Failed> {
    let last: Vec<Source<'_>> = models.last().map(Source::Model).into_iter().collect();
    for (step, (before, after)) in done.iter_mut().zip(models.iter().zip(models.iter().skip(1))) {
        let sources: Vec<Source<'_>> =
            last.iter().copied().chain([Source::Model(after), Source::Model(before)]).collect();
        step.described = batch(&sources, steps, step.named());
    }
    refused.map(|(step, reports, edit)| {
        let sources: Vec<Source<'_>> = [Source::Edit(&edit)].into_iter().chain(last.iter().copied()).collect();
        let reports = reports.into_iter().map(|r| report(r, &sources, steps)).collect();
        Failed { step: Some(step), reports }
    })
}

/// What each of `ids` is in `model`, one description per id in the order asked, an id that names nothing live there
/// described as [`Description::NONE`]: what `occam describe` answers between requests, from the same resolver.
#[must_use]
pub fn describe(model: &Snapshot, ids: &[EntityRef]) -> Vec<Description> {
    let sources = [Source::Model(model)];
    ids.iter().map(|&entity| one(&sources, &Steps::new(), entity)).collect()
}

/// Each of `ids` once, in id order, described from the first of `sources` it is live in.
fn batch(sources: &[Source<'_>], steps: &Steps, ids: impl IntoIterator<Item = EntityRef>) -> Vec<Description> {
    let ids: BTreeSet<EntityRef> = ids.into_iter().collect();
    ids.into_iter().map(|entity| one(sources, steps, entity)).collect()
}

/// `report`, with what each entity it names is.
pub(crate) fn report(report: Report, sources: &[Source<'_>], steps: &Steps) -> Report {
    let named = report.named();
    if named.is_empty() {
        return report;
    }
    let described = batch(sources, steps, named);
    report.with_described(described)
}

/// `entity` described from the first of `sources` it is live in.
fn one(sources: &[Source<'_>], steps: &Steps, entity: EntityRef) -> Description {
    let said = sources.iter().find_map(|&source| said(source, entity));
    said.map_or_else(|| Description::none(entity), |said| said.of(entity, steps))
}

/// What is said of `entity` in `source`, if it is live there.
fn said(source: Source<'_>, entity: EntityRef) -> Option<Said> {
    match entity.kind {
        EntityKind::Body => source.view(entity).map(|v| body(source, v)),
        EntityKind::Shell => source.view(entity).map(|v| shell(source, v)),
        EntityKind::Face => source.view(entity).map(|v| face(source, v)),
        EntityKind::Loop => source.view(entity).map(|v| looped(source, v)),
        EntityKind::Wire => source.view(entity).map(|v| wire(source, v)),
        EntityKind::EdgeUse => source.view(entity).map(|v| edge_use(source, v)),
        EntityKind::Edge => source.view(entity).map(|v| edge(source, v)),
        EntityKind::Vertex => source.view(entity).map(vertex),
        EntityKind::Surface => source.view::<SurfaceGeom>(entity).map(|v| Said::geometry(v.get().shape())),
        EntityKind::Curve => source.view::<CurveGeom>(entity).map(|v| Said::geometry(v.get().shape())),
        EntityKind::ParamCurve => source.view::<ParamCurveGeom>(entity).map(|v| Said::geometry(v.get().shape())),
        EntityKind::Model | EntityKind::Section | EntityKind::Region => None,
    }
}

/// What is said of one entity, before its id and origin are put to it.
struct Said {
    form: String,
    values: Vec<NamedValue>,
    born: Option<Provenance>,
    adjacent: Vec<EntityRef>,
}

impl Said {
    /// Of the form `form`, with nothing more yet.
    fn new(form: &str) -> Self {
        Self { form: form.to_string(), values: Vec::new(), born: None, adjacent: Vec::new() }
    }

    /// Of a geometry's form and with its parameters.
    fn geometry(shape: Shape) -> Self {
        Self { values: shape.values, ..Self::new(shape.form) }
    }

    /// Of the form and with the parameters of the geometry `shape` gives, or of `form` where it is missing.
    fn shaped(shape: Option<Shape>, form: &str) -> Self {
        shape.map_or_else(|| Self::new(form), Self::geometry)
    }

    fn value(mut self, value: NamedValue) -> Self {
        self.values.push(value);
        self
    }

    fn count(self, name: &str, count: usize) -> Self {
        self.value(NamedValue::count(name, u32::try_from(count).unwrap_or(u32::MAX)))
    }

    /// With where the vertex `id` is, as `name`, if it is live.
    fn at(self, source: Source<'_>, name: &str, id: Id<Vertex>) -> Self {
        match source.of(id) {
            Some(v) => self.value(point(name, v.point())),
            None => self,
        }
    }

    /// With the corners of `bounds`, if there is a box.
    fn boxed(self, bounds: Option<Bounds>) -> Self {
        match bounds {
            Some(bounds) => self.value(point("low", bounds.low())).value(point("high", bounds.high())),
            None => self,
        }
    }

    const fn born(mut self, born: Provenance) -> Self {
        self.born = Some(born);
        self
    }

    /// With `ids` as neighbours, each once.
    fn near<K: Kind>(mut self, ids: impl IntoIterator<Item = Id<K>>) -> Self {
        for entity in ids.into_iter().map(Id::entity) {
            if !self.adjacent.contains(&entity) {
                self.adjacent.push(entity);
            }
        }
        self
    }

    /// The description of `entity`, its origin read against `steps`.
    fn of(self, entity: EntityRef, steps: &Steps) -> Description {
        let origin = self.born.map(|born| {
            let here = steps.get(&born.step);
            Origin {
                step: born.step,
                request_step: here.map(|&(index, _)| index),
                operation: here.map(|&(_, operation)| operation.name().to_string()),
                role: born.role,
            }
        });
        Description { entity, form: self.form, values: self.values, origin, adjacent: self.adjacent }
    }
}

/// `p` as a named point.
fn point(name: &str, p: Point<World>) -> NamedValue {
    let v = p.to_vec3();
    NamedValue::point(name, [v.x, v.y, v.z])
}

/// The box holding every box of `boxes`, or `None` if there are none or any is missing.
fn union(boxes: impl IntoIterator<Item = Option<Bounds>>) -> Option<Bounds> {
    let mut all: Option<Bounds> = None;
    for one in boxes {
        let one = one?;
        all = Some(all.map_or(one, |all| all.union(&one)));
    }
    all
}

/// The smaller of two numbers, the first if they are equal, so the sign of a zero is fixed (not `f64::min`).
fn smaller(a: f64, b: f64) -> f64 {
    if b < a { b } else { a }
}

/// The larger of two numbers, the first if they are equal.
fn larger(a: f64, b: f64) -> f64 {
    if b > a { b } else { a }
}

/// The rectangle holding both.
fn uv_union(a: UvBox, b: UvBox) -> UvBox {
    let lo = UvPoint::new(smaller(a.lo.u(), b.lo.u()), smaller(a.lo.v(), b.lo.v()));
    let hi = UvPoint::new(larger(a.hi.u(), b.hi.u()), larger(a.hi.v(), b.hi.v()));
    UvBox::new(lo, hi)
}

/// A box holding the edge `id`: its curve's over its range.
fn edge_box(source: Source<'_>, id: Id<Edge>) -> Option<Bounds> {
    let edge = source.of(id)?;
    let (lo, hi) = edge.range();
    Some(edge.curve()?.bounds(lo, hi))
}

/// A box holding the face `id`: its surface's over the rectangle that holds its outer loop in the face's parameters,
/// read from the curve in the parameters of each of the loop's uses. A face lies inside its outer loop, so the box holds
/// it however its surface bulges between its edges. `None` where a use has no such curve.
fn face_box(source: Source<'_>, id: Id<Face>) -> Option<Bounds> {
    let face = source.of(id)?;
    let mut patch: Option<UvBox> = None;
    for u in source.of(face.outer())?.uses() {
        let u = source.of(u)?;
        let (lo, hi) = source.of(u.edge())?.range();
        let piece = u.pcurve()?.bounds(lo, hi);
        patch = Some(patch.map_or(piece, |patch| uv_union(patch, piece)));
    }
    Some(face.surface()?.bounds(patch?))
}

/// The edges `uses` use, in order.
fn edges_of(source: Source<'_>, uses: &[Id<EdgeUse>]) -> Vec<Id<Edge>> {
    uses.iter().filter_map(|&u| source.of(u)).map(View::edge).collect()
}

fn body(source: Source<'_>, b: View<'_, Body>) -> Said {
    let (shells, wires) = (b.shells(), b.wires());
    let faces: Vec<Id<Face>> = shells.iter().filter_map(|&s| source.of(s)).flat_map(View::<Shell>::faces).collect();
    let uses: Vec<Id<EdgeUse>> = wires.iter().filter_map(|&w| source.of(w)).flat_map(View::<Wire>::uses).collect();
    let edges = edges_of(source, &uses);
    let boxes = faces.iter().map(|&f| face_box(source, f)).chain(edges.iter().map(|&e| edge_box(source, e)));
    let form = match b.kind() {
        BodyKind::Solid => "solid",
        BodyKind::Sheet => "sheet",
        BodyKind::Wire => "wire",
    };
    Said::new(form)
        .count("shells", shells.len())
        .count("faces", faces.len())
        .count("wires", wires.len())
        .boxed(union(boxes))
        .born(b.born())
        .near(shells)
        .near(wires)
}

fn shell(source: Source<'_>, s: View<'_, Shell>) -> Said {
    let faces = s.faces();
    let bounds = union(faces.iter().map(|&f| face_box(source, f)));
    Said::new("shell").count("faces", faces.len()).boxed(bounds).near([s.body()]).near(faces)
}

fn face(source: Source<'_>, f: View<'_, Face>) -> Said {
    let loops = f.loops();
    Said::shaped(f.surface().map(Surface::shape), "face")
        .count("loops", loops.len())
        .boxed(face_box(source, f.id()))
        .born(f.born())
        .near([f.shell()])
        .near([f.surface_id()])
        .near(loops)
        .near(f.edges())
}

fn looped(source: Source<'_>, l: View<'_, Loop>) -> Said {
    let edges = edges_of(source, &l.uses());
    let outer = source.of(l.face()).is_some_and(|f| f.outer() == l.id());
    let bounds = union(edges.iter().map(|&e| edge_box(source, e)));
    let form = if outer { "outer-loop" } else { "inner-loop" };
    Said::new(form).count("edges", edges.len()).boxed(bounds).near([l.face()]).near(edges)
}

fn wire(source: Source<'_>, w: View<'_, Wire>) -> Said {
    let edges = edges_of(source, &w.uses());
    let bounds = union(edges.iter().map(|&e| edge_box(source, e)));
    Said::new("wire").count("edges", edges.len()).boxed(bounds).near([w.body()]).near(edges)
}

fn edge_use(source: Source<'_>, u: View<'_, EdgeUse>) -> Said {
    let said = match u.vertices() {
        Some((start, finish)) => Said::new("edge-use").at(source, "start", start).at(source, "finish", finish),
        None => Said::new("edge-use"),
    };
    let said = match u.owner() {
        UseOwner::Loop(owner) => said.near([owner]),
        UseOwner::Wire(owner) => said.near([owner]),
    };
    said.boxed(edge_box(source, u.edge())).near([u.edge()]).near(u.pcurve_id())
}

fn edge(source: Source<'_>, e: View<'_, Edge>) -> Said {
    let (start, finish) = e.ends();
    let owners = e.uses().into_iter().filter_map(|u| source.of(u)).map(View::owner);
    let wires = owners.filter_map(|owner| match owner {
        UseOwner::Wire(wire) => Some(wire),
        UseOwner::Loop(_) => None,
    });
    Said::shaped(e.curve().map(Curve::shape), "edge")
        .at(source, "start", start)
        .at(source, "finish", finish)
        .boxed(edge_box(source, e.id()))
        .born(e.born())
        .near(e.faces())
        .near(wires)
        .near([start, finish])
        .near([e.curve_id()])
}

fn vertex(v: View<'_, Vertex>) -> Said {
    let said = Said::new("vertex").value(point("point", v.point()));
    let radius = v.radius().millimetres();
    let said = if radius > 0.0 { said.value(NamedValue::length("radius", radius)) } else { said };
    said.near(v.edges())
}

#[cfg(test)]
mod tests;
