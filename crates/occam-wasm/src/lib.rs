//! Source: original
//! The browser build: a thin facade over the `occam` crate. A JavaScript caller passes numbers, in millimetres and radians, and
//! requests as JSON text, and gets the one JSON form of the result or the report (L8, L25).

use occam::{Answered, DistanceQuery, Encoding, EntityKind, EntityRef, Exec, Pick, distance_json, ran_json};
use wasm_bindgen::prelude::wasm_bindgen;

/// A model in the browser, as the editing worker keeps it (P10). Its requests run as the command line's do: one that goes
/// through and changes the model is kept, and its native bytes, the binary twin, wait to be published; one that stops
/// short leaves the model as it was. A model it opens is measured and checked whole, and its check is the one the mesh
/// workers trust ([`Trusted`]).
#[wasm_bindgen]
pub struct Session {
    session: occam::Session,
    saved: Option<Vec<u8>>,
}

impl Default for Session {
    fn default() -> Self {
        Self::new()
    }
}

#[wasm_bindgen]
impl Session {
    /// A session with no model yet: its first request makes one.
    #[wasm_bindgen(constructor)]
    #[must_use]
    pub fn new() -> Self {
        Self { session: occam::Session::new(Encoding::Binary), saved: None }
    }

    /// The model `native` holds, in either encoding, measured and checked whole.
    ///
    /// # Errors
    ///
    /// The reports refusing it, in the commands' one JSON form.
    pub fn open(native: &[u8]) -> Result<Self, String> {
        let mut session = occam::Session::open(native, &Exec::serial()).map_err(|r| Answered::Refused(r).json())?;
        session.save_as(Encoding::Binary);
        Ok(Self { session, saved: None })
    }

    /// Runs `request`, written as JSON, and gives the run's one JSON form.
    #[must_use]
    pub fn run(&mut self, request: &str) -> String {
        let (ran, saved) = self.session.run(request);
        self.saved = saved;
        ran_json(&ran)
    }

    /// The model's native bytes, the binary twin, if the last run changed it, once: what the editing worker publishes.
    #[must_use]
    pub fn take_saved(&mut self) -> Option<Vec<u8>> {
        self.saved.take()
    }

    /// The model meshed to a chordal `deflection` in millimetres and an `angle` in radians, as `occam mesh` meshes it.
    /// After a run, at the same limits, only the faces the runs since the last mesh touched, and those along an edge
    /// whose samples changed, are meshed again; the rest keep their blocks bit for bit.
    ///
    /// # Errors
    ///
    /// The reports refusing the limits, in the commands' one JSON form.
    pub fn mesh(&mut self, deflection: f64, angle: f64) -> Result<Meshed, String> {
        let meshed = self.session.mesh(&Exec::serial(), deflection, angle).map_err(|r| Answered::Refused(r).json())?;
        Ok(Meshed { meshed })
    }

    /// How many faces the model has: what the page deals out to its mesh workers, in ranges.
    #[must_use]
    pub fn face_count(&self) -> u32 {
        u32::try_from(self.session.face_count()).unwrap_or(u32::MAX)
    }

    /// Each face's id, in id order, as two numbers, its slot and generation: what the page names the faces of a range
    /// by when a mesh worker could not mesh it.
    #[must_use]
    pub fn face_ids(&self) -> Vec<u32> {
        let listed = self.session.list(&[EntityKind::Face]);
        listed.iter().flat_map(|kind| kind.entities.iter().flat_map(|id| [id.slot, id.generation])).collect()
    }

    /// What a page that drew `before`'s model, meshed to a chordal `deflection` in millimetres and an `angle` in radians,
    /// must mesh again to draw this one: the editing worker asks it of each model `occam view` sends, so the mesh
    /// workers mesh only the faces that changed. Its one JSON form gives the faces' places, counted in id order as
    /// [`Trusted::mesh_part`] counts them, and the ids of what keeps its mesh.
    ///
    /// # Errors
    ///
    /// The reports refusing the limits, in the commands' one JSON form.
    pub fn again(&self, before: &Self, deflection: f64, angle: f64) -> Result<String, String> {
        let again = self.session.again(&before.session, &Exec::serial(), deflection, angle);
        again.map(|again| again.json()).map_err(|r| Answered::Refused(r).json())
    }
}

/// A model as a mesh worker holds it: the binary twin the editing worker published, which it checked when it opened or
/// made the model, read again without being measured or checked
/// (docs/decisions/2026-11-16-mesh-workers-trust-the-editing-worker.md). It meshes ranges of faces and picks, and
/// nothing else: it runs no request and gives no bytes, so what it holds never leaves it as a model.
#[wasm_bindgen]
pub struct Trusted {
    session: occam::Session,
}

#[wasm_bindgen]
impl Trusted {
    /// The model `twin` holds, in either encoding, read but neither measured nor checked: only for a copy of the twin the
    /// editing worker published.
    ///
    /// # Errors
    ///
    /// The reports refusing bytes that do not read as a model, in the commands' one JSON form.
    pub fn open(twin: &[u8]) -> Result<Self, String> {
        let session = occam::Session::open_trusted(twin).map_err(|r| Answered::Refused(r).json())?;
        Ok(Self { session })
    }

    /// The model's faces from `first` up to `end`, counted in id order, meshed to a chordal `deflection` in millimetres
    /// and an `angle` in radians: what a mesh worker gives for its range. Its arrays are numbered from its own start,
    /// and each face's block holds the bits [`Session::mesh`] gives it, so the parts of the model's faces, laid end to
    /// end in id order, are the whole mesh's arrays. A range past the last face stops at it.
    ///
    /// # Errors
    ///
    /// The reports refusing the limits, in the commands' one JSON form.
    pub fn mesh_part(&self, deflection: f64, angle: f64, first: u32, end: u32) -> Result<Meshed, String> {
        let at = |n: u32| usize::try_from(n).unwrap_or(usize::MAX);
        let part = self.session.mesh_part(&Exec::serial(), deflection, angle, at(first)..at(end));
        Ok(Meshed { meshed: part.map_err(|r| Answered::Refused(r).json())? })
    }

    /// What a ray hits first in the model's mesh made to a chordal `deflection` in millimetres and an `angle` in
    /// radians, as `occam pick --json` gives it: `ray` is six numbers, where it starts and its direction, in
    /// millimetres in the model's frame, and an edge is hit within `tolerance` millimetres of it. The mesh is the one
    /// [`Session::mesh`] gives, made the first time and kept for the next pick at the same limits, so a mesh worker
    /// picks against the CPU mesh, never the GPU's.
    ///
    /// # Errors
    ///
    /// The reports refusing the limits, the ray or the tolerance, in the commands' one JSON form.
    pub fn pick(&mut self, deflection: f64, angle: f64, ray: &[f64], tolerance: f64) -> Result<String, String> {
        let at = |k: usize| ray.get(k).copied().unwrap_or(f64::NAN);
        let asked = Pick { from: [at(0), at(1), at(2)], direction: [at(3), at(4), at(5)], tolerance };
        let picked = self.session.pick(&Exec::serial(), deflection, angle, asked);
        picked.map(|picked| picked.json()).map_err(|r| Answered::Refused(r).json())
    }

    /// The samples of the edge `edge`, written as its id, in the mesh [`Trusted::pick`] picks against: three numbers a
    /// point, in millimetres, what the page draws a selected or highlighted edge through. None for an id that is not
    /// an edge a face uses.
    ///
    /// # Errors
    ///
    /// The reports refusing the limits, in the commands' one JSON form.
    pub fn samples(&mut self, deflection: f64, angle: f64, edge: &str) -> Result<Vec<f64>, String> {
        let Some(edge) = EntityRef::parse(edge).filter(|id| id.kind == EntityKind::Edge) else { return Ok(Vec::new()) };
        let samples = self.session.samples(&Exec::serial(), deflection, angle, edge);
        let samples = samples.map_err(|r| Answered::Refused(r).json())?;
        Ok(samples.unwrap_or_default().into_iter().flatten().collect())
    }
}

/// A model's mesh as the renderer contract wants it: flat arrays, each face a vertex block whose indices count from its
/// start, and the summary `occam mesh --json` gives.
#[wasm_bindgen]
pub struct Meshed {
    meshed: occam::Meshed,
}

#[wasm_bindgen]
impl Meshed {
    /// One answer per face, its vertex block and its range of triangles or the report refusing it, and the reports
    /// refusing edges and what the watertight check found, in the one JSON form `occam mesh --json` gives.
    #[must_use]
    pub fn summary(&self) -> String {
        self.meshed.json(None)
    }

    /// Each vertex's position, three numbers each, in millimetres.
    #[must_use]
    pub fn positions(&self) -> Vec<f64> {
        self.meshed.buffers.positions.clone()
    }

    /// Each vertex's unit normal, out of its face, three numbers each.
    #[must_use]
    pub fn normals(&self) -> Vec<f64> {
        self.meshed.buffers.normals.clone()
    }

    /// Each vertex's place in its face's parameters, two numbers each.
    #[must_use]
    pub fn uvs(&self) -> Vec<f64> {
        self.meshed.buffers.uvs.clone()
    }

    /// Each triangle's corners, three each, counted from the start of its face's vertex block.
    #[must_use]
    pub fn indices(&self) -> Vec<u32> {
        self.meshed.buffers.indices.clone()
    }

    /// Each face that meshed, in id order, as six numbers: its slot and generation, its first vertex and how many, and
    /// its first triangle and how many, as the table that ends `occam mesh`'s file.
    #[must_use]
    pub fn faces(&self) -> Vec<u32> {
        self.meshed.buffers.table()
    }
}

/// The distance between two points, as the JSON form of its result or of the report refusing it. Everything is in millimetres
/// and radians.
#[wasm_bindgen]
#[must_use]
#[expect(clippy::too_many_arguments, reason = "JavaScript passes flat numbers, so the boundary stays free of objects")]
pub fn distance(
    from_x: f64,
    from_y: f64,
    from_z: f64,
    to_x: f64,
    to_y: f64,
    to_z: f64,
    model_length_tolerance: f64,
    model_angle_tolerance: f64,
    tolerance: f64,
) -> String {
    let query = DistanceQuery {
        from: [from_x, from_y, from_z],
        to: [to_x, to_y, to_z],
        model_length_tolerance,
        model_angle_tolerance,
        tolerance,
    };
    distance_json(&query).json
}

#[cfg(test)]
mod tests {

    use occam::{Code, Report};

    use super::*;

    const BOX: &str =
        r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [{"op": "box", "size": [1, 2, 3]}]}"#;

    #[test]
    fn a_run_that_changes_the_model_gives_its_binary_twin_once() {
        let mut session = Session::new();
        let ran = session.run(BOX);
        assert!(ran.starts_with(r#"{"schema":2,"ok":true,"#), "{ran}");
        let saved = session.take_saved().unwrap();
        assert!(saved.starts_with(b"\x89occam"), "the binary twin");
        assert_eq!(session.take_saved(), None, "once");
        let measured = session.run(
            r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
            {"op": "measure", "target": "body:0/1"}]}"#,
        );
        assert!(measured.contains(r#""measured":{"target":"body:0/1","values":[{"name":"volume""#), "{measured}");
        assert_eq!(session.take_saved(), None, "a measure changes nothing");
    }

    #[test]
    fn a_saved_model_opens_again_and_bytes_that_are_not_one_are_refused() {
        let mut first = Session::new();
        let _ = first.run(BOX);
        let mut opened = Session::open(&first.take_saved().unwrap()).unwrap();
        let again = opened.run(
            r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
            {"op": "copy", "target": "body:0/1", "frame": {"origin": [5, 0, 0]}}]}"#,
        );
        assert!(again.contains(r#""made":"body:1/1""#), "{again}");
        let refused = Session::open(b"not a model").err().unwrap();
        let value: serde_json::Value = serde_json::from_str(&refused).unwrap();
        assert_eq!(value["reports"][0]["code"], "io.not-readable");
    }

    #[test]
    fn a_run_that_stops_short_saves_nothing() {
        let mut session = Session::new();
        let ran = session.run(
            r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
            {"op": "sphere", "radius": -1}]}"#,
        );
        assert!(ran.starts_with(r#"{"schema":2,"ok":false,"#), "{ran}");
        assert_eq!(session.take_saved(), None);
    }

    /// The numbers `meshed`'s summary gives for face `k`: its first vertex and how many, its first triangle and how many.
    fn ranges(summary: &serde_json::Value, k: usize) -> [u64; 4] {
        let mesh = &summary["answers"][k]["mesh"];
        let at = |name: &str, field: &str| mesh[name][field].as_u64().unwrap();
        [at("vertices", "first"), at("vertices", "count"), at("triangles", "first"), at("triangles", "count")]
    }

    #[test]
    fn a_session_meshes_into_flat_arrays_and_after_a_run_keeps_what_it_did_not_touch() {
        let mut session = Session::new();
        let _ = session.run(BOX);
        let first = session.mesh(0.1, 0.35).unwrap();
        let summary: serde_json::Value = serde_json::from_str(&first.summary()).unwrap();
        assert_eq!((&summary["ok"], &summary["vertices"]), (&serde_json::json!(true), &serde_json::json!(24)));
        assert_eq!((first.positions().len(), first.normals().len(), first.uvs().len()), (72, 72, 48));
        assert_eq!(first.indices().len(), 36);
        let faces = first.faces();
        assert_eq!(faces.len(), 6 * 6, "six numbers a face");
        for (k, row) in faces.chunks(6).enumerate() {
            assert_eq!(row[..2], [u32::try_from(k).unwrap(), 1], "face:{k}/1");
            assert_eq!(row[2..].iter().map(|&n| u64::from(n)).collect::<Vec<_>>(), ranges(&summary, k));
        }
        let _ = session.run(
            r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
            {"op": "sphere", "radius": 2, "frame": {"origin": [20, 0, 0]}}]}"#,
        );
        let second = session.mesh(0.1, 0.35).unwrap();
        let summary: serde_json::Value = serde_json::from_str(&second.summary()).unwrap();
        let kept: Vec<bool> =
            summary["answers"].as_array().unwrap().iter().map(|a| a["mesh"]["kept"].as_bool().unwrap()).collect();
        assert_eq!(kept[..6], [true; 6], "the box's faces kept");
        assert!(kept.len() > 6 && kept[6..].iter().all(|kept| !kept), "the sphere's meshed: {kept:?}");
        assert_eq!(second.positions()[..72], first.positions()[..], "the box's blocks bit for bit");
        assert_eq!(second.indices()[..36], first.indices()[..]);
        let refused = serde_json::from_str::<serde_json::Value>(&session.mesh(-1.0, 0.35).err().unwrap()).unwrap();
        assert_eq!(refused["reports"][0]["code"], "mesh.limit-not-positive");
    }

    #[test]
    fn a_mesh_worker_meshes_ranges_of_the_faces_whose_arrays_laid_end_to_end_are_the_whole_mesh() {
        let mut session = Session::new();
        let _ = session.run(BOX);
        let _ = session.run(
            r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
            {"op": "cylinder", "radius": 1, "height": 2, "frame": {"origin": [5, 0, 0]}}]}"#,
        );
        assert_eq!(session.face_count(), 10);
        let ids: Vec<u32> = (0..10).flat_map(|slot| [slot, 1]).collect();
        assert_eq!(session.face_ids(), ids, "each face's slot and generation, in id order");
        let worker = Trusted::open(&session.take_saved().unwrap()).unwrap();
        let parts: Vec<Meshed> =
            [(0, 4), (4, 9), (9, 64)].map(|(first, end)| worker.mesh_part(0.1, 0.35, first, end).unwrap()).into();
        let whole = session.mesh(0.1, 0.35).unwrap();
        let joined = |of: fn(&Meshed) -> Vec<f64>| parts.iter().flat_map(of).map(f64::to_bits).collect::<Vec<_>>();
        let bits = |numbers: Vec<f64>| numbers.into_iter().map(f64::to_bits).collect::<Vec<_>>();
        assert_eq!(joined(Meshed::positions), bits(whole.positions()));
        assert_eq!(joined(Meshed::normals), bits(whole.normals()));
        assert_eq!(joined(Meshed::uvs), bits(whole.uvs()));
        assert_eq!(parts.iter().flat_map(Meshed::indices).collect::<Vec<_>>(), whole.indices());
        let slots: Vec<u32> =
            parts.iter().flat_map(|p| p.faces().chunks(6).map(|row| row[0]).collect::<Vec<_>>()).collect();
        assert_eq!(slots, (0..10).collect::<Vec<_>>(), "each face once, in id order");
        assert_eq!(parts[1].faces()[2], 0, "each part numbered from its own start");
        let refused =
            serde_json::from_str::<serde_json::Value>(&worker.mesh_part(0.1, -1.0, 0, 1).err().unwrap()).unwrap();
        assert_eq!(refused["reports"][0]["code"], "mesh.limit-not-positive");
    }

    #[test]
    fn a_model_opened_after_another_names_the_faces_to_mesh_again_and_what_keeps_its_mesh() {
        let mut first = Session::new();
        let _ = first.run(BOX);
        let mut grown = Session::open(&first.take_saved().unwrap()).unwrap();
        let _ = grown.run(
            r#"{"schema": 1, "units": {"length": "mm", "angle": "deg"}, "steps": [
            {"op": "sphere", "radius": 1, "frame": {"origin": [5, 0, 0]}}]}"#,
        );
        let opened = Session::open(&grown.take_saved().unwrap()).unwrap();
        let again: serde_json::Value = serde_json::from_str(&opened.again(&first, 0.1, 0.35).unwrap()).unwrap();
        assert_eq!(again["faces"], serde_json::json!([6, 7]), "the sphere's two faces alone");
        assert_eq!(again["kept"][0], "face:0/1");
        assert_eq!(again["kept"].as_array().map(Vec::len), Some(6 + 12), "the box's faces and edges");
        let refused =
            serde_json::from_str::<serde_json::Value>(&opened.again(&first, 0.1, 0.0).err().unwrap()).unwrap();
        assert_eq!(refused["reports"][0]["code"], "mesh.limit-not-positive");
    }

    #[test]
    fn a_mesh_worker_picks_what_a_ray_hits_first_and_gives_an_edges_samples() {
        let mut editing = Session::new();
        let _ = editing.run(BOX);
        let mut session = Trusted::open(&editing.take_saved().unwrap()).unwrap();
        let picked: serde_json::Value =
            serde_json::from_str(&session.pick(0.1, 0.35, &[0.5, 1.0, 9.0, 0.0, 0.0, -1.0], 0.0).unwrap()).unwrap();
        assert_eq!((&picked["ok"], &picked["point"]), (&serde_json::json!(true), &serde_json::json!([0.5, 1.0, 3.0])));
        assert!(picked["entity"].as_str().unwrap().starts_with("face:"), "{picked}");
        let edge: serde_json::Value =
            serde_json::from_str(&session.pick(0.1, 0.35, &[0.5, 0.01, 9.0, 0.0, 0.0, -1.0], 0.05).unwrap()).unwrap();
        let id = edge["entity"].as_str().unwrap();
        assert!(id.starts_with("edge:"), "{edge}");
        let samples = session.samples(0.1, 0.35, id).unwrap();
        assert!(samples.len() >= 6 && samples.chunks(3).all(|p| p[1..] == [0.0, 3.0]), "{samples:?}");
        assert_eq!(session.samples(0.1, 0.35, "face:0/1").unwrap(), Vec::<f64>::new(), "a face has none");
        let missed: serde_json::Value =
            serde_json::from_str(&session.pick(0.1, 0.35, &[0.5, 1.0, 9.0, 0.0, 0.0, 1.0], 0.0).unwrap()).unwrap();
        assert_eq!(
            (&missed["entity"], &missed["reports"][0]["code"]),
            (&serde_json::Value::Null, &serde_json::json!("mesh.ray-missed"))
        );
        let refused = session.pick(0.1, 0.35, &[0.0; 3], 0.0).err().unwrap();
        let refused: serde_json::Value = serde_json::from_str(&refused).unwrap();
        assert_eq!(refused["reports"][0]["code"], "input.no-direction", "a ray of three numbers has no direction");
    }

    /// The box saved in the readable JSON, as `occam run` would write it, and changed by `change`.
    fn box_changed(change: impl FnOnce(&mut serde_json::Value)) -> Vec<u8> {
        let (_, saved) = occam::Session::new(Encoding::Json).run(BOX);
        let mut file: serde_json::Value = serde_json::from_slice(&saved.unwrap()).unwrap();
        change(&mut file);
        serde_json::to_vec(&file).unwrap()
    }

    /// The code of the first report `refused` gives.
    fn code(refused: &str) -> String {
        let refused: serde_json::Value = serde_json::from_str(refused).unwrap();
        refused["reports"][0]["code"].as_str().unwrap().to_string()
    }

    #[test]
    fn a_mesh_workers_open_measures_and_checks_nothing_but_refuses_bytes_that_do_not_read_as_a_model() {
        let moved = box_changed(|file| file["vertices"][0]["record"]["point"][2] = serde_json::json!(5));
        assert_eq!(code(&Session::open(&moved).err().unwrap()), "check.invariant-failed", "a corner off its edges");
        assert!(Trusted::open(&moved).is_ok(), "nothing checked");
        let written = box_changed(|file| file["edges"][0]["record"]["deviation"] = serde_json::json!(1e-9));
        let checked = occam::Session::open(&written, &Exec::serial()).unwrap();
        let (_, saved) = occam::Session::new(Encoding::Json).run(BOX);
        let digest = occam::Session::open(&saved.unwrap(), &Exec::serial()).unwrap().digest();
        assert_eq!(checked.digest(), digest, "the editing worker's open measures the edge again");
        assert_ne!(
            Trusted::open(&written).unwrap().session.digest(),
            digest,
            "nothing measured: the deviation as written"
        );
        let mut damaged = {
            let mut editing = Session::new();
            let _ = editing.run(BOX);
            editing.take_saved().unwrap()
        };
        let middle = damaged.len() / 2;
        damaged[middle] ^= 0xff;
        for (bytes, refused) in [(&b"not a model"[..], "io.not-readable"), (&damaged[..], "io.damaged")] {
            assert_eq!(code(&Trusted::open(bytes).err().unwrap()), refused);
        }
    }

    #[test]
    fn the_twin_the_editing_worker_checked_opens_in_a_mesh_worker_as_the_same_model() {
        let mut editing = Session::new();
        let ran = editing.run(occam::operation_workloads::TRANSFORMS);
        assert!(ran.contains(r#""ok":true"#), "{ran}");
        let twin = editing.take_saved().unwrap();
        let checked = occam::Session::open(&twin, &Exec::serial()).unwrap();
        let worker = Trusted::open(&twin).unwrap();
        assert!(checked.face_count() > 10, "a box, its copy and two cylinders, one with B-spline sides");
        assert_eq!(worker.session.digest(), editing.session.digest(), "the editing worker's model");
        assert_eq!(worker.session.digest(), checked.digest(), "and the one a checked open gives, so picks name it");
    }

    #[test]
    fn every_number_reaches_the_query() {
        let got = distance(1.0, 2.0, 3.0, 4.0, 6.0, 3.0, 0.001, 0.00001, 0.01);
        assert!(got.starts_with(r#"{"schema":2,"value":5.0,"#), "{got}");
    }

    #[test]
    fn a_refusal_is_the_reports_json() {
        let got = distance(0.0, 0.0, 0.0, 3.0, 4.0, 0.0, 0.001, 0.00001, 0.0001);
        assert_eq!(Report::from_json(&got).map(|report| report.code), Ok(Code::ToleranceBelowModel));
    }

    #[test]
    fn the_models_tolerances_are_kept_apart() {
        let got = distance(0.0, 0.0, 0.0, 3.0, 4.0, 0.0, 0.001, 0.0, 0.01);
        assert_eq!(Report::from_json(&got).map(|report| report.code), Ok(Code::ToleranceOutOfRange));
        let got = distance(0.0, 0.0, 0.0, 3.0, 4.0, 0.0, 0.0, 0.00001, 0.01);
        assert_eq!(Report::from_json(&got).map(|report| report.code), Ok(Code::ToleranceOutOfRange));
    }
}
