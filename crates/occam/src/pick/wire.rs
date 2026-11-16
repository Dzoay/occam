//! Source: original
//! A pick's one JSON form: whether the ray hit something, the model's digest, the ray as asked with its direction of
//! length one, the tolerance, the id hit, the point hit and how far along the ray, or `null` and the report saying
//! why nothing was, and the mesh's reports. In words, a line says what was hit, and each report is a line of its own,
//! for standard error. Numbers are in millimetres.

use occam_report::{DisplayUnits, NamedValue, SCHEMA};
use serde_json::{Value, json};

use super::Picked;
use crate::run::report_value;
use crate::view::digest_text;

impl Picked {
    /// Its one JSON form.
    #[must_use]
    pub fn json(&self) -> String {
        let units = DisplayUnits::default();
        let missed = self.hit.as_ref().err();
        let reports: Vec<Value> = missed.into_iter().chain(&self.reports).map(|r| report_value(r, units)).collect();
        let mut answer = json!({
            "schema": SCHEMA,
            "ok": self.ok(),
            "digest": self.digest.map(digest_text),
            "from": self.asked.from,
            "direction": self.asked.direction,
            "tolerance": self.asked.tolerance,
            "entity": Value::Null,
        });
        if let (Ok(hit), Some(fields)) = (&self.hit, answer.as_object_mut()) {
            fields.insert("entity".into(), json!(hit.entity));
            fields.insert("point".into(), json!(hit.point));
            fields.insert("distance".into(), json!(hit.distance));
        }
        if let Some(fields) = answer.as_object_mut() {
            fields.insert("reports".into(), reports.into());
        }
        answer.to_string()
    }

    /// It in words, a line each, with whether each goes to standard output: what the ray hit, or why it hit nothing,
    /// then each of the mesh's reports.
    #[must_use]
    pub fn words(&self) -> Vec<(bool, String)> {
        let units = DisplayUnits::default();
        let head = match &self.hit {
            Ok(hit) => {
                let point = NamedValue::point("point", hit.point).shown(units);
                let along = NamedValue::length("distance", hit.distance).shown(units);
                (true, format!("The ray hits {} at {point}, {along} along it.", hit.entity.text()))
            }
            Err(report) => (false, report.message(units)),
        };
        let reports = self.reports.iter().map(|report| (false, report.message(units)));
        std::iter::once(head).chain(reports).collect()
    }
}
