// The faces on the GPU: a vertex buffer and an index buffer each, filled through the bounded uploads. A face whose
// bits are unchanged keeps its buffers, so an edit uploads only the faces it touched.

import { pack } from "./layout.ts";
import { type Bounds, type FaceBlock, type Mesh, type Vec3, blocks, bounds, centre, malformed, sameBits, union }
  from "./mesh.ts";
import { type Report, report } from "./report.ts";
import type { Uploads } from "./uploads.ts";

/** A face's block on the GPU. */
export interface GpuFace {
  readonly block: FaceBlock;
  readonly bounds: Bounds | null;
  /** Readable by a vertex fetch and by a storage binding. */
  readonly vertices: GPUBuffer;
  readonly indices: GPUBuffer;
  readonly count: number;
}

export const VERTEX_USAGE = GPUBufferUsage.VERTEX | GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST;
export const INDEX_USAGE = GPUBufferUsage.INDEX | GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST;

function release(face: GpuFace): void {
  face.vertices.destroy();
  face.indices.destroy();
}

export class Scene {
  /** What the GPU's offsets are measured from: the model's centre when the scene last started from nothing. */
  origin: Vec3 | null = null;
  /** Changes whenever the set of faces drawn does. */
  version = 0;
  private readonly drawn = new Map<string, GpuFace>();
  /** Blocks still uploading. Each replaces the face drawn under its id, if any, once its last piece is written. */
  private readonly arriving = new Map<string, GpuFace>();
  private readonly device: GPUDevice;
  private readonly uploads: Uploads;
  private readonly usage: number;

  /** `usage` adds to every buffer's usage, as the inspector's read-back needs. */
  constructor(device: GPUDevice, uploads: Uploads, usage = 0) {
    this.device = device;
    this.uploads = uploads;
    this.usage = usage;
  }

  /** The faces drawn, each complete on the GPU. */
  faces(): IterableIterator<GpuFace> {
    return this.drawn.values();
  }

  drawnFace(id: string): GpuFace | undefined {
    return this.drawn.get(id);
  }

  /** Faces still uploading. */
  get waiting(): number {
    return this.arriving.size;
  }

  /** The newest block of every face, drawn or arriving. */
  private newest(): Map<string, GpuFace> {
    return new Map([...this.drawn, ...this.arriving]);
  }

  /** The box around the faces named, or around the whole model. */
  bounds(ids?: Iterable<string>): Bounds | null {
    const newest = this.newest();
    const faces = ids ? [...ids].flatMap((id) => newest.get(id) ?? []) : [...newest.values()];
    return faces.reduce<Bounds | null>((box, face) => union(box, face.bounds), null);
  }

  /** Shows `mesh` in place of what is shown, uploading only the faces whose bits are not already on the GPU. */
  show(mesh: Mesh): Report | null {
    const problem = malformed(mesh);
    if (problem) return report("view.mesh-malformed", problem);
    const next = blocks(mesh);
    const newest = this.newest();
    const kept = new Set(next.filter((block) => {
      const held = newest.get(block.id);
      return held !== undefined && sameBits(held.block, block);
    }).map((block) => block.id));
    const wanted = new Set(next.map((block) => block.id));
    for (const id of newest.keys()) if (kept.size === 0 || !wanted.has(id)) this.drop(id);
    if (kept.size === 0) {
      const box = next.reduce<Bounds | null>((all, block) => union(all, bounds(block.positions)), null);
      this.origin = box ? centre(box) : null;
    }
    for (const block of next) if (!kept.has(block.id)) this.upload(block);
    return null;
  }

  private drop(id: string): void {
    this.stopArriving(id);
    const face = this.drawn.get(id);
    if (!face) return;
    this.drawn.delete(id);
    release(face);
    this.version += 1;
  }

  private stopArriving(id: string): void {
    const face = this.arriving.get(id);
    if (!face) return;
    this.uploads.cancel(id);
    this.arriving.delete(id);
    release(face);
  }

  private upload(block: FaceBlock): void {
    const packed = pack(block, this.origin ?? [0, 0, 0]);
    const buffer = (size: number, usage: number) =>
      this.device.createBuffer({ size: Math.max(4, size), usage: usage | this.usage });
    const face: GpuFace = {
      block,
      bounds: bounds(block.positions),
      vertices: buffer(packed.byteLength, VERTEX_USAGE),
      indices: buffer(block.indices.byteLength, INDEX_USAGE),
      count: block.indices.length,
    };
    this.stopArriving(block.id);
    this.arriving.set(block.id, face);
    let left = 2;
    const finished = () => {
      left -= 1;
      if (left === 0) this.arrive(face);
    };
    const bytes = (array: Float32Array | Uint32Array) => new Uint8Array(array.buffer, array.byteOffset, array.byteLength);
    this.uploads.add({ face: block.id, kind: "vertices", target: face.vertices, bytes: bytes(packed), finished });
    this.uploads.add({ face: block.id, kind: "indices", target: face.indices, bytes: bytes(block.indices), finished });
  }

  private arrive(face: GpuFace): void {
    const id = face.block.id;
    if (this.arriving.get(id) !== face) return;
    this.arriving.delete(id);
    const old = this.drawn.get(id);
    if (old) release(old);
    this.drawn.set(id, face);
    this.version += 1;
  }
}
