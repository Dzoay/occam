//! Source: original
//! What a step works on: what an earlier step of the request made, by that step's label, or an entity of the model by
//! its id, `edge:41/1`. A label that reads as an id is refused when the request is read, so the two never meet.

use std::collections::BTreeMap;

use occam_report::{Code, EntityKind, EntityRef, Fault, NamedValue, Report};
use occam_topo::{
    Body, CurveGeom, Edge, EdgeUse, Face, Loop, ParamCurveGeom, Shell, Snapshot, SurfaceGeom, Vertex, Wire,
};

use crate::read::At;

/// An operand as a request writes it.
#[derive(Clone, Debug, PartialEq, Eq)]
pub enum Operand {
    /// An earlier step's label.
    Label(String),
    /// An entity of the model.
    Entity(EntityRef),
}

impl Operand {
    /// The operand written `text`: an entity's id if it reads as one, else a label.
    #[must_use]
    pub fn read(text: &str) -> Self {
        EntityRef::parse(text).map_or_else(|| Self::Label(text.to_string()), Self::Entity)
    }
}

/// The labels of the steps a request has run so far, each with what its step made.
#[derive(Clone, Debug, Default, PartialEq, Eq)]
pub struct Labels {
    made: BTreeMap<String, EntityRef>,
}

impl Labels {
    /// Records that the step labelled `label` made `entity`.
    pub fn insert(&mut self, label: &str, entity: EntityRef) {
        self.made.insert(label.to_string(), entity);
    }

    /// What the step labelled `label` made, if an earlier step has that label.
    #[must_use]
    pub fn get(&self, label: &str) -> Option<EntityRef> {
        self.made.get(label).copied()
    }
}

/// The live entity `operand`, at `at` in the request, names in `model`.
///
/// # Errors
///
/// [`Code::InputUnknownLabel`] for a label no earlier step has, and [`Code::StaleId`] for an id that names no live
/// entity, each saying where the operand is in the request.
pub(crate) fn resolve(at: &At, operand: &Operand, labels: &Labels, model: &Snapshot) -> Result<EntityRef, Report> {
    match operand {
        Operand::Label(label) => labels.get(label).ok_or_else(|| {
            Report::new(Code::InputUnknownLabel, Fault::InvalidInput).with_input(at.input().given(label))
        }),
        Operand::Entity(entity) => live(model, *entity).map_err(|report| report.with_input(at.input())),
    }
}

/// `entity`, if it is live in `model`.
fn live(model: &Snapshot, entity: EntityRef) -> Result<EntityRef, Report> {
    let checked = match entity.kind {
        EntityKind::Vertex => model.resolve::<Vertex>(entity).map(drop),
        EntityKind::Edge => model.resolve::<Edge>(entity).map(drop),
        EntityKind::EdgeUse => model.resolve::<EdgeUse>(entity).map(drop),
        EntityKind::Loop => model.resolve::<Loop>(entity).map(drop),
        EntityKind::Wire => model.resolve::<Wire>(entity).map(drop),
        EntityKind::Face => model.resolve::<Face>(entity).map(drop),
        EntityKind::Shell => model.resolve::<Shell>(entity).map(drop),
        EntityKind::Body => model.resolve::<Body>(entity).map(drop),
        EntityKind::Surface => model.resolve::<SurfaceGeom>(entity).map(drop),
        EntityKind::Curve => model.resolve::<CurveGeom>(entity).map(drop),
        EntityKind::ParamCurve => model.resolve::<ParamCurveGeom>(entity).map(drop),
        // Not entities of a model: the model itself, and what exists only in a failed operation's output.
        EntityKind::Model | EntityKind::Section | EntityKind::Region => {
            Err(Report::new(Code::StaleId, Fault::InvalidInput)
                .with_entity(entity)
                .with_value(NamedValue::count("live", 0)))
        }
    };
    checked.map(|()| entity)
}

#[cfg(test)]
mod tests {
    use occam_report::Input;
    use occam_topo::VertexRecord;
    use occam_units::{Angle, Length, Point, Tolerance};

    use super::*;

    fn model() -> (Snapshot, EntityRef) {
        let tolerance = Tolerance::new(Length::from_millimetres(1e-7), Angle::from_radians(1e-9)).unwrap();
        let mut edit = Snapshot::empty(tolerance).edit();
        let origin =
            Point::new(Length::from_millimetres(0.0), Length::from_millimetres(0.0), Length::from_millimetres(0.0));
        let vertex = edit.add(VertexRecord { point: origin, radius: Length::from_millimetres(0.0), star: None });
        (edit.commit(&occam_math::Exec::serial()).unwrap().0, vertex.entity())
    }

    #[test]
    fn an_operand_is_an_id_if_it_reads_as_one_and_a_label_otherwise() {
        assert_eq!(Operand::read("edge:41/1"), Operand::Entity(EntityRef::new(EntityKind::Edge, 41, 1)));
        assert_eq!(Operand::read("plate"), Operand::Label("plate".to_string()));
        assert_eq!(Operand::read("edge:41"), Operand::Label("edge:41".to_string()));
    }

    #[test]
    fn a_label_names_what_its_step_made_and_an_unknown_one_is_reported_where_it_is() {
        let (snapshot, vertex) = model();
        let at = At::root().field("steps").item(1).field("target").in_step(1, Some("extrude"));
        let mut labels = Labels::default();
        labels.insert("corner", vertex);
        assert_eq!(resolve(&at, &Operand::read("corner"), &labels, &snapshot), Ok(vertex));
        let report = resolve(&at, &Operand::read("plate"), &labels, &snapshot).unwrap_err();
        assert_eq!((report.code, report.fault), (Code::InputUnknownLabel, Some(Fault::InvalidInput)));
        assert_eq!(report.input, Some(Input::at("steps[1].target").in_step(1, Some("extrude")).given("plate")));
        assert_eq!(
            report.message(occam_report::DisplayUnits::default()),
            "steps[1].target names plate, which no earlier step is labelled."
        );
    }

    #[test]
    fn an_id_must_name_a_live_entity_of_its_kind() {
        let (snapshot, vertex) = model();
        let at = At::root().field("steps").item(0).field("target");
        assert_eq!(resolve(&at, &Operand::Entity(vertex), &Labels::default(), &snapshot), Ok(vertex));
        let kinds = [
            EntityKind::Edge,
            EntityKind::EdgeUse,
            EntityKind::Loop,
            EntityKind::Wire,
            EntityKind::Face,
            EntityKind::Shell,
            EntityKind::Body,
            EntityKind::Surface,
            EntityKind::Curve,
            EntityKind::ParamCurve,
            EntityKind::Model,
            EntityKind::Section,
            EntityKind::Region,
        ];
        for kind in kinds {
            let other = EntityRef::new(kind, vertex.slot, vertex.generation);
            let report = resolve(&at, &Operand::Entity(other), &Labels::default(), &snapshot).unwrap_err();
            assert_eq!((report.code, report.entities.clone()), (Code::StaleId, vec![other]), "{kind:?}");
            assert_eq!(report.input, Some(Input::at("steps[0].target")));
            assert_eq!(report.value("live").and_then(NamedValue::number), Some(0.0), "{kind:?}");
        }
        let older = EntityRef::new(EntityKind::Vertex, vertex.slot, vertex.generation + 1);
        assert_eq!(
            resolve(&at, &Operand::Entity(older), &Labels::default(), &snapshot).unwrap_err().code,
            Code::StaleId
        );
    }
}
