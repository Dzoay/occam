//! Source: original
//! The method list, the section and `sections` against stub methods, whose answers are made up but depend on argument
//! order, so that only canonical order makes them symmetric.

use std::num::NonZeroUsize;
use std::sync::Arc;

use occam_geom::{Analytic, Content, Curve, Line, ParamCurve, Plane, Sphere, Surface, UvCurve};
use occam_math::{Exec, Tally};
use occam_report::{Code, Fault, NamedValue, Reason};
use occam_units::{Angle, Direction, Length, Local, Placement, Point, Tolerance, UvPoint, Vec3, Vector, World};

use crate::{Answer, Budget, Contact, Method, Methods, Section, SectionCurve, Spent, SurfacePair, Work, sections};

fn tol() -> Tolerance {
    Tolerance::new(Length::from_millimetres(1e-7), Angle::from_radians(1e-9)).unwrap()
}

const fn budget(steps: u32) -> Budget {
    Budget { steps }
}

/// Turned by `turn` about X, then moved to `(x, y, z)`.
fn placed(x: f64, y: f64, z: f64, turn: f64) -> Placement<Local, World> {
    let spin = Placement::rotation(Direction::normalised(Vec3::new(1.0, 0.0, 0.0)), Angle::from_radians(turn));
    let shift = Placement::translation(Vector::from_vec3(Vec3::new(x, y, z)));
    Placement::identity().then(&spin).then(&shift)
}

fn plane(x: f64, y: f64, z: f64, turn: f64) -> Arc<dyn Surface> {
    Arc::new(Plane::placed(&placed(x, y, z, turn)))
}

fn ball(x: f64, y: f64, z: f64) -> Arc<dyn Surface> {
    Arc::new(Sphere::placed(&placed(x, y, z, 0.0), Length::from_millimetres(2.0)).unwrap())
}

/// The unit segment from `from` along `along`.
fn segment(from: Point<World>, along: Direction<World>) -> Arc<dyn Curve> {
    Arc::new(Line::new(from, along).trimmed(0.0, 1.0).unwrap())
}

/// The unit segment from the parameters' origin along `along`, as a parameter curve.
fn image(along: Vec3) -> Arc<dyn ParamCurve> {
    let line = Line::new(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)), Direction::normalised(along));
    Arc::new(UvCurve::new(line.trimmed(0.0, 1.0).unwrap()).unwrap())
}

/// What the stubs give as the image on the first surface they are given, and on the second.
fn first_image() -> Arc<dyn ParamCurve> {
    image(Vec3::new(1.0, 0.0, 0.0))
}

fn second_image() -> Arc<dyn ParamCurve> {
    image(Vec3::new(0.0, 1.0, 0.0))
}

/// A stub of the closed forms: it applies only where both surfaces present a plane, and gives the segment from the
/// first plane's origin along the second's normal, and a contact at the first's origin, for one step and one decision.
struct Planes;

impl Method for Planes {
    fn name(&self) -> &'static str {
        "planes"
    }

    fn attempt(
        &self,
        a: &dyn Surface,
        b: &dyn Surface,
        tol: Tolerance,
        work: &mut Work,
        section: &mut Section,
    ) -> Result<Answer, Spent> {
        let (Some(Analytic::Plane { origin, .. }), Some(Analytic::Plane { normal, .. })) =
            (a.analytic(tol), b.analytic(tol))
        else {
            return Ok(Answer::Declined);
        };
        work.spend(1)?;
        let curve = segment(origin, normal);
        section.curves.push(SectionCurve { curve, on_a: first_image(), on_b: second_image() });
        let (on_a, on_b) = (UvPoint::new(0.25, 0.5), UvPoint::new(0.75, 1.0));
        section.contacts.push(Contact::Point { at: origin, on_a, on_b });
        section.tally = section.tally.merged(Tally { decided: 1, exact: 0, undecided: 0 });
        Ok(Answer::Answered)
    }
}

/// A stub of a method that finds one curve a step, of `self.0` in all, on any pair: the `i`th from the first surface's
/// point at `(i, 0)`.
struct Steps(u32);

impl Method for Steps {
    fn name(&self) -> &'static str {
        "steps"
    }

    fn attempt(
        &self,
        a: &dyn Surface,
        _: &dyn Surface,
        _: Tolerance,
        work: &mut Work,
        section: &mut Section,
    ) -> Result<Answer, Spent> {
        for i in 0..self.0 {
            work.spend(1)?;
            let curve =
                segment(a.point(UvPoint::new(f64::from(i), 0.0)), Direction::normalised(Vec3::new(0.0, 0.0, 1.0)));
            section.curves.push(SectionCurve { curve, on_a: first_image(), on_b: second_image() });
        }
        Ok(Answer::Answered)
    }
}

/// A stub of a method that never applies, but finds that out only after a step, two decisions, a curve and a failure.
struct Refuses;

impl Method for Refuses {
    fn name(&self) -> &'static str {
        "refuses"
    }

    fn attempt(
        &self,
        a: &dyn Surface,
        _: &dyn Surface,
        _: Tolerance,
        work: &mut Work,
        section: &mut Section,
    ) -> Result<Answer, Spent> {
        work.spend(1)?;
        let curve = segment(a.point(UvPoint::new(0.0, 0.0)), Direction::normalised(Vec3::new(1.0, 0.0, 0.0)));
        section.curves.push(SectionCurve { curve, on_a: first_image(), on_b: second_image() });
        section.failures.push(occam_report::Report::new(Code::Incomplete, Fault::Internal));
        section.tally = section.tally.merged(Tally { decided: 2, exact: 1, undecided: 0 });
        Ok(Answer::Declined)
    }
}

fn words(write: impl Fn(&mut Content)) -> Vec<u64> {
    let mut content = Content::default();
    write(&mut content);
    content.words().to_vec()
}

/// Everything a section holds as text, every curve by its content's bits and every number as it round-trips, with the
/// images on the two surfaces exchanged when `swap` is set.
fn print(section: &Section, swap: bool) -> String {
    let curves: Vec<_> = section
        .curves
        .iter()
        .map(|c| {
            let (a, b) = (words(|out| c.on_a.content(out)), words(|out| c.on_b.content(out)));
            (words(|out| c.curve.content(out)), if swap { (b, a) } else { (a, b) })
        })
        .collect();
    let contacts: Vec<Contact> = section
        .contacts
        .iter()
        .map(|contact| match *contact {
            Contact::Point { at, on_a, on_b } if swap => Contact::Point { at, on_a: on_b, on_b: on_a },
            other => other,
        })
        .collect();
    let rest = (section.method, &section.coincidence, &section.failures, section.tally);
    format!("{curves:?} {contacts:?} {rest:?}")
}

#[test]
fn a_swapped_pair_gives_the_same_section_with_its_images_swapped() {
    let methods = Methods::new(vec![Box::new(Planes)]);
    let (p, q) = (plane(0.0, 0.0, 0.0, 0.0), plane(1.0, 2.0, 3.0, 0.5));
    let one = methods.section(p.as_ref(), q.as_ref(), tol(), budget(1));
    let other = methods.section(q.as_ref(), p.as_ref(), tol(), budget(1));
    assert_eq!((one.method, one.curves.len(), one.contacts.len()), (Some("planes"), 1, 1));
    assert!(one.failures.is_empty(), "{:?}", one.failures);
    assert_eq!(print(&one, false), print(&other, true));
    assert_ne!(print(&one, false), print(&other, false), "the stub's images differ, so the swap is seen");
}

#[test]
fn a_pair_whose_keys_are_equal_keeps_the_order_given() {
    let methods = Methods::new(vec![Box::new(Planes)]);
    let (p, twin) = (plane(1.0, 2.0, 3.0, 0.5), plane(1.0, 2.0, 3.0, 0.5));
    let got = methods.section(p.as_ref(), twin.as_ref(), tol(), budget(1));
    assert_eq!(print(&got, false), print(&methods.section(twin.as_ref(), p.as_ref(), tol(), budget(1)), false));
    let first = words(|out| first_image().content(out));
    assert_eq!(words(|out| got.curves[0].on_a.content(out)), first, "not swapped");
}

#[test]
fn a_method_is_given_the_lesser_key_first() {
    let methods = Methods::new(vec![Box::new(Steps(1))]);
    let (s, p) = (ball(4.0, 0.0, 0.0), plane(0.0, 0.0, 0.0, 0.3));
    let got = methods.section(s.as_ref(), p.as_ref(), tol(), budget(1));
    let start = got.curves[0].curve.point(0.0).to_vec3();
    assert_eq!(start, p.point(UvPoint::new(0.0, 0.0)).to_vec3(), "a plane comes before a sphere");
}

#[test]
fn a_spent_budget_is_reported_and_keeps_the_curves_found_before_it() {
    let methods = Methods::new(vec![Box::new(Steps(5))]);
    let (p, s) = (plane(0.0, 0.0, 0.0, 0.3), ball(4.0, 0.0, 0.0));
    let whole = methods.section(p.as_ref(), s.as_ref(), tol(), budget(5));
    assert_eq!((whole.curves.len(), whole.failures.len()), (5, 0), "five steps are enough: {:?}", whole.failures);
    let part = methods.section(p.as_ref(), s.as_ref(), tol(), budget(3));
    assert_eq!(part.method, Some("steps"));
    assert_eq!(part.curves.len(), 3);
    let kept = Section { curves: whole.curves[..3].to_vec(), ..part.clone() };
    assert_eq!(print(&part, false), print(&kept, false), "the curves found first are kept");
    let [report] = part.failures.as_slice() else { panic!("{:?}", part.failures) };
    assert_eq!(
        (report.code, report.fault, report.reason),
        (Code::Incomplete, Some(Fault::NumericallySensitive), Some(Reason::BudgetSpent))
    );
    let value = |name: &str| report.value(name).and_then(NamedValue::number);
    assert_eq!((value("budget"), value("curves"), value("contacts")), (Some(3.0), Some(3.0), Some(0.0)));
    assert!(report.remedies.is_empty(), "no remedy yet: an agent cannot set a budget");
}

#[test]
fn the_first_method_that_applies_answers_and_each_has_the_whole_budget() {
    let methods = Methods::new(vec![Box::new(Refuses), Box::new(Planes), Box::new(Steps(1))]);
    let (p, q, s) = (plane(0.0, 0.0, 0.0, 0.0), plane(1.0, 2.0, 3.0, 0.5), ball(4.0, 0.0, 0.0));
    let planes = methods.section(p.as_ref(), q.as_ref(), tol(), budget(1));
    assert_eq!((planes.method, planes.curves.len()), (Some("planes"), 1));
    assert!(planes.failures.is_empty(), "what a method that declined found is dropped: {:?}", planes.failures);
    assert_eq!(planes.tally, Tally { decided: 3, exact: 1, undecided: 0 }, "its decisions are kept");
    let other = methods.section(p.as_ref(), s.as_ref(), tol(), budget(1));
    assert_eq!((other.method, other.curves.len(), other.contacts.len()), (Some("steps"), 1, 0));
    assert!(other.failures.is_empty(), "{:?}", other.failures);
    assert_eq!(other.tally, Tally { decided: 2, exact: 1, undecided: 0 });
}

#[test]
fn a_pair_no_method_applies_to_is_reported_as_not_supported() {
    for list in [vec![], vec![Box::new(Planes) as Box<dyn Method>]] {
        let (s, t) = (ball(0.0, 0.0, 0.0), ball(1.0, 0.0, 0.0));
        let got = Methods::new(list).section(s.as_ref(), t.as_ref(), tol(), budget(4));
        assert_eq!((got.method, got.curves.len(), got.contacts.len()), (None, 0, 0));
        let [report] = got.failures.as_slice() else { panic!("{:?}", got.failures) };
        assert_eq!(
            (report.code, report.fault, report.reason),
            (Code::Incomplete, Some(Fault::NotImplemented), Some(Reason::PairNotSupported))
        );
        assert_eq!(report.value("curves").and_then(NamedValue::number), Some(0.0));
        assert!(report.remedies.is_empty(), "no budget would help");
    }
}

#[test]
fn a_pair_no_method_applies_to_keeps_the_decisions_made_in_declining() {
    let methods = Methods::new(vec![Box::new(Refuses), Box::new(Planes)]);
    let (s, t) = (ball(0.0, 0.0, 0.0), ball(1.0, 0.0, 0.0));
    let got = methods.section(s.as_ref(), t.as_ref(), tol(), budget(1));
    assert_eq!((got.method, got.curves.len(), got.failures.len()), (None, 0, 1), "{:?}", got.failures);
    assert_eq!(got.failures[0].reason, Some(Reason::PairNotSupported));
    assert_eq!(got.tally, Tally { decided: 2, exact: 1, undecided: 0 });
}

#[test]
fn many_pairs_give_one_result_each_in_order_at_one_and_many_threads() {
    let methods = Arc::new(Methods::new(vec![Box::new(Planes), Box::new(Steps(2))]));
    let kinds = ["planes", "ball and plane", "balls"].into_iter().cycle();
    let pairs: Arc<[SurfacePair]> = (0..12_u8)
        .zip(kinds)
        .map(|(i, kind)| {
            let x = f64::from(i);
            match kind {
                "planes" => (plane(x, 0.0, 0.0, 0.0), plane(0.0, x, 1.0, 0.25)),
                "balls" => (ball(0.0, x, 0.0), ball(x, 0.0, 0.0)),
                _ => (ball(x, 1.0, 0.0), plane(0.0, 0.0, x, 0.0)),
            }
        })
        .collect();
    let alone: Vec<String> =
        pairs.iter().map(|(a, b)| print(&methods.section(a.as_ref(), b.as_ref(), tol(), budget(1)), false)).collect();
    for (i, text) in alone.iter().enumerate() {
        assert!(!alone[..i].contains(text), "every pair's section differs, so order shows: {i}");
    }
    let many = Exec::threads(NonZeroUsize::new(4).unwrap()).unwrap();
    for exec in [Exec::serial(), many] {
        let got = sections(&exec, Arc::clone(&methods), Arc::clone(&pairs), tol(), budget(1));
        let got: Vec<String> = got.iter().map(|section| print(section, false)).collect();
        assert_eq!(got, alone, "{} threads", exec.thread_count());
    }
}
