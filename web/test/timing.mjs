// The `view` milestone's timings in the browser (docs/plan/view.md, `view.timing`): goals, not gates, each printed as
// one JSON line for the evidence. Headless Chromium, on the software adapter `just web-test` gives it: these time the
// workers, which run on the CPU, not the GPU.
//
//   pool <model file> [workers ...]  the page `occam view` serves, at each count of mesh workers (1 to 6 if none are
//                                    given): cold, from a fresh browser context's navigation to the model drawn; then
//                                    warm, the model opened again in the editing worker and meshed whole by the live
//                                    pool, three times. Each call's time is kept, a worker's first including its open.
//   editing                          the editing worker's call on each of construction's requests, on a fresh page each
//                                    time, five times: the longest call any edit makes on its worker.
//   redraw <model file> [rounds]     the time from a run's save, the file's modification time, to the page's redraw of
//                                    the model the run made, ten times unless `rounds` says otherwise. A request's
//                                    `.json` in place of a model is run first to make one.
//   memory <model file> [workers]    the page's peak memory with two mesh workers unless `workers` says otherwise, from
//                                    navigation to the model drawn and picked once, a pick meshing it whole in a
//                                    worker: the most each of the browser's processes held resident, the renderer's
//                                    being the page's and its workers', read from Linux's /proc.
//
// Run by `just view-timing-browser`, which builds the page and a release `occam`, which serves it.
import { execFile } from "node:child_process";
import { copyFile, mkdtemp, readdir, readFile, rm, stat, writeFile } from "node:fs/promises";
import { tmpdir } from "node:os";
import { basename, join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { chromium } from "playwright";

import { serve } from "./serve.mjs";

const exec = promisify(execFile);
const OCCAM = process.env.OCCAM ?? "";
const WEBGPU = (process.env.OCCAM_WEBGPU_FLAGS ?? "").split(/\s+/).filter(Boolean);
const DIST = fileURLToPath(new URL("../../crates/occam-cli/page", import.meta.url));
/** Long enough for P9's models, whose every open measures and checks each spline edge. */
const PATIENCE = 30 * 60_000;

const usage = () => {
  console.error("usage: timing.mjs pool <model file> [workers ...] | editing | redraw <model file> [rounds] | memory <model file> [workers]");
  process.exit(2);
};
const print = (row) => console.log(JSON.stringify(row));
const median = (xs) => [...xs].sort((a, b) => a - b)[Math.floor(xs.length / 2)];
const least = (xs) => Math.min(...xs);

/** What the native `occam` answers `args` in JSON, whatever its exit status. */
async function occam(args) {
  const { stdout } = await exec(OCCAM, [...args, "--json"], { maxBuffer: 1 << 28 }).catch((refused) => refused);
  return JSON.parse(stdout);
}

/**
 * `occam view` of a copy of `model` in a directory of its own, or of the model a request's `.json` makes, opening no
 * page; `done` stops it and cleans up.
 */
async function viewing(model) {
  const dir = await mkdtemp(join(tmpdir(), "occam-timing-"));
  const file = join(dir, model.endsWith(".json") ? "part.occam" : basename(model));
  if (model.endsWith(".json")) {
    const made = await occam(["run", "--model", file, model]);
    if (!made.ok) throw new Error(`${model} did not make a model: ${JSON.stringify(made).slice(0, 400)}`);
  } else {
    await copyFile(model, file);
  }
  const started = await occam(["view", "--model", file, "--no-open", "--idle", "3600"]);
  if (!started.running) throw new Error(`occam view did not start: ${JSON.stringify(started)}`);
  return {
    file,
    started,
    done: async () => {
      await occam(["view", "--model", file, "--stop"]);
      await rm(dir, { recursive: true, force: true });
    },
  };
}

/** A page in a context of its own, so that nothing is cached from another, its errors thrown when it is closed. */
async function page(browser, url) {
  const context = await browser.newContext();
  const opened = await context.newPage();
  opened.setDefaultTimeout(PATIENCE);
  const errors = [];
  opened.on("pageerror", (error) => errors.push(String(error)));
  opened.on("crash", () => console.error(`the page at ${url} crashed`));
  await opened.goto(url);
  return {
    page: opened,
    close: async () => {
      await context.close();
      if (errors.length > 0) throw new Error(errors.join("\n"));
    },
  };
}

/** Each worker's first call, which waits for its open, and the rest, which do not: their times in milliseconds. */
function calls(made) {
  const firsts = new Map();
  for (const call of made) if (!firsts.has(call.worker)) firsts.set(call.worker, call);
  const rest = made.filter((call) => firsts.get(call.worker) !== call).map((call) => call.took);
  return {
    first_calls_ms: [...firsts.values()].map((call) => Math.round(call.took)),
    other_calls_ms_sum: Math.round(rest.reduce((sum, took) => sum + took, 0)),
    other_calls_ms_median: rest.length > 0 ? Math.round(median(rest)) : null,
    calls: made.length,
  };
}

async function pool(browser, model, counts) {
  const view = await viewing(model);
  try {
    for (const workers of counts) {
      const opened = await page(browser, `${view.started.url}?inspect&workers=${workers}`);
      await opened.page.waitForFunction(() => (window.occamViewer?.live()?.models ?? 0) >= 1, null, { polling: 50 });
      const cold = await opened.page.evaluate(async () => {
        const drawnAt = performance.now();
        await window.occamViewer.settled();
        return { drawn: drawnAt, settled: performance.now(), calls: window.occamViewer.calls() };
      });
      const warm = [];
      for (let round = 0; round < 3; round += 1) warm.push(await opened.page.evaluate(() => window.occamViewer.remesh()));
      print({
        mode: "pool",
        model: basename(model),
        workers,
        faces: warm[0].faces,
        triangles: warm[0].triangles,
        cold_ms: Math.round(cold.drawn),
        cold_settled_ms: Math.round(cold.settled),
        cold: calls(cold.calls),
        warm_ms: warm.map((w) => Math.round(w.mesh)),
        warm_open_ms: warm.map((w) => Math.round(w.open)),
        warm_least_ms: Math.round(least(warm.map((w) => w.mesh))),
        warm_median_ms: Math.round(median(warm.map((w) => w.mesh))),
        warm: calls(warm.at(-1).calls),
      });
      await opened.close();
    }
  } finally {
    await view.done();
  }
}

async function editing(browser) {
  const { stdout } = await exec("just", ["view-timing", "requests"], { maxBuffer: 1 << 26 });
  const requests = JSON.parse(stdout);
  const site = await serve(DIST);
  try {
    for (const [name, request] of Object.entries(requests)) {
      const times = [];
      for (let round = 0; round < 5; round += 1) {
        const opened = await page(browser, `${site.url}/?inspect&workers=1`);
        await opened.page.waitForFunction(() => window.occamViewer);
        times.push(await opened.page.evaluate(async ([name, request, primitives]) => {
          const viewer = window.occamViewer;
          // The measures measure the primitives: they are made first, untimed.
          if (name === "measure_primitives") await viewer.run(primitives);
          const ran = await viewer.run(request);
          if (!JSON.parse(ran.ran).ok) throw new Error(`${name} did not go through: ${ran.ran.slice(0, 400)}`);
          return viewer.edited();
        }, [name, request, requests.construct_primitives]));
        await opened.close();
      }
      print({ mode: "editing", request: name, least_ms: least(times), median_ms: median(times), ms: times.map(Math.round) });
    }
  } finally {
    site.server.close();
  }
}

async function redraw(browser, model, rounds) {
  const view = await viewing(model);
  try {
    const opened = await page(browser, `${view.started.url}?inspect`);
    await opened.page.waitForFunction(() => (window.occamViewer?.live()?.models ?? 0) >= 1, null, { polling: 50 });
    const times = [];
    for (let round = 1; round <= rounds; round += 1) {
      const steps = [{ op: "box", size: [2, 2, 2], frame: { origin: [-20 - 5 * round, -20, 0] } }];
      const request = `${view.file}.request.json`;
      await writeFile(request, JSON.stringify({ schema: 1, units: { length: "mm", angle: "deg" }, steps }));
      const before = await opened.page.evaluate(() => window.occamViewer.live().models);
      const ran = await occam(["run", "--model", view.file, request]);
      if (!ran.ok) throw new Error(`the run did not go through: ${JSON.stringify(ran).slice(0, 400)}`);
      const saved = (await stat(view.file)).mtimeMs;
      await opened.page.waitForFunction((n) => window.occamViewer.live().models > n, before, { polling: 10 });
      const shown = await opened.page.evaluate(async () => {
        const drawn = Date.now();
        await window.occamViewer.settled();
        return { drawn, settled: Date.now() };
      });
      times.push({ drawn: shown.drawn - saved, settled: shown.settled - saved });
    }
    const settled = times.map((t) => t.settled);
    print({
      mode: "redraw",
      model: basename(model),
      rounds,
      least_ms: Math.round(least(settled)),
      median_ms: Math.round(median(settled)),
      settled_ms: settled.map(Math.round),
      drawn_ms: times.map((t) => Math.round(t.drawn)),
    });
    await opened.close();
  } finally {
    await view.done();
  }
}

/**
 * Each process this one started, and those they started, as its kind (Chromium's `--type`, or `browser`) and the most
 * it has held resident so far, in MB: Linux's `VmHWM`. `occam view`'s server is not among them, since `occam view`
 * leaves it to run apart.
 */
async function peaks() {
  const parent = new Map();
  for (const name of await readdir("/proc")) {
    const stat = /^\d+$/.test(name) ? await readFile(`/proc/${name}/stat`, "utf8").catch(() => null) : null;
    // The parent is the second field after the command, which is in parentheses and may hold spaces.
    if (stat) parent.set(Number(name), Number(stat.slice(stat.lastIndexOf(")") + 2).split(" ")[1]));
  }
  const ours = (pid) => {
    for (let at = parent.get(pid); at !== undefined && at > 1; at = parent.get(at)) if (at === process.pid) return true;
    return false;
  };
  const found = [];
  for (const pid of [...parent.keys()].filter(ours)) {
    const status = await readFile(`/proc/${pid}/status`, "utf8").catch(() => "");
    const line = await readFile(`/proc/${pid}/cmdline`, "utf8").catch(() => "");
    const peak = /VmHWM:\s+(\d+) kB/.exec(status);
    const kind = /--type=([\w-]+)/.exec(line)?.[1] ?? "browser";
    if (peak) found.push({ kind, peak_mb: Math.round(Number(peak[1]) / 1024) });
  }
  return found;
}

async function memory(browser, model, workers) {
  const view = await viewing(model);
  try {
    const opened = await page(browser, `${view.started.url}?inspect&workers=${workers}`);
    const start = Date.now();
    await opened.page.waitForFunction(() => (window.occamViewer?.live()?.models ?? 0) >= 1, null, { polling: 100 });
    await opened.page.evaluate(() => window.occamViewer.settled());
    const drawn = Date.now() - start;
    const before = await opened.page.evaluate(() => window.occamViewer.picked());
    const box = await opened.page.locator("#view").boundingBox();
    await opened.page.mouse.click(box.x + box.width / 2, box.y + box.height / 2);
    await opened.page.waitForFunction((was) => window.occamViewer.picked()?.json !== was, before?.json, { polling: 100 });
    const picked = Date.now() - start - drawn;
    const found = await peaks();
    const most = (kind) => Math.max(0, ...found.filter((p) => p.kind === kind).map((p) => p.peak_mb));
    print({
      mode: "memory",
      model: basename(model),
      workers,
      faces: await opened.page.evaluate(() => window.occamViewer.mesh().faces.length),
      drawn_ms: drawn,
      first_pick_ms: picked,
      renderer_peak_mb: most("renderer"),
      gpu_peak_mb: most("gpu-process"),
      browser_peak_mb: most("browser"),
      processes: found,
    });
    await opened.close();
  } finally {
    await view.done();
  }
}

const [mode, ...rest] = process.argv.slice(2);
if (!OCCAM || WEBGPU.length === 0) {
  console.error("OCCAM and OCCAM_WEBGPU_FLAGS are not set: run this with `just view-timing-browser`");
  process.exit(2);
}
const browser = await chromium.launch({ headless: true, args: WEBGPU });
print({ mode: "browser", chromium: browser.version(), cpus: (await import("node:os")).availableParallelism() });
try {
  if (mode === "pool" && rest[0]) {
    const counts = rest.slice(1).map(Number);
    await pool(browser, rest[0], counts.length > 0 ? counts : [1, 2, 3, 4, 5, 6]);
  } else if (mode === "editing") {
    await editing(browser);
  } else if (mode === "redraw" && rest[0]) {
    await redraw(browser, rest[0], Number(rest[1] ?? 10));
  } else if (mode === "memory" && rest[0]) {
    await memory(browser, rest[0], Number(rest[1] ?? 2));
  } else {
    usage();
  }
} finally {
  await browser.close();
}
