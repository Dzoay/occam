// A pick timed on a real GPU (docs/plan/view.md, `view.timing`): in Chromium or Firefox, the page `occam view` serves
// for a model, a click on each of a sample of its faces, from the pointer's release to the frame that shows it selected
// finished on the GPU. The first pick, which meshes the model whole in a worker, is timed apart. The 16.7 ms it is held
// to is a goal no measurement grounds (docs/decisions/2026-11-13-gpu-figures-are-goals.md), never a gate.
//
// It runs headless first; if the browser reaches only a software adapter there, as headless Chromium does, it runs
// headed, which opens a browser window. `--headed` runs headed from the start, as a person would see it. It refuses a
// run on a software adapter, exiting 3 with the adapter it found, since a software renderer's frame says nothing of a
// GPU's.
//
//   node web/test/pick-timing.mjs <model file> [chromium|firefox] [--picks N] [--workers N] [--headless-only|--headed]
//
// Run by `just view-pick-gpu`, which builds the page and a release `occam`. `OCCAM_WEBGPU_FLAGS`, if set, replaces
// Chromium's flags: the test of the refusal gives it `just web-test`'s, which ask for SwiftShader.
import { spawn } from "node:child_process";
import { copyFile, mkdtemp, rm } from "node:fs/promises";
import { createServer } from "node:http";
import { createInterface } from "node:readline";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { chromium, firefox } from "playwright";

import { pixelOf } from "./demo/harness.mjs";

/** How long to wait for anything: P9's 10,001-face model takes minutes to open, in the server and in each worker. */
const PATIENCE = 60 * 60_000;
/** What the page is held to: one frame at 60 Hz. A goal, not a gate. */
const GOAL_MS = 16.7;
/** Chromium's flags for WebGPU on a real GPU on Linux, where it is not on by default. */
const CHROMIUM = ["--enable-unsafe-webgpu", "--enable-features=Vulkan", "--ignore-gpu-blocklist"];
/** Firefox's preferences for WebGPU, where it is not on by default. */
const FIREFOX = { "dom.webgpu.enabled": true, "gfx.webgpu.ignore-blocklist": true };
/** What names a software adapter, in what the adapter says of itself. */
const SOFTWARE = /swiftshader|llvmpipe|lavapipe|softpipe|software|basic render/i;

/** The arguments: the model, the browser, and the options. */
function read(args) {
  const options = { browser: "chromium", picks: 50, workers: null, headlessOnly: false, headed: false, model: null };
  for (let k = 0; k < args.length; k += 1) {
    const arg = args[k];
    if (arg === "--picks") options.picks = Number(args[(k += 1)]);
    else if (arg === "--workers") options.workers = Number(args[(k += 1)]);
    else if (arg === "--headless-only") options.headlessOnly = true;
    else if (arg === "--headed") options.headed = true;
    else if (arg === "chromium" || arg === "firefox") options.browser = arg;
    else if (!options.model) options.model = arg;
    else return null;
  }
  return options.model && options.picks > 0 && !(options.headed && options.headlessOnly) ? options : null;
}

/** `kind` launched headless or headed, with WebGPU on. */
function launch(kind, headless) {
  if (kind === "firefox") return firefox.launch({ headless, firefoxUserPrefs: FIREFOX });
  const flags = (process.env.OCCAM_WEBGPU_FLAGS ?? "").split(/\s+/).filter(Boolean);
  return chromium.launch({ headless, args: flags.length > 0 ? flags : CHROMIUM });
}

/** The adapter `browser` gives a page asking as the viewer asks, and whether it is a software one. */
async function adapterOf(browser) {
  const server = createServer((_, response) => response.writeHead(200, { "content-type": "text/html" }).end("<!doctype html>"));
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  try {
    const page = await browser.newPage();
    await page.goto(`http://127.0.0.1:${server.address().port}/`);
    const info = await page.evaluate(async () => {
      const adapter = await navigator.gpu?.requestAdapter({ powerPreference: "high-performance" }).catch(() => null);
      if (!adapter) return null;
      const { vendor, architecture, device, description, isFallbackAdapter } = adapter.info;
      return { vendor, architecture, device, description, isFallbackAdapter: Boolean(isFallbackAdapter) };
    });
    await page.close();
    const named = info ? [info.vendor, info.architecture, info.device, info.description].join(" ") : "";
    return { info, software: !info || info.isFallbackAdapter || SOFTWARE.test(named) };
  } finally {
    server.close();
  }
}

/** The browser, headless if that reaches a real adapter, else headed unless `headlessOnly`, or headed if `headed`. */
async function reach(kind, headlessOnly, headed) {
  for (const headless of headed ? [false] : headlessOnly ? [true] : [true, false]) {
    const browser = await launch(kind, headless);
    const adapter = await adapterOf(browser);
    if (!adapter.software) return { browser, headless, adapter };
    await browser.close();
    if (headless && !headlessOnly) console.error(`${kind} headless reached only ${JSON.stringify(adapter.info)}; running headed`);
    if (!headless || headlessOnly) return { browser: null, headless, adapter };
  }
  return null;
}

/** One click at `(x, y)`: from the pointer's release to the pick's answer, and to the frame showing it finished. */
async function pick(page, [x, y]) {
  await page.evaluate(() => {
    const canvas = document.getElementById("view");
    window.timedPick = { before: window.occamViewer.picked(), released: null };
    canvas.addEventListener("pointerup", (event) => (window.timedPick.released = event.timeStamp), { once: true, capture: true });
  });
  await page.mouse.click(x, y);
  return page.evaluate(async () => {
    // The next turn of the page's event loop, without a timer's clamping.
    const turn = () => new Promise((resolve) => {
      const channel = new MessageChannel();
      channel.port1.onmessage = resolve;
      channel.port2.postMessage(0);
    });
    const timed = window.timedPick;
    while (timed.released === null || window.occamViewer.picked() === timed.before) await turn();
    const answered = performance.now() - timed.released;
    await window.occamViewer.settled();
    return { answered, shown: performance.now() - timed.released, entity: window.occamViewer.picked()?.entity ?? null };
  });
}

/** Where to click: the middle of the largest triangle of every `step`th face, those inside the canvas. */
async function targets(page, picks) {
  const centres = await page.evaluate((picks) => {
    const mesh = window.occamViewer.mesh();
    const step = Math.max(1, Math.floor(mesh.faces.length / picks));
    const point = (k) => [0, 1, 2].map((axis) => mesh.positions[k * 3 + axis]);
    return mesh.faces.filter((_, k) => k % step === 0).slice(0, picks).map((face) => {
      const first = face.triangles.first * 3;
      const corners = [0, 1, 2].map((c) => point(face.vertices.first + mesh.indices[first + c]));
      return [0, 1, 2].map((axis) => corners.reduce((sum, p) => sum + p[axis], 0) / 3);
    });
  }, picks);
  const box = await page.locator("#view").boundingBox();
  const inside = ([x, y]) => x > box.x && y > box.y && x < box.x + box.width && y < box.y + box.height;
  const pixels = [];
  for (const centre of centres) pixels.push(await pixelOf(page, centre));
  return pixels.filter(inside);
}

/**
 * The viewer's server for `file`, started as `occam view` starts it but waited for as long as the model takes to open:
 * `occam view` gives a server 30 s to say it listens, and on P9's larger model its first look takes longer. Resolves
 * with the page's address and a way to stop it.
 */
async function serving(occam, file) {
  const server = spawn(occam, ["view", "--serve", "--idle", "3600", "--model", file], { stdio: ["ignore", "pipe", "inherit"] });
  const lines = createInterface({ input: server.stdout });
  const first = await new Promise((resolve, reject) => {
    lines.once("line", resolve);
    server.once("exit", (code) => reject(new Error(`occam view --serve ended, ${code}, before it listened`)));
  });
  const said = JSON.parse(first);
  if (!said.port || !said.token) throw new Error(`occam view --serve did not start: ${first}`);
  return { url: `http://127.0.0.1:${said.port}/${said.token}/`, stop: () => server.kill("SIGTERM") };
}

const quantile = (xs, q) => [...xs].sort((a, b) => a - b)[Math.min(xs.length - 1, Math.floor(q * xs.length))];
const summary = (xs) => ({ median: quantile(xs, 0.5), p95: quantile(xs, 0.95), max: Math.max(...xs) });

async function measure(options, reached) {
  const occam = process.env.OCCAM ?? "occam";
  const dir = await mkdtemp(join(tmpdir(), "occam-pick-"));
  const file = join(dir, basename(options.model));
  let started = null;
  try {
    await copyFile(options.model, file);
    started = await serving(occam, file);
    const page = await reached.browser.newPage({ viewport: { width: 1280, height: 800 } });
    page.setDefaultTimeout(PATIENCE);
    const workers = options.workers ? `&workers=${options.workers}` : "";
    await page.goto(`${started.url}?inspect${workers}`);
    await page.waitForFunction(() => (window.occamViewer?.live()?.models ?? 0) >= 1, null, { polling: 100 });
    await page.evaluate(() => window.occamViewer.settled());
    const where = await targets(page, options.picks + 1);
    const first = await pick(page, where[0]);
    const timed = [];
    for (const at of where.slice(1)) timed.push(await pick(page, at));
    const shown = timed.map((t) => t.shown);
    return {
      mode: "pick-gpu",
      browser: options.browser,
      version: reached.browser.version(),
      headless: reached.headless,
      adapter: reached.adapter.info,
      model: basename(options.model),
      faces: await page.evaluate(() => window.occamViewer.mesh().faces.length),
      workers: await page.evaluate(() => window.occamViewer.workers()),
      first_pick_ms: first.shown,
      picks: timed.length,
      hit: timed.filter((t) => t.entity !== null).length,
      answered_ms: summary(timed.map((t) => t.answered)),
      shown_ms: summary(shown),
      goal_ms: GOAL_MS,
      within_goal: summary(shown).median <= GOAL_MS,
    };
  } finally {
    started?.stop();
    await rm(dir, { recursive: true, force: true });
  }
}

const options = read(process.argv.slice(2));
if (!options) {
  console.error("usage: pick-timing.mjs <model file> [chromium|firefox] [--picks N] [--workers N] [--headless-only|--headed]");
  process.exit(2);
}
const reached = await reach(options.browser, options.headlessOnly, options.headed);
if (!reached?.browser) {
  console.log(JSON.stringify({
    mode: "pick-gpu",
    browser: options.browser,
    refused: "software-adapter",
    headless: reached?.headless ?? true,
    adapter: reached?.adapter.info ?? null,
    why: "a software adapter's frame says nothing of a GPU's; run this on a machine with a GPU the browser reaches",
  }));
  process.exit(3);
}
try {
  console.log(JSON.stringify(await measure(options, reached)));
} finally {
  await reached.browser.close();
}
