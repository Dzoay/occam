//! Source: original
//! The body a transform or a copy names, and everything it holds: its shells, their faces and the faces' loops, its
//! wires, the uses of the loops and the wires, their edges and the edges' ends, each once, in the order a walk from the
//! body meets them.

use std::collections::BTreeSet;

use occam_report::{Code, EntityKind, Fault, Report};
use occam_topo::{Body, Edge, EdgeUse, Face, Id, Loop, Shell, Snapshot, Vertex, View, Wire};

use crate::operand::{Labels, Operand};
use crate::sweep::Named;

/// Everything a body holds.
#[derive(Debug, Default)]
pub(super) struct Parts {
    /// Its shells.
    pub shells: Vec<Id<Shell>>,
    /// Their faces.
    pub faces: Vec<Id<Face>>,
    /// The faces' loops.
    pub loops: Vec<Id<Loop>>,
    /// Its wires.
    pub wires: Vec<Id<Wire>>,
    /// The uses of the loops, then of the wires.
    pub uses: Vec<Id<EdgeUse>>,
    /// Their edges.
    pub edges: Vec<Id<Edge>>,
    /// The edges' ends.
    pub vertices: Vec<Id<Vertex>>,
}

/// The body `named` names in `model`, with `labels` naming what earlier steps made: a body, or the only face of a
/// sheet or the only wire of a wire body, which a `face` and a `wire` step make, standing for its body.
///
/// # Errors
///
/// [`Code::InputUnknownLabel`] for a label no earlier step has, [`Code::NotABody`] for any other entity, and
/// [`Code::StaleId`] for one that is not live.
pub(super) fn target(model: &Snapshot, named: &Named, labels: &Labels) -> Result<Id<Body>, Report> {
    let at = &named.at;
    let entity = match &named.operand {
        Operand::Label(label) => labels.get(label).ok_or_else(|| {
            Report::new(Code::InputUnknownLabel, Fault::InvalidInput).with_input(at.clone().given(label))
        })?,
        Operand::Entity(entity) => *entity,
    };
    let not_a_body = || Report::new(Code::NotABody, Fault::InvalidInput).with_input(at.clone().given(&entity.text()));
    let placed = |report: Report| report.with_input(at.clone());
    let body = match entity.kind {
        EntityKind::Body => return model.resolve::<Body>(entity).map_err(placed),
        EntityKind::Face => {
            let face = model.resolve::<Face>(entity).map_err(placed)?;
            let shell = model.view(face).and_then(|f| model.view(f.shell()));
            shell.map(View::<Shell>::body).filter(|&b| parts(model, b).faces == [face])
        }
        EntityKind::Wire => {
            let wire = model.resolve::<Wire>(entity).map_err(placed)?;
            model.view(wire).map(View::<Wire>::body).filter(|&b| parts(model, b).wires == [wire])
        }
        _ => None,
    };
    body.filter(|&b| model.view(b).is_some_and(|b| b.shells().len().saturating_add(b.wires().len()) == 1))
        .ok_or_else(not_a_body)
}

/// Everything `body` holds in `model`.
pub(super) fn parts(model: &Snapshot, body: Id<Body>) -> Parts {
    let mut parts = Parts::default();
    let Some(view) = model.view(body) else { return parts };
    parts.shells = view.shells();
    parts.faces = parts.shells.iter().filter_map(|&s| model.view(s)).flat_map(View::<Shell>::faces).collect();
    parts.loops = parts.faces.iter().filter_map(|&f| model.view(f)).flat_map(View::<Face>::loops).collect();
    parts.wires = view.wires();
    let looped = parts.loops.iter().filter_map(|&l| model.view(l)).flat_map(View::<Loop>::uses);
    let wired = parts.wires.iter().filter_map(|&w| model.view(w)).flat_map(View::<Wire>::uses);
    parts.uses = looped.chain(wired).collect();
    let (mut edges, mut vertices) = (BTreeSet::new(), BTreeSet::new());
    for view in parts.uses.iter().filter_map(|&u| model.view(u)) {
        if edges.insert(view.edge()) {
            parts.edges.push(view.edge());
        }
        for vertex in view.vertices().into_iter().flat_map(|(a, b)| [a, b]) {
            if vertices.insert(vertex) {
                parts.vertices.push(vertex);
            }
        }
    }
    parts
}
