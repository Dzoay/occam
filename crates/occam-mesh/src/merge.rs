//! Source: original
//! The faces' meshes merged by index into one mesh, each face's vertices a block of its own (P12's layout), and every
//! output checked watertight by [`watertight`]. Nothing is welded: along an edge, each face's block holds the edge's
//! samples bit for bit, so the faces either side of it hold the same positions there, and the check matches them by
//! position.

use std::ops::Range;

use occam_math::{Digest, Exec};
use occam_report::{DisplayUnits, EntityRef, Fault, Reason, Report};
use occam_topo::{Face, Id, Snapshot};
use occam_units::{Direction, Point, UvPoint, Vec3, World};

use crate::boundary::not_meshed;
use crate::edges::{Edges, sample_edges};
use crate::faces::{FaceMesh, Faces, Side, mesh_faces};
use crate::limits::Limits;
use crate::watertight::watertight;

/// One face's place in a [`Mesh`]: its block of vertices, its triangles, which use only those, and its uses of edges.
#[derive(Clone, Debug, PartialEq, Eq)]
pub struct Block {
    /// The face.
    pub face: Id<Face>,
    /// Its vertices, in the mesh's arrays.
    pub vertices: Range<u32>,
    /// Its triangles, in the mesh's list.
    pub triangles: Range<u32>,
    /// Each use of an edge, as in its [`FaceMesh`], its vertices numbered in the mesh's arrays.
    pub sides: Vec<Side>,
    /// Whether refinement stopped at [`Limits::TRIANGLES_PER_FACE`], short of the deflection.
    pub capped: bool,
}

/// A model's mesh: every face's mesh merged by index, each face's vertices a block of its own in id order, and what
/// the watertight check found. A triangle's corners are numbered in the mesh's arrays, and lie in its face's block.
#[derive(Clone, Debug, PartialEq)]
pub struct Mesh {
    /// Every edge's samples, which the faces were meshed against.
    pub edges: Edges,
    /// Each vertex's position: along an edge, that edge's sample, bit for bit.
    pub points: Vec<Point<World>>,
    /// Each vertex's unit normal, out of its face.
    pub normals: Vec<Direction<World>>,
    /// Each vertex's place in its face's parameters.
    pub uvs: Vec<UvPoint>,
    /// The triangles, three vertices each, facing out of their faces.
    pub triangles: Vec<[u32; 3]>,
    /// Each face that meshed, in id order.
    pub blocks: Vec<Block>,
    /// The reports refusing faces, in id order: none if every face meshed.
    pub refused: Vec<Report>,
    /// What the watertight check found: none where the mesh is watertight.
    pub open: Vec<Report>,
}

/// Meshes `model` to `limits` on `exec`: samples every edge its faces use once, meshes each face against those
/// samples, merges the faces by index and checks the result watertight; the same bits at any thread count.
#[must_use]
pub fn mesh(exec: &Exec, model: &Snapshot, limits: Limits) -> Mesh {
    let edges = sample_edges(exec, model, limits);
    let faces = mesh_faces(exec, model, &edges);
    let mut merged = merge(edges, &faces);
    merged.open = watertight(model, &merged);
    merged
}

/// `faces`, meshed against `edges`, merged by index into one [`Mesh`], each face's vertices a block of its own in id
/// order, its triangles' corners and its uses' vertices numbered in the mesh's arrays. Nothing is checked yet: [`mesh`]
/// runs [`watertight`] on what this gives. A face that would number a vertex or a triangle past `u32::MAX` is refused
/// with [`occam_report::Code::FaceNotMeshed`], as [`Reason::BudgetSpent`].
#[must_use]
pub fn merge(edges: Edges, faces: &Faces) -> Mesh {
    let mut merged = Mesh {
        edges,
        points: Vec::new(),
        normals: Vec::new(),
        uvs: Vec::new(),
        triangles: Vec::new(),
        blocks: Vec::new(),
        refused: Vec::new(),
        open: Vec::new(),
    };
    for (face, meshed) in faces.iter() {
        match meshed {
            Ok(own) => {
                if !merged.add(face, own) {
                    merged.refused.push(too_many(face));
                }
            }
            Err(refused) => merged.refused.push(refused.clone()),
        }
    }
    merged
}

/// `local`, a vertex's number in its face's own mesh, numbered in the mesh's arrays, where its block starts at `first`.
pub(crate) fn offset(first: u32, local: u32) -> Option<u32> {
    first.checked_add(local)
}

/// The range of `count` items from `first`, if its end is at most `u32::MAX`.
pub(crate) fn numbered(first: u32, count: usize) -> Option<Range<u32>> {
    let end = first.checked_add(u32::try_from(count).ok()?)?;
    Some(first..end)
}

/// `range` as indices into a list.
pub(crate) fn span(range: &Range<u32>) -> Option<Range<usize>> {
    Some(usize::try_from(range.start).ok()?..usize::try_from(range.end).ok()?)
}

/// `triangles`, whose corners are numbered in the mesh's arrays, numbered from `first`, the first vertex of their
/// block; `None` if a corner lies before it.
pub(crate) fn local(first: u32, triangles: &[[u32; 3]]) -> Option<Vec<[u32; 3]>> {
    let back = |t: &[u32; 3]| {
        let [a, b, c] = t.map(|k| k.checked_sub(first));
        Some([a?, b?, c?])
    };
    triangles.iter().map(back).collect()
}

/// The report refusing `face`, which would number a vertex or a triangle past `u32::MAX`.
pub(crate) fn too_many(face: Id<Face>) -> Report {
    not_meshed(face, Reason::BudgetSpent, Fault::Internal, &[])
}

/// The block of `face`, whose own mesh is `own`, placed after `vertices` vertices and `triangles` triangles, its uses'
/// vertices numbered in the mesh's arrays; `None` past `u32::MAX`.
fn placed(face: Id<Face>, own: &FaceMesh, (vertices, triangles): (usize, usize)) -> Option<Block> {
    let vertices = numbered(u32::try_from(vertices).ok()?, own.points.len())?;
    let triangles = numbered(u32::try_from(triangles).ok()?, own.triangles.len())?;
    let side = |side: &Side| {
        let numbered = side.vertices.iter().map(|&k| offset(vertices.start, k)).collect::<Option<Vec<u32>>>()?;
        Some(Side { edge: side.edge, reversed: side.reversed, vertices: numbered })
    };
    let sides = own.sides.iter().map(side).collect::<Option<Vec<Side>>>()?;
    Some(Block { face, vertices, triangles, sides, capped: own.capped })
}

impl Mesh {
    /// Adds `own`, the mesh of `face`, as a block after the last; `false`, adding nothing, where a vertex or a triangle
    /// would be numbered past `u32::MAX`.
    fn add(&mut self, face: Id<Face>, own: &FaceMesh) -> bool {
        let Some(block) = placed(face, own, (self.points.len(), self.triangles.len())) else { return false };
        let first = block.vertices.start;
        let corners = |t: &[u32; 3]| {
            let [a, b, c] = t.map(|k| offset(first, k));
            Some([a?, b?, c?])
        };
        let Some(triangles) = own.triangles.iter().map(corners).collect::<Option<Vec<[u32; 3]>>>() else {
            return false;
        };
        self.points.extend_from_slice(&own.points);
        self.normals.extend_from_slice(&own.normals);
        self.uvs.extend_from_slice(&own.uvs);
        self.triangles.extend(triangles);
        self.blocks.push(block);
        true
    }

    /// The block of `face`, if it meshed.
    #[must_use]
    pub fn block(&self, face: Id<Face>) -> Option<&Block> {
        let at = self.blocks.binary_search_by_key(&face, |block| block.face).ok()?;
        self.blocks.get(at)
    }

    /// The mesh of `face` as its block holds it, numbered from the block's first vertex as a [`FaceMesh`] numbers its
    /// own, or the report refusing it: what [`crate::remesh`] keeps of a face it does not mesh again. `None` for a face
    /// this mesh does not hold.
    pub(crate) fn own(&self, face: Id<Face>) -> Option<Result<FaceMesh, Report>> {
        let Some(block) = self.block(face) else {
            return self.refused.iter().find(|r| r.entities.first() == Some(&face.entity())).cloned().map(Err);
        };
        let (vertices, first) = (span(&block.vertices)?, block.vertices.start);
        let triangles = local(first, self.triangles.get(span(&block.triangles)?)?)?;
        let side = |side: &Side| {
            let vertices = side.vertices.iter().map(|&k| k.checked_sub(first)).collect::<Option<Vec<u32>>>()?;
            Some(Side { edge: side.edge, reversed: side.reversed, vertices })
        };
        let sides = block.sides.iter().map(side).collect::<Option<Vec<Side>>>()?;
        Some(Ok(FaceMesh {
            points: self.points.get(vertices.clone())?.to_vec(),
            normals: self.normals.get(vertices.clone())?.to_vec(),
            uvs: self.uvs.get(vertices)?.to_vec(),
            triangles,
            sides,
            capped: block.capped,
        }))
    }

    /// The mesh's hash, which `gates determinism` holds to its record: every edge's samples or the report refusing it,
    /// every vertex's position, normal and place, every triangle, every block, and every report as its JSON, with NaNs
    /// made canonical. Equal meshes give equal hashes.
    #[must_use]
    pub fn digest(&self) -> u64 {
        let mut digest = Digest::EMPTY;
        samples(&mut digest, &self.edges);
        length(&mut digest, self.points.len());
        self.points.iter().for_each(|p| triple(&mut digest, p.to_vec3()));
        length(&mut digest, self.normals.len());
        self.normals.iter().for_each(|n| triple(&mut digest, n.get()));
        length(&mut digest, self.uvs.len());
        for uv in &self.uvs {
            digest.write_value(uv.u());
            digest.write_value(uv.v());
        }
        length(&mut digest, self.triangles.len());
        self.triangles.iter().flatten().for_each(|&k| digest.write_u64(u64::from(k)));
        length(&mut digest, self.blocks.len());
        self.blocks.iter().for_each(|block| placement(&mut digest, block));
        reports(&mut digest, self.refused.iter());
        reports(&mut digest, self.open.iter());
        digest.finish()
    }
}

/// Writes how many.
fn length(digest: &mut Digest, n: usize) {
    digest.write_u64(u64::try_from(n).unwrap_or(u64::MAX));
}

/// Writes three coordinates.
fn triple(digest: &mut Digest, v: Vec3) {
    for x in [v.x, v.y, v.z] {
        digest.write_value(x);
    }
}

/// Writes how many bytes, then the bytes.
fn text(digest: &mut Digest, bytes: &[u8]) {
    length(digest, bytes.len());
    digest.write_bytes(bytes);
}

/// Writes an entity, as its text.
fn entity(digest: &mut Digest, entity: EntityRef) {
    text(digest, entity.text().as_bytes());
}

/// Writes how many reports, then each as its JSON, or its code if it cannot be written.
fn reports<'a>(digest: &mut Digest, reports: impl ExactSizeIterator<Item = &'a Report>) {
    length(digest, reports.len());
    for report in reports {
        match report.to_json(DisplayUnits::default()) {
            Ok(json) => text(digest, json.as_bytes()),
            Err(failed) => text(digest, failed.code.text().as_bytes()),
        }
    }
}

/// Writes every edge with its samples, or the report refusing it.
fn samples(digest: &mut Digest, edges: &Edges) {
    length(digest, edges.iter().count());
    for (edge, sampled) in edges.iter() {
        entity(digest, edge.entity());
        match sampled {
            Ok(sampled) => {
                digest.write_u64(0);
                length(digest, sampled.params.len());
                sampled.params.iter().for_each(|&t| digest.write_value(t));
                sampled.points.iter().for_each(|p| triple(digest, p.to_vec3()));
            }
            Err(refused) => {
                digest.write_u64(1);
                reports(digest, [refused].into_iter());
            }
        }
    }
}

/// Writes a block: its face, its ranges, its uses and whether it was capped.
fn placement(digest: &mut Digest, block: &Block) {
    entity(digest, block.face.entity());
    let ranges = [&block.vertices, &block.triangles];
    ranges.into_iter().flat_map(|r| [r.start, r.end]).for_each(|n| digest.write_u64(u64::from(n)));
    length(digest, block.sides.len());
    for side in &block.sides {
        entity(digest, side.edge.entity());
        digest.write_u64(u64::from(side.reversed));
        length(digest, side.vertices.len());
        side.vertices.iter().for_each(|&k| digest.write_u64(u64::from(k)));
    }
    digest.write_u64(u64::from(block.capped));
}

#[cfg(test)]
mod tests;
