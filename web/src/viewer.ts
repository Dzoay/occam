// The viewer: the scene, the camera and the frame loop, all outside the component tree. The controls reach it only
// through its methods, and it tells them what changed only through signals.

import { Camera } from "./camera.ts";
import type { Gpu } from "./gpu.ts";
import { FrameLoop } from "./loop.ts";
import { type Mesh, centre } from "./mesh.ts";
import { CAMERA_FLOATS, Renderer } from "./renderer.ts";
import { report } from "./report.ts";
import { Scene } from "./scene.ts";
import { failure, notice, selection, status } from "./state.ts";
import { type Piece, Uploads } from "./uploads.ts";

export interface Options {
  /** Bytes a frame may upload. */
  readonly uploadBound: number;
  /** Added to every buffer's usage, as the inspector's read-back needs. */
  readonly usage: number;
  /** Told of every upload, for the inspector. */
  readonly record?: (piece: Piece) => void;
}

export class Viewer {
  readonly camera = new Camera();
  readonly uploads: Uploads;
  readonly scene: Scene;
  readonly renderer: Renderer;
  readonly loop: FrameLoop;
  /** The last mesh shown. */
  mesh: Mesh | null = null;
  private readonly gpu: Gpu;
  private readonly canvas: HTMLCanvasElement;
  private fitted = false;

  constructor(gpu: Gpu, canvas: HTMLCanvasElement, options: Options) {
    this.gpu = gpu;
    this.canvas = canvas;
    this.uploads = new Uploads(options.uploadBound, options.record);
    this.scene = new Scene(gpu.device, this.uploads, options.usage);
    this.renderer = new Renderer(gpu, options.usage);
    this.loop = new FrameLoop((frame) => this.step(frame));
    status.value = { ...status.peek(), fallback: gpu.fallback };
    void gpu.device.lost.then((lost) => {
      failure.value = report("view.device-lost", lost.message || lost.reason);
    });
  }

  /** Shows `mesh`, uploading only the faces not already on the GPU, and fits the first model to the window. */
  show(mesh: Mesh): void {
    const problem = this.scene.show(mesh);
    notice.value = problem;
    if (problem) return;
    this.mesh = mesh;
    if (!this.fitted) this.fit();
    this.loop.wake();
  }

  select(ids: readonly string[]): void {
    selection.value = ids;
  }

  aspect(): number {
    return Math.max(1, this.canvas.width) / Math.max(1, this.canvas.height);
  }

  fit(): void {
    const box = this.scene.bounds();
    if (!box) return;
    this.resize();
    this.camera.fit(box, this.aspect());
    this.fitted = true;
    this.loop.wake();
  }

  /** Puts the centre of the selection's box in the middle of the view, at the same distance. */
  centreOnSelection(): void {
    const box = this.scene.bounds(selection.value);
    if (!box) return;
    this.camera.centre(centre(box));
    this.loop.wake();
  }

  orbit(yaw: number, pitch: number): void {
    this.camera.orbit(yaw, pitch);
    this.loop.wake();
  }

  pan(right: number, up: number): void {
    this.camera.pan(right, up);
    this.loop.wake();
  }

  zoom(factor: number): void {
    this.camera.zoom(factor);
    this.loop.wake();
  }

  /** The camera's uniform as the GPU gets it. */
  uniform(): Float32Array {
    const out = new Float32Array(CAMERA_FLOATS);
    out.set(this.camera.viewProjection(this.scene.origin ?? [0, 0, 0], this.aspect(), this.scene.bounds()));
    out.set(this.camera.backward(), 16);
    return out;
  }

  private resize(): void {
    const scale = window.devicePixelRatio || 1;
    const width = Math.max(1, Math.round(this.canvas.clientWidth * scale));
    const height = Math.max(1, Math.round(this.canvas.clientHeight * scale));
    if (this.canvas.width !== width) this.canvas.width = width;
    if (this.canvas.height !== height) this.canvas.height = height;
  }

  /** One frame: the bounded uploads, then the draw. Says whether uploads are still pending. */
  private step(frame: number): boolean {
    if (failure.peek()) return false;
    this.resize();
    this.uploads.flush(this.gpu.device.queue, frame);
    this.renderer.draw(this.scene, this.uniform());
    const pending = this.uploads.pending;
    let [faces, triangles] = [0, 0];
    for (const face of this.scene.faces()) {
      faces += 1;
      triangles += face.count / 3;
    }
    const now = status.peek();
    if (now.faces !== faces || now.triangles !== triangles || now.pending !== pending) {
      status.value = { faces, triangles, pending, fallback: this.gpu.fallback };
    }
    return pending > 0 || this.scene.waiting > 0;
  }
}
