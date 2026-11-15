//! Source: barnhill-kersey-1990
//! The march (Barnhill and Kersey): from a station, a step along the curve's tangent, the cross product of the two
//! surfaces' normals, then the corrector back onto both surfaces. The step is set by the curvature the step itself
//! shows, the turn of the tangent over it, so that a chord sags from the curve by no more than the trace allows;
//! a step that turns too far is taken again shorter. The march ends where it comes back to its start, at an edge of
//! a domain that is not a seam or a pole, at a touch, or when its steps run out. Where the corrector finds nothing even
//! at the shortest step, alternating projections take over. Whether it closes, whether the surfaces touch, whether a
//! station is at an edge and what the edge is, and whether projections moved it on are decisions against the band
//! ([`Decide`]); one the band cannot make ends the march there, undecided.

use occam_geom::Scratch;
use occam_math::atan2;
use occam_units::Vec3;

use occam_math::Side as Band;

use crate::correct::{Here, Pair, Side};
use crate::decide::{Decide, ahead, distance, touch};
use crate::edge::{Breach, Meets, beside, breach, inside, meets, moved, on, with};
use crate::project::project;
use crate::trace::{End, Jump, Tracer, Uncertain};

/// The most a step's tangent may turn, in radians, before the step is taken again shorter.
const MOST_TURN: f64 = 0.3;

/// The most halvings that locate an edge.
const LOCATE: u32 = 60;

/// The trace's measures, from its [`Tracer`].
#[derive(Clone, Copy, Debug)]
pub(crate) struct Limits {
    /// The most a chord may sag, and never less than the band.
    pub sag: f64,
    /// The longest step.
    pub longest: f64,
    /// The shortest step: `sqrt(8 band longest)`, the step whose chord sags by the band on a curve of radius
    /// `longest`, and twice how far a touch of that radius is within the band of both surfaces.
    pub shortest: f64,
    /// The most steps.
    pub steps: u32,
}

impl Limits {
    /// The measures of `tracer`.
    pub(crate) fn of(tracer: &Tracer) -> Self {
        let band = tracer.tol.band().millimetres();
        let (sag, longest) = (tracer.sag.millimetres(), tracer.longest.millimetres());
        let shortest = (8.0 * band * longest).sqrt();
        Self {
            sag: if sag > band { sag } else { band },
            longest,
            shortest: if shortest < longest { shortest } else { longest },
            steps: tracer.steps,
        }
    }
}

/// A station and the curve's direction there.
#[derive(Clone, Copy, Debug)]
pub(crate) struct Frame {
    /// The station.
    pub here: Here,
    /// The unit tangent, the way the march goes.
    pub along: Vec3,
    /// The unit normals of `p` and `q`.
    pub normals: (Vec3, Vec3),
}

/// `here` with the curve's direction there, turned to go the way of `ahead`, where both surfaces have a normal.
pub(crate) fn frame(pair: &Pair<'_>, here: Here, ahead: Vec3) -> Option<Frame> {
    let normals = pair.normals(&here)?;
    let cross = normals.0.cross(normals.1);
    let sin = cross.norm();
    let along = if sin > 0.0 { cross * (1.0 / sin) } else { ahead };
    let along = if along.dot(ahead) < 0.0 { -along } else { along };
    along.norm().is_finite().then_some(Frame { here, along, normals })
}

/// The angle between two unit vectors.
fn angle(a: Vec3, b: Vec3) -> f64 {
    atan2(a.cross(b).norm(), a.dot(b))
}

/// What a march does next.
enum Next {
    /// Step again from the same station, by `h`.
    Again { h: f64, fresh: Option<Breach> },
    /// Step on from `here`, by `h`.
    On { here: Frame, h: f64, fresh: Option<Breach> },
    /// It has ended.
    Done(End),
}

/// One trace's work: its stations and crossings as it marches, in the trace's own order, and the steps it has taken.
pub(crate) struct Run<'p, 's> {
    /// The surfaces.
    pub pair: &'p Pair<'s>,
    /// Its measures.
    pub limits: Limits,
    /// The steps taken.
    pub steps: u32,
    /// The stations, in the order marched.
    pub stations: Vec<Here>,
    /// Where stations `at` and `at + 1` are one point across a seam or a pole of `side`.
    pub crossings: Vec<(usize, Side, Jump)>,
    /// Its decisions.
    pub decide: Decide,
    /// The decisions the band could not make.
    pub undecided: Vec<Uncertain>,
}

impl<'p, 's> Run<'p, 's> {
    /// A run on `pair` with no station yet.
    pub(crate) fn new(pair: &'p Pair<'s>, limits: Limits) -> Self {
        let decide = Decide::new(pair.band());
        Self { pair, limits, steps: 0, stations: Vec::new(), crossings: Vec::new(), decide, undecided: Vec::new() }
    }

    /// Whether the surfaces touch at `at` rather than cross: the gap a touch of radius `longest` has where the normals
    /// are as far apart as they are there is within the band.
    ///
    /// # Errors
    ///
    /// Where the band cannot decide it.
    pub(crate) fn touches(&mut self, at: &Frame) -> Result<bool, Uncertain> {
        Ok(self.decide.side(touch(self.limits.longest, at.normals), at.here.at)? != Band::Above)
    }

    /// `here` put inside both domains, as [`inside`] puts it.
    ///
    /// # Errors
    ///
    /// Where the band cannot decide whether a parameter is on or past an end.
    pub(crate) fn inside(&mut self, here: Here) -> Result<Option<Here>, Uncertain> {
        inside(self.pair, &mut self.decide, here)
    }

    /// The march at a decision the band cannot make: it is kept, and the march ends there.
    fn unsure(&mut self, uncertain: Uncertain) -> Next {
        self.undecided.push(uncertain);
        Next::Done(End::Undecided)
    }

    /// Counts `n` steps.
    pub(crate) const fn count(&mut self, n: u32) {
        self.steps = self.steps.saturating_add(n);
    }

    /// Adds `here`, unless it is the station already last.
    fn push(&mut self, here: Here) {
        if self.stations.last().is_none_or(|last| last.at != here.at) {
            self.stations.push(here);
        }
    }

    /// `h` scaled by how far a step of it sagged from what is allowed, at most doubled, and within the shortest and
    /// longest steps: the sag of a chord grows as its square.
    fn scaled(&self, h: f64, sag: f64) -> f64 {
        let factor = if sag > 0.0 { 0.9 * (self.limits.sag / sag).sqrt() } else { 2.0 };
        let h = h * if factor < 2.0 { factor } else { 2.0 };
        let h = if h > self.limits.longest { self.limits.longest } else { h };
        if h < self.limits.shortest { self.limits.shortest } else { h }
    }

    /// The station a step of `h` from `here` leads to, and the curve's direction there: the tangent predictor, then
    /// the corrector. Where `fresh` names a pole just crossed, its side's parameters are found again from its closest
    /// point, as those on the near side do not lead to the far one.
    fn step(&mut self, here: &Frame, h: f64, fresh: Option<&Breach>) -> Option<Frame> {
        self.count(1);
        let aim = here.here.at + here.along * h;
        let seed = match fresh {
            None => here.here,
            Some(pole) => {
                let surface = self.pair.surface(pole.side);
                let point = occam_units::Point::from_vec3(aim);
                let again = surface.closest(point, None, self.pair.tol, &mut Scratch::default()).ok()?.at;
                moved(here.here, pole.side, again)
            }
        };
        let next = self.pair.correct(&seed, aim, here.along)?;
        frame(self.pair, next, here.along)
    }

    /// Whether the march, at `here` with the step `h` ahead, closes on `start`: when the start is ahead within the
    /// step, whether the corrector onto the start's own plane lands within the band of the start.
    fn closes(&mut self, here: &Frame, start: &Frame, h: f64) -> Result<bool, Uncertain> {
        let to = start.here.at - here.here.at;
        if self.stations.len() < 3 || to.norm() > 1.5 * h || to.dot(here.along) <= 0.0 {
            return Ok(false);
        }
        self.count(1);
        let Some(landed) = self.pair.correct(&here.here, start.here.at, start.along) else { return Ok(false) };
        Ok(self.decide.side(distance(landed.at, start.here.at), landed.at)? == Band::Within)
    }

    /// Marches from `from`, its station already added, until the curve closes on `start`, where one is given, or
    /// ends.
    pub(crate) fn march(&mut self, from: Frame, start: Option<&Frame>) -> End {
        let (mut here, mut h, mut fresh) = (from, self.limits.longest * 0.25, None);
        loop {
            if self.steps >= self.limits.steps {
                return End::Stalled;
            }
            match start.map(|start| self.closes(&here, start, h)) {
                Some(Ok(true)) => return End::Closed,
                Some(Err(uncertain)) => {
                    self.undecided.push(uncertain);
                    return End::Undecided;
                }
                Some(Ok(false)) | None => {}
            }
            match self.next(&here, h, fresh.take()) {
                Next::Again { h: shorter, fresh: kept } => (h, fresh) = (shorter, kept),
                Next::On { here: next, h: longer, fresh: kept } => (here, h, fresh) = (next, longer, kept),
                Next::Done(end) => return end,
            }
        }
    }

    /// One step from `here` by `h`, and what follows from it.
    fn next(&mut self, here: &Frame, h: f64, fresh: Option<Breach>) -> Next {
        let Some(next) = self.step(here, h, fresh.as_ref()) else {
            return if h > self.limits.shortest { Next::Again { h: h * 0.5, fresh } } else { self.unstick(here) };
        };
        let turn = angle(here.along, next.along);
        let sag = h * turn * 0.125;
        if (sag > 2.0 * self.limits.sag || turn > MOST_TURN) && h > self.limits.shortest {
            return Next::Again { h: self.scaled(h, sag), fresh };
        }
        match breach(self.pair, &mut self.decide, &next.here) {
            Ok(Some(edge)) => return self.edge(here, h, &edge),
            Ok(None) => {}
            Err(uncertain) => return self.unsure(uncertain),
        }
        match self.touches(&next) {
            Ok(true) => return self.touch(&next),
            Ok(false) => {}
            Err(uncertain) => return self.unsure(uncertain),
        }
        if let Some(pole) = fresh {
            self.far_side(&pole, &next.here);
        }
        self.stations.push(next.here);
        Next::On { here: next, h: self.scaled(h, sag), fresh: None }
    }

    /// After a pole, the pole's station again with its side's parameters on the far side, as `next` has them there.
    fn far_side(&mut self, pole: &Breach, next: &Here) {
        let Some(&near) = self.stations.last() else { return };
        let far = moved(near, pole.side, with(on(next, pole.side), pole.param, pole.end));
        self.crossings.push((self.stations.len().saturating_sub(1), pole.side, Jump::Pole));
        self.stations.push(far);
    }

    /// The station on the edge `edge` names, between `here`, inside, and a step of `h`, on or past it: by halving the
    /// step until a station is on the end within the band, then putting the parameter on it, which moves the point by
    /// no more than the band.
    fn locate(&mut self, here: &Frame, h: f64, edge: &Breach) -> Result<Option<Frame>, Uncertain> {
        let (mut inner, mut outer) = (0.0, h);
        for _ in 0..LOCATE {
            let s = f64::midpoint(inner, outer);
            let Some(found) = self.step(here, s, None) else {
                outer = s;
                continue;
            };
            match beside(self.pair, &mut self.decide, &found.here, edge)? {
                Band::Below => inner = s,
                Band::Above => outer = s,
                Band::Within => {
                    let uv = with(on(&found.here, edge.side), edge.param, edge.end);
                    return Ok(Some(Frame { here: moved(found.here, edge.side, uv), ..found }));
                }
            }
        }
        Ok(None)
    }

    /// The march at an edge: it leaves the surface there, or carries on across a seam from the other end, or through
    /// a pole.
    fn edge(&mut self, here: &Frame, h: f64, edge: &Breach) -> Next {
        let at = match self.locate(here, h, edge) {
            Ok(Some(at)) => at,
            Ok(None) => return Next::Done(End::Stalled),
            Err(uncertain) => return self.unsure(uncertain),
        };
        self.push(at.here);
        match meets(self.pair, &mut self.decide, &at.here, edge) {
            Ok(Meets::Leaves) => Next::Done(End::Edge),
            Ok(Meets::Across(Jump::Seam)) => {
                let far = moved(at.here, edge.side, with(on(&at.here, edge.side), edge.param, edge.other));
                self.crossings.push((self.stations.len().saturating_sub(1), edge.side, Jump::Seam));
                self.stations.push(far);
                Next::On { here: Frame { here: far, ..at }, h, fresh: None }
            }
            Ok(Meets::Across(Jump::Pole)) => Next::On { here: at, h, fresh: Some(*edge) },
            Err(uncertain) => self.unsure(uncertain),
        }
    }

    /// The march at a touch: alternating projections from `at` to where the surfaces touch, which ends it.
    fn touch(&mut self, at: &Frame) -> Next {
        let (found, rounds) = project(self.pair, &at.here);
        self.count(rounds);
        let put = match found.map(|touch| self.inside(touch)) {
            Some(Ok(put)) => put,
            Some(Err(uncertain)) => return self.unsure(uncertain),
            None => None,
        };
        self.push(put.unwrap_or(at.here));
        Next::Done(End::Touch)
    }

    /// The march where the corrector finds nothing even at the shortest step: alternating projections from a shortest
    /// step ahead, which come to a touch, or to a station inside both domains more than the band further on, from
    /// which it carries on, or to neither, where it has stalled.
    fn unstick(&mut self, here: &Frame) -> Next {
        let shortest = self.limits.shortest;
        let aim = Here { at: here.here.at + here.along * shortest, ..here.here };
        let (found, rounds) = project(self.pair, &aim);
        self.count(rounds);
        let Some(found) = found.and_then(|f| frame(self.pair, f, here.along)) else { return Next::Done(End::Stalled) };
        match self.touches(&found) {
            Ok(true) => return self.touch(&found),
            Ok(false) => {}
            Err(uncertain) => return self.unsure(uncertain),
        }
        let gone = ahead(here.here.at, found.here.at, here.along);
        let onward = self
            .decide
            .side(gone, found.here.at)
            .and_then(|band| Ok(band == Band::Above && breach(self.pair, &mut self.decide, &found.here)?.is_none()));
        match onward {
            Ok(true) => {
                self.stations.push(found.here);
                Next::On { here: found, h: shortest, fresh: None }
            }
            Ok(false) => Next::Done(End::Stalled),
            Err(uncertain) => self.unsure(uncertain),
        }
    }
}
