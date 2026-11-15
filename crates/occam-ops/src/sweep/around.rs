//! Source: original
//! What one piece of a profile sweeps as it turns about an axis in its plane, and on which surface. Seen in that plane,
//! as a radius from the axis and a height along it, a line on the axis sweeps nothing, a line square to it a plane, a
//! line along it a cylinder and a slanting line a cone; an arc about a point of the axis sweeps a sphere, an arc about a
//! point further from the axis than its radius a torus, and any other arc a surface of revolution of it, which no
//! analytic family fits. Each patch's surface is placed so that the patch lies in the middle of its turn, `u` = π, and
//! the piece's parameter `t` is `v = v₀ + v₁ t` on it, so no face reaches the seam of its surface's turn. A torus also
//! comes round in `v`, from its outer equator, so a piece is cut where it crosses that; a surface of revolution turns
//! the piece's own arc, trimmed, so its `v` does not come round.

use core::f64::consts::{PI, TAU};
use std::sync::Arc;

use occam_geom::{Circle, Cone, Cylinder, Revolution, Sphere, Surface, Torus};
use occam_math::atan2;
use occam_report::Report;
use occam_units::{Angle, Direction, Length, Local, Placement, Point, Vec3, World};

use super::profile::Piece;
use super::shape::Turn;
use crate::construct::space::{Bent, frame};

/// An axis, and the direction across it to the side of it the profile is on.
#[derive(Clone, Copy, Debug)]
pub(super) struct Axes {
    /// A point of the axis.
    pub origin: Vec3,
    /// Its unit direction, about which the profile turns anticlockwise.
    pub axis: Vec3,
    /// The unit direction square to it, in the profile's plane, towards the profile.
    pub x: Vec3,
}

impl Axes {
    /// The radius from the axis and height along it of `p`, which lies in the plane of the axis and `x`.
    pub(super) fn local(self, p: Vec3) -> (f64, f64) {
        let w = p - self.origin;
        (w.dot(self.x), w.dot(self.axis))
    }

    /// The turn by `angle` about the axis.
    pub(super) fn turn(self, angle: f64) -> Turn {
        Turn::new(self.origin, self.axis, angle)
    }

    /// The placement at height `height` on the axis, its `z` the axis times `up`, 1 or −1, and its `x` `across`.
    fn placed(self, height: f64, up: f64, across: Vec3) -> Placement<Local, World> {
        frame(self.origin + self.axis * height, self.axis * up, Some(across))
    }
}

/// What a piece sweeps, in the radius and height of the profile's plane.
#[derive(Clone, Copy, Debug)]
pub(super) enum Family {
    /// A line on the axis: nothing.
    Axis,
    /// A line square to the axis: a plane.
    Flat,
    /// A line along the axis at `radius`: a cylinder.
    Tube { radius: f64 },
    /// A slanting line: a cone through the circle of `radius` at `height`, the piece's start, opening at `half_angle`
    /// along the axis times `up`, 1 or −1.
    Cone { radius: f64, height: f64, half_angle: f64, up: f64 },
    /// An arc about the point of the axis at `height`: a sphere.
    Ball { height: f64, radius: f64 },
    /// An arc about the point `major` from the axis at `height`, further than its radius `minor`: a torus.
    Ring { major: f64, height: f64, minor: f64 },
    /// Any other arc, on the circle `circle`, which a surface of revolution turns.
    Turned { circle: Bent },
}

/// A piece's family, and its parameter on that family's surfaces as `v = v.0 + v.1 t`.
#[derive(Clone, Copy, Debug)]
pub(super) struct Meridian {
    pub family: Family,
    pub v: (f64, f64),
}

/// The meridian of `piece` about `axes`, its ends further from the axis than `band` unless they are on it.
pub(super) fn meridian(piece: &Piece, axes: Axes, band: f64) -> Meridian {
    let (lo, hi) = piece.range;
    let ((r0, h0), (r1, h1)) = (axes.local(piece.bent.point(lo)), axes.local(piece.bent.point(hi)));
    match piece.bent {
        Bent::Line { origin, along } => {
            let (start, rate) = (axes.local(origin).1, along.dot(axes.axis));
            if r0.abs() <= band && r1.abs() <= band {
                Meridian { family: Family::Axis, v: (0.0, 1.0) }
            } else if (h1 - h0).abs() <= band {
                Meridian { family: Family::Flat, v: (0.0, 1.0) }
            } else if (r1 - r0).abs() <= band {
                Meridian { family: Family::Tube { radius: f64::midpoint(r0, r1) }, v: (start, rate) }
            } else {
                // The cone's own `v` runs along the line from the piece's start, `up` choosing the nappe it runs on.
                let up = ((r1 - r0) * (h1 - h0)).signum();
                let (spread, rise) = ((r1 - r0).abs(), (h1 - h0).abs());
                let over = up * rise / (spread * spread + rise * rise).sqrt();
                let cone = Family::Cone { radius: r0.abs(), height: h0, half_angle: atan2(spread, rise), up };
                Meridian { family: cone, v: ((start - h0) / over, rate / over) }
            }
        }
        Bent::Circle { centre, x, y, radius } => {
            // The angle about the circle's centre, out from the axis towards the axis's direction, at the piece's middle,
            // and which way the piece's parameter turns it.
            let (rc, hc) = axes.local(centre);
            let middle = f64::midpoint(lo, hi);
            let (r, h) = axes.local(piece.bent.point(middle));
            let sigma = x.cross(y).dot(axes.x.cross(axes.axis)).signum();
            if rc.abs() <= band {
                let latitude = atan2(h - hc, r);
                Meridian { family: Family::Ball { height: hc, radius }, v: (latitude - sigma * middle, sigma) }
            } else if Torus::placed(&axes.placed(hc, 1.0, axes.x), mm(rc), mm(radius)).is_ok() {
                let round = PI + atan2(hc - h, rc - r);
                Meridian {
                    family: Family::Ring { major: rc, height: hc, minor: radius },
                    v: (round - sigma * middle, sigma),
                }
            } else {
                Meridian { family: Family::Turned { circle: piece.bent }, v: (0.0, 1.0) }
            }
        }
    }
}

/// A length in millimetres.
const fn mm(value: f64) -> Length {
    Length::from_millimetres(value)
}

/// Where a piece on a torus crosses the torus's outer equator, its `v` a whole number of turns: the parameters, inside
/// its range, it is cut at.
pub(super) fn seams(piece: &Piece, axes: Axes, band: f64) -> Vec<f64> {
    let meridian = meridian(piece, axes, band);
    let (Family::Ring { .. }, (v0, v1)) = (meridian.family, meridian.v) else { return Vec::new() };
    let (lo, hi) = piece.range;
    // Its middle is in the turn from 0 to 2π, and it covers at most a whole turn, so it can only cross 0 or 2π.
    [0.0, TAU].map(|v| (v - v0) / v1).into_iter().filter(|&t| t > lo && t < hi).collect()
}

impl Meridian {
    /// Which way `u` runs on its surfaces as the profile turns: 1 with the turn, −1 against it.
    pub(super) const fn turns(self) -> f64 {
        match self.family {
            Family::Cone { up, .. } => up,
            _ => 1.0,
        }
    }

    /// Its parameter on its surfaces at the piece's parameter `t`.
    pub(super) fn v(self, t: f64) -> f64 {
        self.v.0 + self.v.1 * t
    }

    /// The surface of the patch whose middle is at `angle` about the axis, placed so that the middle is at `u` = π, or
    /// none for a plane or a piece on the axis. The piece covers `range` of its curve.
    ///
    /// # Errors
    ///
    /// What a family reports, which no piece of a profile on one side of the axis gives.
    pub(super) fn surface(self, axes: Axes, angle: f64, range: (f64, f64)) -> Result<Option<Arc<dyn Surface>>, Report> {
        // The direction half a turn from the patch's middle, where its surface's `u` is 0.
        let back = axes.turn(angle).vector(axes.x) * -1.0;
        let surface: Arc<dyn Surface> = match self.family {
            Family::Axis | Family::Flat => return Ok(None),
            Family::Tube { radius } => Arc::new(Cylinder::placed(&axes.placed(0.0, 1.0, back), mm(radius))?),
            Family::Cone { radius, height, half_angle, up } => {
                let placement = axes.placed(height, up, back);
                Arc::new(Cone::placed(&placement, mm(radius), Angle::from_radians(half_angle))?)
            }
            Family::Ball { height, radius } => Arc::new(Sphere::placed(&axes.placed(height, 1.0, back), mm(radius))?),
            Family::Ring { major, height, minor } => {
                Arc::new(Torus::placed(&axes.placed(height, 1.0, back), mm(major), mm(minor))?)
            }
            Family::Turned { circle } => {
                let Bent::Circle { centre, x, y, radius } = axes.turn(PI).bent(axes.turn(angle).bent(circle)) else {
                    return Ok(None);
                };
                let whole = Circle::placed(&frame(centre, x.cross(y), Some(x)), mm(radius))?;
                let arc = whole.trimmed(range.0, range.1)?;
                Arc::new(Revolution::new(Point::from_vec3(axes.origin), Direction::normalised(axes.axis), arc))
            }
        };
        Ok(Some(surface))
    }
}
