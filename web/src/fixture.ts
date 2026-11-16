// A cube in the renderer contract's exact layout, and nothing else. The page draws it until `view.buffers` gives real
// per-face buffers from `occam mesh`; then the tests switch to that output and this file goes.

import type { Mesh, Vec3 } from "./mesh.ts";

/** Each face: its id, outward normal, and the directions of `u` and `v` on it, with `u × v` the normal. */
const FACES: readonly (readonly [string, Vec3, Vec3, Vec3])[] = [
  ["face:0/1", [0, 0, -1], [0, 1, 0], [1, 0, 0]],
  ["face:1/1", [0, 0, 1], [1, 0, 0], [0, 1, 0]],
  ["face:2/1", [0, -1, 0], [1, 0, 0], [0, 0, 1]],
  ["face:3/1", [1, 0, 0], [0, 1, 0], [0, 0, 1]],
  ["face:4/1", [0, 1, 0], [-1, 0, 0], [0, 0, 1]],
  ["face:5/1", [-1, 0, 0], [0, -1, 0], [0, 0, 1]],
];

/** The corners of a face in `(u, v)` steps, and its two triangles, counter-clockwise seen from outside. */
const CORNERS: readonly (readonly [number, number])[] = [[0, 0], [1, 0], [1, 1], [0, 1]];
const TRIANGLES = [0, 1, 2, 0, 2, 3];

/**
 * A cube of side `size` about `centre`: four vertices and two triangles a face, each face's `(u, v)` measured from
 * one corner in the model's units. The default sits far from the world's origin, where plain `f32` positions would be
 * off by about a thousandth, so drawing it relies on the page's `f64` origin.
 */
export function cube(centre: Vec3 = [100000.3, -50000.7, 25000.1], size = 20): Mesh {
  const half = size / 2;
  const positions = new Float64Array(FACES.length * 12);
  const normals = new Float64Array(FACES.length * 12);
  const uvs = new Float64Array(FACES.length * 8);
  const indices = new Uint32Array(FACES.length * 6);
  const faces = FACES.map(([id, normal, u, v], face) => {
    CORNERS.forEach(([a, b], corner) => {
      const vertex = face * 4 + corner;
      for (let axis = 0; axis < 3; axis += 1) {
        const along = (a - 0.5) * u[axis] + (b - 0.5) * v[axis];
        positions[vertex * 3 + axis] = centre[axis] + half * normal[axis] + size * along;
        normals[vertex * 3 + axis] = normal[axis];
      }
      uvs[vertex * 2] = a * size;
      uvs[vertex * 2 + 1] = b * size;
    });
    indices.set(TRIANGLES, face * 6);
    return { id, vertices: { first: face * 4, count: 4 }, triangles: { first: face * 2, count: 2 } };
  });
  return { positions, normals, uvs, indices, faces };
}
