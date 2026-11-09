//! Source: original
//! Instruction counts per operation class, scalar and lanes (L31). `gates perf` compares them with `ci/perf-baseline.txt`.
//! They run as a static musl build, so that no machine's libc or CPU features change the counts.
#![allow(clippy::unwrap_used, missing_docs, reason = "a benchmark, where a failed setup should stop the run")]

use std::hint::black_box;

use gungraun::{library_benchmark, library_benchmark_group, main};
use occam::{Angle, Lanes, Length, Model, Point, Real, Tolerance, World, distance};
use occam_math::stream;

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

/// One thread, because the operation class is the arithmetic and threads add scheduling noise.
fn maths<R: Real>() -> stream::Hashes {
    stream::run::<R>(black_box(1))
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
main!(library_benchmark_groups = operations);
