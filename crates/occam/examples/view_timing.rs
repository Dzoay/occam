//! Source: original
//! The `view` milestone's native measurements on P9's models (`just view-timing`), each printed as one JSON line:
//!
//! - `build <target>`: the model of P9's parts for `target` faces, how long it took and its faces and digest;
//! - `quality <target> <deflection>`: P9's quality table on the mesher, meshed on six threads;
//! - `scale <target> <deflection> <repeats> <threads>...`: the whole mesh at each thread count, its least and median
//!   time over the repeats and its digest, which must be the same at every count;
//! - `phases <target> <deflection> <threads>...`: the mesh's four phases timed apart at each thread count, the median of
//!   three: the edges sampled, the faces meshed, the merge and the watertight check;
//! - `save <target> <file>`: the model in the native format's binary encoding, for `occam view`;
//! - `open <file>`: a saved model read, measured and checked, as a page's workers each open it, on one thread and six;
//! - `requests`: construction's requests that `gates perf` times, and its polygon of 1,000 edges, by name, for the
//!   editing worker's timing in the browser.
#![allow(clippy::print_stdout, clippy::print_stderr, reason = "a command-line tool prints")]
#![allow(clippy::arithmetic_side_effects, reason = "timings and counts in a tool")]

use std::env;
use std::num::NonZeroUsize;
use std::process::ExitCode;
use std::sync::Arc;
use std::time::Instant;

use occam::mesh_workloads::{P9_SEED, face_count, quality, synthetic};
use occam::{Exec, Limits};
use occam_report::{DisplayUnits, Report};
use occam_topo::Snapshot;
use occam_units::{Angle, Length};

const USAGE: &str = "usage: view_timing build <target> | quality <target> <deflection> | \
                     scale <target> <deflection> <repeats> <threads>... | phases <target> <deflection> <threads>... | save <target> <file> | open <file> | requests";

/// P9's angular limit.
const ANGLE: f64 = 0.35;

fn said(reports: &[Report]) -> String {
    reports.iter().map(|report| report.message(DisplayUnits::default())).collect::<Vec<_>>().join("; ")
}

fn number<T: std::str::FromStr>(text: &str) -> Result<T, String> {
    text.parse().map_err(|_| format!("not a number: {text}\n{USAGE}"))
}

fn exec(threads: usize) -> Result<Exec, String> {
    let count = NonZeroUsize::new(threads).ok_or_else(|| USAGE.to_string())?;
    Exec::threads(count).map_err(|report| said(&[report]))
}

fn limits(deflection: f64) -> Result<Limits, String> {
    Limits::new(Length::from_millimetres(deflection), Angle::from_radians(ANGLE)).map_err(|report| said(&[report]))
}

fn model(target: usize) -> Result<(Snapshot, f64), String> {
    let start = Instant::now();
    let model = synthetic(&exec(6)?, target, P9_SEED).map_err(|reports| said(&reports))?;
    Ok((model, start.elapsed().as_secs_f64() * 1e3))
}

/// The furthest any edge of `model` strays from its faces, as measured when it was committed.
fn strays(model: &Snapshot) -> f64 {
    let edges = model.ids::<occam_topo::Edge>().into_iter().filter_map(|edge| model.view(edge));
    edges.map(|edge| edge.deviation().millimetres()).fold(0.0, |most, d| if d > most { d } else { most })
}

fn build(target: usize) -> Result<String, String> {
    let (model, ms) = model(target)?;
    Ok(format!(
        r#"{{"mode":"build","target":{target},"faces":{},"ms":{ms:.1},"digest":"{:016x}","edges_stray_mm":{:e}}}"#,
        face_count(&model),
        model.digest(),
        strays(&model)
    ))
}

fn quality_rows(target: usize, deflection: f64) -> Result<String, String> {
    let (model, _) = model(target)?;
    let six = exec(6)?;
    let start = Instant::now();
    let mesh = occam_mesh::mesh(&six, &model, limits(deflection)?);
    let ms = start.elapsed().as_secs_f64() * 1e3;
    let (refused, open, capped) =
        (mesh.refused.len(), mesh.open.len(), mesh.blocks.iter().filter(|block| block.capped).count());
    let faces = face_count(&model);
    let (model, mesh) = (Arc::new(model), Arc::new(mesh));
    let rows = quality(&six, &model, &mesh).into_iter().map(|q| {
        format!(
            r#"{{"mode":"quality","target":{target},"model_faces":{faces},"deflection":{deflection},"angle":{ANGLE},"family":"{}","faces":{},"triangles":{},"deviation_over_deflection":{:.4},"over_deflection":{},"turn":{:.4},"over_angle":{},"over_angle_slivers":{},"area":{:.3},"area_over_angle":{:.6},"smallest":{:.3},"mean_smallest":{:.3},"under_20":{},"refused":{refused},"open":{open},"capped":{capped},"mesh_ms":{ms:.1}}}"#,
            q.family,
            q.faces,
            q.triangles,
            q.deviation,
            q.over_deflection,
            q.turn,
            q.over_angle,
            q.over_angle_slivers,
            q.area,
            q.area_over_angle,
            q.smallest,
            q.mean_smallest,
            q.under_20
        )
    });
    Ok(rows.collect::<Vec<_>>().join("\n"))
}

/// The least and the median of `times`.
fn least_and_median(mut times: Vec<f64>) -> (f64, f64) {
    times.sort_by(f64::total_cmp);
    (times.first().copied().unwrap_or(f64::NAN), times.get(times.len() / 2).copied().unwrap_or(f64::NAN))
}

fn scale(target: usize, deflection: f64, repeats: usize, threads: &[usize]) -> Result<String, String> {
    let (model, _) = model(target)?;
    let limits = limits(deflection)?;
    let mut rows = Vec::new();
    for &count in threads {
        let runner = exec(count)?;
        let mut times = Vec::new();
        let mut digests = Vec::new();
        let mut triangles = 0;
        for _ in 0..repeats {
            let start = Instant::now();
            let mesh = occam_mesh::mesh(&runner, &model, limits);
            times.push(start.elapsed().as_secs_f64() * 1e3);
            digests.push(mesh.digest());
            triangles = mesh.triangles.len();
            drop(mesh);
        }
        digests.dedup();
        let (least, median) = least_and_median(times.clone());
        let digests: Vec<String> = digests.iter().map(|d| format!("\"{d:016x}\"")).collect();
        let times: Vec<String> = times.iter().map(|t| format!("{t:.1}")).collect();
        rows.push(format!(
            r#"{{"mode":"scale","target":{target},"faces":{},"deflection":{deflection},"threads":{count},"triangles":{triangles},"least_ms":{least:.1},"median_ms":{median:.1},"ms":[{}],"digests":[{}]}}"#,
            face_count(&model),
            times.join(","),
            digests.join(",")
        ));
    }
    Ok(rows.join("\n"))
}

fn phases(target: usize, deflection: f64, threads: &[usize]) -> Result<String, String> {
    let (model, _) = model(target)?;
    let limits = limits(deflection)?;
    let mut rows = Vec::new();
    for &count in threads {
        let runner = exec(count)?;
        let mut times: [Vec<f64>; 4] = Default::default();
        for _ in 0..3 {
            let mut lap = Instant::now();
            let mut took = |k: usize| {
                if let Some(phase) = times.get_mut(k) {
                    phase.push(lap.elapsed().as_secs_f64() * 1e3);
                }
                lap = Instant::now();
            };
            let edges = occam_mesh::sample_edges(&runner, &model, limits);
            took(0);
            let faces = occam_mesh::mesh_faces(&runner, &model, &edges);
            took(1);
            let mesh = occam_mesh::merge(edges, &faces);
            took(2);
            let open = occam_mesh::watertight(&model, &mesh);
            took(3);
            drop((faces, mesh, open));
        }
        let [edges, faces, merge, check] = times.map(|t| least_and_median(t).1);
        rows.push(format!(
            r#"{{"mode":"phases","target":{target},"deflection":{deflection},"threads":{count},"edges_ms":{edges:.1},"faces_ms":{faces:.1},"merge_ms":{merge:.1},"watertight_ms":{check:.1}}}"#
        ));
    }
    Ok(rows.join("\n"))
}

fn save(target: usize, file: &str) -> Result<String, String> {
    let (model, _) = model(target)?;
    let bytes = occam_io::save_binary(&model).map_err(|report| said(&[report]))?;
    std::fs::write(file, &bytes).map_err(|e| format!("{file}: {e}"))?;
    Ok(format!(
        r#"{{"mode":"save","target":{target},"faces":{},"bytes":{},"digest":"{:016x}","file":"{file}"}}"#,
        face_count(&model),
        bytes.len(),
        model.digest()
    ))
}

fn requests() -> String {
    use occam::operation_workloads::{MEASURES, POLYGON_CORNERS, PRIMITIVES, SWEEPS, TRANSFORMS, polygon_extruded};
    let polygon = polygon_extruded(POLYGON_CORNERS);
    let named = [
        ("construct_primitives", PRIMITIVES),
        ("construct_sweeps", SWEEPS),
        ("construct_transforms", TRANSFORMS),
        ("construct_polygon_extrude", polygon.as_str()),
        ("measure_primitives", MEASURES),
    ];
    let pairs: Vec<String> =
        named.iter().map(|(name, request)| format!("{name:?}:{}", serde_json::json!(request))).collect();
    format!("{{{}}}", pairs.join(","))
}

fn open(file: &str) -> Result<String, String> {
    let bytes = std::fs::read(file).map_err(|e| format!("{file}: {e}"))?;
    let mut rows = Vec::new();
    for threads in [1, 6] {
        let start = Instant::now();
        let model = occam_io::load(&bytes, &exec(threads)?).map_err(|reports| said(&reports))?;
        let ms = start.elapsed().as_secs_f64() * 1e3;
        rows.push(format!(
            r#"{{"mode":"open","file":"{file}","threads":{threads},"faces":{},"ms":{ms:.1}}}"#,
            face_count(&model)
        ));
    }
    Ok(rows.join("\n"))
}

fn run(args: &[String]) -> Result<String, String> {
    let words: Vec<&str> = args.iter().map(String::as_str).collect();
    match words.as_slice() {
        ["build", target] => build(number(target)?),
        ["quality", target, deflection] => quality_rows(number(target)?, number(deflection)?),
        ["scale", target, deflection, repeats, threads @ ..] if !threads.is_empty() => {
            let threads = threads.iter().map(|t| number(t)).collect::<Result<Vec<usize>, _>>()?;
            scale(number(target)?, number(deflection)?, number(repeats)?, &threads)
        }
        ["phases", target, deflection, threads @ ..] if !threads.is_empty() => {
            let threads = threads.iter().map(|t| number(t)).collect::<Result<Vec<usize>, _>>()?;
            phases(number(target)?, number(deflection)?, &threads)
        }
        ["save", target, file] => save(number(target)?, file),
        ["open", file] => open(file),
        ["requests"] => Ok(requests()),
        _ => Err(USAGE.to_string()),
    }
}

fn main() -> ExitCode {
    let args: Vec<String> = env::args().skip(1).collect();
    match run(&args) {
        Ok(text) => {
            println!("{text}");
            ExitCode::SUCCESS
        }
        Err(why) => {
            eprintln!("{why}");
            ExitCode::FAILURE
        }
    }
}
