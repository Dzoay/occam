//! Source: original
//! The native format, which is also the readable dump (PARITY 2.17): a model as JSON, every record by name and every
//! finite number exact, versioned from the first commit. It holds the tolerance, the step counter and every slot of
//! every table, its generation and, if live, its record, so a model read back has the same ids, reuses the same free
//! slots and has the same digest. Bytes at the seam, not text.
//!
//! The same file has a binary twin, for loading fast (docs/decisions/2026-11-12-binary-twin.md): [`save_binary`] writes
//! [`MAGIC`], the version as four bytes, little-endian, then the same records in `postcard`'s encoding, links as their
//! kind, slot and generation and numbers as their eight bytes, and last the CRC-32 of everything before it, four bytes
//! little-endian (docs/decisions/2026-11-13-binary-twin-checksum.md). One version covers both, and [`parse`] reads
//! either, telling them apart by the magic, which no JSON text begins with.
//!
//! Reading trusts nothing. [`parse`] reads the bytes, and refuses a twin whose checksum does not match before it reads
//! its records, what is not the format where it is written, and a curve or surface its family would not make, naming
//! it; then `Restore::finish` measures every edge and checks the whole model, as `commit` would. [`load`] is the two in
//! turn, which the `construction` exit times apart.

mod crc;
mod exact;
mod geometry;
mod link;
mod records;

use core::num::NonZeroU32;

use occam_math::Exec;
use occam_report::{Code, EntityRef, Fault, NamedValue, Quantity, Report};
use occam_topo::{
    Body, BodyRecord, CurveGeom, CurveRecord, Edge, EdgeRecord, EdgeUse, EdgeUseRecord, Face, FaceRecord, Kind, Loop,
    LoopRecord, ParamCurveGeom, ParamCurveRecord, Restore, Saved, Shell, ShellRecord, Snapshot, SurfaceGeom,
    SurfaceRecord, Vertex, VertexRecord, Wire, WireRecord,
};
use occam_units::{Angle, Length, Tolerance};
use serde::ser::SerializeStruct;
use serde::{Deserialize, Serialize, Serializer};

use exact::Exact;
use geometry::{CurveFamily, Geometry, SurfaceFamily};
use records::{BodyFile, EdgeFile, FaceFile, LoopFile, NoteFile, ShellFile, UseFile, VertexFile, WireFile};

/// What a native file says it is.
pub const FORMAT: &str = "occam-native";

/// The version of the format this build writes and reads, in either encoding.
pub const VERSION: u32 = 1;

/// What a binary file begins with, before its version: a byte no text begins with, the format's short name, and a line
/// end of each kind and an end-of-file mark, so a file mangled as text in transit is told from one never written.
pub const MAGIC: &[u8; 10] = b"\x89occam\r\n\x1a\n";

/// One slot as written: the generation of its latest entity, and that entity's record if it is live.
#[derive(Clone, Debug, Deserialize)]
#[serde(deny_unknown_fields)]
struct Slot<R> {
    generation: NonZeroU32,
    record: Option<R>,
}

/// The readable file leaves out a free slot's record; the binary twin, whose fields have no names, writes every one.
impl<R: Serialize> Serialize for Slot<R> {
    fn serialize<S: Serializer>(&self, serializer: S) -> Result<S::Ok, S::Error> {
        let skip = self.record.is_none() && serializer.is_human_readable();
        let mut slot = serializer.serialize_struct("Slot", if skip { 1 } else { 2 })?;
        slot.serialize_field("generation", &self.generation)?;
        if skip {
            slot.skip_field("record")?;
        } else {
            slot.serialize_field("record", &self.record)?;
        }
        slot.end()
    }
}

/// The model's tolerance as written, in millimetres and radians.
#[derive(Clone, Copy, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct ToleranceFile {
    length: Exact,
    angle: Exact,
}

/// A whole model as written.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
struct File {
    format: String,
    version: u32,
    tolerance: ToleranceFile,
    step: u64,
    steps: Vec<NoteFile>,
    bodies: Vec<Slot<BodyFile>>,
    shells: Vec<Slot<ShellFile>>,
    faces: Vec<Slot<FaceFile>>,
    loops: Vec<Slot<LoopFile>>,
    wires: Vec<Slot<WireFile>>,
    uses: Vec<Slot<UseFile>>,
    edges: Vec<Slot<EdgeFile>>,
    vertices: Vec<Slot<VertexFile>>,
    surfaces: Vec<Slot<Geometry<SurfaceFamily>>>,
    curves: Vec<Slot<Geometry<CurveFamily>>>,
    pcurves: Vec<Slot<Geometry<CurveFamily>>>,
}

/// What a file says it is, read alone when the rest will not read, so a newer version is named as such.
#[derive(Deserialize)]
struct Header {
    format: String,
    version: u32,
}

/// Kind `K`'s slots of `model`, each record written by `write`.
fn slots<K: Kind, R>(model: &Snapshot, write: impl Fn(&K::Record) -> R) -> Vec<Slot<R>> {
    model.saved::<K>().map(|s| Slot { generation: s.generation, record: s.record.map(&write) }).collect()
}

/// Kind `K`'s geometry slots of `model`, each written by `write`; the slots `write` cannot write, named.
fn geometry_slots<K: Kind, F>(
    model: &Snapshot,
    write: impl Fn(&K::Record) -> Option<Geometry<F>>,
    refused: &mut Vec<EntityRef>,
) -> Vec<Slot<Geometry<F>>> {
    (0_u32..)
        .zip(model.saved::<K>())
        .map(|(slot, s)| {
            let record = s.record.and_then(|r| {
                let written = write(r);
                if written.is_none() {
                    refused.push(EntityRef::new(K::ENTITY, slot, s.generation.get()));
                }
                written
            });
            Slot { generation: s.generation, record }
        })
        .collect()
}

/// `model` as the format's records.
fn file_of(model: &Snapshot) -> Result<File, Report> {
    let tolerance = model.tolerance();
    let mut refused = Vec::new();
    let surfaces = geometry_slots::<SurfaceGeom, _>(model, |r: &SurfaceRecord| geometry::surface(&*r.0), &mut refused);
    let curves = geometry_slots::<CurveGeom, _>(model, |r: &CurveRecord| geometry::curve(&*r.0), &mut refused);
    let pcurves =
        geometry_slots::<ParamCurveGeom, _>(model, |r: &ParamCurveRecord| geometry::param_curve(&*r.0), &mut refused);
    if !refused.is_empty() {
        let report = Report::new(Code::NotSaved, Fault::NotImplemented);
        return Err(refused.into_iter().fold(report, Report::with_entity));
    }
    Ok(File {
        format: FORMAT.to_string(),
        version: VERSION,
        tolerance: ToleranceFile {
            length: Exact(tolerance.length().millimetres()),
            angle: Exact(tolerance.angle().radians()),
        },
        step: model.step(),
        steps: model.notes().map(|(step, note)| NoteFile::of(step, note)).collect(),
        bodies: slots::<Body, _>(model, |r: &BodyRecord| BodyFile::from(r)),
        shells: slots::<Shell, _>(model, |r: &ShellRecord| ShellFile::from(r)),
        faces: slots::<Face, _>(model, |r: &FaceRecord| FaceFile::from(r)),
        loops: slots::<Loop, _>(model, |r: &LoopRecord| LoopFile::from(r)),
        wires: slots::<Wire, _>(model, |r: &WireRecord| WireFile::from(r)),
        uses: slots::<EdgeUse, _>(model, |r: &EdgeUseRecord| UseFile::from(r)),
        edges: slots::<Edge, _>(model, |r: &EdgeRecord| EdgeFile::from(r)),
        vertices: slots::<Vertex, _>(model, |r: &VertexRecord| VertexFile::from(r)),
        surfaces,
        curves,
        pcurves,
    })
}

/// The report of a file that could not be written to memory, which plain data always can be: a bug, reported rather
/// than hidden.
fn not_written<E>(_: E) -> Report {
    Report::new(Code::NotSaved, Fault::Internal)
}

/// `model` in the native format, as readable JSON.
///
/// # Errors
///
/// `io.not-saved`, naming every curve and surface of a family the format does not write yet.
pub fn save(model: &Snapshot) -> Result<Vec<u8>, Report> {
    serde_json::to_vec(&file_of(model)?).map_err(not_written)
}

/// The length of a binary twin's checksum, which ends it.
const CHECKSUM: usize = 4;

/// `bytes` with their CRC-32 after them, as a binary twin ends.
fn sealed(mut bytes: Vec<u8>) -> Vec<u8> {
    let sum = crc::crc32(&bytes);
    bytes.extend_from_slice(&sum.to_le_bytes());
    bytes
}

/// `file` as the binary twin: [`MAGIC`], the version, the records, then the checksum of them all.
fn binary(file: &File) -> Result<Vec<u8>, postcard::Error> {
    let mut bytes = MAGIC.to_vec();
    bytes.extend_from_slice(&VERSION.to_le_bytes());
    postcard::to_extend(file, bytes).map(sealed)
}

/// `model` in the native format's binary twin, which holds the same records as [`save`]'s file and loads faster.
///
/// # Errors
///
/// As [`save`].
pub fn save_binary(model: &Snapshot) -> Result<Vec<u8>, Report> {
    binary(&file_of(model)?).map_err(not_written)
}

/// The refusal of bytes that are not the format, where reading stopped.
fn not_readable(line: usize, column: usize) -> Report {
    let count = |n: usize| f64::from(u32::try_from(n).unwrap_or(u32::MAX));
    Report::new(Code::NotReadable, Fault::InvalidInput)
        .with_value(NamedValue::new("line", Quantity::Count, count(line)))
        .with_value(NamedValue::new("column", Quantity::Count, count(column)))
}

/// The refusal of a file of another version.
fn other_version(version: u32) -> Report {
    Report::new(Code::OtherVersion, Fault::InvalidInput)
        .with_value(NamedValue::new("version", Quantity::Count, f64::from(version)))
        .with_value(NamedValue::new("supported", Quantity::Count, f64::from(VERSION)))
}

/// The refusal of a binary file whose checksum does not match what it holds.
fn damaged() -> Report {
    Report::new(Code::Damaged, Fault::InvalidInput)
}

/// The binary file `bytes`, which begin with [`MAGIC`], if it is one of this version and its checksum matches. Its
/// version is read before its checksum, since another version may end otherwise. A binary file is one line, and its
/// columns are its bytes, counted from 1 at the first byte of the magic.
fn binary_file(bytes: &[u8]) -> Result<File, Report> {
    let at = |offset: usize| not_readable(1, offset.saturating_add(1));
    let rest = bytes.get(MAGIC.len()..).unwrap_or_default();
    let Some((version, rest)) = rest.split_first_chunk::<4>() else { return Err(at(MAGIC.len())) };
    let version = u32::from_le_bytes(*version);
    if version != VERSION {
        return Err(other_version(version));
    }
    let Some((body, sum)) = rest.split_last_chunk::<CHECKSUM>() else { return Err(at(bytes.len())) };
    let summed = bytes.len().saturating_sub(CHECKSUM);
    if crc::crc32(bytes.get(..summed).unwrap_or_default()) != u32::from_le_bytes(*sum) {
        return Err(damaged());
    }
    let start = summed.saturating_sub(body.len());
    let mut reader = postcard::Deserializer::from_bytes(body);
    let read = File::deserialize(&mut reader);
    let left = reader.finalize().map_or(0, <[u8]>::len);
    let stopped = summed.saturating_sub(left);
    match read {
        Ok(_) if left > 0 => Err(at(stopped)),
        Ok(file) if file.format != FORMAT || file.version != VERSION => Err(at(start)),
        Ok(file) => Ok(file),
        Err(_) => Err(at(stopped)),
    }
}

/// The file `bytes` hold, if they are one of this version, in either encoding.
fn file(bytes: &[u8]) -> Result<File, Report> {
    if bytes.starts_with(MAGIC) {
        return binary_file(bytes);
    }
    match serde_json::from_slice::<File>(bytes) {
        Ok(file) if file.format != FORMAT => Err(not_readable(1, 1)),
        Ok(file) if file.version != VERSION => Err(other_version(file.version)),
        Ok(file) => Ok(file),
        Err(error) => match serde_json::from_slice::<Header>(bytes) {
            Ok(header) if header.format == FORMAT && header.version != VERSION => Err(other_version(header.version)),
            _ => Err(not_readable(error.line(), error.column())),
        },
    }
}

/// Kind `K`'s slots as written, each record read by `read`.
fn read<K: Kind, R>(slots: Vec<Slot<R>>, read: impl Fn(R) -> K::Record) -> Vec<Saved<K::Record>> {
    slots.into_iter().map(|s| Saved { generation: s.generation, record: s.record.map(&read) }).collect()
}

/// Kind `K`'s geometry slots as written, each restored by `restore`; every one its family refuses, named.
fn read_geometry<K: Kind, F, G>(
    slots: &[Slot<Geometry<F>>],
    restore: impl Fn(&Geometry<F>) -> Result<G, Report>,
    wrap: impl Fn(G) -> K::Record,
    failures: &mut Vec<Report>,
) -> Vec<Saved<K::Record>> {
    (0_u32..)
        .zip(slots)
        .map(|(slot, s)| {
            let record = s.record.as_ref().and_then(|g| match restore(g) {
                Ok(geometry) => Some(wrap(geometry)),
                Err(report) => {
                    failures.push(report.with_entity(EntityRef::new(K::ENTITY, slot, s.generation.get())));
                    None
                }
            });
            Saved { generation: s.generation, record }
        })
        .collect()
}

/// The model `bytes` hold, in either encoding, read but not yet measured or checked: `Restore::finish` does that.
///
/// # Errors
///
/// `io.not-readable` with the line and column where the bytes stop being the format (in a binary file, line 1 and the
/// byte), `io.other-version` with the version of a file this build does not read, `io.damaged` for a binary file whose
/// checksum does not match it, `units.tolerance-out-of-range` for the tolerance, and each curve or surface its family
/// refuses, with the family's code and the record named.
pub fn parse(bytes: &[u8]) -> Result<Restore, Vec<Report>> {
    let f = file(bytes).map_err(|report| vec![report])?;
    let tolerance =
        Tolerance::new(Length::from_millimetres(f.tolerance.length.0), Angle::from_radians(f.tolerance.angle.0))
            .map_err(|report| vec![report])?;
    let mut failures = Vec::new();
    let surfaces =
        read_geometry::<SurfaceGeom, _, _>(&f.surfaces, geometry::restored_surface, SurfaceRecord, &mut failures);
    let curves = read_geometry::<CurveGeom, _, _>(&f.curves, geometry::restored_curve, CurveRecord, &mut failures);
    let pcurves = read_geometry::<ParamCurveGeom, _, _>(
        &f.pcurves,
        geometry::restored_param_curve,
        ParamCurveRecord,
        &mut failures,
    );
    if !failures.is_empty() {
        return Err(failures);
    }
    let mut restore = Restore::new(tolerance, f.step);
    for (step, note) in f.steps.into_iter().map(NoteFile::read) {
        restore.note(step, note);
    }
    restore.table::<Body>(read::<Body, _>(f.bodies, Into::into));
    restore.table::<Shell>(read::<Shell, _>(f.shells, Into::into));
    restore.table::<Face>(read::<Face, _>(f.faces, Into::into));
    restore.table::<Loop>(read::<Loop, _>(f.loops, Into::into));
    restore.table::<Wire>(read::<Wire, _>(f.wires, Into::into));
    restore.table::<EdgeUse>(read::<EdgeUse, _>(f.uses, Into::into));
    restore.table::<Edge>(read::<Edge, _>(f.edges, Into::into));
    restore.table::<Vertex>(read::<Vertex, _>(f.vertices, Into::into));
    restore.table::<SurfaceGeom>(surfaces);
    restore.table::<CurveGeom>(curves);
    restore.table::<ParamCurveGeom>(pcurves);
    Ok(restore)
}

/// The model `bytes` hold: [`parse`], then every edge measured and the whole model checked on `exec`, as `commit`
/// would. Nothing from outside is trusted.
///
/// # Errors
///
/// As [`parse`], then every failure `Restore::finish` finds, each naming its entities.
pub fn load(bytes: &[u8], exec: &Exec) -> Result<Snapshot, Vec<Report>> {
    parse(bytes)?.finish(exec)
}

#[cfg(test)]
mod tests;
