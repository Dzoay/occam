// The page: Preact draws the controls into `#ui`; the viewer draws into `#view` from its own frame loop; the kernel
// runs in workers, an editing worker and a pool of mesh workers, and never on this thread. Served by `occam view`,
// which marks its `<html>` as `data-live`, the page follows the model file through the server's events.
//
// `?upload-bound=<bytes>` sets how much a frame may upload, `?workers=<n>` how many mesh workers there are,
// `?deflection=<mm>` and `?angle=<rad>` what each model is meshed to, and `?inspect` installs what the tests read.

import { effect } from "@preact/signals";
import { render } from "preact";

import { App } from "./controls.tsx";
import { acquire } from "./gpu.ts";
import { INSPECT_USAGE, Inspector } from "./inspect.ts";
import { attach } from "./input.ts";
import { Kernel } from "./kernel.ts";
import { Live } from "./live.ts";
import type { Limits, Meshing } from "./pool.ts";
import { report } from "./report.ts";
import { failure, notice, refusals } from "./state.ts";
import { DEFAULT_BOUND } from "./uploads.ts";
import { Viewer } from "./viewer.ts";

/** What a model is meshed to unless the address says otherwise: `occam mesh`'s angle, and a tenth of a millimetre. */
const LIMITS: Limits = { deflection: 0.1, angle: 0.35 };
/** At most as many mesh workers as P10 measured, leaving a core for the page and the editing worker. */
const MOST_WORKERS = 6;

/** The number `name` in `params`, if it passes `fits`; otherwise `fallback`. */
function asked(params: URLSearchParams, name: string, fits: (n: number) => boolean, fallback: number): number {
  const raw = params.get(name);
  const n = raw === null ? NaN : Number(raw);
  return fits(n) ? n : fallback;
}

const positive = (n: number) => Number.isFinite(n) && n > 0;

/**
 * Runs `request` in the editing worker and shows the model it made, once the pool has meshed it: every face that
 * meshed, and, if a call threw, `view.mesh-failed` naming the faces it held, unless the mesh broke the contract.
 */
async function follow(kernel: Kernel, viewer: Viewer, limits: Limits, request: string): Promise<[string, Meshing | null]> {
  const { ran, meshing } = await kernel.run(request, limits);
  const meshed = meshing && (await meshing);
  if (meshed) {
    viewer.show(meshed.mesh);
    refusals.value = meshed.reports;
    const [first] = meshed.failed;
    if (first && !notice.peek()) notice.value = report("view.mesh-failed", first.reason, meshed.failed.flatMap((f) => f.faces));
  }
  return [ran, meshed];
}

async function start(): Promise<void> {
  const params = new URLSearchParams(location.search);
  const canvas = document.getElementById("view");
  const ui = document.getElementById("ui");
  if (!(canvas instanceof HTMLCanvasElement) || !ui) throw new Error("the page has no #view canvas or #ui");
  effect(() => {
    canvas.hidden = failure.value !== null;
  });
  const gpu = await acquire((navigator as { gpu?: GPU }).gpu, canvas);
  if ("code" in gpu) {
    failure.value = gpu;
    render(<App actions={null} />, ui);
    return;
  }
  const inspector = params.has("inspect") ? new Inspector() : null;
  const viewer = new Viewer(gpu, canvas, {
    uploadBound: asked(params, "upload-bound", (n) => Number.isInteger(n) && n >= 4, DEFAULT_BOUND),
    usage: inspector ? INSPECT_USAGE : 0,
    ...(inspector ? { record: inspector.record } : {}),
  });
  const spare = Math.max(1, Math.min(MOST_WORKERS, (navigator.hardwareConcurrency || 2) - 1));
  const kernel = new Kernel(asked(params, "workers", (n) => Number.isInteger(n) && n >= 1, spare), new URL(import.meta.url));
  const limits: Limits = {
    deflection: asked(params, "deflection", positive, LIMITS.deflection),
    angle: asked(params, "angle", positive, LIMITS.angle),
  };
  attach(canvas, viewer);
  render(<App actions={viewer} />, ui);
  const live = document.documentElement.hasAttribute("data-live") ? new Live(kernel, viewer, limits) : null;
  live?.start();
  inspector?.install(viewer, gpu, {
    run: (request) => follow(kernel, viewer, limits, request),
    calls: () => kernel.pool.last,
    workers: kernel.pool.size,
    live: () => live && { digest: live.drawn?.digest ?? null, models: live.models },
  });
}

void start();
