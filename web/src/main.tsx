// The page: Preact draws the controls into `#ui`; the viewer draws into `#view` from its own frame loop.
//
// `?upload-bound=<bytes>` sets how much a frame may upload, and `?inspect` installs what the tests read.

import { effect } from "@preact/signals";
import { render } from "preact";

import { App } from "./controls.tsx";
import { cube } from "./fixture.ts";
import { acquire } from "./gpu.ts";
import { INSPECT_USAGE, Inspector } from "./inspect.ts";
import { attach } from "./input.ts";
import { failure } from "./state.ts";
import { DEFAULT_BOUND } from "./uploads.ts";
import { Viewer } from "./viewer.ts";

function uploadBound(params: URLSearchParams): number {
  const asked = Number(params.get("upload-bound"));
  return Number.isInteger(asked) && asked >= 4 ? asked : DEFAULT_BOUND;
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
    uploadBound: uploadBound(params),
    usage: inspector ? INSPECT_USAGE : 0,
    ...(inspector ? { record: inspector.record } : {}),
  });
  attach(canvas, viewer);
  render(<App actions={viewer} />, ui);
  inspector?.install(viewer, gpu);
  viewer.show(cube());
}

void start();
