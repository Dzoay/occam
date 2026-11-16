// The viewer in headless Chromium on its software adapter (SwiftShader). CI has no GPU, so these assert buffers, not
// pixels: what the page uploaded, read back, and the camera it drew with. Every model is a request run in the page's
// editing worker and meshed by its mesh workers, and the meshes they make are held to the native `occam mesh`'s.
//
// Run by `just web-test`, which builds the page and `occam` first.
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { createHash } from "node:crypto";
import { mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { after, before, test } from "node:test";
import { chromium } from "playwright";

import { serve } from "./serve.mjs";

/** The flags that give headless Chromium its software WebGPU adapter: `just web-test` gives them, and says why. */
const WEBGPU = (process.env.OCCAM_WEBGPU_FLAGS ?? "").split(/\s+/).filter(Boolean);
const DIST = fileURLToPath(new URL("../dist", import.meta.url));
/** occam-report's catalogue as `just errors` lists it: `just web-test` writes it there. */
const CATALOGUE = process.env.OCCAM_CATALOGUE ?? "";
/** The native `occam` command: `just web-test` builds it. */
const OCCAM = process.env.OCCAM ?? "";
const USAGE = { INDEX: 0x10, VERTEX: 0x20, STORAGE: 0x80 };
const CUBE_FACES = ["face:0/1", "face:1/1", "face:2/1", "face:3/1", "face:4/1", "face:5/1"];

const request = (steps) => JSON.stringify({ schema: 1, units: { length: "mm", angle: "deg" }, steps });
/** A cube of side 20 so far from the world's origin that plain `f32` positions would be off by about a thousandth. */
const CUBE = request([{ op: "box", size: [20, 20, 20], frame: { origin: [99990.3, -50010.7, 24990.1] } }]);
/** A box, a cylinder and a sphere: eleven faces, flat and bent, for the mesh workers to share out. */
const BODIES = request([
  { op: "box", size: [10, 10, 10] },
  { op: "cylinder", radius: 4, height: 10, frame: { origin: [20, 0, 0] } },
  { op: "sphere", radius: 3, frame: { origin: [0, 20, 0] } },
]);
/** A sector with a hole near its rim, extruded: at 0.1 mm both caps are refused, their boundaries crossing (P9). */
const SECTOR = request([
  { op: "wire", label: "rim", edges: [
    { arc: [[9.807852804032304, -1.9509032201612824, 0], [10, 0, 0], [9.807852804032304, 1.9509032201612824, 0]] },
    { line: [[9.807852804032304, 1.9509032201612824, 0], [0, 0, 0]] },
    { line: [[0, 0, 0], [9.807852804032304, -1.9509032201612824, 0]] },
  ] },
  { op: "wire", label: "hole", edges: [
    { circle: { centre: [9.702055900327249, 0.9555691010728864, 0], radius: 0.25, normal: [0, 0, 1] } },
  ] },
  { op: "face", label: "face", outer: "rim", holes: ["hole"] },
  { op: "extrude", face: "face", vector: [0, 0, 5] },
]);

let site;
let browser;

before(async () => {
  assert.ok(WEBGPU.length > 0, "OCCAM_WEBGPU_FLAGS is not set: run the tests with `just web-test`");
  assert.ok(CATALOGUE, "OCCAM_CATALOGUE is not set: run the tests with `just web-test`");
  assert.ok(OCCAM, "OCCAM is not set: run the tests with `just web-test`");
  site = await serve(DIST);
  browser = await chromium.launch({ headless: true, args: WEBGPU });
});

after(async () => {
  await browser?.close();
  site?.server.close();
});

/**
 * A page of the viewer with one mesh worker unless `query` says otherwise, once it reports or has run `model` in its
 * editing worker and shown what its mesh workers made of it, which `page.ran` keeps; its uncaught errors are kept on
 * `page.errorsSeen`. `before` runs in the page before its scripts, and `intercept` is given the page before it loads.
 */
async function open(query = "", { on = browser, before: prepare, intercept, model = CUBE } = {}) {
  const page = await on.newPage();
  // Generous, for a loaded machine: where the browser gets only idle CPU, a step can take a minute.
  page.setDefaultTimeout(120_000);
  page.errorsSeen = [];
  page.on("pageerror", (error) => page.errorsSeen.push(String(error)));
  page.on("console", (message) => message.type() === "error" && page.errorsSeen.push(message.text()));
  if (prepare) await page.addInitScript(prepare);
  if (intercept) await intercept(page);
  const workers = /&workers=/.test(query) ? "" : "&workers=1";
  await page.goto(`${site.url}/?inspect${workers}${query}`);
  return ready(page, model);
}

/** The page, once it reports or has run `model` and shown its mesh. */
async function ready(page, model = CUBE) {
  await page.waitForFunction(() => window.occamViewer || document.querySelector(".report.failure"));
  page.ran = await page.evaluate((r) => window.occamViewer?.run(r) ?? null, model);
  return page;
}

/** Nothing went wrong in the page or on its GPU. */
async function clean(page) {
  const gpu = await page.evaluate(() => window.occamViewer?.errors() ?? []);
  assert.deepEqual([...page.errorsSeen, ...gpu], []);
  await page.close();
}

/** Once the uploads are done: the mesh shown, the origin, the faces drawn, and each face's buffers read back. */
async function snapshot(page) {
  return page.evaluate(async () => {
    const viewer = window.occamViewer;
    await viewer.settled();
    const faces = viewer.faces();
    const read = {};
    for (const face of faces) read[face.id] = await viewer.readFace(face.id);
    return { mesh: viewer.mesh(), origin: viewer.origin(), faces, read };
  });
}

const range = (face) => face.vertices;

/** The mesh's face `id`, its block cut out. */
function block(mesh, id) {
  const face = mesh.faces.find((f) => f.id === id);
  const { first, count } = range(face);
  const t = face.triangles;
  return {
    face,
    positions: mesh.positions.subarray(first * 3, (first + count) * 3),
    normals: mesh.normals.subarray(first * 3, (first + count) * 3),
    uvs: mesh.uvs.subarray(first * 2, (first + count) * 2),
    indices: mesh.indices.subarray(t.first * 3, (t.first + t.count) * 3),
  };
}

/** The mesh is a cube: six faces of four vertices and two triangles, on two planes an axis, facing out. */
function assertCube(mesh) {
  assert.deepEqual(mesh.faces.map((f) => f.id), CUBE_FACES);
  const lo = [0, 1, 2].map((axis) => Math.min(...mesh.positions.filter((_, i) => i % 3 === axis)));
  const hi = [0, 1, 2].map((axis) => Math.max(...mesh.positions.filter((_, i) => i % 3 === axis)));
  const side = hi[0] - lo[0];
  assert.ok(Math.abs(hi[1] - lo[1] - side) < 1e-9 && Math.abs(hi[2] - lo[2] - side) < 1e-9, "the sides are equal");
  const centre = lo.map((low, axis) => (low + hi[axis]) / 2);
  for (const id of CUBE_FACES) {
    const { face, positions, normals, indices } = block(mesh, id);
    assert.equal(face.vertices.count, 4);
    assert.equal(face.triangles.count, 2);
    for (let v = 0; v < 4; v += 1) {
      const p = [0, 1, 2].map((axis) => positions[v * 3 + axis]);
      const n = [0, 1, 2].map((axis) => normals[v * 3 + axis]);
      p.forEach((x, axis) => assert.ok(x === lo[axis] || x === hi[axis], `${id} has a corner of the cube`));
      assert.equal(Math.hypot(...n), 1);
      assert.ok(n.reduce((sum, x, axis) => sum + x * (p[axis] - centre[axis]), 0) > 0, `${id} faces out`);
    }
    assert.ok(indices.every((index) => index < 4), `${id}'s indices are in its block`);
  }
}

/** Each drawn face's buffers hold its block: `f32` offsets within an `f32` rounding of the `f64` positions. */
function assertUploaded({ mesh, origin, faces, read }) {
  assert.deepEqual(faces.map((f) => f.id).sort(), mesh.faces.map((f) => f.id).sort());
  let plainWouldMiss = false;
  for (const { id } of faces) {
    const { positions, normals, uvs, indices } = block(mesh, id);
    const { vertices, indices: gpuIndices } = read[id];
    assert.equal(vertices.length, (positions.length / 3) * 8, `${id}: 32 bytes a vertex`);
    assert.deepEqual([...gpuIndices], [...indices], `${id}: the indices as given`);
    for (let v = 0; v < positions.length / 3; v += 1) {
      for (let axis = 0; axis < 3; axis += 1) {
        const [p, offset] = [positions[v * 3 + axis], vertices[v * 8 + axis]];
        const gap = Math.abs(origin[axis] + offset - p);
        const allowed = Math.abs(p - origin[axis]) * 2 ** -24 + Math.abs(p) * 2 ** -51;
        assert.ok(gap <= allowed, `${id} vertex ${v}: ${origin[axis]} + ${offset} is ${gap} from ${p}`);
        plainWouldMiss ||= Math.abs(Math.fround(p) - p) > allowed;
        assert.equal(vertices[v * 8 + 4 + axis], Math.fround(normals[v * 3 + axis]), `${id} vertex ${v}: normal`);
      }
      assert.equal(vertices[v * 8 + 3], Math.fround(uvs[v * 2]), `${id} vertex ${v}: u`);
      assert.equal(vertices[v * 8 + 7], Math.fround(uvs[v * 2 + 1]), `${id} vertex ${v}: v`);
    }
  }
  return plainWouldMiss;
}

/** `m · (x, y, z, 1)`, `m` column-major. */
function project(m, [x, y, z]) {
  return [0, 1, 2, 3].map((row) => m[row] * x + m[4 + row] * y + m[8 + row] * z + m[12 + row]);
}

/** Every vertex the GPU holds, through the camera it drew with, as `[x, y, z] / w`, or null behind the eye. */
async function projected(page) {
  const { read } = await snapshot(page);
  const camera = await page.evaluate(() => window.occamViewer.readCamera());
  const out = [];
  for (const { vertices } of Object.values(read)) {
    for (let v = 0; v < vertices.length; v += 8) {
      const [x, y, z, w] = project(camera, [vertices[v], vertices[v + 1], vertices[v + 2]]);
      out.push(w > 0 ? [x / w, y / w, z / w] : null);
    }
  }
  return out;
}

const inside = (p) => p !== null && Math.abs(p[0]) <= 1 && Math.abs(p[1]) <= 1 && p[2] >= 0 && p[2] <= 1;

/** The mesh without face `id`, its flat arrays packed again. */
function without(mesh, id) {
  const kept = mesh.faces.filter((f) => f.id !== id).map((f) => block(mesh, f.id));
  const join = (key, Type) => {
    const out = new Type(kept.reduce((sum, b) => sum + b[key].length, 0));
    kept.reduce((at, b) => (out.set(b[key], at), at + b[key].length), 0);
    return out;
  };
  let [vertex, triangle] = [0, 0];
  const faces = kept.map(({ face }) => {
    const moved = { id: face.id, vertices: { first: vertex, count: face.vertices.count },
      triangles: { first: triangle, count: face.triangles.count } };
    [vertex, triangle] = [vertex + face.vertices.count, triangle + face.triangles.count];
    return moved;
  });
  return { positions: join("positions", Float64Array), normals: join("normals", Float64Array),
    uvs: join("uvs", Float64Array), indices: join("indices", Uint32Array), faces };
}

const exec = promisify(execFile);

/**
 * The native `occam`'s answer for `model` meshed to `deflection` and `angle`: `occam run` builds it in a model file of
 * its own, and `occam mesh` writes its buffers; their bytes, and what `occam mesh --json` says.
 */
async function nativeMesh(model, deflection, angle) {
  const dir = await mkdtemp(join(tmpdir(), "occam-web-"));
  try {
    const [part, ops, out] = ["part.occam", "ops.json", "part.occam.mesh"].map((name) => join(dir, name));
    await writeFile(ops, model);
    await exec(OCCAM, ["run", "--model", part, "--binary", ops]);
    const asked = ["mesh", "--model", part, "--deflection", String(deflection), "--angle", String(angle), "--json"];
    // `occam mesh` exits 1 when anything is refused, and writes the buffers all the same.
    const { stdout } = await exec(OCCAM, asked).catch((refused) => refused);
    return { bytes: await readFile(out), said: JSON.parse(stdout) };
  } finally {
    await rm(dir, { recursive: true, force: true });
  }
}

/** The page's mesh as `occam mesh` writes a mesh (docs/architecture/mesh-and-renderer.md), little-endian. */
function fileBytes(mesh) {
  const [vertices, triangles] = [mesh.positions.length / 3, mesh.indices.length / 3];
  const view = new DataView(new ArrayBuffer(24 + 64 * vertices + 12 * triangles + 24 * mesh.faces.length));
  let at = 0;
  const put = (write, size) => (value) => (write.call(view, at, value, true), (at += size));
  [vertices, triangles, mesh.faces.length].map(BigInt).forEach(put(view.setBigUint64, 8));
  for (const reals of [mesh.positions, mesh.normals, mesh.uvs]) reals.forEach(put(view.setFloat64, 8));
  mesh.indices.forEach(put(view.setUint32, 4));
  for (const { id, vertices: v, triangles: t } of mesh.faces) {
    const [, slot, generation] = id.match(/^face:(\d+)\/(\d+)$/).map(Number);
    [slot, generation, v.first, v.count, t.first, t.count].forEach(put(view.setUint32, 4));
  }
  assert.equal(at, view.byteLength);
  return new Uint8Array(view.buffer);
}

const sha256 = (bytes) => createHash("sha256").update(bytes).digest("hex");

test("a request run in the editing worker is meshed by four mesh workers to the hash the native mesher gives", async () => {
  const page = await open("&workers=4&deflection=0.1&angle=0.35", { model: BODIES });
  assert.equal(JSON.parse(page.ran.ran).ok, true);
  assert.equal(await page.evaluate(() => window.occamViewer.workers()), 4);
  const state = await snapshot(page);
  const native = await nativeMesh(BODIES, 0.1, 0.35);
  assert.equal(native.said.ok, true);
  assert.equal(state.mesh.faces.length, native.said.answers.length);
  assert.equal(sha256(fileBytes(state.mesh)), sha256(native.bytes), "the parts laid end to end, bit for bit");
  const calls = await page.evaluate(() => window.occamViewer.calls());
  assert.deepEqual(calls.map(({ first, end }) => [first, end]), state.mesh.faces.map((_, k) => [k, k + 1]),
    "under 64 faces, a call a face, in id order");
  assert.deepEqual(new Set(calls.map((call) => call.worker)), new Set([0, 1, 2, 3]), "each worker meshed some");
  assertUploaded(state);
  await clean(page);
});

test("a face that will not mesh is named, as the native mesher names it, and the rest arrive and are drawn", async () => {
  const page = await open("&workers=4&deflection=0.1", { model: SECTOR });
  const native = await nativeMesh(SECTOR, 0.1, 0.35);
  const refusedNatively = native.said.answers.filter((answer) => answer.report).map((answer) => answer.entity);
  assert.equal(refusedNatively.length, 2, "both caps");
  const shown = await page.locator(".report.refusal").evaluateAll((sections) =>
    sections.map(({ dataset }) => [dataset.code, dataset.entity, dataset.reason]));
  assert.deepEqual(shown.filter(([code]) => code === "mesh.face-not-meshed"),
    refusedNatively.map((face) => ["mesh.face-not-meshed", face, "boundary-crosses"]));
  assert.deepEqual(shown.filter(([code]) => code === "mesh.not-watertight"),
    refusedNatively.map((face) => ["mesh.not-watertight", face, "face-missing"]), "each a hole where it is");
  const state = await snapshot(page);
  const meshed = native.said.answers.filter((answer) => answer.mesh).map((answer) => answer.entity);
  assert.equal(meshed.length, 5);
  assert.deepEqual(state.faces.map((face) => face.id).sort(), [...meshed].sort(), "the rest drawn");
  assert.equal(sha256(fileBytes(state.mesh)), sha256(native.bytes));
  assertUploaded(state);
  await clean(page);
});

/** Notes on `window.wasmSeen` every use of WebAssembly's API in the page's main thread, which workers do not share. */
function watchWebAssembly() {
  window.wasmSeen = [];
  for (const name of ["compile", "compileStreaming", "instantiate", "instantiateStreaming", "validate"]) {
    const real = WebAssembly[name];
    WebAssembly[name] = (...args) => (window.wasmSeen.push(name), real.apply(WebAssembly, args));
  }
  for (const name of ["Module", "Instance", "Memory"]) {
    const construct = (target, args) => (window.wasmSeen.push(name), Reflect.construct(target, args));
    WebAssembly[name] = new Proxy(WebAssembly[name], { construct });
  }
}

test("the main thread never loads the kernel's module, each worker loads its own, and no COOP or COEP is needed", async () => {
  const page = await open("&workers=4", { before: watchWebAssembly });
  const { faces } = await snapshot(page);
  assert.equal(faces.length, 6, "drawn from the workers' buffers");
  const main = await page.evaluate(() => ({
    seen: window.wasmSeen,
    fetched: performance.getEntriesByType("resource").map((entry) => entry.name).filter((name) => name.endsWith(".wasm")),
    isolated: crossOriginIsolated,
  }));
  assert.deepEqual(main, { seen: [], fetched: [], isolated: false });
  const workers = page.workers();
  assert.equal(workers.length, 5, "the editing worker and four mesh workers");
  const loaded = await Promise.all(workers.map((worker) =>
    worker.evaluate(() => performance.getEntriesByType("resource").filter((e) => e.name.endsWith("/occam.wasm")).length)));
  assert.deepEqual(loaded, [1, 1, 1, 1, 1], "each its own");
  const script = await readFile(join(DIST, "viewer.js"), "utf8");
  assert.ok(!script.includes("WebAssembly"), "the main thread's script holds nothing that loads a module");
  await clean(page);
});

test("the page draws a cube, and the buffers it uploaded match the mesh, as f32 offsets from its f64 origin", async () => {
  const page = await open();
  const state = await snapshot(page);
  assertCube(state.mesh);
  assert.ok(assertUploaded(state), "the cube is far enough out that plain f32 positions would miss");
  for (const face of state.faces) {
    assert.ok(face.vertexUsage & USAGE.VERTEX && face.vertexUsage & USAGE.STORAGE, `${face.id}: vertex and storage`);
    assert.ok(face.indexUsage & USAGE.INDEX && face.indexUsage & USAGE.STORAGE, `${face.id}: index and storage`);
  }
  await clean(page);
});

test("a face's vertex block reads the same through a storage binding as the draw's vertex fetch is given", async () => {
  const page = await open();
  const { faces, read } = await snapshot(page);
  assert.equal(faces.length, 6);
  for (const { id } of faces) {
    const stored = await page.evaluate((face) => window.occamViewer.readThroughStorage(face), id);
    assert.ok(stored, `${id} reads through a storage binding`);
    assert.deepEqual([...new Uint32Array(stored.buffer)], [...new Uint32Array(read[id].vertices.buffer)], id);
  }
  await clean(page);
});

test("a model over the per-frame bound is uploaded in pieces, no frame over the bound", async () => {
  const page = await open("&upload-bound=64");
  const state = await snapshot(page);
  const pieces = await page.evaluate(() => window.occamViewer.uploads());
  assert.equal(await page.evaluate(() => window.occamViewer.bound()), 64);
  const perFrame = new Map();
  for (const piece of pieces) perFrame.set(piece.frame, (perFrame.get(piece.frame) ?? 0) + piece.size);
  const total = [...perFrame.values()].reduce((a, b) => a + b, 0);
  assert.equal(total, 6 * (4 * 32 + 6 * 4), "every byte of the cube, once");
  assert.ok([...perFrame.values()].every((bytes) => bytes <= 64), "no frame over the bound");
  assert.ok(perFrame.size >= Math.ceil(total / 64), `${perFrame.size} frames`);
  const split = pieces.filter((p) => p.kind === "vertices" && p.offset > 0);
  assert.ok(split.length > 0, "a vertex block larger than the bound arrives in more than one piece");
  for (const id of CUBE_FACES) {
    for (const kind of ["vertices", "indices"]) {
      const mine = pieces.filter((p) => p.face === id && p.kind === kind);
      mine.reduce((at, p) => (assert.equal(p.offset, at, `${id} ${kind} in order`), at + p.size), 0);
    }
  }
  assertUploaded(state);
  await clean(page);
});

test("a face edited away disappears without the others being uploaded again", async () => {
  const page = await open();
  const before = await snapshot(page);
  await page.evaluate(() => window.occamViewer.clearUploads());
  await page.evaluate((mesh) => window.occamViewer.show(mesh), without(before.mesh, "face:2/1"));
  const afterwards = await snapshot(page);
  assert.deepEqual(await page.evaluate(() => window.occamViewer.uploads()), [], "nothing uploaded");
  assert.deepEqual(afterwards.faces.map((f) => f.id), CUBE_FACES.filter((id) => id !== "face:2/1"));
  assert.deepEqual(afterwards.origin, before.origin);
  for (const { id } of afterwards.faces) {
    assert.deepEqual([...afterwards.read[id].vertices], [...before.read[id].vertices], `${id} unchanged`);
  }
  assert.equal(await page.evaluate(() => window.occamViewer.readFace("face:2/1")), null);
  await clean(page);
});

test("fit leaves every vertex inside the view, and fills it", async () => {
  const page = await open();
  await snapshot(page);
  const canvas = await page.locator("#view").boundingBox();
  const [x, y] = [canvas.x + canvas.width / 2, canvas.y + canvas.height / 2];
  await page.mouse.move(x, y);
  await page.mouse.wheel(0, -2500);
  await page.mouse.down({ button: "right" });
  await page.mouse.move(x + 300, y + 200, { steps: 4 });
  await page.mouse.up({ button: "right" });
  assert.ok(!(await projected(page)).every(inside), "zoomed in and panned, the cube leaves the view");
  await page.getByRole("button", { name: "Fit" }).click();
  const fitted = await projected(page);
  assert.ok(fitted.every(inside), "fitted, every vertex is inside");
  const reach = Math.max(...fitted.map(([px, py]) => Math.max(Math.abs(px), Math.abs(py))));
  assert.ok(reach >= 0.5, `and the cube fills the view, out to ${reach}`);
  await clean(page);
});

test("dragging orbits, a right drag pans, the wheel zooms, and Centre puts the selection in the middle", async () => {
  const page = await open();
  const { mesh } = await snapshot(page);
  const camera = () => page.evaluate(() => window.occamViewer.camera());
  const canvas = await page.locator("#view").boundingBox();
  const [x, y] = [canvas.x + canvas.width / 2, canvas.y + canvas.height / 2];
  const drag = async (button) => {
    await page.mouse.move(x, y);
    await page.mouse.down({ button });
    await page.mouse.move(x + 120, y + 40, { steps: 4 });
    await page.mouse.up({ button });
  };
  const start = await camera();
  await drag("left");
  const orbited = await camera();
  assert.notEqual(orbited.yaw, start.yaw);
  assert.notEqual(orbited.pitch, start.pitch);
  assert.deepEqual([orbited.target, orbited.distance], [start.target, start.distance], "orbit keeps the target");
  await drag("right");
  const panned = await camera();
  assert.notDeepEqual(panned.target, orbited.target);
  assert.deepEqual([panned.yaw, panned.pitch, panned.distance], [orbited.yaw, orbited.pitch, orbited.distance]);
  await page.mouse.wheel(0, 400);
  await page.waitForFunction((before) => window.occamViewer.camera().distance !== before, panned.distance);
  const zoomed = await camera();
  assert.ok(zoomed.distance > panned.distance * 1.2, "wheeling down moves away");
  assert.deepEqual(zoomed.target, panned.target, "zoom keeps the target");
  const centre = page.getByRole("button", { name: "Centre" });
  assert.ok(await centre.isDisabled(), "nothing is selected");
  await page.evaluate(() => window.occamViewer.select(["face:1/1"]));
  await centre.click();
  const top = block(mesh, "face:1/1").positions;
  const middle = [0, 1, 2].map((axis) => {
    const values = top.filter((_, i) => i % 3 === axis);
    return (Math.min(...values) + Math.max(...values)) / 2;
  });
  assert.deepEqual((await camera()).target, middle, "the target is the selection's centre");
  await page.evaluate(() => window.occamViewer.settled());
  const origin = await page.evaluate(() => window.occamViewer.origin());
  const matrix = await page.evaluate(() => window.occamViewer.readCamera());
  const [cx, cy, , cw] = project(matrix, middle.map((m, axis) => m - origin[axis]));
  assert.ok(Math.abs(cx / cw) < 1e-4 && Math.abs(cy / cw) < 1e-4, `in the middle of the view: ${cx / cw}, ${cy / cw}`);
  await clean(page);
});

/** Frames the page submits while the page's own animation frames tick `ticks` times. */
const framesOver = (page, ticks) =>
  page.evaluate(async (count) => {
    const start = window.occamViewer.frames();
    for (let i = 0; i < count; i += 1) await new Promise((resolve) => requestAnimationFrame(resolve));
    return window.occamViewer.frames() - start;
  }, ticks);

/** Whether the page's loop comes to rest, submitting nothing over ten animation frames, within 30 s. */
const comesToRest = (page) =>
  page.evaluate(async () => {
    const end = performance.now() + 30_000;
    while (performance.now() < end) {
      const start = window.occamViewer.frames();
      for (let i = 0; i < 10; i += 1) await new Promise((resolve) => requestAnimationFrame(resolve));
      if (window.occamViewer.frames() === start) return true;
    }
    return false;
  });

test("the frame loop keeps submitting while the mouse is held still, and rests once it is let go", async () => {
  const page = await open();
  await snapshot(page);
  assert.ok(await comesToRest(page), "with nothing to do, it rests");
  const canvas = await page.locator("#view").boundingBox();
  await page.mouse.move(canvas.x + canvas.width / 2, canvas.y + canvas.height / 2);
  await page.mouse.down();
  await page.waitForTimeout(1000);
  assert.ok((await framesOver(page, 30)) >= 28, "held still past the loop's linger, a frame every animation frame");
  await page.mouse.up();
  assert.ok(await comesToRest(page), "let go, it rests again");
  await clean(page);
});

test("a malformed mesh is reported by name, and the last good model stays drawn", async () => {
  const page = await open();
  const { mesh } = await snapshot(page);
  const broken = { ...mesh, indices: mesh.indices.map((index, i) => (i === 7 ? 99 : index)) };
  await page.evaluate((m) => window.occamViewer.show(m), broken);
  const notice = page.locator(".report.notice");
  await notice.waitFor();
  assert.equal(await notice.getAttribute("data-code"), "view.mesh-malformed");
  assert.match(await notice.textContent(), /face:1\/1 has an index outside its block/);
  const { faces } = await snapshot(page);
  assert.equal(faces.length, 6);
  await clean(page);
});

/**
 * Follows the remedy of the report `page` shows in place of its canvas, or `over` it as it goes on drawing (L2): the
 * report has `code` and offers the fix `kind`, and `retry`, which does what the fix says, gives a page that draws the
 * whole model with no report. `gates errors` counts each call as the test that follows its pair, as it counts
 * `followed_remedy!` in Rust, so name the pair on the call's line.
 */
async function followedRemedy(page, code, kind, retry, { over = false } = {}) {
  const shown = page.locator(over ? ".report.notice" : ".report.failure");
  await shown.waitFor();
  assert.equal(await shown.getAttribute("data-code"), code);
  assert.equal(await shown.getAttribute("data-remedy"), kind);
  assert.ok((await shown.locator(".remedy").textContent()).length > 0, "with its remedy");
  assert.equal(await page.locator("#view").isVisible(), over, over ? "drawing what it can" : "not a blank canvas");
  const retried = await retry();
  const { faces } = await snapshot(retried);
  assert.equal(faces.length, 6, "following the remedy, the page draws");
  assert.equal(await retried.locator(".report").count(), 0, "and reports nothing");
  for (const each of new Set([page, retried])) await clean(each);
}

test("a page denied WebGPU by a browser without it shows the report, and in one with it draws", async () => {
  const page = await open("", { before: () => delete Navigator.prototype.gpu });
  await followedRemedy(page, "view.webgpu-missing", "use-webgpu-browser", () => open());
});

test("a page denied WebGPU by a browser that gives no adapter shows the report, and with WebGPU on draws", async () => {
  const off = await chromium.launch({ headless: true });
  try {
    const page = await open("", { on: off });
    await followedRemedy(page, "view.no-adapter", "turn-on-webgpu", () => open());
  } finally {
    await off.close();
  }
});

test("a page refused a device shows the report, and reloaded draws", async () => {
  const refuseOnce = () => {
    if (sessionStorage.getItem("refused")) return;
    sessionStorage.setItem("refused", "yes");
    GPUAdapter.prototype.requestDevice = () => Promise.reject(new DOMException("refused once", "OperationError"));
  };
  const page = await open("", { before: refuseOnce });
  await followedRemedy(page, "view.no-device", "reload-page", () => page.reload().then(() => ready(page)));
});

test("a page whose device is lost shows the report, and reloaded draws", async () => {
  const page = await open();
  await snapshot(page);
  await page.evaluate(() => window.occamViewer.loseDevice());
  await followedRemedy(page, "view.device-lost", "reload-page", () => page.reload().then(() => ready(page)));
});

/** Gives the first mesh worker to fetch the kernel's module bytes that are not one, once, so its calls throw. */
async function breakOneMeshWorker(page) {
  let broken = false;
  await page.route("**/occam.wasm", (route) => {
    // A worker's fetch names its own script as the referrer.
    if (broken || !(route.request().headers().referer ?? "").endsWith("/mesher.js")) return route.continue();
    broken = true;
    return route.fulfill({ contentType: "application/wasm", body: "not a module" });
  });
}

test("a mesh call that throws is reported by the faces it held, the rest are drawn, and reloaded the page draws all", async () => {
  const page = await open("&workers=2", { intercept: breakOneMeshWorker });
  const { faces } = await snapshot(page);
  const drawn = faces.map((face) => face.id);
  assert.ok(drawn.length > 0 && drawn.length < 6, `the other worker's faces drawn: ${drawn}`);
  const shown = page.locator(".report.notice");
  assert.equal(await shown.count(), 1, "a report over the drawing");
  const missing = CUBE_FACES.filter((id) => !drawn.includes(id));
  assert.equal(await shown.getAttribute("data-entities"), missing.join(" "), "naming every face not drawn");
  assert.match(await shown.locator(".message").textContent(), new RegExp(`mesh ${missing.join(", ")}, .*CompileError`));
  await followedRemedy(page, "view.mesh-failed", "reload-page", () => page.reload().then(() => ready(page)), { over: true });
});

test("every report the page can show is in occam-report's catalogue, worded as there, with the same remedy", async () => {
  const catalogue = JSON.parse(await readFile(CATALOGUE, "utf8"));
  const page = await open();
  const reports = await page.evaluate(() => window.occamViewer.reports());
  assert.ok(reports.length > 0);
  for (const shown of reports) {
    const row = catalogue.codes.find((entry) => entry.code === shown.code);
    assert.ok(row, `${shown.code} is in the catalogue`);
    assert.equal(shown.template, row.template, `${shown.code} is worded as the catalogue words it`);
    assert.deepEqual(shown.remedy ? [shown.remedy] : [], row.remedies, `${shown.code} offers the catalogue's remedy`);
    const remedy = catalogue.remedies.find((entry) => entry.kind === shown.remedy);
    if (shown.remedy) assert.equal(shown.remedyText, remedy?.summary, `${shown.remedy} is worded as there`);
  }
  await clean(page);
});
