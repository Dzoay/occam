//! Source: original
//! Every family that can be saved comes back from its content as it was, field for field and bit for bit, and what no
//! constructor would make is refused.

use core::f64::consts::{FRAC_PI_4, TAU};
use occam_fixtures::mm;

use occam_report::Code;
use occam_units::{Angle, Direction, Local, Placement, Point, Vec3, Vector, World};

use crate::bspline::{BsplineCurve, BsplineSurface};
use crate::curve::ParametricCurve;
use crate::identity::Content;
use crate::surface::Parametric;
use crate::{
    Circle, Cone, Cylinder, Ellipse, Hyperbola, Line, Parabola, Plane, Sphere, SplineCurve, SplineSurface, Torus,
};

/// A placement turned about a skew axis and moved, so that no axis is a world axis.
fn skewed() -> Placement<Local, World> {
    let axis = Direction::<Local>::normalised(Vec3::new(1.0, 2.0, 3.0));
    Placement::<Local, Local>::rotation(axis, Angle::from_radians(0.7)).then(&Placement::identity()).then(&Placement::<
        World,
        World,
    >::translation(
        Vector::from_vec3(Vec3::new(3.0, -2.0, 5.5)),
    ))
}

/// The numbers a surface's content holds after its family's name.
fn surface_numbers<S: Parametric>(s: &S) -> Vec<f64> {
    let mut out = Content::default();
    Parametric::content(s, &mut out);
    out.words().iter().map(|w| f64::from_bits(*w)).collect()
}

/// The numbers a curve's content holds after its family's name.
fn curve_numbers<C: ParametricCurve>(c: &C) -> Vec<f64> {
    let mut out = Content::default();
    ParametricCurve::content(c, &mut out);
    out.words().iter().map(|w| f64::from_bits(*w)).collect()
}

/// `s` restored from its numbers is `s`, its numbers bit for bit.
fn surface_comes_back<S: Parametric + PartialEq + core::fmt::Debug>(
    s: &S,
    restore: fn(&[f64]) -> Result<S, occam_report::Report>,
) {
    let numbers = surface_numbers(s);
    let back = restore(&numbers).unwrap_or_else(|r| panic!("{r:?}"));
    assert_eq!(&back, s);
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&surface_numbers(&back)), bits(&numbers));
}

/// `c` restored from its numbers is `c`, its numbers bit for bit.
fn curve_comes_back<C: ParametricCurve + PartialEq + core::fmt::Debug>(
    c: &C,
    restore: fn(&[f64]) -> Result<C, occam_report::Report>,
) {
    let numbers = curve_numbers(c);
    let back = restore(&numbers).unwrap_or_else(|r| panic!("{r:?}"));
    assert_eq!(&back, c);
    let bits = |v: &[f64]| v.iter().map(|x| x.to_bits()).collect::<Vec<_>>();
    assert_eq!(bits(&curve_numbers(&back)), bits(&numbers));
}

fn spline_curve(rational: bool) -> SplineCurve {
    let poles = vec![
        Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)),
        Point::from_vec3(Vec3::new(1.0, 2.0, -0.5)),
        Point::from_vec3(Vec3::new(3.0, 1.0, 0.25)),
        Point::from_vec3(Vec3::new(4.0, -1.0, 1.0)),
        Point::from_vec3(Vec3::new(6.0, 0.5, 0.0)),
    ];
    let weights = rational.then(|| vec![1.0, 0.5, 2.0, 0.75, 1.0]);
    SplineCurve::new(&BsplineCurve {
        degree: 3,
        knots: vec![-1.0, -1.0, -1.0, -1.0, 0.3, 2.0, 2.0, 2.0, 2.0],
        poles,
        weights,
    })
    .unwrap()
}

fn spline_surface(rational: bool) -> SplineSurface {
    let poles = (0..12)
        .map(|k| Point::from_vec3(Vec3::new(f64::from(k % 4), f64::from(k / 4), f64::from(k % 3) * 0.5)))
        .collect();
    let weights = rational.then(|| (0..12).map(|k| 1.0 + f64::from(k) / 8.0).collect());
    let form = BsplineSurface {
        degree: (2, 1),
        count: (4, 3),
        knots_u: vec![0.0, 0.0, 0.0, 0.5, 1.0, 1.0, 1.0],
        knots_v: vec![0.0, 0.0, 0.4, 1.0, 1.0],
        poles,
        weights,
    };
    SplineSurface::new(&form).unwrap()
}

#[test]
fn every_surface_family_comes_back_from_its_content() {
    let p = skewed();
    surface_comes_back(&Plane::placed(&p), Plane::restore);
    surface_comes_back(&Cylinder::placed(&p, mm(2.5)).unwrap(), Cylinder::restore);
    surface_comes_back(&Cone::placed(&p, mm(1.5), Angle::from_radians(0.3)).unwrap(), Cone::restore);
    surface_comes_back(&Cone::placed(&p, mm(0.0), Angle::from_radians(FRAC_PI_4)).unwrap(), Cone::restore);
    surface_comes_back(&Sphere::placed(&p, mm(4.0)).unwrap(), Sphere::restore);
    surface_comes_back(&Torus::placed(&p, mm(5.0), mm(1.25)).unwrap(), Torus::restore);
    for rational in [false, true] {
        surface_comes_back(&spline_surface(rational), SplineSurface::restore);
    }
    let bezier = SplineSurface::bezier(
        (2, 2),
        (0..4).map(|k| Point::from_vec3(Vec3::new(f64::from(k % 2), f64::from(k / 2), 0.0))).collect(),
        Some(vec![1.0, 2.0, 2.0, 1.0]),
    )
    .unwrap();
    surface_comes_back(&bezier, SplineSurface::restore);
}

#[test]
fn every_curve_family_comes_back_from_its_content_with_its_range_and_how_it_wraps() {
    let p = skewed();
    let along = Direction::<World>::normalised(Vec3::new(-1.0, 0.5, 2.0));
    let line = Line::new(Point::from_vec3(Vec3::new(1.0, 2.0, 3.0)), along);
    curve_comes_back(&line, Line::restore);
    curve_comes_back(&line.trimmed(-1.5, 4.0).unwrap(), Line::restore);
    let circle = Circle::placed(&p, mm(3.0)).unwrap();
    for c in [circle, circle.trimmed(0.25, 2.0).unwrap(), circle.trimmed(0.0, TAU).unwrap()] {
        curve_comes_back(&c, Circle::restore);
    }
    assert_ne!(
        curve_numbers(&circle),
        curve_numbers(&circle.trimmed(0.0, TAU).unwrap()),
        "a whole turn trimmed stops at its seam"
    );
    let ellipse = Ellipse::placed(&p, mm(3.0), mm(1.0)).unwrap();
    curve_comes_back(&ellipse, Ellipse::restore);
    curve_comes_back(&ellipse.trimmed(1.0, 6.0).unwrap(), Ellipse::restore);
    let hyperbola = Hyperbola::placed(&p, mm(2.0), mm(1.0)).unwrap();
    curve_comes_back(&hyperbola, Hyperbola::restore);
    curve_comes_back(&hyperbola.trimmed(-1.0, 2.0).unwrap(), Hyperbola::restore);
    let parabola = Parabola::placed(&p, mm(0.75)).unwrap();
    curve_comes_back(&parabola, Parabola::restore);
    curve_comes_back(&parabola.trimmed(-3.0, 1.0).unwrap(), Parabola::restore);
    for rational in [false, true] {
        curve_comes_back(&spline_curve(rational), SplineCurve::restore);
    }
    let bezier = SplineCurve::bezier(
        vec![
            Point::from_vec3(Vec3::new(0.0, 0.0, 0.0)),
            Point::from_vec3(Vec3::new(1.0, 1.0, 0.0)),
            Point::from_vec3(Vec3::new(2.0, 0.0, 1.0)),
        ],
        None,
    )
    .unwrap();
    curve_comes_back(&bezier, SplineCurve::restore);
}

#[test]
fn numbers_no_constructor_would_make_are_refused() {
    let p = skewed();
    let plane = surface_numbers(&Plane::placed(&p));
    let mut extra = plane.clone();
    extra.push(0.0);
    assert_eq!(Plane::restore(&extra).unwrap_err().code, Code::WrongCount);
    assert_eq!(Plane::restore(&plane[..11]).unwrap_err().code, Code::WrongCount);
    let mut skew = plane.clone();
    skew[3] *= 1.001;
    assert_eq!(Plane::restore(&skew).unwrap_err().code, Code::DegenerateDirection, "an axis that is not a unit");
    let mut left = plane.clone();
    for k in 3..6 {
        left[k] = -plane[k];
    }
    assert_eq!(Plane::restore(&left).unwrap_err().code, Code::DegenerateDirection, "left-handed axes");
    let mut far = plane.clone();
    far[0] = f64::INFINITY;
    assert_eq!(Plane::restore(&far).unwrap_err().code, Code::NotFinite);
    let mut cylinder = surface_numbers(&Cylinder::placed(&p, mm(2.0)).unwrap());
    *cylinder.last_mut().unwrap() = -2.0;
    assert_eq!(Cylinder::restore(&cylinder).unwrap_err().code, Code::SizeNotPositive);
    let mut torus = surface_numbers(&Torus::placed(&p, mm(5.0), mm(1.0)).unwrap());
    *torus.last_mut().unwrap() = 6.0;
    assert_eq!(Torus::restore(&torus).unwrap_err().code, Code::OutOfRange, "a tube wider than the ring");
    let mut circle = curve_numbers(&Circle::placed(&p, mm(1.0)).unwrap().trimmed(0.5, 1.0).unwrap());
    *circle.last_mut().unwrap() = 2.0;
    assert_eq!(Circle::restore(&circle).unwrap_err().code, Code::OutOfRange, "an arc does not carry on round");
    let mut spline = curve_numbers(&spline_curve(false));
    *spline.last_mut().unwrap() = 2.0;
    assert_eq!(
        SplineCurve::restore(&spline).unwrap_err().code,
        Code::OutOfRange,
        "a weight in a form that is not rational"
    );
    let mut rational = curve_numbers(&spline_curve(true));
    *rational.last_mut().unwrap() = -1.0;
    assert_eq!(SplineCurve::restore(&rational).unwrap_err().code, Code::WeightNotPositive);
    let mut flags = curve_numbers(&spline_curve(false));
    flags[0] = 1.0;
    assert_eq!(
        SplineCurve::restore(&flags).unwrap_err().code,
        Code::KnotsNotValid,
        "a Bézier form must have its own knots"
    );
    let mut order = curve_numbers(&spline_curve(false));
    order.swap(7, 8);
    assert_eq!(SplineCurve::restore(&order).unwrap_err().code, Code::KnotsNotValid);
    let mut short = surface_numbers(&spline_surface(false));
    short.pop();
    assert_eq!(SplineSurface::restore(&short).unwrap_err().code, Code::WrongCount);
}
