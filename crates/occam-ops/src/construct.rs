//! Source: original
//! Construction from points and curves (PARITY 1.6): wires through points, lines, arcs and circles, and planar faces
//! bounded by closed wires, with holes. What a request gives is read in [`given`]; the wire and the face are made in
//! [`wire`] and [`face`], checked first in the plane's parameters ([`flat`]) and in space ([`space`], [`outline`]), and
//! written into the edit by [`build`].

mod build;
mod face;
mod flat;
pub(crate) mod given;
mod outline;
mod space;
mod wire;

pub(crate) use face::face;
pub(crate) use wire::wire;

#[cfg(test)]
mod tests;
