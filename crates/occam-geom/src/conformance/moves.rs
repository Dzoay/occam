//! Source: original
//! [`Check::Moves`]: a family moved by a map keeps its form, and every point goes where the map takes it, at the
//! parameter the family says it goes to (`docs/decisions/2026-11-12-moving-geometry.md`). Under a rigid map and under a
//! scale the same in every direction, every family keeps its form; under a stretch, a family may decline, and where it
//! does not, it is exact the same way.

use occam_units::{Affine, Angle, Direction, Placement, UvPoint, Vec3, Vector, World};

use super::{Check, Findings, fractions, lerp, placement};
use crate::{Curve, ParamCurve, ParamMap, Surface, UvBox, UvMap};

/// A turn about a skew axis and a shift, so that no axis of the map is a world axis.
fn turn() -> Placement<World, World> {
    let axis = Direction::normalised(Vec3::new(-2.0, 1.0, 3.0));
    Placement::rotation(axis, Angle::from_radians(1.1))
        .then(&Placement::translation(Vector::from_vec3(Vec3::new(3.0, -2.0, 5.5))))
}

/// The maps a family is moved by, each with whether it must keep its form under it: a rigid map, a scale up and a scale
/// down, each after a turn, and a stretch along the axes of the suite's placement.
fn maps() -> [(&'static str, Affine, bool); 4] {
    let scaled = |s: f64| Affine::similarity(turn(), s).unwrap_or_else(Affine::identity);
    let stretch = Affine::stretch(&placement(), [2.0, 0.5, 1.3]).unwrap_or_else(Affine::identity);
    [
        ("rigid", Affine::placed(turn()), true),
        ("up", scaled(2.5), true),
        ("down", scaled(0.4), true),
        ("stretch", stretch, false),
    ]
}

/// The maps of a face's parameters a parameter curve is moved by, each with whether it must keep its form: a scale the
/// same in both and a shift, which is a similarity of the plane, then one that differs between them, and one that mixes
/// them, as a plane stretched askew moves its parameters.
const UV_MAPS: [(&str, UvMap, bool); 3] = [
    ("alike", UvMap { u: [2.0, 0.0, 0.5], v: [0.0, 2.0, -1.0] }, true),
    ("apart", UvMap { u: [1.0, 0.0, 0.3], v: [0.0, 3.0, 0.0] }, false),
    ("mixed", UvMap { u: [1.5, 0.4, 0.1], v: [0.0, 0.8, -0.2] }, false),
];

/// Whether `got` is `want` to a part in 10^11 of its size and one.
fn near(got: Vec3, want: Vec3) -> bool {
    let off = (got - want).norm();
    off <= 1e-11 * (1.0 + want.norm())
}

/// Whether `params` keeps the sense of a curve's parameter.
fn keeps_sense(params: ParamMap) -> bool {
    params.scale > 0.0 && params.scale.is_finite() && params.shift.is_finite()
}

/// `s` moved by each map: kept where it must be, as its own family, and over `window` each point where the map takes it.
pub(super) fn surface(s: &dyn Surface, window: UvBox, found: &mut Findings) {
    let samples: Vec<UvPoint> = fractions(5)
        .flat_map(|fu| {
            fractions(5).map(move |fv| {
                UvPoint::new(lerp(window.lo.u(), window.hi.u(), fu), lerp(window.lo.v(), window.hi.v(), fv))
            })
        })
        .collect();
    for (name, map, kept) in maps() {
        let Some((moved, params)) = s.moved(&map) else {
            found.expect(Check::Moves, !kept, || format!("{:?} declines the {name} map", s.family()));
            continue;
        };
        found
            .expect(Check::Moves, moved.family() == s.family(), || format!("{name}: a {:?} came back", moved.family()));
        let turns = params.u[0] * params.v[1] - params.u[1] * params.v[0];
        found.expect(Check::Moves, turns > 0.0, || format!("{name}: {params:?} turns the parameters over"));
        for &uv in &samples {
            let (want, got) = (map.point(s.point(uv)).to_vec3(), moved.point(params.of(uv)).to_vec3());
            found.expect(Check::Moves, near(got, want), || format!("{name} at {uv:?}: {got:?} for {want:?}"));
        }
    }
}

/// `c` moved by each map: kept where it must be, as its own family, and from `lo` to `hi` each point where the map takes
/// it.
pub(super) fn curve(c: &dyn Curve, (lo, hi): (f64, f64), found: &mut Findings) {
    for (name, map, kept) in maps() {
        let Some((moved, params)) = c.moved(&map) else {
            found.expect(Check::Moves, !kept, || format!("{:?} declines the {name} map", c.family()));
            continue;
        };
        found
            .expect(Check::Moves, moved.family() == c.family(), || format!("{name}: a {:?} came back", moved.family()));
        found.expect(Check::Moves, keeps_sense(params), || format!("{name}: {params:?} turns the parameter over"));
        for t in fractions(9).map(|f| lerp(lo, hi, f)) {
            let (want, got) = (map.point(c.point(t)).to_vec3(), moved.point(params.of(t)).to_vec3());
            found.expect(Check::Moves, near(got, want), || format!("{name} at {t}: {got:?} for {want:?}"));
        }
    }
}

/// The parameter curve `c` moved by each map of the face's parameters, as [`curve`] checks a curve.
pub(super) fn param_curve(c: &dyn ParamCurve, (lo, hi): (f64, f64), found: &mut Findings) {
    let flat = |uv: UvPoint| Vec3::new(uv.u(), uv.v(), 0.0);
    for (name, map, kept) in UV_MAPS {
        let Some((moved, params)) = c.moved(&map) else {
            found.expect(Check::Moves, !kept, || format!("{:?} declines the {name} map", c.family()));
            continue;
        };
        found
            .expect(Check::Moves, moved.family() == c.family(), || format!("{name}: a {:?} came back", moved.family()));
        found.expect(Check::Moves, keeps_sense(params), || format!("{name}: {params:?} turns the parameter over"));
        for t in fractions(9).map(|f| lerp(lo, hi, f)) {
            let (want, got) = (flat(map.of(c.uv(t))), flat(moved.uv(params.of(t))));
            found.expect(Check::Moves, near(got, want), || format!("{name} at {t}: {got:?} for {want:?}"));
        }
    }
}

#[cfg(test)]
mod tests {
    use occam_math::Real;
    use occam_units::{Bounds, Point, Tangent, Tolerance, World};

    use super::*;
    use crate::conformance::{checks, curve as curve_suite, surface as surface_suite};
    use crate::{
        Closest, Content, Domain, FamilyName, Jet1, Jet2, Line, Parametric, ParametricCurve, Plane, Scratch, Shape,
    };

    /// A plane that moves as it should but says its parameters go elsewhere, or declines every map.
    struct Misplaced {
        plane: Plane,
        declines: bool,
    }

    impl Parametric for Misplaced {
        fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R> {
            self.plane.eval_point(uv)
        }

        fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R> {
            self.plane.eval_first(uv)
        }

        fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R> {
            self.plane.eval_second(uv)
        }

        fn derivatives(
            &self,
            uv: UvPoint,
            order: u8,
            out: &mut Vec<Tangent<World>>,
            scratch: &mut Scratch,
        ) -> Result<(), occam_report::Report> {
            Parametric::derivatives(&self.plane, uv, order, out, scratch)
        }

        fn domain(&self) -> Domain {
            Parametric::domain(&self.plane)
        }

        fn bounds(&self, patch: UvBox) -> Bounds {
            Parametric::bounds(&self.plane, patch)
        }

        fn invert(
            &self,
            p: Point<World>,
            seed: Option<UvPoint>,
            tol: Tolerance,
            scratch: &mut Scratch,
        ) -> Result<UvPoint, occam_report::Report> {
            Parametric::invert(&self.plane, p, seed, tol, scratch)
        }

        fn closest(
            &self,
            p: Point<World>,
            seed: Option<UvPoint>,
            tol: Tolerance,
            scratch: &mut Scratch,
        ) -> Result<Closest, occam_report::Report> {
            Parametric::closest(&self.plane, p, seed, tol, scratch)
        }

        fn family(&self) -> FamilyName {
            Parametric::family(&self.plane)
        }

        fn content(&self, out: &mut Content) {
            Parametric::content(&self.plane, out);
        }

        fn shape(&self) -> Shape {
            Parametric::shape(&self.plane)
        }

        fn analytic(&self, tol: Tolerance) -> Option<crate::Analytic> {
            Parametric::analytic(&self.plane, tol)
        }

        fn to_bspline(&self, patch: UvBox) -> Option<crate::BsplineSurface> {
            Parametric::to_bspline(&self.plane, patch)
        }

        fn moved(&self, map: &Affine) -> Option<(Self, UvMap)> {
            let (plane, params) = Parametric::moved(&self.plane, map).filter(|_| !self.declines)?;
            let off = UvMap { u: [params.u[0], params.u[1], params.u[2] + 1e-6], ..params };
            Some((Self { plane, declines: false }, off))
        }
    }

    #[test]
    fn the_suite_catches_a_surface_that_moves_its_parameters_wrongly_or_declines_a_similarity() {
        let window = UvBox::new(UvPoint::new(-2.0, -1.0), UvPoint::new(3.0, 4.0));
        let plane = Plane::placed(&placement());
        assert_eq!(checks(&surface_suite(&plane, window)), []);
        for declines in [false, true] {
            let found = surface_suite(&Misplaced { plane, declines }, window);
            assert_eq!(checks(&found), [Check::Moves], "declines: {declines}");
        }
    }

    /// A line that says its parameter goes nowhere when it is stretched.
    struct Unstretched(Line);

    impl ParametricCurve for Unstretched {
        fn eval_point<R: Real>(&self, t: R) -> Point<World, R> {
            self.0.eval_point(t)
        }

        fn eval_first<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>) {
            self.0.eval_first(t)
        }

        fn eval_second<R: Real>(&self, t: R) -> (Point<World, R>, Tangent<World, R>, Tangent<World, R>) {
            self.0.eval_second(t)
        }

        fn derivatives(
            &self,
            t: f64,
            order: u8,
            out: &mut Vec<Tangent<World>>,
            scratch: &mut Scratch,
        ) -> Result<(), occam_report::Report> {
            ParametricCurve::derivatives(&self.0, t, order, out, scratch)
        }

        fn range(&self) -> crate::ParamRange {
            ParametricCurve::range(&self.0)
        }

        fn bounds(&self, lo: f64, hi: f64) -> Bounds {
            ParametricCurve::bounds(&self.0, lo, hi)
        }

        fn invert(
            &self,
            p: Point<World>,
            seed: Option<f64>,
            tol: Tolerance,
            scratch: &mut Scratch,
        ) -> Result<f64, occam_report::Report> {
            ParametricCurve::invert(&self.0, p, seed, tol, scratch)
        }

        fn within(&self, centre: Point<World>, along: Option<Direction<World>>, radius: f64) -> Option<(f64, f64)> {
            ParametricCurve::within(&self.0, centre, along, radius)
        }

        fn family(&self) -> FamilyName {
            ParametricCurve::family(&self.0)
        }

        fn content(&self, out: &mut Content) {
            ParametricCurve::content(&self.0, out);
        }

        fn shape(&self) -> Shape {
            ParametricCurve::shape(&self.0)
        }

        fn moved(&self, map: &Affine) -> Option<(Self, ParamMap)> {
            let (line, params) = ParametricCurve::moved(&self.0, map)?;
            let params = if map.similar().is_some() { params } else { ParamMap::IDENTITY };
            Some((Self(line), params))
        }
    }

    #[test]
    fn the_suite_catches_a_curve_that_moves_its_parameter_wrongly_under_a_stretch() {
        let line =
            Line::new(Point::from_vec3(Vec3::new(1.0, 2.0, 3.0)), Direction::normalised(Vec3::new(1.0, 1.0, 0.0)));
        assert_eq!(checks(&curve_suite(&line, -5.0, 7.0)), []);
        assert_eq!(checks(&curve_suite(&Unstretched(line), -5.0, 7.0)), [Check::Moves]);
    }
}
