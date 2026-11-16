// The viewer: the scene, the camera and the frame loop, all outside the component tree. The controls reach it only
// through its methods, and it tells them what changed only through signals. It draws the selection and the highlight
// in a colour each, the faces through their marks and the edges as lines, and builds a click's ray from its camera.

import { effect } from "@preact/signals";

import { Camera, FIELD_OF_VIEW } from "./camera.ts";
import type { Gpu } from "./gpu.ts";
import { FrameLoop } from "./loop.ts";
import { type Mesh, type Vec3, centre } from "./mesh.ts";
import { CAMERA_FLOATS, HIGHLIGHTED, LINE_FLOATS, Renderer, SELECTED } from "./renderer.ts";
import { report } from "./report.ts";
import { Scene } from "./scene.ts";
import { failure, highlight, notice, selection, status } from "./state.ts";
import { type Piece, Uploads } from "./uploads.ts";

/** What a click asks a mesh worker: its ray, the tolerance a millimetre along it, and how far along to take it. */
export interface Aim {
  /** Where the ray starts and its direction, in millimetres in the model's frame. */
  readonly ray: readonly number[];
  readonly slope: number;
  readonly fallback: number;
}

/** An edge outlined: its id, and its samples, three numbers a point. */
export type Outline = readonly [id: string, points: Float64Array];

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
  /** The edges outlined, and the origin their lines were packed against, if they are packed. */
  outlines: readonly Outline[] = [];
  private packed: Vec3 | null = null;
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
    effect(() => {
      void [selection.value, highlight.value];
      this.packed = null;
      this.loop.wake();
    });
  }

  /** Whether `id` is selected, highlighted, or both. */
  mark(id: string): number {
    return (selection.peek().includes(id) ? SELECTED : 0) | (highlight.peek().includes(id) ? HIGHLIGHTED : 0);
  }

  /**
   * What a click at the point `(x, y)` of the window asks: the ray through it from the eye, in `f64` in the model's
   * frame, and a tolerance of `pixels` pixels at the depth of what it hits, as a length a millimetre along it, with
   * the distance along it of the camera's target, where it hits nothing.
   */
  rayAt(x: number, y: number, pixels: number): Aim {
    const box = this.canvas.getBoundingClientRect();
    const across = (2 * (x - box.left)) / Math.max(1, box.width) - 1;
    const up = 1 - (2 * (y - box.top)) / Math.max(1, box.height);
    const { from, direction } = this.camera.ray(across, up, this.aspect());
    const length = Math.hypot(...direction);
    const pixel = (2 * Math.tan(FIELD_OF_VIEW / 2)) / Math.max(1, box.height);
    return { ray: [...from, ...direction], slope: (pixels * pixel) / length, fallback: this.camera.distance * length };
  }

  /** Draws `edges` as lines through their samples, over the faces, each in its mark's colour. */
  outline(edges: readonly Outline[]): void {
    this.outlines = edges;
    this.packed = null;
    this.loop.wake();
  }

  /** The outlines as line segments, `LINE_FLOATS` a vertex, against the scene's origin. */
  private lines(origin: Vec3): Float32Array<ArrayBuffer> {
    const segments = this.outlines.reduce((sum, [, points]) => sum + points.length / 3 - 1, 0);
    const out = new Float32Array(segments * 2 * LINE_FLOATS);
    let at = 0;
    for (const [id, points] of this.outlines) {
      const mark = this.mark(id);
      for (let k = 3; k < points.length; k += 3) {
        for (const p of [k - 3, k]) {
          out.set([points[p] - origin[0], points[p + 1] - origin[1], points[p + 2] - origin[2], mark], at);
          at += LINE_FLOATS;
        }
      }
    }
    return out;
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
    const origin = this.scene.origin ?? [0, 0, 0];
    if (this.packed === null || this.packed.some((x, axis) => x !== origin[axis])) {
      this.renderer.outline(this.lines(origin));
      this.packed = origin;
    }
    this.renderer.draw(this.scene, this.uniform(), (id) => this.mark(id));
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
