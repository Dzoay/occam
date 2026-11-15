//! Source: original
//! A body as a sweep lays it out before the solid writing writes it: every point, edge and face with the entity of the
//! profile it was swept from, and the step's part in it the profile's. Each loop runs anticlockwise about the face's
//! outward normal, so each edge's two uses run opposite ways. Lines and circles are laid out as [`Bent`]s, moved and
//! turned whole.

use std::sync::Arc;

use occam_geom::{Circle, Curve, Line, Plane, Surface};
use occam_report::{EntityRef, Report, StepRole};
use occam_topo::{BodyKind, Derivation, EdgeRole};
use occam_units::{Direction, Length, Point, UvPoint, Vec3};

use crate::construct::space::{Bent, Flatland, frame};
use crate::primitive::solid::{EdgeSpec, FaceSpec, LoopSpec, Origin, Solid, Sources, UseSpec};

/// A body as a sweep lays it out: the solid writing makes of it, the profile's entity each point, edge and face came
/// from, and its kind.
pub(super) struct Shape {
    pub solid: Solid,
    pub origin: Origin,
}

impl Shape {
    /// An empty body of `kind`, swept from the profile's body `from`.
    pub(super) const fn new(kind: BodyKind, from: EntityRef) -> Self {
        let sources =
            Sources { how: Derivation::Swept, body: from, points: Vec::new(), edges: Vec::new(), faces: Vec::new() };
        Self {
            solid: Solid { points: Vec::new(), edges: Vec::new(), faces: Vec::new() },
            origin: Origin { role: StepRole::Profile, kind, from: Some(sources) },
        }
    }

    /// The sources being gathered.
    fn sources(&mut self) -> Option<&mut Sources> {
        self.origin.from.as_mut()
    }

    /// The point at `index`, or the origin for none.
    pub(super) fn at(&self, index: usize) -> Vec3 {
        self.solid.points.get(index).map_or(Vec3::new(0.0, 0.0, 0.0), |p| p.to_vec3())
    }

    /// Adds the point `at`, swept from `from`, and gives its index.
    pub(super) fn point(&mut self, at: Vec3, from: EntityRef) -> usize {
        self.solid.points.push(Point::from_vec3(at));
        self.sources().into_iter().for_each(|s| s.points.push(from));
        self.solid.points.len().saturating_sub(1)
    }

    /// Adds the edge along `bent` over `range` from point `ends.0` to point `ends.1`, swept from `from`, and gives its
    /// index.
    ///
    /// # Errors
    ///
    /// What the circle's family reports, which a circle of a positive radius does not.
    pub(super) fn edge(
        &mut self,
        bent: Bent,
        range: (f64, f64),
        ends: (usize, usize),
        from: EntityRef,
    ) -> Result<usize, Report> {
        self.solid.edges.push(EdgeSpec { curve: curve(bent)?, range, ends, role: EdgeRole::Ordinary });
        self.sources().into_iter().for_each(|s| s.edges.push(from));
        Ok(self.solid.edges.len().saturating_sub(1))
    }

    /// Adds the face on `surface` bounded by `loops`, the outer first, swept from `from`.
    pub(super) fn face(&mut self, surface: Arc<dyn Surface>, loops: Vec<LoopSpec>, from: EntityRef) {
        self.solid.faces.push(FaceSpec { surface, loops });
        self.sources().into_iter().for_each(|s| s.faces.push(from));
    }
}

/// The curve `bent` describes, with the same parameter.
///
/// # Errors
///
/// What the circle's family reports, which a circle of a positive radius does not.
pub(super) fn curve(bent: Bent) -> Result<Arc<dyn Curve>, Report> {
    match bent {
        Bent::Line { origin, along } => Ok(Arc::new(Line::new(Point::from_vec3(origin), Direction::normalised(along)))),
        Bent::Circle { centre, x, y, radius } => {
            let placement = frame(centre, x.cross(y), Some(x));
            Ok(Arc::new(Circle::placed(&placement, Length::from_millimetres(radius))?))
        }
    }
}

/// `bent` moved by `d`.
pub(super) fn moved(bent: Bent, d: Vec3) -> Bent {
    match bent {
        Bent::Line { origin, along } => Bent::Line { origin: origin + d, along },
        Bent::Circle { centre, x, y, radius } => Bent::Circle { centre: centre + d, x, y, radius },
    }
}

/// A turn by an angle about an axis through a point.
#[derive(Clone, Copy, Debug)]
pub(super) struct Turn {
    /// A point of the axis.
    pub origin: Vec3,
    /// The axis's unit direction, about which the turn is anticlockwise.
    pub axis: Vec3,
    /// The sine and cosine of the angle.
    pub sin_cos: (f64, f64),
}

impl Turn {
    /// The turn by `angle` radians about the axis through `origin` along the unit `axis`.
    pub(super) fn new(origin: Vec3, axis: Vec3, angle: f64) -> Self {
        Self { origin, axis, sin_cos: occam_math::sin_cos(angle) }
    }

    /// The direction `v` turned.
    pub(super) fn vector(self, v: Vec3) -> Vec3 {
        let (sin, cos) = self.sin_cos;
        let k = self.axis;
        v * cos + k.cross(v) * sin + k * (k.dot(v) * (1.0 - cos))
    }

    /// The point `p` turned.
    pub(super) fn point(self, p: Vec3) -> Vec3 {
        self.origin + self.vector(p - self.origin)
    }

    /// `bent` turned.
    pub(super) fn bent(self, bent: Bent) -> Bent {
        match bent {
            Bent::Line { origin, along } => Bent::Line { origin: self.point(origin), along: self.vector(along) },
            Bent::Circle { centre, x, y, radius } => {
                Bent::Circle { centre: self.point(centre), x: self.vector(x), y: self.vector(y), radius }
            }
        }
    }
}

/// One loop of a planar face: its edges in order, each with whether its use runs against it and the line or circle it
/// lies on, and a point in space strictly inside the loop.
pub(super) struct Flat {
    pub uses: Vec<(usize, bool, Bent)>,
    pub witness: Vec3,
}

/// Adds the face on `plane`, whose normal points out of the body, bounded by `loops`, the outer first, swept from
/// `from`.
///
/// # Errors
///
/// What a curve's family reports in the plane's parameters, which a line or circle in the plane does not.
pub(super) fn planar(shape: &mut Shape, plane: &Flatland, loops: Vec<Flat>, from: EntityRef) -> Result<(), Report> {
    let mut rings = Vec::new();
    for ring in loops {
        let mut uses = Vec::new();
        for (edge, reversed, bent) in ring.uses {
            uses.push(UseSpec { edge, reversed, pcurve: plane.pcurve(bent)?.0 });
        }
        let ([u, v], _) = plane.uv(ring.witness);
        rings.push(LoopSpec { uses, witness: UvPoint::new(u, v) });
    }
    shape.face(Arc::new(Plane::placed(&plane.placement)), rings, from);
    Ok(())
}
