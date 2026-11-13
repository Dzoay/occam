//! Source: original
//! A body's geometry moved by a map, and what follows from it. Each curve and surface moves through the seam: where its
//! family keeps its form under the map it moves itself, and says how its parameters move; otherwise the exact B-spline
//! of a surface over its faces' patch, or of a curve over its edge's range, is moved, whose parameters agree with the
//! family's at the patch's or range's ends (`docs/decisions/2026-11-12-moving-geometry.md`).
//!
//! Each face's parameter curves and each loop's witness follow the face's surface: through the map of its parameters
//! where the family kept its form, and onto the B-spline it became otherwise, by finding there where each point went. A
//! parameter curve that the map does not keep in its form becomes its own B-spline, mapped. On a face that became a
//! B-spline, only a line of constant parameter is followed exactly, and anything else is refused, since nothing is
//! approximated yet. Each edge's range follows its curve: through the map of its parameter, or kept, where the curve
//! became a B-spline over that range.

use std::collections::BTreeMap;
use std::sync::Arc;

use occam_geom::{
    BsplineCurve, Curve, ParamCurve, ParamRange, Scratch, SplineCurve, SplineSurface, Surface, UvBox, UvCurve, UvMap,
    Wrap,
};
use occam_report::{Code, EntityRef, Fault, Input, Report};
use occam_topo::{Edge, EdgeUse, Edit, Face, Id, Loop, Sheet, SurfaceGeom, UseOwner, Vertex, View};
use occam_units::{Affine, Point, Tolerance, UvPoint, Vec3, World};

use super::body::Parts;

/// How far a parameter curve's moved ends may stray from its edge's moved range, as a part of each, and still be the
/// curve kept: a map of the face's parameters and of the edge's agree, rounding aside.
const AGREE: f64 = 1e-12;

/// How a face's parameters move.
enum Params {
    /// By a map, under which the face's surface family kept its form.
    Mapped(UvMap),
    /// Onto the B-spline the surface became, found there point by point.
    Found,
}

/// A surface of the body moved, and how its faces' parameters follow it.
struct Moved {
    /// The surface it was.
    old: Arc<dyn Surface>,
    /// The surface it is.
    new: Arc<dyn Surface>,
    /// How its parameters move.
    params: Params,
}

/// An edge's curve moved.
pub(super) struct EdgeMove {
    /// The curve.
    pub curve: Arc<dyn Curve>,
    /// The edge's range on it.
    pub range: (f64, f64),
    /// Whether the curve is this edge's own, a B-spline over its range, rather than the moved curve every edge on the
    /// same curve shares.
    pub own: bool,
}

/// Everything a map does to a body's records.
pub(super) struct Moves {
    /// Each surface, moved.
    pub surfaces: BTreeMap<Id<SurfaceGeom>, Arc<dyn Surface>>,
    /// Each face whose sheet the move resets: one that became a B-spline, whose parameters do not come round.
    pub sheets: BTreeMap<Id<Face>, Sheet>,
    /// Each loop's witness, moved.
    pub witnesses: BTreeMap<Id<Loop>, UvPoint>,
    /// Each edge's curve and range.
    pub edges: BTreeMap<Id<Edge>, EdgeMove>,
    /// Each use's parameter curve, if it had one.
    pub pcurves: BTreeMap<Id<EdgeUse>, Arc<dyn ParamCurve>>,
    /// Each vertex's point.
    pub points: BTreeMap<Id<Vertex>, Point<World>>,
    /// A factor no length grows by more than.
    pub most: f64,
}

/// `entity` has no exact form under the scale at `at`.
fn no_exact_form(entity: EntityRef, at: &Input) -> Report {
    Report::new(Code::NoExactForm, Fault::NotImplemented).with_entity(entity).with_input(at.clone())
}

/// The lowest and highest of two numbers.
fn spread(a: f64, b: f64) -> (f64, f64) {
    if b < a { (b, a) } else { (a, b) }
}

/// The box holding both `a` and `b`.
fn joined(a: UvBox, b: UvBox) -> UvBox {
    let (u, v) = (
        (spread(a.lo.u(), b.lo.u()).0, spread(a.hi.u(), b.hi.u()).1),
        (spread(a.lo.v(), b.lo.v()).0, spread(a.hi.v(), b.hi.v()).1),
    );
    UvBox::new(UvPoint::new(u.0, v.0), UvPoint::new(u.1, v.1))
}

/// `(lo, hi)` within `range`: no more than a period, where it comes round, and inside it, where it does not.
fn within(range: ParamRange, (lo, hi): (f64, f64)) -> (f64, f64) {
    if range.wrap == Wrap::Periodic {
        let top = lo + (range.hi - range.lo);
        return (lo, if hi > top { top } else { hi });
    }
    (if lo < range.lo { range.lo } else { lo }, if hi > range.hi { range.hi } else { hi })
}

/// The patch of `surface` its faces in `parts` cover: the box of their parameter curves over their edges' ranges, kept
/// within its domain. `None` if a use of one of them has no parameter curve.
fn patch(edit: &Edit, parts: &Parts, surface: Id<SurfaceGeom>) -> Option<UvBox> {
    let model = edit.base();
    let mut found: Option<UvBox> = None;
    for face in parts.faces.iter().filter_map(|&f| model.view(f)).filter(|f| f.surface_id() == surface) {
        for view in face
            .loops()
            .into_iter()
            .filter_map(|l| model.view(l))
            .flat_map(View::<Loop>::uses)
            .filter_map(|u| model.view(u))
        {
            let (lo, hi) = model.view(view.edge())?.range();
            let bounds = view.pcurve()?.bounds(lo, hi);
            found = Some(found.map_or(bounds, |box_| joined(box_, bounds)));
        }
    }
    let (found, domain) = (found?, model.view(surface)?.get().domain());
    let (u, v) = (within(domain.u, (found.lo.u(), found.hi.u())), within(domain.v, (found.lo.v(), found.hi.v())));
    Some(UvBox::new(UvPoint::new(u.0, v.0), UvPoint::new(u.1, v.1)))
}

/// `surface` moved by `map`: as its own family where that keeps its form, else its B-spline over `patch`.
fn surface(old: Arc<dyn Surface>, patch: impl FnOnce() -> Option<UvBox>, map: &Affine) -> Option<Moved> {
    if let Some((new, params)) = old.moved(map) {
        return Some(Moved { old, new, params: Params::Mapped(params) });
    }
    let (spline, _) = occam_geom::Parametric::moved(&SplineSurface::new(&old.to_bspline(patch()?)?).ok()?, map)?;
    Some(Moved { old, new: Arc::new(spline), params: Params::Found })
}

/// `curve` from `lo` to `hi` moved by `map`, as its own family where that keeps its form, else as its B-spline over the
/// range, which is the edge's own.
fn curve(old: &dyn Curve, (lo, hi): (f64, f64), map: &Affine) -> Option<EdgeMove> {
    if let Some((curve, params)) = old.moved(map) {
        return Some(EdgeMove { curve, range: (params.of(lo), params.of(hi)), own: false });
    }
    let (spline, _) = occam_geom::ParametricCurve::moved(&SplineCurve::new(&old.to_bspline(lo, hi)?).ok()?, map)?;
    let range = occam_geom::ParametricCurve::range(&spline);
    Some(EdgeMove { curve: Arc::new(spline), range: (range.lo, range.hi), own: true })
}

/// The point of the plane of a face's parameters at `uv`, as a B-spline in them holds it.
fn lifted(uv: UvPoint) -> Point<World> {
    Point::from_vec3(Vec3::new(uv.u(), uv.v(), 0.0))
}

/// `knots` moved from running over `from` to running over `to`, their ends exactly there.
fn rescaled(knots: &[f64], from: (f64, f64), to: (f64, f64)) -> Vec<f64> {
    let rate = (to.1 - to.0) / (from.1 - from.0);
    #[expect(clippy::float_cmp, reason = "a knot at an end of the range, as the B-spline was asked for over it")]
    let at = |k: f64| {
        if k == from.0 {
            to.0
        } else if k == from.1 {
            to.1
        } else {
            to.0 + (k - from.0) * rate
        }
    };
    knots.iter().map(|&k| at(k)).collect()
}

/// The parameter curve in the plane of the face's parameters that `form` is, if it is one.
fn drawn(form: &BsplineCurve) -> Option<Arc<dyn ParamCurve>> {
    Some(Arc::new(UvCurve::new(SplineCurve::new(form).ok()?).ok()?))
}

/// The parameter curve `old` from `from.0` to `from.1` moved by the map `params` of its face's parameters, to run from
/// `to.0` to `to.1`: as its own family where that keeps its form and its parameter moves as the edge's does, else as
/// its B-spline, mapped. A family's moved range is its range moved, so a parameter moved as the edge's stays in it.
fn mapped(old: &dyn ParamCurve, params: &UvMap, from: (f64, f64), to: (f64, f64)) -> Option<Arc<dyn ParamCurve>> {
    let agree = |a: f64, b: f64| (a - b).abs() <= AGREE * (1.0 + b.abs());
    if let Some((moved, m)) = old.moved(params)
        && agree(m.of(from.0), to.0)
        && agree(m.of(from.1), to.1)
    {
        return Some(moved);
    }
    let mut form = old.to_bspline(from.0, from.1)?;
    form.poles = form.poles.iter().map(|p| lifted(params.of(UvPoint::new(p.to_vec3().x, p.to_vec3().y)))).collect();
    form.knots = rescaled(&form.knots, from, to);
    drawn(&form)
}

/// What a map does to a body's geometry, for writing.
struct Mover<'a> {
    edit: &'a Edit,
    map: &'a Affine,
    tol: Tolerance,
    scale_at: &'a Input,
}

impl Mover<'_> {
    /// Where the point of `moved`'s old surface at `uv` goes on its new one, found there from `uv`, near which it is.
    fn found(&self, moved: &Moved, uv: UvPoint) -> Option<UvPoint> {
        let target = self.map.point(moved.old.point(uv));
        moved.new.invert(target, Some(uv), self.tol, &mut Scratch::default()).ok()
    }

    /// Where a point of a face's parameters goes.
    fn point(&self, moved: &Moved, uv: UvPoint) -> Option<UvPoint> {
        match moved.params {
            Params::Mapped(params) => Some(params.of(uv)),
            Params::Found => self.found(moved, uv),
        }
    }

    /// The parameter curve `old` of a face whose surface moved as `moved`, from `from.0` to `from.1` before and from
    /// `to.0` to `to.1` after. On a face that became a B-spline it must be a straight line of constant parameter, whose
    /// ends are found there.
    #[expect(clippy::float_cmp, reason = "a line of constant parameter holds it exactly")]
    fn pcurve(
        &self,
        old: &dyn ParamCurve,
        moved: &Moved,
        from: (f64, f64),
        to: (f64, f64),
    ) -> Option<Arc<dyn ParamCurve>> {
        if let Params::Mapped(params) = moved.params {
            return mapped(old, &params, from, to);
        }
        // Two poles make a straight line: a B-spline has at least one more pole than its degree.
        old.to_bspline(from.0, from.1).filter(|f| f.poles.len() == 2)?;
        let (a, b) = (old.uv(from.0), old.uv(from.1));
        let (fixed_u, fixed_v) = (a.u() == b.u(), a.v() == b.v());
        if fixed_u == fixed_v {
            return None;
        }
        // The constant parameter is found from the line's middle, since an end may be a pole, where it is any.
        let middle = self.found(moved, old.uv(f64::midpoint(from.0, from.1)))?;
        let (p, q) = (self.found(moved, a)?, self.found(moved, b)?);
        let (p, q) = if fixed_u {
            (UvPoint::new(middle.u(), p.v()), UvPoint::new(middle.u(), q.v()))
        } else {
            (UvPoint::new(p.u(), middle.v()), UvPoint::new(q.u(), middle.v()))
        };
        drawn(&BsplineCurve {
            degree: 1,
            knots: vec![to.0, to.0, to.1, to.1],
            poles: vec![lifted(p), lifted(q)],
            weights: None,
        })
    }

    /// `entity` has no exact form under the map.
    fn refused(&self, entity: EntityRef) -> Report {
        no_exact_form(entity, self.scale_at)
    }
}

impl Moves {
    /// What `map`, from the scale at `scale_at`, does to the body whose `parts` are read from `edit`.
    ///
    /// # Errors
    ///
    /// [`Code::NoExactForm`] for a curve or surface with no exact form under `map`, naming it, or for a parameter curve
    /// that cannot follow its face's surface exactly.
    pub(super) fn of(edit: &Edit, parts: &Parts, map: &Affine, scale_at: &Input) -> Result<Self, Report> {
        let model = edit.base();
        let mover = Mover { edit, map, tol: model.tolerance(), scale_at };
        let mut by_surface: BTreeMap<Id<SurfaceGeom>, Moved> = BTreeMap::new();
        for face in parts.faces.iter().filter_map(|&f| model.view(f)) {
            let id = face.surface_id();
            if by_surface.contains_key(&id) {
                continue;
            }
            let old = edit.record(id).map(|record| record.0).ok_or_else(|| mover.refused(id.entity()))?;
            let found = surface(old, || patch(mover.edit, parts, id), map).ok_or_else(|| mover.refused(id.entity()))?;
            by_surface.insert(id, found);
        }
        let mut edges = BTreeMap::new();
        for edge in parts.edges.iter().filter_map(|&e| model.view(e)) {
            let refused = || mover.refused(edge.curve_id().entity());
            let found = edge.curve().and_then(|c| curve(c, edge.range(), map)).ok_or_else(refused)?;
            edges.insert(edge.id(), found);
        }
        let mut pcurves = BTreeMap::new();
        for view in parts.uses.iter().filter_map(|&u| model.view(u)) {
            let (Some(old), Some(id), UseOwner::Loop(owner)) = (view.pcurve(), view.pcurve_id(), view.owner()) else {
                continue;
            };
            let face = model.view(owner).and_then(|l| model.view(l.face()));
            let surface =
                face.and_then(|f| by_surface.get(&f.surface_id())).ok_or_else(|| mover.refused(id.entity()))?;
            let from = model.view(view.edge()).map(View::<Edge>::range).ok_or_else(|| mover.refused(id.entity()))?;
            let to = edges.get(&view.edge()).map_or(from, |m: &EdgeMove| m.range);
            let found = mover.pcurve(old, surface, from, to).ok_or_else(|| mover.refused(id.entity()))?;
            pcurves.insert(view.id(), found);
        }
        let mut witnesses = BTreeMap::new();
        let mut sheets = BTreeMap::new();
        for face in parts.faces.iter().filter_map(|&f| model.view(f)) {
            let surface = by_surface.get(&face.surface_id()).ok_or_else(|| mover.refused(face.id().entity()))?;
            if matches!(surface.params, Params::Found) {
                sheets.insert(face.id(), Sheet { u: 0, v: 0 });
            }
            for l in face.loops().into_iter().filter_map(|l| model.view(l)) {
                let witness = mover.point(surface, l.witness()).ok_or_else(|| mover.refused(face.id().entity()))?;
                witnesses.insert(l.id(), witness);
            }
        }
        let points = parts.vertices.iter().filter_map(|&v| Some((v, map.point(model.view(v)?.point())))).collect();
        let surfaces = by_surface.into_iter().map(|(id, m)| (id, m.new)).collect();
        Ok(Self { surfaces, sheets, witnesses, edges, pcurves, points, most: map.most() })
    }
}

#[cfg(test)]
pub(super) fn surface_moved(old: Arc<dyn Surface>, patch: UvBox, map: &Affine) -> Option<Arc<dyn Surface>> {
    surface(old, || Some(patch), map).map(|m| m.new)
}

#[cfg(test)]
mod tests {
    use core::f64::consts::{FRAC_PI_2, PI, TAU};

    use occam_geom::{Line, ParamMap};
    use occam_units::{Direction, ParamKind};

    use super::*;

    fn range(lo: f64, hi: f64, wrap: Wrap) -> ParamRange {
        ParamRange { lo, hi, wrap, kind: ParamKind::Angle }
    }

    #[test]
    fn a_patch_is_kept_to_a_period_where_its_parameter_comes_round_and_inside_its_range_where_not() {
        let turn = range(-PI, PI, Wrap::Periodic);
        assert_eq!(within(turn, (1.0, 9.0)), (1.0, 1.0 + TAU));
        assert_eq!(within(turn, (1.0, 3.0)), (1.0, 3.0));
        let open = range(-FRAC_PI_2, FRAC_PI_2, Wrap::Closed);
        assert_eq!(within(open, (-2.0, 2.0)), (-FRAC_PI_2, FRAC_PI_2));
        assert_eq!(within(open, (-1.0, 1.0)), (-1.0, 1.0));
    }

    #[test]
    fn knots_move_with_the_range_they_run_over_their_ends_exactly_there() {
        assert_eq!(rescaled(&[1.0, 1.0, 2.0, 3.0, 3.0], (1.0, 3.0), (2.0, 6.0)), [2.0, 2.0, 4.0, 6.0, 6.0]);
        assert_eq!(rescaled(&[0.5, 1.0, 1.5], (0.5, 1.5), (-3.0, 5.0)), [-3.0, 1.0, 5.0]);
    }

    /// The line `u = 0.5` in a face's parameters, its parameter the distance along `v`.
    fn along_v() -> UvCurve<Line> {
        let line = Line::new(lifted(UvPoint::new(0.5, 0.0)), Direction::normalised(Vec3::new(0.0, 1.0, 0.0)));
        UvCurve::new(line).unwrap()
    }

    /// [`along_v`] from `from.0` to `from.1` moved as the face's `v` doubles, to run from `to.0` to `to.1`: its family,
    /// and its points at its ends.
    fn moved(from: (f64, f64), to: (f64, f64)) -> (&'static str, UvPoint, UvPoint) {
        let params = UvMap::apart(ParamMap::IDENTITY, ParamMap::scaled(2.0));
        let curve = mapped(&along_v(), &params, from, to).unwrap();
        (curve.family().0, curve.uv(to.0), curve.uv(to.1))
    }

    #[test]
    fn a_parameter_curve_keeps_its_family_where_its_parameter_moves_as_its_edges_does() {
        let at = |u: f64, v: f64| UvPoint::new(u, v);
        // Doubled with its edge's range: the line it was, its ends where the map takes them.
        assert_eq!(moved((0.0, 3.0), (0.0, 6.0)), ("line", at(0.5, 0.0), at(0.5, 6.0)));
        // Within a part in 10^12 of each end and one, as rounding leaves a range moved two ways: a line still.
        assert_eq!(moved((0.0, 3.0), (1e-14, 6.0)).0, "line");
        assert_eq!(moved((0.0, 500.0), (0.0, 1000.0 + 1e-11)).0, "line");
        // An edge whose range did not move: a B-spline, mapped, running over the edge's range from end to end.
        let (family, start, end) = moved((0.0, 3.0), (0.0, 3.0));
        assert_eq!((family, start, end), ("bspline", at(0.5, 0.0), at(0.5, 6.0)));
    }
}
