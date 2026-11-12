//! Source: original
//! The suite's checks of a curve, through [`Curve`] alone, from `lo` to `hi`: the surface's checks that apply to one
//! parameter.

use occam_report::Code;
use occam_units::{Point, Vec3, World};

use super::{Check, Failure, Findings, bits, close, distance, fractions, junk, lerp, rates, tolerance, unit};
use crate::{Content, Curve, Scratch, SplineCurve, Wrap};

/// Every check of `c` from `lo` to `hi`, which must lie in its range. What failed, or nothing.
pub(crate) fn curve(c: &dyn Curve, lo: f64, hi: f64) -> Vec<Failure> {
    let mut found = Findings::default();
    let params: Vec<f64> = fractions(9).map(|f| lerp(lo, hi, f)).collect();
    orders(c, &params, &mut found);
    differences(c, &params, 1e-5 * (hi - lo), &mut found);
    closest(c, lo, hi, &mut found);
    bounds(c, lo, hi, &mut found);
    seams(c, &mut found);
    presents(c, lo, hi, &mut found);
    identity(c, &mut found);
    found.0
}

/// The [`Check::Presents`] check of `c` from `lo` to `hi` alone: what failed, or nothing.
pub(crate) fn presented_curve(c: &dyn Curve, lo: f64, hi: f64) -> Vec<Failure> {
    let mut found = Findings::default();
    presents(c, lo, hi, &mut found);
    found.0
}

/// The derivative of order `k` at `t`: NaN if there is none.
fn derivative(c: &dyn Curve, t: f64, k: u8) -> Vec3 {
    let mut out = vec![];
    let none = Vec3::new(f64::NAN, f64::NAN, f64::NAN);
    match c.derivatives(t, k, &mut out, &mut Scratch::default()) {
        Ok(()) => out.last().map_or(none, |d| d.rate()),
        Err(_) => none,
    }
}

fn orders(c: &dyn Curve, params: &[f64], found: &mut Findings) {
    let (mut scratch, mut three, mut four) = (Scratch::default(), vec![junk()], vec![junk()]);
    for &t in params {
        let (p, d) = c.first(t);
        found.expect(Check::Orders, bits(p.to_vec3()) == bits(c.point(t).to_vec3()), || {
            format!("the rungs disagree at {t}")
        });
        let asked =
            c.derivatives(t, 3, &mut three, &mut scratch).and_then(|()| c.derivatives(t, 4, &mut four, &mut scratch));
        let (p2, d1, d2) = c.second(t);
        let second = bits(p2.to_vec3()) == bits(p.to_vec3()) && d1 == d && three.get(1) == Some(&d2);
        let agree = three.len() == 3 && four.get(..3).map(rates) == Some(rates(&three)) && three.first() == Some(&d);
        found.expect(Check::Orders, asked.is_ok() && agree && second, || {
            format!("derivatives disagree with the rungs at {t}")
        });
    }
    let outside = c.derivatives(f64::NAN, 1, &mut three, &mut scratch);
    let refused = outside.is_err_and(|report| report.code == Code::UndefinedParameter) && three.is_empty();
    found.expect(Check::Orders, refused, || "a parameter that is not finite was not refused".to_string());
}

fn differences(c: &dyn Curve, params: &[f64], h: f64, found: &mut Findings) {
    for &t in params {
        let central = |f: &dyn Fn(f64) -> Vec3| (f(t + h) - f(t - h)) * (0.5 / h);
        let pairs = [
            (1, central(&|at| c.point(at).to_vec3())),
            (2, central(&|at| c.first(at).1.rate())),
            (3, central(&|at| derivative(c, at, 2))),
        ];
        for (k, want) in pairs {
            let got = derivative(c, t, k);
            found.expect(Check::Differences, close(got, want), || {
                format!("derivative {k} at {t} is {got:?}, not {want:?}")
            });
        }
    }
}

/// A unit vector across the tangent `d`.
fn across(d: Vec3) -> Vec3 {
    let axes = [Vec3::new(1.0, 0.0, 0.0), Vec3::new(0.0, 1.0, 0.0), Vec3::new(0.0, 0.0, 1.0)];
    unit(
        axes.map(|axis| d.cross(axis))
            .into_iter()
            .fold(Vec3::new(0.0, 0.0, 0.0), |a, b| if b.norm() > a.norm() { b } else { a }),
    )
}

/// The closest point to `off` is no further than any sample, lies on the curve within its range, and is as far as it
/// says; `invert` finds a foot as near.
fn off_the_curve(c: &dyn Curve, off: Point<World>, dense: &[Point<World>], found: &mut Findings) {
    let (tol, mut scratch) = (tolerance(), Scratch::default());
    let band = tol.band().millimetres();
    let nearest = dense.iter().map(|q| distance(*q, off)).fold(f64::INFINITY, |a, b| if b < a { b } else { a });
    let answer = c.closest(off, None, tol, &mut scratch);
    let good = answer.as_ref().is_ok_and(|a| {
        let d = a.distance.millimetres();
        d <= nearest + band
            && (distance(a.point, off) - d).abs() <= band
            && distance(c.point(a.at), a.point) <= band
            && c.range().contains(a.at)
    });
    found.expect(Check::Closest, good, || format!("{off:?} is closest to {answer:?}, against {nearest} sampled"));
    let foot = c.invert(off, None, tol, &mut scratch);
    let as_near =
        |t: &f64| answer.as_ref().is_ok_and(|a| distance(c.point(*t), off) <= a.distance.millimetres() + band);
    found.expect(Check::Closest, foot.as_ref().is_ok_and(as_near), || format!("{off:?} inverts to {foot:?}"));
}

fn closest(c: &dyn Curve, lo: f64, hi: f64, found: &mut Findings) {
    let (tol, mut scratch) = (tolerance(), Scratch::default());
    let band = tol.band().millimetres();
    let ends = [c.point(lo), c.point(hi)];
    let dense: Vec<Point<World>> = fractions(4000).map(|f| c.point(lerp(lo, hi, f))).chain(ends).collect();
    for (f, i) in fractions(4).zip(0u32..) {
        let t = lerp(lo, hi, f);
        let (p, d) = c.first(t);
        let back = c.invert(p, None, tol, &mut scratch);
        let inverted = back.as_ref().is_ok_and(|b| distance(c.point(*b), p) <= band);
        found.expect(Check::Closest, inverted, || format!("{p:?} at {t} inverts to {back:?}"));
        let on = c.closest(p, None, tol, &mut scratch);
        let itself = on.as_ref().is_ok_and(|a| a.distance.millimetres() <= band && distance(a.point, p) <= band);
        found.expect(Check::Closest, itself, || format!("{p:?} at {t} is closest to {on:?}"));
        for height in [0.4 + 0.1 * f64::from(i), 3.0 + 2.0 * f64::from(i)] {
            off_the_curve(c, Point::from_vec3(p.to_vec3() + across(d.rate()) * height), &dense, found);
        }
    }
    for (end, outward) in [(lo, -1.0), (hi, 1.0)] {
        let (p, d) = c.first(end);
        let beyond = p.to_vec3() + unit(d.rate()) * (0.5 * outward) + across(d.rate()) * 0.2;
        off_the_curve(c, Point::from_vec3(beyond), &dense, found);
    }
    let nan = Point::from_vec3(Vec3::new(0.0, f64::NAN, 0.0));
    let refused = |code: Option<Code>| code == Some(Code::NotFinite);
    let (inverted, nearest) = (c.invert(nan, None, tol, &mut scratch), c.closest(nan, None, tol, &mut scratch));
    found.expect(
        Check::Closest,
        refused(inverted.err().map(|r| r.code)) && refused(nearest.err().map(|r| r.code)),
        || "a point that is not finite was not refused".to_string(),
    );
}

fn bounds(c: &dyn Curve, lo: f64, hi: f64, found: &mut Findings) {
    let part = (lerp(lo, hi, 0.3), lerp(lo, hi, 0.7));
    let one = lerp(lo, hi, 0.61);
    for (name, (from, to)) in [("range", (lo, hi)), ("part", part), ("point", (one, one))] {
        let b = c.bounds(from, to);
        for t in fractions(17).map(|f| lerp(from, to, f)).chain([from, to]) {
            let p = c.point(t);
            found.expect(Check::Bounds, b.contains(p), || format!("the {name}'s box {b:?} misses {p:?} at {t}"));
        }
    }
}

fn seams(c: &dyn Curve, found: &mut Findings) {
    let (range, band) = (c.range(), tolerance().band().millimetres());
    if range.wrap == Wrap::Open {
        return;
    }
    let (lo, hi) = (c.point(range.lo), c.point(range.hi));
    found.expect(Check::Seams, distance(lo, hi) <= band, || format!("the ends {lo:?} and {hi:?} do not meet"));
    if range.wrap == Wrap::Periodic {
        let t = lerp(range.lo, range.hi, 0.3);
        let (once, again) = (c.point(t), c.point(t + (range.hi - range.lo)));
        found.expect(Check::Seams, distance(once, again) <= band, || format!("it does not come round at {t}"));
    }
}

fn presents(c: &dyn Curve, lo: f64, hi: f64, found: &mut Findings) {
    let Some(form) = c.to_bspline(lo, hi) else { return };
    let spline = match SplineCurve::new(&form) {
        Ok(spline) => spline,
        Err(report) => return found.expect(Check::Presents, false, || format!("{form:?} is not valid: {report:?}")),
    };
    let (tol, mut scratch) = (tolerance(), Scratch::default());
    let band = tol.band().millimetres();
    let range = spline.range();
    let runs = [range.lo, range.hi].map(f64::to_bits) == [lo, hi].map(f64::to_bits);
    found.expect(Check::Presents, runs, || format!("the B-spline runs over {range:?}, not {lo} to {hi}"));
    for end in [lo, hi] {
        let (got, want) = (spline.point(end), c.point(end));
        found.expect(Check::Presents, distance(got, want) <= band, || format!("at {end}, {got:?} is not {want:?}"));
    }
    for t in fractions(9).map(|f| lerp(lo, hi, f)) {
        let on = c.closest(spline.point(t), None, tol, &mut scratch);
        let lies = on.as_ref().is_ok_and(|a| a.distance.millimetres() <= band);
        found.expect(Check::Presents, lies, || format!("the B-spline at {t} is off the curve: {on:?}"));
        let back = spline.closest(c.point(t), None, tol, &mut scratch);
        let covers = back.as_ref().is_ok_and(|a| a.distance.millimetres() <= band);
        found.expect(Check::Presents, covers, || format!("the curve at {t} is off the B-spline: {back:?}"));
        let Ok(on) = on else { continue };
        let (got, want) = (unit(spline.first(t).1.rate()), unit(c.first(on.at).1.rate()));
        found.expect(Check::Presents, (got - want).norm() <= 1e-6, || {
            format!("at {t} the B-spline runs along {got:?}, not {want:?}")
        });
    }
}

fn identity(c: &dyn Curve, found: &mut Findings) {
    let (mut once, mut again, mut name) = (Content::default(), Content::default(), Content::default());
    again.push(2.0);
    c.content(&mut once);
    c.content(&mut again);
    name.start(c.family());
    let leads = once.words().starts_with(name.words()) && once.words().len() > name.words().len();
    found.expect(Check::Identity, once == again && leads, || format!("{:?}'s content is {once:?}", c.family()));
    found.expect(Check::Identity, !c.shape().form.is_empty(), || "the shape has no form".to_string());
}
