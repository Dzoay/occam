//! Source: original
//! A mesh as the renderer contract wants it (docs/architecture/mesh-and-renderer.md): flat arrays of `f64` positions,
//! unit normals and each vertex's `(u, v)` on its face, kept for write-through, and `u32` indices, three to a triangle;
//! and for each face that meshed, in id order, its vertex block and its range of triangles. Each index counts from the
//! start of its face's vertex block, so a block is kept, moved or dropped without rewriting the others, and a face is
//! drawn with its block's first vertex as the base.
//!
//! [`Buffers::to_bytes`] writes them as they are, which is what `occam mesh` puts in its file: the counts of vertices,
//! triangles and faces as three `u64`, then the positions, the normals and the places as `f64`, then the indices and
//! the faces' table as `u32`, all little-endian. Each face is six numbers in the table: its slot and generation, its
//! first vertex and how many, and its first triangle and how many. The `f64` arrays start on a multiple of eight bytes.

use std::ops::Range;

use occam_topo::{Face, Id};
use occam_units::Vec3;

use crate::merge::{Block, Mesh, local, span};

/// One face's place in [`Buffers`]: its vertex block, and its triangles, whose indices count from the block's start.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct FaceRange {
    /// The face.
    pub face: Id<Face>,
    /// Its vertices.
    pub vertices: Range<u32>,
    /// Its triangles.
    pub triangles: Range<u32>,
}

/// A mesh as flat arrays, each face a vertex block and a range of triangles.
#[derive(Clone, Debug, PartialEq)]
pub struct Buffers {
    /// Each vertex's position, [`Buffers::POSITION`] numbers each, in millimetres.
    pub positions: Vec<f64>,
    /// Each vertex's unit normal, out of its face, [`Buffers::NORMAL`] numbers each.
    pub normals: Vec<f64>,
    /// Each vertex's place in its face's parameters, [`Buffers::UV`] numbers each.
    pub uvs: Vec<f64>,
    /// Each triangle's corners, [`Buffers::TRIANGLE`] each, counted from the start of its face's vertex block.
    pub indices: Vec<u32>,
    /// Each face that meshed, in id order.
    pub faces: Vec<FaceRange>,
}

impl Buffers {
    /// How many numbers a position takes.
    pub const POSITION: usize = 3;
    /// How many numbers a normal takes.
    pub const NORMAL: usize = 3;
    /// How many numbers a place in a face's parameters takes.
    pub const UV: usize = 2;
    /// How many indices a triangle takes.
    pub const TRIANGLE: usize = 3;

    /// `mesh` as flat arrays: its vertices in its order, and each block's triangles, their corners counted from the
    /// block's first vertex.
    #[must_use]
    pub fn of(mesh: &Mesh) -> Self {
        let triple = |v: Vec3| [v.x, v.y, v.z];
        let mut indices = Vec::with_capacity(mesh.triangles.len().saturating_mul(Self::TRIANGLE));
        for block in &mesh.blocks {
            let corners = span(&block.triangles).and_then(|t| mesh.triangles.get(t));
            let own = corners.and_then(|corners| local(block.vertices.start, corners)).unwrap_or_default();
            indices.extend(own.into_iter().flatten());
        }
        let range = |block: &Block| FaceRange {
            face: block.face,
            vertices: block.vertices.clone(),
            triangles: block.triangles.clone(),
        };
        Self {
            positions: mesh.points.iter().flat_map(|p| triple(p.to_vec3())).collect(),
            normals: mesh.normals.iter().flat_map(|n| triple(n.get())).collect(),
            uvs: mesh.uvs.iter().flat_map(|uv| [uv.u(), uv.v()]).collect(),
            indices,
            faces: mesh.blocks.iter().map(range).collect(),
        }
    }

    /// How many vertices.
    #[must_use]
    pub const fn vertices(&self) -> usize {
        self.positions.len() / Self::POSITION
    }

    /// How many triangles.
    #[must_use]
    pub const fn triangles(&self) -> usize {
        self.indices.len() / Self::TRIANGLE
    }

    /// The range of `face`, if it meshed.
    #[must_use]
    pub fn face(&self, face: Id<Face>) -> Option<&FaceRange> {
        let at = self.faces.binary_search_by_key(&face, |range| range.face).ok()?;
        self.faces.get(at)
    }

    /// The faces' table: six numbers a face, in id order, its slot and generation, its first vertex and how many, and
    /// its first triangle and how many.
    #[must_use]
    pub fn table(&self) -> Vec<u32> {
        let length = |r: &Range<u32>| r.end.saturating_sub(r.start);
        let row = |range: &FaceRange| {
            let (id, vertices, triangles) = (range.face.entity(), &range.vertices, &range.triangles);
            [id.slot, id.generation, vertices.start, length(vertices), triangles.start, length(triangles)]
        };
        self.faces.iter().flat_map(row).collect()
    }

    /// The buffers as `occam mesh` writes them: the counts of vertices, triangles and faces as `u64`, then the
    /// positions, normals and places as `f64`, then the indices and the faces' table as `u32`, all little-endian. Each
    /// face in the table is its slot and generation, its first vertex and how many, and its first triangle and how
    /// many.
    #[must_use]
    pub fn to_bytes(&self) -> Vec<u8> {
        let count = |n: usize| u64::try_from(n).unwrap_or(u64::MAX).to_le_bytes();
        let counts = [self.vertices(), self.triangles(), self.faces.len()];
        let mut bytes: Vec<u8> = counts.into_iter().flat_map(count).collect();
        let reals = self.positions.iter().chain(&self.normals).chain(&self.uvs);
        bytes.extend(reals.flat_map(|x| x.to_le_bytes()));
        bytes.extend(self.indices.iter().flat_map(|k| k.to_le_bytes()));
        bytes.extend(self.table().iter().flat_map(|n| n.to_le_bytes()));
        bytes
    }
}

#[cfg(test)]
mod tests;
