// The "show mesh" overlay (docs/architecture/mesh-and-renderer.md): each drawn face's triangle edges, each once within
// its face, as pairs of indices into the face's own vertex block. The renderer draws them as lines from the vertex
// buffer the face's triangles are drawn from, so they lie on the triangles, at the same `f32` offsets from the page's
// origin. They are built only while the toggle is on, and a face's only once the face is drawn; a face kept keeps its
// lines, so an edit rebuilds only those of the faces it meshed again. They go through the faces' bounded uploads, and
// no more are built in a frame than the queue can write in one. Display only: nothing meshed, picked or selected
// reads them.

import type { GpuFace, Scene } from "./scene.ts";
import type { Uploads } from "./uploads.ts";

export const WIRE_USAGE = GPUBufferUsage.INDEX | GPUBufferUsage.COPY_DST;

/** A face's triangle edges as the page holds them. */
interface Held {
  /** The drawn face whose vertex buffer the pairs index. */
  readonly face: GpuFace;
  /** Two `u32` indices a segment, or none for a face with no triangles. */
  readonly pairs: GPUBuffer | null;
  /** Indices, two a segment. */
  readonly count: number;
  /** Whether its last piece is written. */
  ready: boolean;
}

/** A face's triangle edges ready to draw. */
export interface WireFace {
  readonly face: GpuFace;
  readonly pairs: GPUBuffer;
  readonly count: number;
}

/**
 * Each edge of the triangles `indices`, three a triangle, once: its two ends, the smaller first, in the order first
 * met. The key `lo · n + hi` is exact while a block has under 2^26 vertices, which its cap of 2^20 triangles keeps it.
 */
export function edges(indices: Uint32Array): Uint32Array {
  const n = indices.reduce((most, index) => Math.max(most, index + 1), 0);
  const seen = new Set<number>();
  const out = new Uint32Array(indices.length * 2);
  let at = 0;
  for (let t = 0; t < indices.length; t += 3) {
    for (let k = 0; k < 3; k += 1) {
      const [a, b] = [indices[t + k], indices[t + ((k + 1) % 3)]];
      const [lo, hi] = a < b ? [a, b] : [b, a];
      const key = lo * n + hi;
      if (seen.has(key)) continue;
      seen.add(key);
      out[at] = lo;
      out[at + 1] = hi;
      at += 2;
    }
  }
  return out.slice(0, at);
}

export class Wire {
  /** Changes whenever the set of lines ready to draw does. */
  version = 0;
  /** Each face's lines, by its id, uploaded or still uploading. */
  private readonly held = new Map<string, Held>();

  constructor(
    private readonly device: GPUDevice,
    private readonly uploads: Uploads,
    /** Added to every buffer's usage, as the inspector's read-back needs. */
    private readonly usage = 0,
  ) {}

  /** The lines ready to draw: each face's that has any. */
  drawn(): WireFace[] {
    const drawn = [...this.held.values()].filter((held) => held.ready);
    return drawn.flatMap(({ face, pairs, count }) => (pairs ? [{ face, pairs, count }] : []));
  }

  /**
   * Makes the lines held match `scene`'s faces while `shown`, and holds none while not: drops a face's lines once the
   * face is gone or replaced, then builds those of the faces drawn without any, while the queue holds under a frame.
   */
  sync(scene: Scene, shown: boolean): void {
    for (const [id, held] of this.held) if (!shown || scene.drawnFace(id) !== held.face) this.drop(id, held);
    if (!shown) return;
    let room = this.uploads.bound - this.uploads.pending;
    for (const face of scene.faces()) {
      if (room <= 0) return;
      if (!this.held.has(face.block.id)) room -= this.build(face);
    }
  }

  /** Whether the lines held are not yet what `scene` and `shown` want: some are still to build, write or drop. */
  waiting(scene: Scene, shown: boolean): boolean {
    const wanted = shown ? [...scene.faces()] : [];
    return wanted.length !== this.held.size || wanted.some((face) => {
      const held = this.held.get(face.block.id);
      return held?.face !== face || !held.ready;
    });
  }

  /** Builds `face`'s lines and queues their upload; their size in bytes. */
  private build(face: GpuFace): number {
    const id = face.block.id;
    const pairs = edges(face.block.indices);
    const buffer = pairs.length > 0
      ? this.device.createBuffer({ size: pairs.byteLength, usage: WIRE_USAGE | this.usage })
      : null;
    const held: Held = { face, pairs: buffer, count: pairs.length, ready: buffer === null };
    this.held.set(id, held);
    if (buffer) {
      const bytes = new Uint8Array(pairs.buffer, pairs.byteOffset, pairs.byteLength);
      this.uploads.add({ face: id, kind: "wire", target: buffer, bytes, finished: () => this.arrive(held) });
    }
    return pairs.byteLength;
  }

  private arrive(held: Held): void {
    if (this.held.get(held.face.block.id) !== held) return;
    held.ready = true;
    this.version += 1;
  }

  private drop(id: string, held: Held): void {
    this.held.delete(id);
    this.uploads.cancel(id, ["wire"]);
    held.pairs?.destroy();
    if (held.ready) this.version += 1;
  }
}
