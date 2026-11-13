//! Source: original
//! What an entity is in the caller's terms (S6, O26): a form, named values, the step that made it and its neighbours.
//! One resolver, `occam`'s describer, fills these in as a result leaves the kernel, never a failure site, so no site can
//! forget to. A description depends on nothing of the kernel's: its form is a name and the rest are named values and
//! ids, so a new geometry family edits nothing here.

use serde::{Deserialize, Serialize};

use crate::{DisplayUnits, EntityRef, NamedValue, StepRole};

/// Which step made an entity, and its part in it (S6: what agents asked about first).
#[derive(Clone, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub struct Origin {
    /// The model's step that made it, counted over the model's whole life, so unique across requests.
    pub step: u64,
    /// That step's index in the request being answered, if that request made it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub request_step: Option<u32>,
    /// What that step asked for, as a request names it, if the request being answered made it.
    #[serde(default, skip_serializing_if = "Option::is_none")]
    pub operation: Option<String>,
    /// Its part in that step.
    pub role: StepRole,
}

/// What an entity is in the caller's terms (S6, O26).
#[derive(Clone, Debug, PartialEq, Deserialize)]
pub struct Description {
    /// The entity described.
    pub entity: EntityRef,
    /// What it is: its geometry's form for a face or an edge (`plane`, `line`), as the geometry names it; what a body
    /// is (`solid`, `sheet`, `wire`); the kind otherwise; and [`Description::NONE`] for an id that names nothing.
    pub form: String,
    /// Its geometry's parameters, where it is and how far it reaches, and what it is made of, named.
    pub values: Vec<NamedValue>,
    /// The step that made it, for the entities that keep one.
    #[serde(default)]
    pub origin: Option<Origin>,
    /// Its neighbours, one link up and one down, and for a face its edges and for an edge its faces (PARITY 1.1).
    pub adjacent: Vec<EntityRef>,
}

impl Description {
    /// The form of an id that names nothing live where the result was made: removed, never written, or of a slot used
    /// again since.
    pub const NONE: &'static str = "none";

    /// The description of `entity`, which names nothing.
    #[must_use]
    pub fn none(entity: EntityRef) -> Self {
        Self { entity, form: Self::NONE.to_string(), values: Vec::new(), origin: None, adjacent: Vec::new() }
    }

    /// The value of this name.
    #[must_use]
    pub fn value(&self, name: &str) -> Option<&NamedValue> {
        self.values.iter().find(|value| value.name == name)
    }

    /// The description as it is written, each value shown in `units` as well as given in occam's own.
    #[must_use]
    pub fn shown(&self, units: DisplayUnits) -> Shown<'_> {
        Shown {
            entity: self.entity,
            form: &self.form,
            values: self.values.iter().map(|value| ValueShown::new(value, units)).collect(),
            origin: self.origin.as_ref(),
            adjacent: &self.adjacent,
        }
    }
}

/// A description as it is written. Its numbers are in millimetres and radians, as every result gives them, and each
/// value is shown in the caller's units too, as a report's wording is (L9).
#[derive(Debug, Serialize)]
pub struct Shown<'a> {
    entity: EntityRef,
    form: &'a str,
    values: Vec<ValueShown<'a>>,
    #[serde(skip_serializing_if = "Option::is_none")]
    origin: Option<&'a Origin>,
    adjacent: &'a [EntityRef],
}

/// A value as a result writes it: in occam's own units, and shown in the caller's (L9).
#[derive(Debug, Serialize)]
pub struct ValueShown<'a> {
    #[serde(flatten)]
    value: &'a NamedValue,
    shown: String,
}

impl<'a> ValueShown<'a> {
    /// `value` as it is written, shown in `units`.
    #[must_use]
    pub fn new(value: &'a NamedValue, units: DisplayUnits) -> Self {
        Self { value, shown: value.shown(units) }
    }
}

#[cfg(test)]
mod tests {
    use serde_json::json;

    use super::*;
    use crate::{AngleUnit, EntityKind, LengthUnit, Quantity};

    fn edge() -> Description {
        let origin =
            Origin { step: 4, request_step: Some(1), operation: Some("wire".to_string()), role: StepRole::Made };
        Description {
            entity: EntityRef::new(EntityKind::Edge, 3, 1),
            form: "line".to_string(),
            values: vec![NamedValue::point("start", [25.4, 0.0, 0.0]), NamedValue::new("turn", Quantity::Angle, 0.5)],
            origin: Some(origin),
            adjacent: vec![EntityRef::new(EntityKind::Vertex, 0, 1)],
        }
    }

    #[test]
    fn a_description_is_written_with_each_value_shown_in_the_callers_units() {
        let inches = DisplayUnits { length: LengthUnit::Inch, angle: AngleUnit::Radian };
        let value = serde_json::to_value(edge().shown(inches)).unwrap();
        let want = json!({
            "entity": "edge:3/1",
            "form": "line",
            "values": [
                {"name": "start", "quantity": "length", "value": [25.4, 0.0, 0.0], "shown": "(1, 0, 0) in"},
                {"name": "turn", "quantity": "angle", "value": 0.5, "shown": "0.5 rad"},
            ],
            "origin": {"step": 4, "request-step": 1, "operation": "wire", "role": "made"},
            "adjacent": ["vertex:0/1"],
        });
        assert_eq!(value, want);
    }

    #[test]
    fn a_description_is_read_back_without_what_is_shown() {
        let text = serde_json::to_string(&edge().shown(DisplayUnits::default())).unwrap();
        assert_eq!(serde_json::from_str::<Description>(&text).unwrap(), edge());
        let older = Origin { step: 2, request_step: None, operation: None, role: StepRole::Profile };
        let text = serde_json::to_string(&older).unwrap();
        assert_eq!(
            text, r#"{"step":2,"role":"profile"}"#,
            "a step of an earlier request has no index or operation here"
        );
        assert_eq!(serde_json::from_str::<Origin>(&text).unwrap(), older);
    }

    #[test]
    fn an_id_that_names_nothing_is_described_as_none() {
        let entity = EntityRef::new(EntityKind::Face, 9, 2);
        let none = Description::none(entity);
        let parts = (none.entity, none.form.as_str(), none.values.len(), none.origin.clone(), none.adjacent.len());
        assert_eq!(parts, (entity, "none", 0, None, 0));
        let value = serde_json::to_value(none.shown(DisplayUnits::default())).unwrap();
        assert_eq!(value, json!({"entity": "face:9/2", "form": "none", "values": [], "adjacent": []}));
        assert_eq!(edge().value("turn").and_then(NamedValue::number), Some(0.5));
        assert_eq!(edge().value("absent"), None);
    }
}
