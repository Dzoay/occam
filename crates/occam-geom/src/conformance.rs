//! Source: original
//! The conformance suite: S4's eight checks as one harness, which every family runs in the step that adds it (L21). If a
//! family needed a special case here, the seam would not be common. The checks report what failed rather than stopping,
//! so that a test can show a seeded fault caught by the check meant to catch it.

mod curves;
mod seeded;
mod surfaces;

use std::collections::BTreeSet;

use occam_math::canonical_bits;
use occam_units::{Angle, Direction, Length, Local, Placement, Point, Tangent, Tolerance, Vec3, Vector, World};

pub(crate) use curves::curve;
pub(crate) use seeded::{WrongSecondCurve, wrong_form, wrong_second};
pub(crate) use surfaces::surface;

/// What the suite checks.
#[derive(Clone, Copy, Debug, PartialEq, Eq, PartialOrd, Ord)]
pub(crate) enum Check {
    /// Every order agrees with the orders below it bit for bit, and `derivatives` agrees with the rungs.
    Orders,
    /// A batch of any length, at every level and order, gives the bits of single calls and says which are defined.
    Batches,
    /// Derivatives to the third agree with central differences of the order below.
    Differences,
    /// A point on it inverts to itself, and its closest point to a point off it is no further than any sample.
    Closest,
    /// A box holds every point of what it bounds.
    Bounds,
    /// The ends of a closed parameter meet, and a periodic one carries on round.
    Seams,
    /// A declared degenerate line collapses to its point, and has the normal it declares.
    Degeneracies,
    /// The normal of the derivatives is the presented form's, orientation included, and every point is on the form.
    Normals,
    /// The content leads with the family's name and is the same each time, and so is the batch key's family.
    Identity,
}

/// One thing a check found wrong.
#[derive(Debug)]
pub(crate) struct Failure {
    /// Which check.
    pub check: Check,
    /// What, and where, for the test's message.
    #[expect(dead_code, reason = "read through `Debug` when a test shows what failed")]
    pub what: String,
}

/// What the checks have found so far.
#[derive(Default)]
pub(crate) struct Findings(Vec<Failure>);

impl Findings {
    /// Records a failure of `check` unless `holds`.
    fn expect(&mut self, check: Check, holds: bool, what: impl FnOnce() -> String) {
        if !holds {
            self.0.push(Failure { check, what: what() });
        }
    }
}

/// Which checks failed, each once, in the order above.
pub(crate) fn checks(failures: &[Failure]) -> Vec<Check> {
    let failed: BTreeSet<Check> = failures.iter().map(|f| f.check).collect();
    failed.into_iter().collect()
}

/// The model tolerance the suite works to.
pub(crate) fn tolerance() -> Tolerance {
    Tolerance::new(Length::from_millimetres(1e-7), Angle::from_radians(1e-9)).unwrap()
}

/// A placement neither on the axes nor at the origin, so that no symmetry hides a fault.
pub(crate) fn placement() -> Placement<Local, World> {
    let turn =
        Placement::<Local, Local>::rotation(Direction::normalised(Vec3::new(1.0, 2.0, 3.0)), Angle::from_radians(0.7));
    let shift = Placement::<World, World>::translation(Vector::from_vec3(Vec3::new(10.0, -20.0, 5.0)));
    turn.then(&Placement::identity()).then(&shift)
}

/// A point given in [`placement`]'s frame, in `World`.
pub(crate) fn local(x: f64, y: f64, z: f64) -> Vec3 {
    placement().point(Point::from_vec3(Vec3::new(x, y, z))).to_vec3()
}

/// Whether two vectors agree to a part in 10^12 of the larger and one.
pub(crate) fn agree(a: Vec3, b: Vec3) -> bool {
    (a - b).norm() <= 1e-12 * (1.0 + b.norm())
}

/// How far the box of `s` over `patch` reaches beyond the extremes of `n` by `n` points of it, edges included, in the
/// coordinate where it reaches furthest: a box that holds the patch and is no larger shows a little more than the
/// spacing of the samples misses.
pub(crate) fn slack(s: &dyn crate::Surface, patch: crate::UvBox, n: u32) -> f64 {
    let at = |f: f64, lo: f64, hi: f64| lerp(lo, hi, f);
    let points: Vec<Vec3> = (0..=n)
        .flat_map(|i| (0..=n).map(move |j| (f64::from(i) / f64::from(n), f64::from(j) / f64::from(n))))
        .map(|(fu, fv)| {
            let uv = occam_units::UvPoint::new(at(fu, patch.lo.u(), patch.hi.u()), at(fv, patch.lo.v(), patch.hi.v()));
            s.point(uv).to_vec3()
        })
        .collect();
    let b = s.bounds(patch);
    let (low, high) = (b.low().to_vec3(), b.high().to_vec3());
    let least =
        |pick: fn(Vec3) -> f64| points.iter().map(|p| pick(*p)).fold(f64::INFINITY, |a, x| if x < a { x } else { a });
    let most = |pick: fn(Vec3) -> f64| {
        points.iter().map(|p| pick(*p)).fold(f64::NEG_INFINITY, |a, x| if x > a { x } else { a })
    };
    let picks: [fn(Vec3) -> f64; 3] = [|v| v.x, |v| v.y, |v| v.z];
    picks
        .into_iter()
        .map(|pick| (least(pick) - pick(low)) + (pick(high) - most(pick)))
        .fold(0.0, |a, x| if x > a { x } else { a })
}

/// The fractions of the way across a range at which the suite samples it: `n` of them, none at an end.
fn fractions(n: u32) -> impl Iterator<Item = f64> {
    (0..n).map(move |k| (f64::from(k) + 0.37) / f64::from(n))
}

/// The value a fraction `f` of the way from `lo` to `hi`.
fn lerp(lo: f64, hi: f64, f: f64) -> f64 {
    lo + (hi - lo) * f
}

/// Three numbers' canonical bits, so that two NaNs compare equal.
fn bits(v: Vec3) -> [u64; 3] {
    [canonical_bits(v.x), canonical_bits(v.y), canonical_bits(v.z)]
}

/// The bits of each rate, in order.
fn rates(tangents: &[Tangent<World>]) -> Vec<[u64; 3]> {
    tangents.iter().map(|t| bits(t.rate())).collect()
}

/// How far apart two points are.
fn distance(a: Point<World>, b: Point<World>) -> f64 {
    (a.to_vec3() - b.to_vec3()).norm()
}

/// Whether `got` is within a millionth of `want`, relative to its size and at least one.
fn close(got: Vec3, want: Vec3) -> bool {
    (got - want).norm() <= 1e-6 * (1.0 + want.norm())
}

/// `v` scaled to unit length: NaN where it has none.
fn unit(v: Vec3) -> Vec3 {
    v * (1.0 / v.norm())
}

/// Whether every number is finite.
fn finite(v: Vec3) -> bool {
    v.x.is_finite() && v.y.is_finite() && v.z.is_finite()
}

/// A rate that no evaluation gives, to show that an output was replaced.
fn junk() -> Tangent<World> {
    Tangent::new(Vec3::new(9.0, -9.0, 9.0))
}
