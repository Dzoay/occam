// The mesh as the renderer contract wants it (docs/architecture/mesh-and-renderer.md#the-renderer-contract): flat
// arrays of `f64` positions, unit normals, each vertex's `(u, v)` on its face and `u32` indices, with a vertex block
// and a triangle range per face.

/** Three coordinates, in `f64`. */
export type Vec3 = readonly [number, number, number];

/** A run of a flat array: its first element and how many, counted in vertices or triangles. */
export interface Span {
  readonly first: number;
  readonly count: number;
}

/** Where one face's vertex block and triangles sit in the flat arrays. */
export interface FaceRange {
  /** The model's own id for the face, such as `face:0/1`. */
  readonly id: string;
  readonly vertices: Span;
  readonly triangles: Span;
}

export interface Mesh {
  /** Three a vertex, in the model's units. */
  readonly positions: Float64Array;
  /** Unit normals from the surface, three a vertex. */
  readonly normals: Float64Array;
  /** Each vertex's `(u, v)` on its face, two a vertex, kept for write-through. */
  readonly uvs: Float64Array;
  /**
   * Three a triangle. Each index counts from the start of its face's vertex block, so a block can be kept, moved or
   * dropped without rewriting any other face's indices.
   */
  readonly indices: Uint32Array;
  readonly faces: readonly FaceRange[];
}

/** One face's block, cut from a mesh into arrays the page owns. */
export interface FaceBlock {
  readonly id: string;
  readonly positions: Float64Array;
  readonly normals: Float64Array;
  readonly uvs: Float64Array;
  readonly indices: Uint32Array;
}

/** An axis-aligned box, in `f64`. */
export interface Bounds {
  readonly min: Vec3;
  readonly max: Vec3;
}

function spanFits(span: Span, size: number): boolean {
  return Number.isInteger(span.first) && Number.isInteger(span.count) && span.first >= 0 && span.count >= 0
    && span.first + span.count <= size;
}

/** What is wrong with a mesh's layout, or `null` if nothing is. */
export function malformed(mesh: Mesh): string | null {
  const vertices = mesh.positions.length / 3;
  if (!Number.isInteger(vertices) || mesh.normals.length !== mesh.positions.length || mesh.uvs.length !== vertices * 2) {
    return `the arrays disagree: ${mesh.positions.length} positions, ${mesh.normals.length} normals, ${mesh.uvs.length} uvs`;
  }
  if (mesh.indices.length % 3 !== 0) return `${mesh.indices.length} indices is not a whole number of triangles`;
  const triangles = mesh.indices.length / 3;
  const seen = new Set<string>();
  for (const face of mesh.faces) {
    if (seen.has(face.id)) return `${face.id} appears twice`;
    seen.add(face.id);
    if (!spanFits(face.vertices, vertices) || !spanFits(face.triangles, triangles)) {
      return `${face.id}'s ranges leave the arrays`;
    }
    const indices = mesh.indices.subarray(face.triangles.first * 3, (face.triangles.first + face.triangles.count) * 3);
    if (indices.some((index) => index >= face.vertices.count)) return `${face.id} has an index outside its block`;
  }
  return null;
}

/** Each face's block, copied out of the flat arrays. The mesh must not be malformed. */
export function blocks(mesh: Mesh): FaceBlock[] {
  return mesh.faces.map((face) => {
    const [v0, v1] = [face.vertices.first, face.vertices.first + face.vertices.count];
    const [t0, t1] = [face.triangles.first * 3, (face.triangles.first + face.triangles.count) * 3];
    return {
      id: face.id,
      positions: mesh.positions.slice(v0 * 3, v1 * 3),
      normals: mesh.normals.slice(v0 * 3, v1 * 3),
      uvs: mesh.uvs.slice(v0 * 2, v1 * 2),
      indices: mesh.indices.slice(t0, t1),
    };
  });
}

function sameWords(a: Float64Array | Uint32Array, b: Float64Array | Uint32Array): boolean {
  if (a.byteLength !== b.byteLength) return false;
  const x = new Uint32Array(a.buffer, a.byteOffset, a.byteLength / 4);
  const y = new Uint32Array(b.buffer, b.byteOffset, b.byteLength / 4);
  return x.every((word, i) => word === y[i]);
}

/** Whether two blocks hold the same bits, so the one on the GPU can stay. */
export function sameBits(a: FaceBlock, b: FaceBlock): boolean {
  return sameWords(a.positions, b.positions) && sameWords(a.normals, b.normals) && sameWords(a.uvs, b.uvs)
    && sameWords(a.indices, b.indices);
}

/** The box around some positions, or `null` if there are none. */
export function bounds(positions: Float64Array): Bounds | null {
  if (positions.length < 3) return null;
  const min = [Infinity, Infinity, Infinity] as [number, number, number];
  const max = [-Infinity, -Infinity, -Infinity] as [number, number, number];
  for (let i = 0; i < positions.length; i += 3) {
    for (let axis = 0; axis < 3; axis += 1) {
      min[axis] = Math.min(min[axis], positions[i + axis]);
      max[axis] = Math.max(max[axis], positions[i + axis]);
    }
  }
  return { min, max };
}

/** The box around both, either of which may be absent. */
export function union(a: Bounds | null, b: Bounds | null): Bounds | null {
  if (!a || !b) return a ?? b;
  const pick = (f: (x: number, y: number) => number, x: Vec3, y: Vec3): Vec3 => [
    f(x[0], y[0]),
    f(x[1], y[1]),
    f(x[2], y[2]),
  ];
  return { min: pick(Math.min, a.min, b.min), max: pick(Math.max, a.max, b.max) };
}

export function centre(box: Bounds): Vec3 {
  return [(box.min[0] + box.max[0]) / 2, (box.min[1] + box.max[1]) / 2, (box.min[2] + box.max[2]) / 2];
}

/** Half the box's diagonal: the radius of a sphere about its centre that holds it. */
export function radius(box: Bounds): number {
  return Math.hypot(box.max[0] - box.min[0], box.max[1] - box.min[1], box.max[2] - box.min[2]) / 2;
}
