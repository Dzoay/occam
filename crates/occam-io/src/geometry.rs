//! Source: original
//! Curves and surfaces as the native format writes them: the family's name and the numbers its content holds after it,
//! which define it bit for bit (`occam_geom::Content`), so the family restores itself from them and refuses what none of
//! its constructors would make. The families the first release builds with are written; a model holding any other is
//! refused at save with `io.not-saved`, naming the record, until its family can be.

use std::sync::Arc;

use occam_geom::{
    Circle, Cone, Content, Curve, Cylinder, Ellipse, Hyperbola, Line, Parabola, ParamCurve, Plane, Sphere, SplineCurve,
    SplineSurface, Surface, Torus, UvCurve,
};
use occam_report::Report;
use serde::{Deserialize, Serialize};

use crate::exact::Exact;

/// The families of surface the format writes.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum SurfaceFamily {
    Plane,
    Cylinder,
    Cone,
    Sphere,
    Torus,
    Bspline,
}

/// The families of curve the format writes, in space or, as a face's parameter curve, in its parameters.
#[derive(Clone, Copy, Debug, PartialEq, Eq, Serialize, Deserialize)]
#[serde(rename_all = "kebab-case")]
pub(crate) enum CurveFamily {
    Line,
    Circle,
    Ellipse,
    Hyperbola,
    Parabola,
    Bspline,
}

/// A curve or surface as written: its family and the numbers that define it.
#[derive(Clone, Debug, Serialize, Deserialize)]
#[serde(deny_unknown_fields)]
pub(crate) struct Geometry<F> {
    family: F,
    numbers: Vec<Exact>,
}

/// Numbers as written.
fn written(numbers: &[f64]) -> Vec<Exact> {
    numbers.iter().copied().map(Exact).collect()
}

/// Numbers as read.
fn read(numbers: &[Exact]) -> Vec<f64> {
    numbers.iter().map(|n| n.0).collect()
}

/// The family named `name`, and the numbers `content` holds after that name; `None` if the content does not begin with
/// it.
fn split<'a>(content: &'a Content, name: &str) -> Option<&'a [u64]> {
    let words = content.words();
    let length = usize::try_from(*words.first()?).ok()?;
    let (head, numbers) = words.get(1..)?.split_at_checked(length)?;
    head.iter().map(|w| u8::try_from(*w).ok()).collect::<Option<Vec<u8>>>().filter(|b| b == name.as_bytes())?;
    Some(numbers)
}

/// The numbers of `content` after the family's name, as written.
fn numbers(content: &Content, name: &str) -> Option<Vec<Exact>> {
    split(content, name).map(|words| written(&words.iter().map(|w| f64::from_bits(*w)).collect::<Vec<_>>()))
}

/// `s` as written, or `None` if its family is not one the format writes.
pub(crate) fn surface(s: &dyn Surface) -> Option<Geometry<SurfaceFamily>> {
    let name = s.family().0;
    let family = match name {
        "plane" => SurfaceFamily::Plane,
        "cylinder" => SurfaceFamily::Cylinder,
        "cone" => SurfaceFamily::Cone,
        "sphere" => SurfaceFamily::Sphere,
        "torus" => SurfaceFamily::Torus,
        "bspline" => SurfaceFamily::Bspline,
        _ => return None,
    };
    let mut content = Content::default();
    s.content(&mut content);
    Some(Geometry { family, numbers: numbers(&content, name)? })
}

/// The family of curve named `name`, if the format writes it.
fn curve_family(name: &str) -> Option<CurveFamily> {
    Some(match name {
        "line" => CurveFamily::Line,
        "circle" => CurveFamily::Circle,
        "ellipse" => CurveFamily::Ellipse,
        "hyperbola" => CurveFamily::Hyperbola,
        "parabola" => CurveFamily::Parabola,
        "bspline" => CurveFamily::Bspline,
        _ => return None,
    })
}

/// `c` as written, or `None` if its family is not one the format writes.
pub(crate) fn curve(c: &dyn Curve) -> Option<Geometry<CurveFamily>> {
    let name = c.family().0;
    let mut content = Content::default();
    c.content(&mut content);
    Some(Geometry { family: curve_family(name)?, numbers: numbers(&content, name)? })
}

/// `c` as written, or `None` if its family is not one the format writes. A parameter curve is a curve of one of the
/// families drawn in its face's parameters, and is named as that family.
pub(crate) fn param_curve(c: &dyn ParamCurve) -> Option<Geometry<CurveFamily>> {
    let name = c.family().0;
    let mut content = Content::default();
    c.content(&mut content);
    Some(Geometry { family: curve_family(name)?, numbers: numbers(&content, name)? })
}

/// The surface `g` writes.
///
/// # Errors
///
/// As the family's `restore`.
pub(crate) fn restored_surface(g: &Geometry<SurfaceFamily>) -> Result<Arc<dyn Surface>, Report> {
    let n = read(&g.numbers);
    Ok(match g.family {
        SurfaceFamily::Plane => Arc::new(Plane::restore(&n)?),
        SurfaceFamily::Cylinder => Arc::new(Cylinder::restore(&n)?),
        SurfaceFamily::Cone => Arc::new(Cone::restore(&n)?),
        SurfaceFamily::Sphere => Arc::new(Sphere::restore(&n)?),
        SurfaceFamily::Torus => Arc::new(Torus::restore(&n)?),
        SurfaceFamily::Bspline => Arc::new(SplineSurface::restore(&n)?),
    })
}

/// The curve `g` writes.
///
/// # Errors
///
/// As the family's `restore`.
pub(crate) fn restored_curve(g: &Geometry<CurveFamily>) -> Result<Arc<dyn Curve>, Report> {
    let n = read(&g.numbers);
    Ok(match g.family {
        CurveFamily::Line => Arc::new(Line::restore(&n)?),
        CurveFamily::Circle => Arc::new(Circle::restore(&n)?),
        CurveFamily::Ellipse => Arc::new(Ellipse::restore(&n)?),
        CurveFamily::Hyperbola => Arc::new(Hyperbola::restore(&n)?),
        CurveFamily::Parabola => Arc::new(Parabola::restore(&n)?),
        CurveFamily::Bspline => Arc::new(SplineCurve::restore(&n)?),
    })
}

/// The parameter curve `g` writes: the curve of its family drawn in a face's parameters.
///
/// # Errors
///
/// As the family's `restore`, and `geom.not-in-plane` for a curve that leaves the plane its parameters are drawn in.
pub(crate) fn restored_param_curve(g: &Geometry<CurveFamily>) -> Result<Arc<dyn ParamCurve>, Report> {
    let n = read(&g.numbers);
    Ok(match g.family {
        CurveFamily::Line => Arc::new(UvCurve::new(Line::restore(&n)?)?),
        CurveFamily::Circle => Arc::new(UvCurve::new(Circle::restore(&n)?)?),
        CurveFamily::Ellipse => Arc::new(UvCurve::new(Ellipse::restore(&n)?)?),
        CurveFamily::Hyperbola => Arc::new(UvCurve::new(Hyperbola::restore(&n)?)?),
        CurveFamily::Parabola => Arc::new(UvCurve::new(Parabola::restore(&n)?)?),
        CurveFamily::Bspline => Arc::new(UvCurve::new(SplineCurve::restore(&n)?)?),
    })
}
