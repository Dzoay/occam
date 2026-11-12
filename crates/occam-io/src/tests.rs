//! Source: original
//! The native format's tests: every clean model, and a model holding every family the format writes, comes back with
//! its digest; a file with one record wrong is refused naming it; a file of another version is refused with its version;
//! and a family the format does not write yet is refused at save, named.

use std::sync::Arc;

use occam_geom::{
    Circle, Cone, Cylinder, Ellipse, Hyperbola, Line, OffsetSurface, Parabola, Plane, Sphere, SplineCurve,
    SplineSurface, Torus, UvCurve,
};
use occam_report::{Code, Reason};
use occam_topo::workloads::models;
use occam_topo::{CurveRecord, ParamCurveRecord, SurfaceRecord};
use occam_units::{Angle, Direction, Local, Placement, Point, Vec3, Vector, World};
use serde_json::Value;

use super::*;

fn serial() -> Exec {
    Exec::serial()
}

fn mm(x: f64) -> Length {
    Length::from_millimetres(x)
}

/// A placement turned about a skew axis and moved, so that no axis is a world axis.
fn skewed() -> Placement<Local, World> {
    let axis = occam_units::Direction::<Local>::normalised(Vec3::new(1.0, 2.0, 3.0));
    Placement::<Local, Local>::rotation(axis, Angle::from_radians(0.7)).then(&Placement::identity()).then(&Placement::<
        World,
        World,
    >::translation(
        Vector::from_vec3(Vec3::new(3.0, -2.0, 5.5)),
    ))
}

fn at(x: f64, y: f64, z: f64) -> Point<World> {
    Point::from_vec3(Vec3::new(x, y, z))
}

/// A model holding one curve or surface of every family the format writes, and a parameter curve of each family of
/// curve, none of them yet used by a face or an edge.
fn zoo() -> Snapshot {
    let p = skewed();
    let tolerance = Tolerance::new(mm(1e-7), Angle::from_radians(1e-9)).unwrap();
    let mut edit = Snapshot::empty(tolerance).edit();
    let surfaces: Vec<Arc<dyn occam_geom::Surface>> = vec![
        Arc::new(Plane::placed(&p)),
        Arc::new(Cylinder::placed(&p, mm(2.0)).unwrap()),
        Arc::new(Cone::placed(&p, mm(1.0), Angle::from_radians(0.4)).unwrap()),
        Arc::new(Sphere::placed(&p, mm(3.0)).unwrap()),
        Arc::new(Torus::placed(&p, mm(4.0), mm(1.0)).unwrap()),
        Arc::new(
            SplineSurface::bezier(
                (2, 2),
                vec![at(0.0, 0.0, 0.0), at(1.0, 0.0, 0.5), at(0.0, 1.0, 0.25), at(1.0, 1.0, 0.0)],
                None,
            )
            .unwrap(),
        ),
    ];
    for s in surfaces {
        edit.add(SurfaceRecord(s));
    }
    let circle = Circle::placed(&p, mm(2.0)).unwrap();
    let spline =
        SplineCurve::bezier(vec![at(0.0, 0.0, 0.0), at(1.0, 2.0, 0.0), at(3.0, 0.0, 1.0)], Some(vec![1.0, 0.5, 1.0]))
            .unwrap();
    let curves: Vec<Arc<dyn occam_geom::Curve>> = vec![
        Arc::new(Line::new(at(1.0, 2.0, 3.0), Direction::normalised(Vec3::new(0.0, 0.6, 0.8)))),
        Arc::new(circle),
        Arc::new(circle.trimmed(0.0, core::f64::consts::TAU).unwrap()),
        Arc::new(Ellipse::placed(&p, mm(3.0), mm(1.0)).unwrap().trimmed(0.5, 2.0).unwrap()),
        Arc::new(Hyperbola::placed(&p, mm(2.0), mm(1.0)).unwrap()),
        Arc::new(Parabola::placed(&p, mm(0.5)).unwrap()),
        Arc::new(spline),
    ];
    for c in curves {
        edit.add(CurveRecord(c));
    }
    let flat = Placement::<Local, World>::identity();
    let uv = |x: f64, y: f64| at(x, y, 0.0);
    let pcurves: Vec<Arc<dyn occam_geom::ParamCurve>> = vec![
        Arc::new(UvCurve::new(Line::new(uv(0.5, 0.5), Direction::normalised(Vec3::new(1.0, 0.0, 0.0)))).unwrap()),
        Arc::new(UvCurve::new(Circle::placed(&flat, mm(1.0)).unwrap()).unwrap()),
        Arc::new(UvCurve::new(Ellipse::placed(&flat, mm(2.0), mm(1.0)).unwrap()).unwrap()),
        Arc::new(UvCurve::new(Hyperbola::placed(&flat, mm(1.0), mm(1.0)).unwrap()).unwrap()),
        Arc::new(UvCurve::new(Parabola::placed(&flat, mm(1.0)).unwrap()).unwrap()),
        Arc::new(
            UvCurve::new(SplineCurve::bezier(vec![uv(0.0, 0.0), uv(1.0, 1.0), uv(2.0, 0.0)], None).unwrap()).unwrap(),
        ),
    ];
    for c in pcurves {
        edit.add(ParamCurveRecord(c));
    }
    edit.commit(&serial()).unwrap().0
}

/// `model` saved, then loaded.
fn round_trip(model: &Snapshot) -> (Vec<u8>, Snapshot) {
    let bytes = save(model).unwrap_or_else(|r| panic!("{r:?}"));
    let back = load(&bytes, &serial()).unwrap_or_else(|r| panic!("{r:#?}"));
    (bytes, back)
}

#[test]
fn every_clean_model_comes_back_with_its_digest_and_writes_the_same_bytes_again() {
    let mut all = models(&serial());
    assert_eq!(all.len(), 8);
    all.push(("every family", zoo()));
    for (name, model) in all {
        let (bytes, back) = round_trip(&model);
        assert_eq!(back.digest(), model.digest(), "{name}");
        assert_eq!(save(&back).unwrap(), bytes, "{name}: the same bytes again");
    }
}

#[test]
fn free_slots_come_back_so_the_next_edit_gives_the_same_ids() {
    let (_, cube) = models(&serial()).swap_remove(0);
    let mut edit = cube.edit();
    let extra =
        edit.add(CurveRecord(Arc::new(Line::new(at(0.0, 0.0, 0.0), Direction::normalised(Vec3::new(1.0, 0.0, 0.0))))));
    let cube = edit.commit(&serial()).unwrap().0;
    let mut edit = cube.edit();
    edit.remove(extra).unwrap();
    let cube = edit.commit(&serial()).unwrap().0;
    let (_, back) = round_trip(&cube);
    let next = |s: &Snapshot| {
        s.edit()
            .add(CurveRecord(Arc::new(Line::new(at(0.0, 0.0, 0.0), Direction::normalised(Vec3::new(0.0, 1.0, 0.0))))))
    };
    assert_eq!(next(&back), next(&cube));
    assert_eq!(next(&back).generation().get(), 2, "the freed slot, used again");
}

/// `model` saved, with `change` made to its JSON, and loaded.
fn changed(model: &Snapshot, change: impl FnOnce(&mut Value)) -> Vec<Report> {
    let mut json: Value = serde_json::from_slice(&save(model).unwrap()).unwrap();
    change(&mut json);
    load(&serde_json::to_vec(&json).unwrap(), &serial()).unwrap_err()
}

#[test]
fn a_file_with_one_record_wrong_is_refused_naming_it() {
    let (_, cube) = models(&serial()).swap_remove(0);
    let moved = changed(&cube, |json| json["vertices"][0]["record"]["point"] = serde_json::json!([9.0, 9.0, 9.0]));
    assert!(!moved.is_empty() && moved.iter().all(|r| r.code == Code::InvariantFailed), "{moved:#?}");
    let vertex = EntityRef::new(occam_report::EntityKind::Vertex, 0, 1);
    assert!(moved.iter().any(|r| r.reason == Some(Reason::VertexOffEdge) && r.entities.first() == Some(&vertex)));
    let cylinder = models(&serial()).swap_remove(2).1;
    let slot =
        cylinder.saved::<SurfaceGeom>().position(|s| s.record.is_some_and(|r| r.0.family().0 == "cylinder")).unwrap();
    let thin = changed(&cylinder, |json| {
        let numbers = json["surfaces"][slot]["record"]["numbers"].as_array_mut().unwrap();
        *numbers.last_mut().unwrap() = serde_json::json!(-1.0);
    });
    assert_eq!(thin.len(), 1, "{thin:#?}");
    assert_eq!(thin[0].code, Code::SizeNotPositive);
    assert_eq!(thin[0].entities, [EntityRef::new(occam_report::EntityKind::Surface, u32::try_from(slot).unwrap(), 1)]);
}

#[test]
fn what_is_not_the_format_is_refused_where_it_is_written() {
    let (_, cube) = models(&serial()).swap_remove(0);
    let wrong_kind = changed(&cube, |json| json["edges"][0]["record"]["curve"] = serde_json::json!("surface:0/1"));
    assert_eq!(wrong_kind.len(), 1);
    assert_eq!(wrong_kind[0].code, Code::NotReadable);
    assert!(wrong_kind[0].value("line").and_then(NamedValue::number).is_some_and(|line| line >= 1.0));
    let unknown = changed(&cube, |json| json["surfaces"][0]["record"]["family"] = serde_json::json!("blob"));
    assert_eq!(unknown[0].code, Code::NotReadable);
    let extra = changed(&cube, |json| json["faces"][0]["record"]["colour"] = serde_json::json!("red"));
    assert_eq!(extra[0].code, Code::NotReadable, "a field a record does not have");
    let other = changed(&cube, |json| json["format"] = serde_json::json!("something-else"));
    assert_eq!(other[0].code, Code::NotReadable);
    assert_eq!(load(b"not json", &serial()).unwrap_err()[0].code, Code::NotReadable);
}

#[test]
fn a_file_of_another_version_is_refused_with_its_version() {
    let (_, cube) = models(&serial()).swap_remove(0);
    for (version, rest) in [(2, false), (7, true)] {
        let refused = changed(&cube, |json| {
            json["version"] = serde_json::json!(version);
            if rest {
                json["edges"] = serde_json::json!("a newer layout");
            }
        });
        assert_eq!(refused.len(), 1);
        assert_eq!(refused[0].code, Code::OtherVersion);
        assert_eq!(refused[0].value("version").and_then(NamedValue::number), Some(f64::from(version)));
        assert_eq!(refused[0].value("supported").and_then(NamedValue::number), Some(1.0));
    }
}

#[test]
fn a_family_the_format_does_not_write_yet_is_refused_at_save_naming_it() {
    let (_, cube) = models(&serial()).swap_remove(0);
    let mut edit = cube.edit();
    let offset = edit.add(SurfaceRecord(Arc::new(OffsetSurface::new(Plane::placed(&skewed()), mm(1.0)).unwrap())));
    let model = edit.commit(&serial()).unwrap().0;
    let refused = save(&model).unwrap_err();
    assert_eq!((refused.code, refused.fault), (Code::NotSaved, Some(Fault::NotImplemented)));
    assert_eq!(refused.entities, [offset.entity()]);
}
