//! Source: original
//! Instruction counts per operation class, scalar and lanes (L31). `gates perf` compares them with `ci/perf-baseline.txt`.
//! They run as a static musl build, so that no machine's libc or CPU features change the counts. Each lane kernel runs at
//! width 1, `<kernel>_w1`, and at its width, `<kernel>_w<width>`, which `gates perf` holds to a clear gain (L29). An
//! operation measured on models of several sizes, `<operation>_n<faces>`, is held to about the same count on each (P1).
#![allow(clippy::unwrap_used, missing_docs, reason = "a benchmark, where a failed setup should stop the run")]

use std::hint::black_box;

use gungraun::{library_benchmark, library_benchmark_group, main};
use occam::spline_workloads as splines;
use occam::{Angle, Jets, Lanes, Length, Model, Point, Real, SplineSurface, Tolerance, UvPoint, World, distance};
use occam_geom::{OffsetCurve, OffsetSurface, SplineCurve};
use occam_math::{Exec, Level, stream, width, workloads};
use occam_topo::Edit;
use occam_topo::workloads::Plate;
use occam_units::{Direction, Vec3};

fn mm(value: f64) -> Length {
    Length::from_millimetres(value)
}

fn model() -> Model {
    Model::new(Tolerance::new(mm(0.001), Angle::from_radians(1e-6)).unwrap())
}

fn point(x: f64, y: f64, z: f64) -> Point<World> {
    Point::new(mm(x), mm(y), mm(z))
}

#[library_benchmark]
fn measure_distance() -> f64 {
    let (from, to) = (point(1.0, 2.0, 3.0), point(4.0, 6.0, 15.0));
    distance(black_box(model()), black_box(from), black_box(to), black_box(mm(0.01))).unwrap().value.millimetres()
}

/// One thread, because the operation class is the arithmetic and threads add scheduling noise. The baseline level, so
/// that the counts do not depend on the machine.
fn maths<R: Real>() -> stream::Hashes {
    stream::run::<R>(Level::baseline(), black_box(&Exec::serial()))
}

#[library_benchmark]
fn maths_scalar() -> stream::Hashes {
    maths::<f64>()
}

#[library_benchmark]
fn maths_lanes2() -> stream::Hashes {
    maths::<Lanes<2>>()
}

#[library_benchmark]
fn maths_lanes4() -> stream::Hashes {
    maths::<Lanes<4>>()
}

#[library_benchmark]
fn maths_lanes8() -> stream::Hashes {
    maths::<Lanes<8>>()
}

/// How far the offsets move their bases, in millimetres.
const OFFSET: f64 = 0.3;

/// A planar cubic B-spline curve moved within its plane, at 256 parameters, at the spline width. Its second derivative
/// needs the base's third, which no rung gives, so it runs a lane at a time.
fn offset_curve_items() -> (OffsetCurve<SplineCurve>, Vec<f64>) {
    let base = SplineCurve::new(&splines::spline_curve()).unwrap();
    let up = Direction::normalised(Vec3::new(0.0, 0.0, 1.0));
    (OffsetCurve::new(base, mm(OFFSET), up).unwrap(), splines::params(256))
}

#[library_benchmark]
#[bench::items(offset_curve_items())]
fn offset_curves((curve, ts): (OffsetCurve<SplineCurve>, Vec<f64>)) -> f64 {
    black_box(splines::curve_seconds::<Lanes<{ width::SPLINE }>, _>(black_box(&curve), black_box(&ts)))
}

/// The measured B-spline surface moved along its normal, at a 16 by 16 grid, at the spline width, a lane at a time as
/// the curve's.
fn offset_surface_items() -> (OffsetSurface<SplineSurface>, Vec<UvPoint>) {
    let base = SplineSurface::new(&splines::spline()).unwrap();
    (OffsetSurface::new(base, mm(OFFSET)).unwrap(), splines::grid(16))
}

#[library_benchmark]
#[bench::items(offset_surface_items())]
fn offset_surfaces((surface, uv): (OffsetSurface<SplineSurface>, Vec<UvPoint>)) -> Jets {
    let mut out = Jets::default();
    splines::seconds::<{ width::SPLINE }, _>(black_box(&surface), black_box(&uv), &mut out);
    black_box(out)
}

library_benchmark_group!(
    name = operations,
    benchmarks =
        [measure_distance, maths_scalar, maths_lanes2, maths_lanes4, maths_lanes8, offset_curves, offset_surfaces]
);

// The benchmarks' names carry each kernel's width, so they must change with it.
const _: () = assert!(width::MASK == 8 && width::FILTER == 4 && width::ANALYTIC == 4 && width::SPLINE == 4);

fn mask_items() -> Vec<f64> {
    workloads::inputs(3, 4.0, 1)
}

fn filter_items() -> Vec<f64> {
    workloads::inputs(6, 2.0, 2)
}

fn sine_items() -> Vec<f64> {
    workloads::inputs(1, 4.0, 3)
}

fn angle_items() -> Vec<f64> {
    workloads::inputs(2, 2.0, 4)
}

#[library_benchmark]
#[bench::items(mask_items())]
fn masks_w1(items: Vec<f64>) -> Vec<f64> {
    black_box(workloads::masks::<f64>(black_box(&items)))
}

#[library_benchmark]
#[bench::items(mask_items())]
fn masks_w8(items: Vec<f64>) -> Vec<f64> {
    black_box(workloads::masks::<Lanes<{ width::MASK }>>(black_box(&items)))
}

#[library_benchmark]
#[bench::items(filter_items())]
fn filter_w1(items: Vec<f64>) -> Vec<f64> {
    black_box(workloads::filter::<f64>(black_box(&items)))
}

#[library_benchmark]
#[bench::items(filter_items())]
fn filter_w4(items: Vec<f64>) -> Vec<f64> {
    black_box(workloads::filter::<Lanes<{ width::FILTER }>>(black_box(&items)))
}

#[library_benchmark]
#[bench::items(sine_items())]
fn sines_w1(items: Vec<f64>) -> Vec<f64> {
    black_box(workloads::sines::<f64>(black_box(&items)))
}

#[library_benchmark]
#[bench::items(sine_items())]
fn sines_w4(items: Vec<f64>) -> Vec<f64> {
    black_box(workloads::sines::<Lanes<{ width::ANALYTIC }>>(black_box(&items)))
}

#[library_benchmark]
#[bench::items(angle_items())]
fn angles_w1(items: Vec<f64>) -> Vec<f64> {
    black_box(workloads::angles::<f64>(black_box(&items)))
}

#[library_benchmark]
#[bench::items(angle_items())]
fn angles_w4(items: Vec<f64>) -> Vec<f64> {
    black_box(workloads::angles::<Lanes<{ width::ANALYTIC }>>(black_box(&items)))
}

/// A bicubic B-spline and a tessellator's grid of 64 by 64 points on it.
fn spline_items() -> (SplineSurface, Vec<UvPoint>) {
    (SplineSurface::new(&splines::spline()).unwrap(), splines::grid(64))
}

#[library_benchmark]
#[bench::items(spline_items())]
fn splines_w1((surface, uv): (SplineSurface, Vec<UvPoint>)) -> Jets {
    let mut out = Jets::default();
    splines::splines::<1, _>(black_box(&surface), black_box(&uv), &mut out);
    black_box(out)
}

#[library_benchmark]
#[bench::items(spline_items())]
fn splines_w4((surface, uv): (SplineSurface, Vec<UvPoint>)) -> Jets {
    let mut out = Jets::default();
    splines::splines::<{ width::SPLINE }, _>(black_box(&surface), black_box(&uv), &mut out);
    black_box(out)
}

library_benchmark_group!(
    name = kernels,
    benchmarks =
        [masks_w1, masks_w8, filter_w1, filter_w4, sines_w1, sines_w4, angles_w1, angles_w4, splines_w1, splines_w4]
);
/// The plates of the topology's classes, by squares across: 22 gives 1,056 faces and 223 gives 100,350.
const SMALL: usize = 22;
const LARGE: usize = 223;

/// A plate built and committed: setup, which the counts leave out.
fn plate(across: usize) -> Plate {
    Plate::new(across, &Exec::serial()).unwrap()
}

/// A plate and an edit of it splitting its middle top square, not yet committed.
fn split_middle(across: usize) -> (Plate, Edit) {
    let plate = plate(across);
    let edit = plate.split(plate.snapshot(), &[(across / 2, across / 2)]);
    (plate, edit)
}

/// Commits `edit` and leaks what it made and the plate, so that the count is the commit's and not the freeing of a
/// whole model.
fn commit(plate: Plate, edit: Edit) -> bool {
    let committed = edit.commit(&Exec::serial()).map(core::mem::forget).is_ok();
    core::mem::forget(plate);
    committed
}

// Taking a snapshot of a plate of 1,056 faces: a pointer copied for each chunk of each table. The smaller plate, as
// building the larger under Valgrind costs the gate a quarter of an hour.
#[library_benchmark]
#[bench::plate(plate(SMALL))]
fn topo_snapshot(plate: Plate) -> usize {
    let taken = black_box(plate.snapshot().clone());
    core::mem::forget((taken, plate));
    SMALL
}

// An edit splitting one top square of a plate of 30 faces, and its commit, which checks it.
#[library_benchmark]
#[bench::plate(plate(3))]
fn topo_edit(plate: Plate) -> bool {
    let edit = plate.split(plate.snapshot(), black_box(&[(1, 1)]));
    commit(plate, edit)
}

// The commit of an edit splitting one face, of 1,056 faces.
#[library_benchmark]
#[bench::split(split_middle(SMALL))]
fn topo_commit_n1056((plate, edit): (Plate, Edit)) -> bool {
    commit(plate, black_box(edit))
}

// The same commit, of 100,350 faces.
#[library_benchmark]
#[bench::split(split_middle(LARGE))]
fn topo_commit_n100350((plate, edit): (Plate, Edit)) -> bool {
    commit(plate, black_box(edit))
}

library_benchmark_group!(
    name = topology,
    benchmarks = [topo_snapshot, topo_edit, topo_commit_n1056, topo_commit_n100350]
);
main!(library_benchmark_groups = operations, kernels, topology);
