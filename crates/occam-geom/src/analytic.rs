//! Source: original
//! [`Analytic`]: the exact forms, which the closed-form intersections are written in (P5). Any family that holds one
//! presents it, so a B-spline that is exactly a cylinder can say so. This is the one vocabulary of names the seam has, by
//! design: it lists forms, not families (S4).

use occam_units::{Angle, Direction, Length, Point, World};

/// An exact form, unbounded: a patch presents its carrier, and clipping belongs to the caller (S4).
#[derive(Clone, Copy, Debug, PartialEq)]
pub enum Analytic {
    /// The plane through `origin` facing `normal`.
    Plane {
        /// A point on it.
        origin: Point<World>,
        /// The side it faces.
        normal: Direction<World>,
    },
    /// The cylinder of `radius` about the axis through `on_axis`.
    Cylinder {
        /// A point on the axis.
        on_axis: Point<World>,
        /// The axis.
        axis: Direction<World>,
        /// The radius.
        radius: Length,
    },
    /// The cone with its apex at `apex`, opening along `axis` at `half_angle`.
    Cone {
        /// The apex.
        apex: Point<World>,
        /// The way it opens.
        axis: Direction<World>,
        /// The angle between the axis and the surface.
        half_angle: Angle,
    },
    /// The sphere of `radius` about `centre`.
    Sphere {
        /// The centre.
        centre: Point<World>,
        /// The radius.
        radius: Length,
    },
    /// The torus about `axis` through `centre`.
    Torus {
        /// The centre.
        centre: Point<World>,
        /// The axis it turns about.
        axis: Direction<World>,
        /// The distance from the axis to the tube's centre.
        major: Length,
        /// The tube's radius.
        minor: Length,
    },
}
