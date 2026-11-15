//! Source: original
//! The corrector's parts on their own: the dense solve, a start's seed, a surface's normal where its derivatives lose
//! a direction or run slowly, a start's settling by Newton's method, the second side's parameters from a seed whose
//! foot does not hold, and the corrector's own contract over many steps.

use std::f64::consts::{PI, TAU};

use occam_geom::{
    Analytic, BsplineSurface, Cylinder, Degeneracy, Domain, Jet1, Jet2, Line, ParamRange, Parametric, Revolution,
    Scratch, Shape, SplineSurface, Surface, UvBox, Wrap,
};
use occam_geom::{Content, FamilyName};
use occam_math::Real;
use occam_report::Report;
use occam_units::{Angle, Bounds, Direction, Length, ParamKind, Point, Tangent, Tolerance, UvPoint, Vec3, World};

use crate::correct::{Here, Pair, solve};
use crate::march::frame;
use crate::trace::{End, Start};
use crate::trace_tests::{
    CONVERGED, ball, gap, level, mm, moved, nurbs, nurbs_cylinder, on_both, placed, point, start_near, tol, tracer,
};

#[test]
fn a_pivot_no_larger_than_its_share_of_the_largest_entry_is_singular() {
    // The second pivot is 1e-13 of the largest entry, 4, exactly: singular. At twice that it is not.
    assert_eq!(solve([[4.0, 0.0], [0.0, 4e-13]], [1.0, 1.0]), None);
    assert_eq!(solve([[4.0, 0.0], [0.0, 8e-13]], [4.0, 8e-13]), Some([1.0, 1.0]));
}

#[test]
fn of_two_pivots_as_large_the_first_rows_is_taken() {
    let (m, r) = ([[1.0, 1.0 / 3.0], [-1.0, 0.7]], [0.1, 0.2]);
    let factor = m[1][0] / m[0][0];
    let second = (r[1] - factor * r[0]) / (m[1][1] - factor * m[0][1]);
    let first = (r[0] - m[0][1] * second) / m[0][0];
    let bits = |x: [f64; 2]| x.map(f64::to_bits);
    assert_eq!(solve(m, r).map(bits), Some(bits([first, second])), "the first row's elimination, bit for bit");
}

#[test]
fn a_seed_is_the_point_between_its_two_points() {
    let (flat, round) = (level(1.0), ball(2.0, [30.0, 0.0, 0.0]));
    let start = Start { on_a: UvPoint::new(10.0, 2.0), on_b: UvPoint::new(0.3, 0.2) };
    let seed = Pair::new(&flat, &round, tol()).seed(start);
    let want = (flat.point(start.on_a).to_vec3() + round.point(start.on_b).to_vec3()) * 0.5;
    assert_eq!(seed.at, want);
}

/// The disc of radius 3 at height 1 about Z, turned from the line along X whose parameter 2 is on the axis: a pole
/// the disc declares, with its normal down the axis, where its derivative in the angle is nothing.
fn disc() -> Revolution<Line> {
    let line = Line::new(point(-2.0, 0.0, 1.0), Direction::normalised(Vec3::new(1.0, 0.0, 0.0)));
    Revolution::new(
        point(0.0, 0.0, 0.0),
        Direction::normalised(Vec3::new(0.0, 0.0, 1.0)),
        line.trimmed(2.0, 5.0).unwrap(),
    )
}

#[test]
fn a_touch_at_a_pole_where_the_derivatives_vanish_takes_the_normal_declared_there() {
    let (disc, round) = (disc(), ball(1.0, [0.0, 0.0, 2.0]));
    let [pole] = Surface::degeneracies(&disc) else { panic!("{:?}", Surface::degeneracies(&disc)) };
    assert_eq!((pole.at, pole.normal.map(Direction::get)), (2.0, Some(Vec3::new(0.0, 0.0, -1.0))));
    let start = Start { on_a: UvPoint::new(0.3, 2.05), on_b: UvPoint::new(0.0, -1.4) };
    let touch = tracer(1e-4, 0.5, 2000).trace(&disc, &round, start);
    assert_eq!((touch.ends, touch.stations.len()), ([End::Touch; 2], 1), "{touch:?}");
    let [at] = touch.stations.as_slice() else { panic!() };
    assert!(gap(at.at, point(0.0, 0.0, 1.0)) <= CONVERGED, "{at:?}");
    assert_eq!(at.on_a.v(), 2.0, "on the pole, where only the declared normal is a normal");
    on_both(&touch, &disc, &round);
}

/// The square patch from the origin to `(10, 10)` in the plane `z = 0`, its parameters each from 0 to `span`.
fn patch(span: f64) -> SplineSurface {
    let corner = |x: f64, y: f64| point(x, y, 0.0);
    let poles = vec![corner(0.0, 0.0), corner(10.0, 0.0), corner(0.0, 10.0), corner(10.0, 10.0)];
    let knots = vec![0.0, 0.0, span, span];
    let form =
        BsplineSurface { degree: (1, 1), count: (2, 2), knots_u: knots.clone(), knots_v: knots, poles, weights: None };
    SplineSurface::new(&form).unwrap()
}

#[test]
fn a_normal_does_not_depend_on_how_fast_the_parameters_run() {
    let flat = level(3.0);
    for span in [1.0, 1e8] {
        let patch = patch(span);
        let middle = UvPoint::new(span * 0.5, span * 0.5);
        let here = Here { at: patch.point(middle).to_vec3(), p: middle, q: UvPoint::new(5.0, 5.0) };
        let normals = Pair::new(&patch, &flat, tol()).normals(&here);
        assert_eq!(normals.map(|(of_patch, _)| of_patch), Some(Vec3::new(0.0, 0.0, 1.0)), "parameters to {span}");
    }
}

#[test]
fn a_start_off_the_curve_settles_by_newtons_method_in_its_one_step() {
    // A closed rational cylinder of radius 2 about Z and a plane tilted about Y, held by its implicit form, and the
    // patch of that cylinder and a rational sphere of radius 2.2, both held by their parameters. Each start is 0.1
    // or more off the curve. A start settled by Newton's method is the one step a trace of one step takes; projections
    // would take more.
    let (cylinder, tilted) =
        (nurbs_cylinder((0.0, TAU)), occam_geom::Plane::placed(&placed([0.0, 1.0, 0.0], 0.6, [0.0, 0.0, 0.3])));
    let (sin, cos) = occam_math::sin_cos(PI * 0.5);
    let near = point(2.0 * cos, 2.0 * sin, 0.0);
    let (patch, sphere) = (nurbs_cylinder((0.5, 2.5)), nurbs(&ball(2.2, [0.0; 3]), (0.0, 0.2), (TAU, 0.8)));
    let (sin, cos) = occam_math::sin_cos(1.5);
    let off = point(2.0 * cos, 2.0 * sin, 0.84_f64.sqrt() + 0.3);
    let pairs: [(&dyn Surface, &dyn Surface, Point<World>); 2] = [(&cylinder, &tilted, near), (&patch, &sphere, off)];
    for (k, (a, b, near)) in pairs.into_iter().enumerate() {
        let start = start_near(a, b, near);
        // The second pair's seed on the sphere a little round from the foot, so its first parameter must move too.
        let start =
            if k == 1 { Start { on_b: UvPoint::new(start.on_b.u() + 0.2, start.on_b.v()), ..start } } else { start };
        assert!(gap(a.point(start.on_a), b.point(start.on_b)) > 0.1, "off the curve");
        let trace = tracer(1e-4, 1.0, 1).trace(a, b, start);
        assert_eq!((trace.ends, trace.stations.len(), trace.steps), ([End::Stalled; 2], 1, 1), "{trace:?}");
        on_both(&trace, a, b);
    }
}

/// A cylinder of radius 2 about Z whose angle runs a whole turn from `-.1` rather than from 0: the same surface, its
/// angle's range elsewhere, as nothing stops a family declaring.
struct Turned(Cylinder, f64);

impl Parametric for Turned {
    fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R> {
        self.0.eval_point(uv)
    }

    fn domain(&self) -> Domain {
        let turn = ParamRange { lo: -self.1, hi: TAU - self.1, wrap: Wrap::Periodic, kind: ParamKind::Angle };
        Domain { u: turn, ..Parametric::domain(&self.0) }
    }

    fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R> {
        self.0.eval_first(uv)
    }

    fn bounds(&self, patch: UvBox) -> Bounds {
        Parametric::bounds(&self.0, patch)
    }

    fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R> {
        self.0.eval_second(uv)
    }

    fn family(&self) -> FamilyName {
        FamilyName("turned")
    }

    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        Parametric::derivatives(&self.0, uv, order, out, scratch)
    }

    fn invert(
        &self,
        p: Point<World>,
        seed: Option<UvPoint>,
        tol: Tolerance,
        scratch: &mut Scratch,
    ) -> Result<UvPoint, Report> {
        Parametric::invert(&self.0, p, seed, tol, scratch)
    }

    fn content(&self, out: &mut Content) {
        Parametric::content(&self.0, out);
    }

    fn shape(&self) -> Shape {
        Parametric::shape(&self.0)
    }

    fn analytic(&self, tol: Tolerance) -> Option<Analytic> {
        Parametric::analytic(&self.0, tol)
    }

    fn degeneracies(&self) -> &[Degeneracy] {
        &[]
    }
}

#[test]
fn where_a_seeds_foot_does_not_hold_the_parameters_come_round_to_the_seeds_turn() {
    // A seed whose foot does not hold, its height not a number, with its angle two turns and a little on from the
    // point's: the closest point's angle, in the seed's turn, whether the angle's range starts half a turn or a hair
    // before nought.
    for start in [PI, 0.01] {
        let round = Turned(Cylinder::placed(&moved([0.0; 3]), mm(2.0)).unwrap(), start);
        let flat = level(0.5);
        let pair = Pair::new(&flat, &round, tol());
        let (sin, cos) = occam_math::sin_cos(2.0);
        let at = Vec3::new(2.0 * cos, 2.0 * sin, 0.5);
        let seed = UvPoint::new(2.0 + 2.0 * TAU + 0.3, f64::NAN);
        let q = pair.q_at(at, seed).expect("its closest point holds");
        assert!((q.u() - (2.0 + 2.0 * TAU)).abs() < 1e-9 && (q.v() - 0.5).abs() < 1e-9, "from {start}: {q:?}");
    }
}

#[test]
fn a_touchs_parameters_on_a_side_held_implicitly_do_not_depend_on_the_turn_of_its_seed() {
    // A start too far from the touch for Newton's method to settle within the longest step, so projections start from
    // the seed, its angle on the sphere a turn or two on: the touch's parameters are the sphere's own for its point.
    let (plane, sphere) = (level(2.0), ball(2.0, [0.0; 3]));
    let touches: Vec<_> = [0.0, TAU, 2.0 * TAU]
        .into_iter()
        .map(|turns| {
            let start = Start { on_a: UvPoint::new(0.05, -0.03), on_b: UvPoint::new(0.3 + turns, 1.4) };
            tracer(1e-4, 0.01, 2000).trace(&plane, &sphere, start)
        })
        .collect();
    assert_eq!(touches[0].ends, [End::Touch; 2], "{:?}", touches[0]);
    // The seeds' points are a turn's rounding apart, and an angle at a pole is all rounding: the same to that, not to
    // the turns between the seeds.
    let first = touches[0].stations[0];
    for touch in &touches {
        let [at] = touch.stations.as_slice() else { panic!("{touch:?}") };
        assert!(gap(at.at, first.at) < 1e-12 && (at.on_b.u() - first.on_b.u()).abs() < 1e-3, "{at:?} and {first:?}");
    }
}

#[test]
fn a_touch_whose_curvatures_differ_across_it_converges_by_projection() {
    // A sphere of radius 2 resting on a cylinder of radius 1 about X: across the cylinder they part faster than along
    // it, so a round of projections does not head straight for the touch, and the rounds go on until they stop moving.
    let about_x = Cylinder::placed(&placed([0.0, 1.0, 0.0], std::f64::consts::FRAC_PI_2, [0.0; 3]), mm(1.0)).unwrap();
    let resting = ball(2.0, [0.0, 0.0, 3.0]);
    let start = start_near(&about_x, &resting, point(0.05, 0.04, 1.0));
    let touch = tracer(1e-4, 0.5, 2000).trace(&about_x, &resting, start);
    assert_eq!((touch.ends, touch.stations.len()), ([End::Touch; 2], 1), "{touch:?}");
    assert!(gap(touch.stations[0].at, point(0.0, 0.0, 1.0)) <= CONVERGED, "{touch:?}");
    on_both(&touch, &about_x, &resting);
}

#[test]
fn a_start_projections_bring_onto_a_shallow_crossing_marches_from_it() {
    // Two planes 0.01 apart in angle, crossing in the X axis, and a start 0.05 off it, too far for Newton's method to
    // settle within the longest step: projections bring it on, a round's move falling within the band before its gap.
    let (flat, tilted) = (level(0.0), occam_geom::Plane::placed(&placed([1.0, 0.0, 0.0], 0.01, [0.0; 3])));
    let start = Start { on_a: UvPoint::new(0.3, 0.05), on_b: UvPoint::new(0.3, 0.05) };
    let trace = tracer(1e-4, 0.01, 40).trace(&flat, &tilted, start);
    assert_eq!(trace.ends, [End::Stalled; 2], "a line has no end: {trace:?}");
    assert!(trace.stations.iter().all(|s| s.at.to_vec3().y.abs() < 1e-6 && s.at.to_vec3().z.abs() < 1e-6));
    on_both(&trace, &flat, &tilted);
}

#[test]
fn the_corrector_lands_in_its_plane_within_a_quarter_of_the_band_of_both_at_every_step() {
    // The patch of the cylinder and the rational sphere, both held by their parameters, with the band coarse enough
    // that a Newton step can land inside it but outside its quarter.
    let coarse = Tolerance::new(Length::from_millimetres(1e-3), Angle::from_radians(1e-9)).unwrap();
    let (patch, sphere) = (nurbs_cylinder((0.5, 2.5)), nurbs(&ball(2.2, [0.0; 3]), (0.0, 0.2), (TAU, 0.8)));
    let (sin, cos) = occam_math::sin_cos(1.5);
    let start = start_near(&patch, &sphere, point(2.0 * cos, 2.0 * sin, 0.84_f64.sqrt()));
    let pair = Pair::new(&patch, &sphere, coarse);
    let here = pair.settle(&pair.seed(start)).expect("the start settles");
    let along = frame(&pair, here, Vec3::new(0.0, 0.0, 0.0)).expect("a direction").along;
    for k in 1..=60_u8 {
        let aim = here.at + along * (f64::from(k) * 0.01);
        let landed = pair.correct(&here, aim, along).unwrap_or_else(|| panic!("step {k}"));
        let off = (patch.point(landed.p).to_vec3() - sphere.point(landed.q).to_vec3()).norm();
        assert!(off <= 0.25e-3 && (landed.at - aim).dot(along).abs() <= 1e-3, "step {k}: {off:e}");
    }
}
