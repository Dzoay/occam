//! Source: original
//! A run of edges as the model holds them, read back for the checks a wire and a face need: each edge's line or circle
//! and the part of it the edge covers, in the order and direction the run takes them; points along the run; the plane
//! its points lie in; and its pieces in that plane's parameters.

use occam_report::{Code, Fault, Input, Report};
use occam_topo::{Edge, Edit, Id};
use occam_units::Vec3;

use super::flat::Flat;
use super::space::{Bent, Flatland, unit};

/// Steps a circle's part is sampled at, for the plane through a run and the check that it lies in it.
const STEPS: u32 = 16;

/// A run of edges, read back from the model.
pub(crate) struct Run {
    /// The edges in order, each with whether the run takes it against its curve.
    pub edges: Vec<(Id<Edge>, bool)>,
    /// Each edge's line or circle.
    pub bents: Vec<Bent>,
    /// The part of each one's curve it covers, lowest first.
    pub ranges: Vec<(f64, f64)>,
}

impl Run {
    /// The run of `edges` in `edit`.
    ///
    /// # Errors
    ///
    /// [`Code::NotAWire`] at `at` if an edge does not lie on a line or a circle, which are what wires are made of.
    pub(crate) fn read(edit: &Edit, edges: Vec<(Id<Edge>, bool)>, at: &Input) -> Result<Self, Report> {
        let (mut bents, mut ranges) = (Vec::new(), Vec::new());
        for &(edge, _) in &edges {
            let record = edit.record(edge).ok_or_else(|| not_a_wire(at, "an edge no longer there"))?;
            let curve = edit.record(record.curve).ok_or_else(|| not_a_wire(at, "an edge with no curve"))?;
            let form = curve.0.shape().form;
            bents.push(Bent::of(&*curve.0).ok_or_else(|| not_a_wire(at, form))?);
            ranges.push(record.range);
        }
        Ok(Self { edges, bents, ranges })
    }

    /// Each edge's line or circle, the part it covers, and whether the run takes it backwards, in order.
    fn parts(&self) -> impl Iterator<Item = (Bent, (f64, f64), bool)> + '_ {
        self.bents.iter().zip(&self.ranges).zip(&self.edges).map(|((&b, &r), &(_, back))| (b, r, back))
    }

    /// Points along the run in its order: each edge's start, points between for a circle's part, and the last edge's
    /// end, which no edge begins at if the run is open.
    pub(crate) fn samples(&self) -> Vec<Vec3> {
        let mut points = Vec::new();
        let last = self.edges.len().saturating_sub(1);
        for (place, (bent, (lo, hi), back)) in self.parts().enumerate() {
            let steps = if matches!(bent, Bent::Line { .. }) { 1 } else { STEPS };
            for k in 0..steps.saturating_add(u32::from(place == last)) {
                let f = f64::from(k) / f64::from(steps);
                let t = if back { hi + (lo - hi) * f } else { lo + (hi - lo) * f };
                points.push(bent.point(t));
            }
        }
        points
    }

    /// Its pieces in `plane`'s parameters, in order.
    pub(crate) fn flats(&self, plane: &Flatland) -> Vec<Flat> {
        self.parts()
            .map(|(bent, range, back)| {
                let flat = plane.flat(bent, range);
                if back { flat.reversed() } else { flat }
            })
            .collect()
    }

    /// The same run taken the other way round.
    pub(crate) fn reversed(self) -> Self {
        let edges = self.edges.into_iter().rev().map(|(e, back)| (e, !back)).collect();
        Self { edges, bents: self.bents.into_iter().rev().collect(), ranges: self.ranges.into_iter().rev().collect() }
    }
}

/// A plane through `points`, at the first, if they span one beyond `band`: through the point furthest from the first and
/// the point furthest from the line through those two. Which way it faces is the caller's to settle.
pub(crate) fn plane_through(points: &[Vec3], band: f64) -> Option<Flatland> {
    let origin = *points.first()?;
    let furthest = |of: &dyn Fn(Vec3) -> f64| {
        points.iter().map(|&p| (of(p), p)).fold((0.0, origin), |best, next| if next.0 > best.0 { next } else { best })
    };
    let (reach, far) = furthest(&|p| (p - origin).norm());
    if reach <= band {
        return None;
    }
    let along = unit(far - origin);
    let (off, wide) = furthest(&|p| (p - origin).cross(along).norm());
    (off > band).then(|| Flatland::new(origin, unit((far - origin).cross(wide - origin))))
}

/// The point of `points` furthest from `plane`, and how far, if any is further than `band`.
pub(crate) fn off_plane(plane: &Flatland, points: &[Vec3], band: f64) -> Option<(Vec3, f64)> {
    let mut worst: Option<(Vec3, f64)> = None;
    for &p in points {
        let distance = plane.uv(p).1.abs();
        if distance > band && worst.is_none_or(|(_, d)| distance > d) {
            worst = Some((p, distance));
        }
    }
    worst
}

/// `at` names `given`, which is not a wire a face can be made from.
pub(crate) fn not_a_wire(at: &Input, given: &str) -> Report {
    Report::new(Code::NotAWire, Fault::InvalidInput).with_input(at.clone().given(given))
}

#[cfg(test)]
mod tests;
