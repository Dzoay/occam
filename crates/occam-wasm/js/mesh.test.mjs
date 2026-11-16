// The browser build's mesh as a mesh worker meets it: a session meshes its model into flat typed arrays, each face a
// vertex block whose indices count from its start, with the summary `occam mesh --json` gives; after a run, only the
// faces it touched are meshed again; limits that are not positive are refused with the reports' JSON.
// Usage: node mesh.test.mjs <path to the wasm-bindgen output's .js file>
import assert from "node:assert/strict";
import { createRequire } from "node:module";

const occam = createRequire(import.meta.url)(process.argv[2]);
const request = (steps) => JSON.stringify({ schema: 1, units: { length: "mm", angle: "deg" }, steps });

const session = new occam.Session();
assert.equal(JSON.parse(session.run(request([{ op: "box", size: [10, 10, 10] }]))).ok, true);
const first = session.mesh(0.1, 0.35);
const summary = JSON.parse(first.summary());
assert.equal(summary.ok, true);
assert.equal(summary.answers.length, 6);
const [positions, normals, uvs, indices, faces] = [
  first.positions(),
  first.normals(),
  first.uvs(),
  first.indices(),
  first.faces(),
];
for (const floats of [positions, normals, uvs]) assert.ok(floats instanceof Float64Array);
for (const words of [indices, faces]) assert.ok(words instanceof Uint32Array);
assert.deepEqual([positions.length, normals.length, uvs.length], [3, 3, 2].map((n) => n * summary.vertices));
assert.equal(indices.length, 3 * summary.triangles);
assert.equal(faces.length, 6 * summary.answers.length, "six numbers a face");
summary.answers.forEach((answer, k) => {
  const [slot, generation, firstVertex, vertices, firstTriangle, triangles] = faces.subarray(6 * k, 6 * k + 6);
  assert.equal(answer.entity, `face:${slot}/${generation}`);
  assert.deepEqual(answer.mesh.vertices, { first: firstVertex, count: vertices });
  assert.deepEqual(answer.mesh.triangles, { first: firstTriangle, count: triangles });
  const own = indices.subarray(3 * firstTriangle, 3 * (firstTriangle + triangles));
  assert.ok(own.every((index) => index < vertices), "each index counts from its face's block");
});

session.run(request([{ op: "cylinder", radius: 4, height: 10, frame: { origin: [20, 0, 0] } }]));
const second = JSON.parse(session.mesh(0.1, 0.35).summary());
const kept = second.answers.map((answer) => answer.mesh.kept);
assert.deepEqual(kept, [true, true, true, true, true, true, false, false, false, false], "the box kept");

assert.throws(
  () => session.mesh(0, 0.35),
  (thrown) => JSON.parse(thrown).reports[0].code === "mesh.limit-not-positive",
);
console.log("wasm: a session meshes into flat arrays, a block a face, and after a run meshes only what it touched");
