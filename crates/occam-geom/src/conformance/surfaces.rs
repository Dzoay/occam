//! Source: original
//! The suite's checks of a surface, through [`Surface`] alone, over a window of its parameters.

use occam_math::Level;
use occam_report::Code;
use occam_units::{Point, Tangent, UvPoint, Vec3, World};

use super::{Check, Failure, Findings, bits, close, distance, finite, fractions, junk, lerp, rates, tolerance, unit};
use crate::{Analytic, Content, Jets, Order, Param, Scratch, Surface, UvBox, Wrap};

/// Every check of `s` over `window`, which must lie in its domain. What failed, or nothing.
pub(crate) fn surface(s: &dyn Surface, window: UvBox) -> Vec<Failure> {
    let mut found = Findings::default();
    orders(s, window, &mut found);
    batches(s, window, &mut found);
    differences(s, window, &mut found);
    closest(s, window, &mut found);
    bounds(s, window, &mut found);
    seams(s, window, &mut found);
    degeneracies(s, window, &mut found);
    normals(s, window, &mut found);
    identity(s, &mut found);
    found.0
}

/// The point a fraction of the way across `window` in each parameter.
fn across(window: UvBox, fu: f64, fv: f64) -> UvPoint {
    UvPoint::new(lerp(window.lo.u(), window.hi.u(), fu), lerp(window.lo.v(), window.hi.v(), fv))
}

/// `n` by `n` points inside `window`, none on an edge.
fn samples(window: UvBox, n: u32) -> Vec<UvPoint> {
    fractions(n).flat_map(|fu| fractions(n).map(move |fv| across(window, fu, fv))).collect()
}

fn orders(s: &dyn Surface, window: UvBox, found: &mut Findings) {
    let mut scratch = Scratch::default();
    let (mut two, mut three) = (vec![junk()], vec![junk()]);
    for uv in samples(window, 5) {
        let (p, j1, j2) = (s.point(uv), s.first(uv), s.second(uv));
        let points = [p, j1.point, j2.first.point].map(|q| bits(q.to_vec3()));
        let rungs = points[0] == points[1]
            && points[0] == points[2]
            && rates(&[j1.du, j1.dv]) == rates(&[j2.first.du, j2.first.dv]);
        found.expect(Check::Orders, rungs, || format!("the rungs disagree at {uv:?}"));
        let asked =
            s.derivatives(uv, 2, &mut two, &mut scratch).and_then(|()| s.derivatives(uv, 3, &mut three, &mut scratch));
        found.expect(Check::Orders, asked.is_ok(), || format!("no derivatives at {uv:?}: {asked:?}"));
        let want = rates(&[j2.first.du, j2.first.dv, j2.duu, j2.duv, j2.dvv]);
        let agree = rates(&two) == want && three.len() == 9 && three.get(..5).map(rates) == Some(want);
        found.expect(Check::Orders, agree, || format!("derivatives disagree with the rungs at {uv:?}"));
    }
    let outside = s.derivatives(UvPoint::new(f64::NAN, window.lo.v()), 1, &mut two, &mut scratch);
    let refused = outside.is_err_and(|report| report.code == Code::UndefinedParameter) && two.is_empty();
    found.expect(Check::Orders, refused, || "a parameter that is not finite was not refused".to_string());
}

/// What single calls give, as a batch of `order` would hold it.
fn single_calls(s: &dyn Surface, uv: &[UvPoint], order: Order) -> Jets {
    let mut want = Jets::default();
    for at in uv {
        let (mut first, mut second): (Vec<Tangent<World>>, Vec<Tangent<World>>) = (vec![], vec![]);
        let point = match order {
            Order::Point => s.point(*at),
            Order::First => {
                let jet = s.first(*at);
                first = vec![jet.du, jet.dv];
                jet.point
            }
            Order::Second => {
                let jet = s.second(*at);
                (first, second) = (vec![jet.first.du, jet.first.dv], vec![jet.duu, jet.duv, jet.dvv]);
                jet.first.point
            }
        };
        let numbers = first.iter().chain(&second).map(|t| t.rate()).chain([point.to_vec3()]);
        want.defined.push(s.domain().contains(*at) && numbers.into_iter().all(finite));
        want.points.push(point);
        for (into, from) in [(&mut want.du, first.first()), (&mut want.dv, first.get(1))] {
            into.extend(from);
        }
        for (into, from) in
            [(&mut want.duu, second.first()), (&mut want.duv, second.get(1)), (&mut want.dvv, second.get(2))]
        {
            into.extend(from);
        }
    }
    want
}

fn batches(s: &dyn Surface, window: UvBox, found: &mut Findings) {
    let mut uv = samples(window, 5);
    uv.insert(7, UvPoint::new(f64::NAN, window.lo.v()));
    let (mut out, mut scratch) = (Jets::default(), Scratch::default());
    for level in Level::available() {
        for order in [Order::Point, Order::First, Order::Second] {
            for len in [1, 2, 3, 5, 9, 17, uv.len()] {
                s.eval_batch(&uv[..len], order, &mut out, &mut scratch, level);
                let want = single_calls(s, &uv[..len], order);
                let points = |jets: &Jets| jets.points.iter().map(|p| bits(p.to_vec3())).collect::<Vec<_>>();
                let derivatives = |j: &Jets| [&j.du, &j.dv, &j.duu, &j.duv, &j.dvv].map(|d| rates(d));
                let same = points(&out) == points(&want) && derivatives(&out) == derivatives(&want);
                let what = || format!("{} points to {order:?} at {} differ from single calls", len, level.name());
                found.expect(Check::Batches, same && out.defined == want.defined, what);
            }
        }
    }
}

/// The derivative at `index` of those to `order` at `at`: NaN if there is none.
fn nth(s: &dyn Surface, at: UvPoint, order: u8, index: usize) -> Vec3 {
    let mut out = vec![];
    let none = Vec3::new(f64::NAN, f64::NAN, f64::NAN);
    match s.derivatives(at, order, &mut out, &mut Scratch::default()) {
        Ok(()) => out.get(index).map_or(none, |t| t.rate()),
        Err(_) => none,
    }
}

fn differences(s: &dyn Surface, window: UvBox, found: &mut Findings) {
    let (hu, hv) = (1e-5 * (window.hi.u() - window.lo.u()), 1e-5 * (window.hi.v() - window.lo.v()));
    for uv in samples(window, 4) {
        let step = |du: f64, dv: f64| UvPoint::new(uv.u() + du, uv.v() + dv);
        let central = |f: &dyn Fn(UvPoint) -> Vec3, in_u: bool| {
            let (h, plus, minus) =
                if in_u { (hu, step(hu, 0.0), step(-hu, 0.0)) } else { (hv, step(0.0, hv), step(0.0, -hv)) };
            (f(plus) - f(minus)) * (0.5 / h)
        };
        let jet = s.second(uv);
        let third: Vec<Vec3> = (5..9).map(|k| nth(s, uv, 3, k)).collect();
        let pairs = [
            ("du", jet.first.du.rate(), central(&|at| s.point(at).to_vec3(), true)),
            ("dv", jet.first.dv.rate(), central(&|at| s.point(at).to_vec3(), false)),
            ("duu", jet.duu.rate(), central(&|at| s.first(at).du.rate(), true)),
            ("duv", jet.duv.rate(), central(&|at| s.first(at).du.rate(), false)),
            ("dvu", jet.duv.rate(), central(&|at| s.first(at).dv.rate(), true)),
            ("dvv", jet.dvv.rate(), central(&|at| s.first(at).dv.rate(), false)),
            ("duuu", third[0], central(&|at| nth(s, at, 2, 2), true)),
            ("duuv", third[1], central(&|at| nth(s, at, 2, 2), false)),
            ("duvv", third[2], central(&|at| nth(s, at, 2, 4), true)),
            ("dvvv", third[3], central(&|at| nth(s, at, 2, 4), false)),
        ];
        for (name, got, want) in pairs {
            found.expect(Check::Differences, close(got, want), || format!("{name} at {uv:?} is {got:?}, not {want:?}"));
        }
    }
}

fn closest(s: &dyn Surface, window: UvBox, found: &mut Findings) {
    let (tol, mut scratch) = (tolerance(), Scratch::default());
    let band = tol.band().millimetres();
    let dense: Vec<Point<World>> = samples(window, 30).into_iter().map(|uv| s.point(uv)).collect();
    for (uv, i) in samples(window, 3).into_iter().zip(0u32..) {
        let p = s.point(uv);
        let back = s.invert(p, None, tol, &mut scratch);
        let inverted = back.as_ref().is_ok_and(|b| distance(s.point(*b), p) <= band);
        found.expect(Check::Closest, inverted, || format!("{p:?} at {uv:?} inverts to {back:?}"));
        let on = s.closest(p, None, tol, &mut scratch);
        let itself = on.as_ref().is_ok_and(|c| c.distance.millimetres() <= band && distance(c.point, p) <= band);
        found.expect(Check::Closest, itself, || format!("{p:?} at {uv:?} is closest to {on:?}"));
        let jet = s.first(uv);
        let normal = unit(jet.du.rate().cross(jet.dv.rate()));
        if !finite(normal) {
            continue;
        }
        let off = Point::from_vec3(p.to_vec3() + normal * (0.4 + 0.1 * f64::from(i)));
        let nearest = dense.iter().map(|q| distance(*q, off)).fold(f64::INFINITY, |a, b| if b < a { b } else { a });
        let answer = s.closest(off, None, tol, &mut scratch);
        let good = answer.as_ref().is_ok_and(|c| {
            let d = c.distance.millimetres();
            d <= nearest + band
                && (distance(c.point, off) - d).abs() <= band
                && distance(s.point(c.at), c.point) <= band
        });
        found.expect(Check::Closest, good, || format!("{off:?} is closest to {answer:?}, against {nearest} sampled"));
    }
    let nan = Point::from_vec3(Vec3::new(f64::NAN, 0.0, 0.0));
    let refused = |code: Option<Code>| code == Some(Code::NotFinite);
    let inverted = s.invert(nan, None, tol, &mut scratch).err().map(|r| r.code);
    let nearest = s.closest(nan, None, tol, &mut scratch).err().map(|r| r.code);
    found.expect(Check::Closest, refused(inverted) && refused(nearest), || {
        "a point that is not finite was not refused".into()
    });
}

fn bounds(s: &dyn Surface, window: UvBox, found: &mut Findings) {
    let part = UvBox::new(across(window, 0.3, 0.2), across(window, 0.7, 0.6));
    let one = UvBox::new(across(window, 0.61, 0.17), across(window, 0.61, 0.17));
    for (name, patch) in [("window", window), ("part", part), ("point", one)] {
        let b = s.bounds(patch);
        for uv in samples(patch, 8).into_iter().chain(patch.corners()) {
            let p = s.point(uv);
            found.expect(Check::Bounds, b.contains(p), || format!("the {name}'s box {b:?} misses {p:?} at {uv:?}"));
        }
    }
}

fn seams(s: &dyn Surface, window: UvBox, found: &mut Findings) {
    let (domain, band) = (s.domain(), tolerance().band().millimetres());
    for (param, range) in [(Param::U, domain.u), (Param::V, domain.v)] {
        if range.wrap == Wrap::Open {
            continue;
        }
        for f in [0.13, 0.5, 0.87] {
            let at = |t: f64| match param {
                Param::U => UvPoint::new(t, lerp(window.lo.v(), window.hi.v(), f)),
                Param::V => UvPoint::new(lerp(window.lo.u(), window.hi.u(), f), t),
            };
            let (lo, hi) = (s.point(at(range.lo)), s.point(at(range.hi)));
            found.expect(Check::Seams, distance(lo, hi) <= band, || {
                format!("{param:?}'s ends {lo:?} and {hi:?} do not meet")
            });
            if range.wrap == Wrap::Periodic {
                let t = lerp(range.lo, range.hi, 0.3);
                let (once, again) = (s.point(at(t)), s.point(at(t + (range.hi - range.lo))));
                found.expect(Check::Seams, distance(once, again) <= band, || {
                    format!("{param:?} does not come round at {t}")
                });
            }
        }
    }
}

fn degeneracies(s: &dyn Surface, window: UvBox, found: &mut Findings) {
    let (domain, band) = (s.domain(), tolerance().band().millimetres());
    for line in s.degeneracies() {
        let range = if line.param == Param::U { domain.u } else { domain.v };
        found.expect(Check::Degeneracies, range.contains(line.at), || format!("{line:?} is outside the domain"));
        for f in [0.0, 0.3, 0.77] {
            let (at, beside) = match line.param {
                Param::U => (UvPoint::new(line.at, lerp(window.lo.v(), window.hi.v(), f)), UvPoint::new(1.0, 0.0)),
                Param::V => (UvPoint::new(lerp(window.lo.u(), window.hi.u(), f), line.at), UvPoint::new(0.0, 1.0)),
            };
            let p = s.point(at);
            found.expect(Check::Degeneracies, distance(p, line.point) <= band, || format!("{line:?} reaches {p:?}"));
            let Some(declared) = line.normal else { continue };
            for side in [1e-7, -1e-7] {
                let near = UvPoint::new(at.u() + side * beside.u(), at.v() + side * beside.v());
                if !domain.contains(near) {
                    continue;
                }
                let jet = s.first(near);
                let normal = unit(jet.du.rate().cross(jet.dv.rate()));
                let agrees = (normal - declared.get()).norm() <= 1e-4;
                found.expect(Check::Degeneracies, agrees, || format!("{line:?} has {normal:?} beside it at {near:?}"));
            }
        }
    }
}

fn normals(s: &dyn Surface, window: UvBox, found: &mut Findings) {
    let tol = tolerance();
    match s.analytic(tol) {
        None => {}
        Some(Analytic::Plane { origin, normal }) => {
            for uv in samples(window, 4) {
                let height = (s.point(uv).to_vec3() - origin.to_vec3()).dot(normal.get());
                let jet = s.first(uv);
                let of_derivatives = unit(jet.du.rate().cross(jet.dv.rate()));
                let on = height.abs() <= tol.band().millimetres() && (of_derivatives - normal.get()).norm() <= 1e-9;
                found.expect(Check::Normals, on, || {
                    format!("at {uv:?}, {height} off the plane, normal {of_derivatives:?}")
                });
            }
        }
        Some(other) => {
            found.expect(Check::Normals, false, || format!("the suite cannot check a presented {other:?} yet"));
        }
    }
}

fn identity(s: &dyn Surface, found: &mut Findings) {
    let (mut once, mut again, mut name) = (Content::default(), Content::default(), Content::default());
    again.push(1.0);
    s.content(&mut once);
    s.content(&mut again);
    name.start(s.family());
    let leads = once.words().starts_with(name.words()) && once.words().len() > name.words().len();
    found.expect(Check::Identity, once == again && leads, || format!("{:?}'s content is {once:?}", s.family()));
    let key = s.batch_key();
    found.expect(Check::Identity, key.family == s.family(), || format!("the batch key {key:?} names another family"));
    found.expect(Check::Identity, !s.shape().form.is_empty(), || "the shape has no form".to_string());
}
