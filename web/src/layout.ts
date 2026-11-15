// A face's vertex block as the GPU holds it: `f32` offsets from the page's `f64` origin, never absolute positions,
// so a part far from the world's origin draws as precisely as one at it (P12's accuracy table).
//
// A vertex is 32 bytes, two `vec4f`: the offset with `u` in its fourth lane, then the normal with `v`. The same bytes
// are a vertex buffer of two `float32x4` attributes and a storage array of `Vertex { offset_u: vec4f, normal_v:
// vec4f }`, which is what lets a compute pass write a block that the draw then reads, for write-through.

import type { FaceBlock, Vec3 } from "./mesh.ts";

/** Floats a vertex. */
export const FLOATS = 8;
/** Bytes a vertex. */
export const STRIDE = FLOATS * 4;

export const VERTEX_LAYOUT: GPUVertexBufferLayout = {
  arrayStride: STRIDE,
  attributes: [
    { shaderLocation: 0, offset: 0, format: "float32x4" },
    { shaderLocation: 1, offset: 16, format: "float32x4" },
  ],
};

/** The block's vertices, packed for the GPU against `origin`. */
export function pack(block: FaceBlock, origin: Vec3): Float32Array {
  const count = block.positions.length / 3;
  const packed = new Float32Array(count * FLOATS);
  for (let vertex = 0; vertex < count; vertex += 1) {
    const [p, out] = [vertex * 3, vertex * FLOATS];
    packed[out] = block.positions[p] - origin[0];
    packed[out + 1] = block.positions[p + 1] - origin[1];
    packed[out + 2] = block.positions[p + 2] - origin[2];
    packed[out + 3] = block.uvs[vertex * 2];
    packed[out + 4] = block.normals[p];
    packed[out + 5] = block.normals[p + 1];
    packed[out + 6] = block.normals[p + 2];
    packed[out + 7] = block.uvs[vertex * 2 + 1];
  }
  return packed;
}
