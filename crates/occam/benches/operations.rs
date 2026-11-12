//! Source: original
//! Instruction counts per operation class, scalar and lanes (L31). `gates perf` compares them with `ci/perf-baseline.txt`.
//! They run as a static musl build, so that no machine's libc or CPU features change the counts. Each lane kernel runs at
//! width 1, `<kernel>_w1`, and at its width, `<kernel>_w<width>`, which `gates perf` holds to a clear gain (L29).
#![allow(clippy::unwrap_used, missing_docs, reason = "a benchmark, where a failed setup should stop the run")]

use std::hint::black_box;

use gungraun::{library_benchmark, library_benchmark_group, main};
use occam::spline_workloads as splines;
use occam::{Angle, Jets, Lanes, Length, Model, Point, Real, SplineSurface, Tolerance, UvPoint, World, distance};
use occam_math::{Exec, Level, stream, width, workloads};

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

library_benchmark_group!(
    name = operations,
    benchmarks = [measure_distance, maths_scalar, maths_lanes2, maths_lanes4, maths_lanes8]
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
main!(library_benchmark_groups = operations, kernels);
