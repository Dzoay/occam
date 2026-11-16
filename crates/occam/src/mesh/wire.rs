//! Source: original
//! A mesh's one JSON form: whether every face meshed whole, the limits it was made to, how many vertices and triangles
//! the buffers hold, one answer per face, its vertex block and its range of triangles or the report refusing it, and
//! the reports refusing edges and what the watertight check found. In words, a line says what was meshed, and each
//! refusal is a line of its own, for standard error. Numbers are in millimetres and radians.

use std::ops::Range;

use occam_report::{DisplayUnits, NamedValue, Quantity, SCHEMA};
use serde::Serialize;
use serde_json::{Value, json};

use super::{Again, Meshed, Placed};
use crate::run::report_value;
use crate::session::{answer, line};

#[derive(Serialize)]
struct MeshedWire<'a> {
    schema: u32,
    ok: bool,
    #[serde(skip_serializing_if = "Option::is_none")]
    file: Option<&'a str>,
    limits: LimitsWire,
    vertices: usize,
    triangles: usize,
    answers: Vec<Value>,
    reports: Vec<Value>,
}

#[derive(Serialize)]
struct LimitsWire {
    deflection: f64,
    angle: f64,
}

impl Again {
    /// Its one JSON form: the places of the faces to mesh again, and the ids of what keeps its mesh.
    #[must_use]
    pub fn json(&self) -> String {
        json!({"schema": SCHEMA, "faces": self.faces, "kept": self.kept}).to_string()
    }
}

impl Meshed {
    /// The mesh's one JSON form, naming `file` where the buffers were written, if they were.
    #[must_use]
    pub fn json(&self, file: Option<&str>) -> String {
        let units = DisplayUnits::default();
        let wire = MeshedWire {
            schema: SCHEMA,
            ok: self.ok(),
            file,
            limits: LimitsWire {
                deflection: self.limits.deflection().millimetres(),
                angle: self.limits.angle().radians(),
            },
            vertices: self.buffers.vertices(),
            triangles: self.buffers.triangles(),
            answers: self.faces.iter().map(|(id, placed)| answer(*id, placed, "mesh", self::placed)).collect(),
            reports: self.reports.iter().map(|report| report_value(report, units)).collect(),
        };
        serde_json::to_string(&wire).unwrap_or_default()
    }

    /// The mesh in words, a line each, with whether it went well: what was meshed, then each face refused and each
    /// report.
    #[must_use]
    pub fn words(&self) -> Vec<(bool, String)> {
        let units = DisplayUnits::default();
        let deflection = NamedValue::length("deflection", self.limits.deflection().millimetres()).shown(units);
        let angle = NamedValue::new("angle", Quantity::Angle, self.limits.angle().radians()).shown(units);
        let meshed = self.faces.iter().filter(|(_, placed)| placed.is_ok()).count();
        let (vertices, triangles) = (self.buffers.vertices(), self.buffers.triangles());
        let head = format!(
            "Meshed {meshed} of {} faces to a deflection of {deflection} and an angle of {angle}: {vertices} vertices \
             and {triangles} triangles.",
            self.faces.len()
        );
        let refused = self.faces.iter().filter(|(_, placed)| placed.is_err()).map(|each| line(each, |_| String::new()));
        let reports = self.reports.iter().map(|report| (false, report.message(units)));
        std::iter::once((true, head)).chain(refused).chain(reports).collect()
    }
}

/// A face's place in the buffers as a JSON value: its vertex block and its triangles, each as where it starts and how
/// many, whether it was capped, and whether it was kept from the session's last mesh.
fn placed(placed: &Placed) -> Value {
    let range = |r: &Range<u32>| json!({"first": r.start, "count": r.end.saturating_sub(r.start)});
    json!({
        "vertices": range(&placed.vertices),
        "triangles": range(&placed.triangles),
        "capped": placed.capped,
        "kept": placed.kept
    })
}
