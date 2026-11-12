//! Source: original
//! What the checks that read geometry refuse because they cannot vouch for it: an edge further from a face that uses it
//! than the band and the near-tangent shift for its length allow, and an edge or a loop whose points cannot be found
//! on its face's surface.

use std::sync::Arc;

use occam_geom::{Content, Domain, FamilyName, Jet1, Jet2, Parametric, Plane, Scratch, Shape, UvBox};
use occam_math::Real;
use occam_report::{Code, Fault, Reason, Report};
use occam_units::{Bounds, Length, Local, Placement, Point, Tangent, Tolerance, UvPoint, Vec3, Vector, World};

use crate::kind::Edge;
use crate::kind::sealed::Sealed;
use crate::models::{Built, Spec, build, cube};
use crate::records::SurfaceRecord;
use crate::snapshot::Snapshot;
use crate::testing::{number, serial, tolerance};

/// `spec` committed clean, and what it built.
fn committed(spec: &Spec) -> (Snapshot, Built) {
    let mut edit = Snapshot::empty(tolerance()).edit();
    let built = build(&mut edit, spec);
    (edit.commit(&serial()).unwrap().0, built)
}

/// The plane through `p` facing +Z, its parameters along X and Y from `p`.
fn plane(p: Vec3) -> Plane {
    Plane::placed(&Placement::<Local, World>::identity().then(&Placement::translation(Vector::from_vec3(p))))
}

/// The cube with its top face's surface replaced in place by `surface`: the reports of the commit, if it fails, and
/// the snapshot it began from.
fn top_replaced(surface: SurfaceRecord) -> (Result<Snapshot, Vec<Report>>, Snapshot, Built) {
    let (snapshot, built) = committed(&cube());
    let mut edit = snapshot.edit();
    let top = edit.record(built.faces[1]).unwrap().surface;
    edit.set(top, surface).unwrap();
    (edit.commit(&serial()).map(|(s, _)| s), snapshot, built)
}

/// The reports for `reason`.
fn with(reports: &[Report], reason: Reason) -> Vec<&Report> {
    reports.iter().filter(|r| r.reason == Some(reason)).collect()
}

#[test]
fn an_edge_further_from_a_face_than_the_near_tangent_shift_for_its_length_is_refused() {
    // The top lifted 0.01 mm: its unit edges may stray by the band and sqrt(2 * 1 mm * band), 4.5e-4 mm in all.
    let (result, before, built) = top_replaced(SurfaceRecord(Arc::new(plane(Vec3::new(0.5, 0.5, 1.01)))));
    let reports = result.unwrap_err();
    let off = with(&reports, Reason::EdgeOffFace);
    let top = before.view(built.faces[1]).unwrap().edges();
    assert_eq!(off.len(), 4, "{reports:#?}");
    let limit = 1e-7 + (2.0_f64 * 1e-7).sqrt();
    for report in off {
        assert_eq!((report.code, report.fault), (Code::InvariantFailed, Some(Fault::Internal)));
        assert!(top.iter().any(|e| e.entity() == report.entities[0]), "{report:?}");
        assert!(report.entities.contains(&built.faces[1].entity()), "{report:?}");
        assert!((number(report, "distance").unwrap() - 0.01).abs() < 1e-9, "{report:?}");
        assert!((number(report, "limit").unwrap() - limit).abs() < 1e-12, "{report:?}");
        assert!((number(report, "length").unwrap() - 1.0).abs() < 1e-12, "{report:?}");
    }
}

#[test]
fn an_edge_as_far_as_near_tangency_moves_it_is_kept_with_its_deviation() {
    // 1e-5 mm, a hundred bands, as far as S1's near-tangent sections strayed with every volume right.
    let (result, _, built) = top_replaced(SurfaceRecord(Arc::new(plane(Vec3::new(0.5, 0.5, 1.0 + 1e-5)))));
    let after = result.unwrap_or_else(|reports| panic!("{reports:#?}"));
    for e in after.view(built.faces[1]).unwrap().edges() {
        let deviation = after.view(e).unwrap().deviation().millimetres();
        assert!((deviation - 1e-5).abs() < 1e-12, "{e:?} {deviation}");
    }
}

#[test]
fn a_stored_deviation_beyond_the_cap_is_found_by_the_whole_model_check() {
    let (mut snapshot, built) = committed(&cube());
    let e = built.edges[0];
    Edge::table_mut(&mut snapshot.model).live_mut(e.slot(), e.generation()).unwrap().deviation =
        Length::from_millimetres(1.0);
    let reports = snapshot.check_all(&serial());
    let off = with(&reports, Reason::EdgeOffFace);
    assert_eq!(off.len(), 1, "{reports:#?}");
    assert_eq!(off[0].entities[0], e.entity());
    assert_eq!(number(off[0], "distance"), Some(1.0));
}

/// A plane on which no point can be found: what a surface whose search fails looks like to the checks.
struct Unfindable(Plane);

impl Parametric for Unfindable {
    fn eval_point<R: Real>(&self, uv: UvPoint<R>) -> Point<World, R> {
        self.0.eval_point(uv)
    }

    fn eval_first<R: Real>(&self, uv: UvPoint<R>) -> Jet1<R> {
        self.0.eval_first(uv)
    }

    fn eval_second<R: Real>(&self, uv: UvPoint<R>) -> Jet2<R> {
        self.0.eval_second(uv)
    }

    fn derivatives(
        &self,
        uv: UvPoint,
        order: u8,
        out: &mut Vec<Tangent<World>>,
        scratch: &mut Scratch,
    ) -> Result<(), Report> {
        self.0.derivatives(uv, order, out, scratch)
    }

    fn domain(&self) -> Domain {
        self.0.domain()
    }

    fn bounds(&self, patch: UvBox) -> Bounds {
        self.0.bounds(patch)
    }

    fn invert(&self, _: Point<World>, _: Option<UvPoint>, _: Tolerance, _: &mut Scratch) -> Result<UvPoint, Report> {
        Err(Report::new(Code::NotConverged, Fault::NumericallySensitive))
    }

    fn family(&self) -> FamilyName {
        FamilyName("unfindable")
    }

    fn content(&self, out: &mut Content) {
        self.0.content(out);
        out.push_family(self.family());
    }

    fn shape(&self) -> Shape {
        self.0.shape()
    }
}

#[test]
fn an_edge_whose_distance_from_a_face_cannot_be_measured_is_refused_and_so_is_the_loop_it_bounds() {
    let (result, before, built) = top_replaced(SurfaceRecord(Arc::new(Unfindable(plane(Vec3::new(0.5, 0.5, 1.0))))));
    let reports = result.unwrap_err();
    let top = before.view(built.faces[1]).unwrap().edges();
    let unmeasured = with(&reports, Reason::DeviationNotMeasured);
    assert_eq!(unmeasured.len(), 4, "{reports:#?}");
    for report in unmeasured {
        assert_eq!(report.entities.len(), 2, "{report:?}");
        assert!(top.iter().any(|e| e.entity() == report.entities[0]), "{report:?}");
        assert_eq!(report.entities[1], built.faces[1].entity());
        assert_eq!(number(report, "parameter"), Some(0.0), "the first step");
    }
    let untraced = with(&reports, Reason::LoopNotTraced);
    assert_eq!(untraced.len(), 1, "{reports:#?}");
    assert_eq!(untraced[0].entities[0], built.loops[1].entity());
    assert_eq!(untraced[0].entities.last(), Some(&built.faces[1].entity()));
}
