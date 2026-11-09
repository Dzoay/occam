// The browser build as a script meets it: a refusal is JSON, and following its remedy succeeds.
// Usage: node distance.test.mjs <path to the wasm-bindgen output's .js file>
import assert from "node:assert/strict";
import { createRequire } from "node:module";

const occam = createRequire(import.meta.url)(process.argv[2]);

const asking = (tolerance) => JSON.parse(occam.distance(0, 0, 0, 3, 4, 0, 0.001, 0.00001, tolerance));

const refused = asking(0.0001);
assert.equal(refused.schema, 1);
assert.equal(refused.code, "measure.tolerance-below-model");
assert.equal(refused.severity, "error");
assert.equal(refused.message, "The tolerance asked for, 0.0001 mm, is finer than the model's tolerance, 0.001 mm.");

const remedy = refused.remedies.find((r) => r.kind === "use-model-tolerance");
const tolerance = remedy.values.find((v) => v.name === "tolerance").value;
assert.equal(tolerance, 0.001);

const followed = asking(tolerance);
assert.equal(followed.schema, 1);
assert.equal(followed.value, 5);
assert.equal(followed.diagnostics[0].code, "measure.tolerance-used");
console.log("wasm: a refusal is JSON and following its remedy succeeds");
