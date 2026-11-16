//! Source: original
//! The conformance suite over every family in one table (L21; `geometry.freeze`): a row a family, with one
//! representative of it and a window, each run through the suite its kind takes, surface, curve or parameter curve. A
//! family that the source check finds by its `impl` line and the table does not list fails
//! `every_family_is_in_the_table`, so a new family joins the table in the step that adds it. Each family's own module
//! keeps its own suite tests, over more windows and with seeded faults: the table is the one place that shows every
//! family passing the same checks, which is what the seam was frozen on. Each row is its own test, so they run at once.

use std::f64::consts::{FRAC_PI_2, TAU};

use occam_units::{Angle, Direction, Length, Placement, Point, UvPoint, Vec3, World};

use crate::conformance::{self, Failure, arc, local, placement, twist};
use crate::{
    Circle, Cone, Curve, Cylinder, Ellipse, Extrusion, Hyperbola, Line, MeshSurface, OffsetCurve, OffsetSurface,
    Parabola, ParamCurve, Plane, Revolution, Sphere, SplineSurface, Surface, Torus, TrimmedCurve, TrimmedSurface,
    UvBox, UvCurve,
};

/// A row: a family's representative, the window the suite runs over, and so which suite.
enum Row {
    /// A surface over a rectangle of its parameters.
    Surface(Box<dyn Surface>, UvBox),
    /// A curve from one parameter to another.
    Curve(Box<dyn Curve>, (f64, f64)),
    /// A parameter curve from one parameter to another.
    Param(Box<dyn ParamCurve>, (f64, f64)),
}

/// What the row's suite finds wrong.
fn failures(row: &Row) -> Vec<Failure> {
    match row {
        Row::Surface(s, window) => conformance::surface(s.as_ref(), *window),
        Row::Curve(c, (lo, hi)) => conformance::curve(c.as_ref(), *lo, *hi),
        Row::Param(c, (lo, hi)) => conformance::param_curve(c.as_ref(), *lo, *hi),
    }
}

fn mm(x: f64) -> Length {
    Length::from_millimetres(x)
}

fn window(lo: (f64, f64), hi: (f64, f64)) -> UvBox {
    UvBox::new(UvPoint::new(lo.0, lo.1), UvPoint::new(hi.0, hi.1))
}

/// [`placement`]'s Z axis.
fn up() -> Direction<World> {
    placement().direction(Direction::normalised(Vec3::new(0.0, 0.0, 1.0)))
}

fn ball() -> Sphere {
    Sphere::placed(&placement(), mm(5.0)).unwrap()
}

fn ring_of_ten() -> Circle {
    Circle::placed(&placement(), mm(10.0)).unwrap()
}

/// Six vertices over `[0, 2]` by `[0, 1]` and four triangles, on a sheet that bends.
fn mesh() -> MeshSurface {
    let at = |u: f64, v: f64| UvPoint::new(u, v);
    let uv = vec![at(0.0, 0.0), at(1.0, 0.0), at(2.0, 0.0), at(0.0, 1.0), at(1.0, 1.0), at(2.0, 1.0)];
    let points: Vec<Point<World>> =
        uv.iter().map(|p| Point::from_vec3(local(3.0 * p.u(), 2.0 * p.v(), p.u() * p.v()))).collect();
    MeshSurface::new(&points, uv, &[[0, 1, 4], [0, 4, 3], [1, 2, 5], [1, 5, 4]]).unwrap()
}

/// A curve swept along a slant to [`placement`]'s Z axis.
fn extruded() -> Row {
    let slant = placement().direction(Direction::normalised(Vec3::new(0.3, -0.2, 1.0)));
    let range = twist().range();
    Row::Surface(Box::new(Extrusion::new(twist(), slant).unwrap()), window((range.lo, -3.0), (range.hi, 4.0)))
}

/// A circle in the plane of the parameters, read as a parameter curve.
fn in_parameters() -> Row {
    let flat = Circle::placed(&Placement::identity(), mm(2.0)).unwrap();
    Row::Param(Box::new(UvCurve::new(flat).unwrap()), (0.0, TAU))
}

/// An ellipse on a cylinder, where a plane slanting across the axis cuts it, read in the cylinder's parameters: an
/// image that goes once round the axis.
fn imaged() -> Row {
    let cylinder = Cylinder::placed(&placement(), mm(6.0)).unwrap();
    let form = crate::Surface::analytic(&cylinder, conformance::tolerance()).unwrap();
    let chart = crate::Chart::of(&cylinder, form).unwrap();
    let toward = |x: f64, y: f64, z: f64| placement().direction(Direction::normalised(Vec3::new(x, y, z))).get();
    let (minor, normal) = (toward(0.0, -1.0, 0.0), toward(1.0, 0.0, 2.0));
    let major = normal.cross(minor);
    let origin = placement().point(Point::from_vec3(Vec3::new(0.0, 0.0, 0.0))).to_vec3();
    let axes = [origin, minor, major, minor.cross(major)];
    let sizes = [6.0, 3.0 * 5.0_f64.sqrt(), 0.0, TAU, 2.0];
    let numbers: Vec<f64> = axes.iter().flat_map(|v| [v.x, v.y, v.z]).chain(sizes).collect();
    let oval = Ellipse::restore(&numbers).unwrap();
    Row::Param(Box::new(crate::Image::conic(oval, chart).unwrap()), (0.0, TAU))
}

/// Defines the table: a test a row, named for the family, and the families it lists.
macro_rules! every_family {
    ($($test:ident: $family:literal => $row:expr;)*) => {
        /// The families the table lists, by type name.
        const LISTED: &[&str] = &[$($family),*];

        $(
            #[test]
            fn $test() {
                let failures = failures(&$row);
                assert!(failures.is_empty(), "{}: {failures:#?}", $family);
            }
        )*
    };
}

every_family! {
    plane: "Plane" => Row::Surface(Box::new(Plane::placed(&placement())), window((-3.0, 2.0), (4.0, 9.5)));
    cylinder: "Cylinder" =>
        Row::Surface(Box::new(Cylinder::placed(&placement(), mm(6.0)).unwrap()), window((-1.0, -15.0), (2.5, 25.0)));
    cone: "Cone" => Row::Surface(
        Box::new(Cone::placed(&placement(), mm(4.0), Angle::from_radians(0.5)).unwrap()),
        window((-1.0, -20.0), (2.5, 10.0)),
    );
    sphere: "Sphere" => Row::Surface(Box::new(ball()), window((-2.0, -FRAC_PI_2), (4.5, FRAC_PI_2)));
    torus: "Torus" => Row::Surface(
        Box::new(Torus::placed(&placement(), mm(8.0), mm(3.0)).unwrap()),
        window((-1.0, -2.0), (5.0, 3.0)),
    );
    spline_surface: "SplineSurface" => Row::Surface(
        Box::new(SplineSurface::new(&crate::workloads::spline()).unwrap()),
        window((0.0, 0.0), (1.0, 1.0)),
    );
    mesh_surface: "MeshSurface" => Row::Surface(Box::new(mesh()), window((0.2, 0.0), (1.7, 0.8)));
    revolution: "Revolution" => Row::Surface(
        Box::new(Revolution::new(Point::from_vec3(local(0.0, 0.0, 0.0)), up(), arc())),
        window((-1.0, 0.5), (2.5, 4.0)),
    );
    extrusion: "Extrusion" => extruded();
    offset_surface: "OffsetSurface" => Row::Surface(
        Box::new(OffsetSurface::new(ball(), mm(0.7)).unwrap()),
        window((-1.0, -1.2), (2.5, 1.3)),
    );
    trimmed_surface: "TrimmedSurface" => Row::Surface(
        Box::new(TrimmedSurface::new(ball(), window((0.3, 0.2), (2.0, FRAC_PI_2))).unwrap()),
        window((0.3, 0.2), (2.0, FRAC_PI_2)),
    );
    line: "Line" => Row::Curve(
        Box::new(Line::new(Point::from_vec3(Vec3::new(12.0, 3.5, -7.0)), Direction::normalised(Vec3::new(2.0, -1.0, 0.5)))),
        (-50.0, 70.0),
    );
    circle: "Circle" => Row::Curve(Box::new(ring_of_ten()), (0.0, TAU));
    ellipse: "Ellipse" => Row::Curve(Box::new(Ellipse::placed(&placement(), mm(12.0), mm(5.0)).unwrap()), (0.0, TAU));
    hyperbola: "Hyperbola" =>
        Row::Curve(Box::new(Hyperbola::placed(&placement(), mm(4.0), mm(3.0)).unwrap()), (-2.0, 2.5));
    parabola: "Parabola" => Row::Curve(Box::new(Parabola::placed(&placement(), mm(3.0)).unwrap()), (-20.0, 30.0));
    spline_curve: "SplineCurve" => Row::Curve(Box::new(twist()), (twist().range().lo, twist().range().hi));
    offset_curve: "OffsetCurve" =>
        Row::Curve(Box::new(OffsetCurve::new(ring_of_ten(), mm(2.0), up()).unwrap()), (0.0, TAU));
    trimmed_curve: "TrimmedCurve" =>
        Row::Curve(Box::new(TrimmedCurve::new(ring_of_ten(), 5.0, 7.0).unwrap()), (5.0, 7.0));
    uv_curve: "UvCurve" => in_parameters();
    image: "Image" => imaged();
}

#[test]
#[cfg(not(target_family = "wasm"))]
fn every_family_is_in_the_table() {
    let found = crate::source_check::families();
    let mut listed: Vec<&str> = LISTED.to_vec();
    listed.sort_unstable();
    let before = listed.len();
    listed.dedup();
    assert_eq!(listed.len(), before, "a family listed twice: {LISTED:?}");
    let mut found: Vec<&str> = found.iter().map(String::as_str).collect();
    found.sort_unstable();
    assert_eq!(found, listed, "the families the source declares, against the table's");
}

#[test]
fn a_stretch_keeps_the_families_closed_under_one_and_no_others() {
    // A scale that differs along three skew axes keeps a plane, a line, a B-spline, a mesh, and an extrusion or a
    // trimmed curve of one of those; a cylinder, a cone, a sphere, a torus, a conic, a revolution and an offset are not
    // closed under it, and a caller moves their B-spline instead. A trimmed surface keeps itself only where its base
    // moves each parameter on its own, which a sphere does not under a stretch at all.
    let stretch = occam_units::Affine::stretch(&placement(), [2.0, 0.5, 1.3]).unwrap();
    let line =
        || Line::new(Point::from_vec3(Vec3::new(1.0, 2.0, 3.0)), Direction::normalised(Vec3::new(2.0, -1.0, 0.5)));
    let kept: [Box<dyn Surface>; 4] = [
        Box::new(Plane::placed(&placement())),
        Box::new(SplineSurface::new(&crate::workloads::spline()).unwrap()),
        Box::new(mesh()),
        Box::new(Extrusion::new(twist(), up()).unwrap()),
    ];
    for s in kept {
        assert!(s.moved(&stretch).is_some(), "{:?}", s.family());
    }
    let curves: [Box<dyn Curve>; 3] =
        [Box::new(line()), Box::new(twist()), Box::new(TrimmedCurve::new(line(), 1.0, 4.0).unwrap())];
    for c in curves {
        assert!(c.moved(&stretch).is_some(), "{:?}", c.family());
    }
    let declined: [Box<dyn Surface>; 7] = [
        Box::new(Cylinder::placed(&placement(), mm(6.0)).unwrap()),
        Box::new(Cone::placed(&placement(), mm(4.0), Angle::from_radians(0.5)).unwrap()),
        Box::new(ball()),
        Box::new(Torus::placed(&placement(), mm(8.0), mm(3.0)).unwrap()),
        Box::new(Revolution::new(Point::from_vec3(local(0.0, 0.0, 0.0)), up(), arc())),
        Box::new(OffsetSurface::new(Plane::placed(&placement()), mm(0.7)).unwrap()),
        Box::new(TrimmedSurface::new(ball(), window((0.3, 0.2), (2.0, 1.0))).unwrap()),
    ];
    for s in declined {
        assert!(s.moved(&stretch).is_none(), "{:?}", s.family());
    }
    let conics: [Box<dyn Curve>; 5] = [
        Box::new(ring_of_ten()),
        Box::new(Ellipse::placed(&placement(), mm(12.0), mm(5.0)).unwrap()),
        Box::new(Hyperbola::placed(&placement(), mm(4.0), mm(3.0)).unwrap()),
        Box::new(Parabola::placed(&placement(), mm(3.0)).unwrap()),
        Box::new(OffsetCurve::new(line(), mm(2.0), up()).unwrap()),
    ];
    for c in conics {
        assert!(c.moved(&stretch).is_none(), "{:?}", c.family());
    }
}
