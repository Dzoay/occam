//! Source: original
//! The parts of P9's synthetic models, drawn as P9's generator drew them: the same random numbers in the same order,
//! from [`SplitMix`], so that a seed and a target give P9's parts. The faces counted
//! against the target are P9's, so the same parts come out, however the kernel lays each one out.

use core::f64::consts::TAU;

pub(crate) use super::splitmix::SplitMix;

/// The seed P9's models were drawn with.
pub const P9_SEED: u64 = 42;

/// Where a part stands: its origin and its turn about +Z.
#[derive(Clone, Copy, Debug, PartialEq)]
pub(crate) struct Frame {
    pub origin: [f64; 3],
    pub turn: f64,
}

/// One of P9's parts, in its own frame, sizes in millimetres.
#[derive(Clone, Debug, PartialEq)]
pub(crate) enum Shape {
    /// A plate `size.0` by `size.1` by `size.2` with round holes through it, each `(x, y, radius)`.
    Plate {
        size: (f64, f64, f64),
        holes: Vec<(f64, f64, f64)>,
    },
    Sphere {
        radius: f64,
    },
    Torus {
        major: f64,
        minor: f64,
    },
    Cylinder {
        radius: f64,
        length: f64,
    },
    /// A cone from a base of `radius` up to its apex.
    Cone {
        radius: f64,
        length: f64,
    },
    /// A bolt: a shank of `radius` and `length`, a fillet under a head of `head` radius and `height`, and a dome.
    Bolt {
        radius: f64,
        length: f64,
        head: f64,
        fillet: f64,
        height: f64,
    },
    /// A bicubic B-spline panel `size.0` by `size.1`, `size.2` thick, its bump shifted by `phase`, with a hole.
    Panel {
        size: (f64, f64, f64),
        phase: f64,
    },
    /// A torus of `across.0` by `across.1` Bézier patches of `degree`, rational at degree 2, its lattice jittered
    /// from `seed`.
    Quilt {
        across: (u32, u32),
        radius: f64,
        degree: u32,
        seed: u64,
    },
}

impl Shape {
    /// How many faces P9's generator counted for it.
    pub(crate) fn p9_faces(&self) -> usize {
        match self {
            Self::Plate { holes, .. } => holes.len().saturating_add(6),
            Self::Sphere { .. } | Self::Torus { .. } => 1,
            Self::Cone { .. } => 2,
            Self::Cylinder { .. } => 3,
            Self::Bolt { .. } => 6,
            Self::Panel { .. } => 7,
            Self::Quilt { across, .. } => usize::try_from(across.0.saturating_mul(across.1)).unwrap_or(usize::MAX),
        }
    }
}

/// A part: its shape and where it stands.
#[derive(Clone, Debug, PartialEq)]
pub(crate) struct Part {
    pub frame: Frame,
    pub shape: Shape,
}

/// A plate's holes, as P9 drew them: `across` columns and `down` rows, each radius a fraction of the smaller pitch.
fn holes(rng: &mut SplitMix, (w, d): (f64, f64), (across, down): (u32, u32)) -> Vec<(f64, f64, f64)> {
    let pitch = (w / (f64::from(across) + 1.0), d / (f64::from(down) + 1.0));
    let smaller = if pitch.1 < pitch.0 { pitch.1 } else { pitch.0 };
    let mut out = Vec::new();
    for j in 1..=down {
        for i in 1..=across {
            let radius = rng.range(0.15, 0.4) * smaller;
            out.push((f64::from(i) * pitch.0, f64::from(j) * pitch.1, radius));
        }
    }
    out
}

/// A solid of revolution, as P9 drew one.
fn revolved(rng: &mut SplitMix) -> Shape {
    let (radius, length) = (rng.range(1.0, 6.0), rng.range(4.0, 20.0));
    match rng.int(0, 4) {
        0 => Shape::Sphere { radius },
        1 => Shape::Torus { major: radius * 2.5, minor: radius * 0.6 },
        2 => Shape::Cylinder { radius, length },
        3 => Shape::Cone { radius, length },
        _ => {
            let head = radius * rng.range(1.5, 2.2);
            let fillet = radius * rng.range(0.1, 0.25);
            let height = radius * rng.range(0.6, 1.1);
            Shape::Bolt { radius, length, head, fillet, height }
        }
    }
}

/// A quilt of `degree`, as P9 drew one.
fn quilt(rng: &mut SplitMix, degree: u32) -> Shape {
    let across = (rng.int(4, 12), rng.int(4, 12));
    let radius = rng.range(1.0, 3.0);
    Shape::Quilt { across, radius, degree, seed: rng.next_u64() }
}

/// The shape of the kind drawn as `kind`, its sizes drawn next, as P9 drew them.
fn shape(rng: &mut SplitMix, kind: u32) -> Shape {
    match kind {
        0..=34 => {
            let (w, d, h) = (rng.range(10.0, 40.0), rng.range(10.0, 40.0), rng.range(1.0, 6.0));
            let across = rng.int(1, 6);
            let down = rng.int(1, 3);
            Shape::Plate { size: (w, d, h), holes: holes(rng, (w, d), (across, down)) }
        }
        35..=64 => revolved(rng),
        65..=84 => {
            let (w, d, h) = (rng.range(10.0, 30.0), rng.range(10.0, 30.0), rng.range(2.0, 6.0));
            Shape::Panel { size: (w, d, h), phase: rng.range(0.0, 3.0) }
        }
        85..=92 => quilt(rng, 3),
        _ => quilt(rng, 2),
    }
}

/// P9's parts for `target` faces from `seed`: drawn until P9 would have counted at least `target` faces, each on a
/// grid 60 mm apart, 200 to a row, turned about +Z. Each part draws its kind, then its turn, then its sizes. Every part
/// counts at least one face, so the target is reached within `target` parts: drawing stops there whatever the count
/// says, so that a count gone wrong cannot draw without end.
pub(crate) fn parts(target: usize, seed: u64) -> Vec<Part> {
    let mut rng = SplitMix(seed);
    let (mut out, mut faces) = (Vec::new(), 0_usize);
    for place in 0..target {
        if faces >= target {
            break;
        }
        let place = u32::try_from(place).unwrap_or(u32::MAX);
        let origin = [60.0 * f64::from(place % 200), 60.0 * f64::from(place / 200), 0.0];
        let kind = rng.int(0, 99);
        let turn = rng.range(0.0, TAU);
        let shape = shape(&mut rng, kind);
        faces = faces.saturating_add(shape.p9_faces());
        out.push(Part { frame: Frame { origin, turn }, shape });
    }
    out
}
