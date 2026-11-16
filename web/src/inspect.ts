// What the tests read, installed as `window.occamViewer` only when the page's address has `?inspect`. CI has no GPU, so
// the tests assert buffers, not pixels: they read back what the page uploaded, the camera it drew with and each face's
// mark. Nothing else in the page maps a GPU buffer: a pick never does.

import type { Gpu } from "./gpu.ts";
import type { Picked } from "./pick.ts";
import type { Mesh, Vec3 } from "./mesh.ts";
import type { Call, Failed, Meshing } from "./pool.ts";
import type { KernelReport } from "./protocol.ts";
import { LINE_FLOATS } from "./renderer.ts";
import { type Code, REMEDIES, type RemedyKind, TEMPLATES } from "./report.ts";
import { highlight, selection } from "./state.ts";
import type { Piece } from "./uploads.ts";
import type { Viewer } from "./viewer.ts";

/** The usage every buffer gets while inspecting, so that it can be copied out. */
export const INSPECT_USAGE = GPUBufferUsage.COPY_SRC;

export interface FaceState {
  readonly id: string;
  readonly vertices: number;
  readonly indices: number;
  readonly vertexUsage: number;
  readonly indexUsage: number;
}

/** What the page can report: each code, its wording and its fix, for the test that holds them to the catalogue. */
export interface ReportEntry {
  readonly code: Code;
  readonly template: string;
  readonly remedy: RemedyKind | null;
  readonly remedyText: string | null;
}

/** What the page's kernel gives the inspector. */
export interface KernelApi {
  /** Runs a request in the editing worker and shows the model it made once the pool has meshed it. */
  run(request: string): Promise<[string, Meshing | null]>;
  /** The calls the last model was cut into, and which worker meshed each. */
  calls(): readonly Call[];
  readonly workers: number;
  /** What the page following `occam view` has drawn, if it follows one. */
  live(): Following | null;
  /** The last pick's answer. */
  picked(): Picked | null;
  /** Posts a selection to `occam view` as a click does, on a page that follows one. */
  post(digest: string, ids: readonly string[]): Promise<void>;
}

/** What a page following `occam view` has drawn: the digest of the model drawn last, and how many it has drawn. */
export interface Following {
  readonly digest: string | null;
  readonly models: number;
}

/** What a request gave, as the tests read it. */
export interface Followed {
  /** The run's one JSON form. */
  readonly ran: string;
  /** Whether a model it made was meshed and shown, what refused any of it, and why any call failed. */
  readonly shown: boolean;
  readonly reports: readonly KernelReport[];
  readonly failed: readonly Failed[];
}

export interface InspectorApi {
  /** Runs `request` in the editing worker and resolves once the model it made, if any, is meshed and shown. */
  run(request: string): Promise<Followed>;
  calls(): Call[];
  workers(): number;
  /** What the page following `occam view` has drawn, or null on a page no server follows. */
  live(): Following | null;
  show(mesh: Mesh): void;
  select(ids: readonly string[]): void;
  /** What is selected and what is highlighted, and the last pick's answer. */
  selection(): readonly string[];
  highlight(): readonly string[];
  picked(): Picked | null;
  /** Posts a selection to `occam view` as a click does, on a page that follows one. */
  post(digest: string, ids: readonly string[]): Promise<void>;
  /** Puts the camera where `view` says. */
  look(view: { target: Vec3; distance: number; yaw: number; pitch: number }): void;
  /** Each face's mark as the GPU holds it, read back: 1 selected, 2 highlighted, 3 both. */
  readMarks(): Promise<Record<string, number>>;
  /** The edges outlined, and the lines' vertices as the GPU holds them, read back. */
  readLines(): Promise<{ edges: string[]; vertices: Float32Array }>;
  mesh(): Mesh | null;
  origin(): Vec3 | null;
  bound(): number;
  /** The faces drawn. */
  faces(): FaceState[];
  uploads(): Piece[];
  clearUploads(): void;
  frames(): number;
  /** GPU errors nobody caught. */
  errors(): string[];
  camera(): { target: Vec3; distance: number; yaw: number; pitch: number };
  /** Resolves once nothing is left to upload and the GPU has finished a frame drawn after that. */
  settled(): Promise<void>;
  readFace(id: string): Promise<{ vertices: Float32Array; indices: Uint32Array } | null>;
  readCamera(): Promise<Float32Array>;
  /** A face's vertex block as a compute pass reads it through a storage binding, copied out. */
  readThroughStorage(id: string): Promise<Float32Array | null>;
  reports(): ReportEntry[];
  /** Destroys the GPU device, as a lost one would be lost. */
  loseDevice(): void;
}

declare global {
  interface Window {
    occamViewer?: InspectorApi;
  }
}

const COPY = /* wgsl */ `
struct Vertex {
  offset_u: vec4f,
  normal_v: vec4f,
}
@group(0) @binding(0) var<storage, read> block: array<Vertex>;
@group(0) @binding(1) var<storage, read_write> copied: array<Vertex>;

@compute @workgroup_size(64)
fn main(@builtin(global_invocation_id) id: vec3u) {
  if (id.x < arrayLength(&block)) {
    copied[id.x] = block[id.x];
  }
}
`;

const nextFrame = () => new Promise<void>((resolve) => requestAnimationFrame(() => resolve()));

async function read(device: GPUDevice, source: GPUBuffer, size: number): Promise<ArrayBuffer> {
  const staging = device.createBuffer({ size, usage: GPUBufferUsage.MAP_READ | GPUBufferUsage.COPY_DST });
  const encoder = device.createCommandEncoder();
  encoder.copyBufferToBuffer(source, 0, staging, 0, size);
  device.queue.submit([encoder.finish()]);
  await staging.mapAsync(GPUMapMode.READ);
  const copy = staging.getMappedRange().slice(0);
  staging.destroy();
  return copy;
}

/** The block copied out through a storage binding, or `null` if the device refused to bind it so. */
async function throughStorage(device: GPUDevice, block: GPUBuffer, size: number): Promise<ArrayBuffer | null> {
  device.pushErrorScope("validation");
  const pipeline = device.createComputePipeline({
    layout: "auto",
    compute: { module: device.createShaderModule({ code: COPY }), entryPoint: "main" },
  });
  const copied = device.createBuffer({ size, usage: GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_SRC });
  const bindGroup = device.createBindGroup({
    layout: pipeline.getBindGroupLayout(0),
    entries: [{ binding: 0, resource: { buffer: block, size } }, { binding: 1, resource: { buffer: copied } }],
  });
  const encoder = device.createCommandEncoder();
  const pass = encoder.beginComputePass();
  pass.setPipeline(pipeline);
  pass.setBindGroup(0, bindGroup);
  pass.dispatchWorkgroups(Math.ceil(size / 32 / 64));
  pass.end();
  device.queue.submit([encoder.finish()]);
  const refused = await device.popErrorScope();
  const out = refused ? null : await read(device, copied, size);
  copied.destroy();
  return out;
}

export class Inspector {
  private pieces: Piece[] = [];
  private readonly uncaught: string[] = [];

  readonly record = (piece: Piece): void => {
    this.pieces.push(piece);
  };

  install(viewer: Viewer, gpu: Gpu, kernel: KernelApi): void {
    const { device } = gpu;
    device.addEventListener("uncapturederror", (event) => this.uncaught.push(event.error.message));
    window.occamViewer = {
      run: async (request) => {
        const [ran, meshed] = await kernel.run(request);
        return { ran, shown: meshed !== null, reports: meshed?.reports ?? [], failed: meshed?.failed ?? [] };
      },
      calls: () => kernel.calls().map((call) => ({ ...call })),
      workers: () => kernel.workers,
      live: () => kernel.live(),
      show: (mesh) => viewer.show(mesh),
      select: (ids) => viewer.select(ids),
      selection: () => selection.peek(),
      highlight: () => highlight.peek(),
      picked: () => kernel.picked(),
      post: (digest, ids) => kernel.post(digest, ids),
      look: (view) => {
        Object.assign(viewer.camera, view);
        viewer.loop.wake();
      },
      readMarks: async () => {
        const { marks, marked } = viewer.renderer;
        const held = new Uint32Array(await read(device, marks, marks.size));
        return Object.fromEntries(marked.map((id, k) => [id, held[k]]));
      },
      readLines: async () => {
        const { lines, lineVertices } = viewer.renderer;
        const size = lineVertices * LINE_FLOATS * 4;
        const vertices = lines && size > 0 ? new Float32Array(await read(device, lines, size)) : new Float32Array(0);
        return { edges: viewer.outlines.map(([id]) => id), vertices };
      },
      mesh: () => viewer.mesh,
      origin: () => viewer.scene.origin,
      bound: () => viewer.uploads.bound,
      faces: () =>
        [...viewer.scene.faces()].map((face) => ({
          id: face.block.id,
          vertices: face.block.positions.length / 3,
          indices: face.count,
          vertexUsage: face.vertices.usage,
          indexUsage: face.indices.usage,
        })),
      uploads: () => [...this.pieces],
      clearUploads: () => {
        this.pieces = [];
      },
      frames: () => viewer.loop.frames,
      errors: () => [...this.uncaught],
      camera: () => {
        const { target, distance, yaw, pitch } = viewer.camera;
        return { target, distance, yaw, pitch };
      },
      settled: async () => {
        while (viewer.uploads.pending > 0 || viewer.scene.waiting > 0) await nextFrame();
        viewer.loop.wake();
        await nextFrame();
        await nextFrame();
        await device.queue.onSubmittedWorkDone();
      },
      readFace: async (id) => {
        const face = viewer.scene.drawnFace(id);
        if (!face) return null;
        const vertices = new Float32Array(await read(device, face.vertices, face.block.positions.length / 3 * 32));
        const indices = new Uint32Array(await read(device, face.indices, face.count * 4));
        return { vertices, indices };
      },
      readCamera: async () => new Float32Array(await read(device, viewer.renderer.camera, viewer.renderer.camera.size)),
      readThroughStorage: async (id) => {
        const face = viewer.scene.drawnFace(id);
        if (!face || face.count === 0) return null;
        const copied = await throughStorage(device, face.vertices, face.block.positions.length / 3 * 32);
        return copied && new Float32Array(copied);
      },
      reports: () =>
        Object.entries(TEMPLATES).map(([code, [template, remedy]]) => ({
          code: code as Code,
          template,
          remedy,
          remedyText: remedy ? REMEDIES[remedy] : null,
        })),
      loseDevice: () => device.destroy(),
    };
  }
}
