// The page's WebGPU device and canvas, or the report that says why there are none.

import { type Report, report } from "./report.ts";

export interface Gpu {
  readonly device: GPUDevice;
  readonly context: GPUCanvasContext;
  readonly format: GPUTextureFormat;
  /** A software or otherwise limited adapter, such as headless Chromium's. */
  readonly fallback: boolean;
}

/** WebGPU from `gpu`, which a browser without it does not have, configured to draw into `canvas`. */
export async function acquire(gpu: GPU | undefined, canvas: HTMLCanvasElement): Promise<Gpu | Report> {
  if (!gpu) return report("view.webgpu-missing");
  const adapter = await gpu.requestAdapter({ powerPreference: "high-performance" }).catch(() => null);
  if (!adapter) return report("view.no-adapter");
  let device: GPUDevice;
  try {
    device = await adapter.requestDevice();
  } catch (error) {
    return report("view.no-device", String(error));
  }
  const context = canvas.getContext("webgpu");
  if (!context) return report("view.no-device", "the canvas has no WebGPU context.");
  const format = gpu.getPreferredCanvasFormat();
  context.configure({ device, format, alphaMode: "opaque" });
  return { device, context, format, fallback: adapter.info.isFallbackAdapter };
}
