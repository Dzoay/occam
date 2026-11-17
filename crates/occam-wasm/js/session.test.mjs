// The browser build's session as the editing worker meets it: a request runs and its JSON says what it made, the
// changed model comes back as the binary twin, which opens in another session and in a mesh worker, and bytes that are
// not a model are refused by both with the reports' JSON.
// Usage: node session.test.mjs <path to the wasm-bindgen output's .js file>
import assert from "node:assert/strict";
import { createRequire } from "node:module";

const occam = createRequire(import.meta.url)(process.argv[2]);
const request = (steps) => JSON.stringify({ schema: 1, units: { length: "mm", angle: "deg" }, steps });

const editing = new occam.Session();
const ran = JSON.parse(editing.run(request([{ op: "box", size: [10, 20, 30] }])));
assert.equal(ran.ok, true);
assert.equal(ran.steps[0].made, "body:0/1");
const twin = editing.take_saved();
assert.ok(twin instanceof Uint8Array);
assert.deepEqual([...twin.subarray(1, 6)].map((b) => String.fromCharCode(b)).join(""), "occam");
assert.equal(editing.take_saved(), undefined, "given once");

const opened = occam.Session.open(twin);
const measured = JSON.parse(opened.run(request([{ op: "measure", target: "body:0/1" }])));
assert.equal(measured.ok, true);
assert.ok(Math.abs(measured.steps[0].measured.values[0].value - 6000) < 1e-6);

// A mesh worker opens the twin without checking it again, and meshes it.
const worker = occam.Trusted.open(twin);
assert.equal(JSON.parse(worker.mesh_part(0.1, 0.35, 0, 6).summary()).answers.length, 6);

const garbage = new TextEncoder().encode("not a model");
for (const open of [(bytes) => occam.Session.open(bytes), (bytes) => occam.Trusted.open(bytes)]) {
  assert.throws(() => open(garbage), (thrown) => {
    const refused = JSON.parse(thrown);
    return refused.ok === false && refused.reports[0].code === "io.not-readable";
  });
}
console.log("wasm: a session runs a request, gives the binary twin, which opens again, and a mesh worker opens it");
