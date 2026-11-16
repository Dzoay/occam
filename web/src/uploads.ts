// Uploads bounded per frame. In P12, large `writeBuffer` traffic lost the GPU device outright until it was bounded,
// and `writeBuffer` cost the main thread 0.5 to 0.6 ms a MB. A block larger than a frame's bound arrives over several
// frames, in order.

/** Bytes a frame may upload unless the page is told otherwise: about 2 ms of the main thread at P12's rate. */
export const DEFAULT_BOUND = 4 * 1024 * 1024;
/** The largest single `writeBuffer` call: P12's final run wrote vertex data in calls of at most this. */
export const CALL_LIMIT = 2 * 1024 * 1024;

export type Kind = "vertices" | "indices";

/** One `writeBuffer` call, as the page's inspector records it. */
export interface Piece {
  readonly frame: number;
  readonly face: string;
  readonly kind: Kind;
  readonly offset: number;
  readonly size: number;
}

/** A buffer to fill from `bytes`; `finished` runs once its last piece is written. */
export interface Job {
  readonly face: string;
  readonly kind: Kind;
  readonly target: GPUBuffer;
  readonly bytes: Uint8Array;
  readonly finished: () => void;
}

interface Queued extends Job {
  written: number;
  cancelled: boolean;
}

export class Uploads {
  /** Bytes a frame may upload: a multiple of 4, as `writeBuffer` wants. */
  readonly bound: number;
  private readonly queued: Queued[] = [];
  private readonly record: ((piece: Piece) => void) | undefined;

  constructor(bound: number, record?: (piece: Piece) => void) {
    this.bound = Math.max(4, Math.floor(bound / 4) * 4);
    this.record = record;
  }

  add(job: Job): void {
    this.queued.push({ ...job, written: 0, cancelled: false });
  }

  /** Drops what is left to write for `face`, whose buffers are going. */
  cancel(face: string): void {
    for (const job of this.queued) if (job.face === face) job.cancelled = true;
  }

  /** Bytes still to write. */
  get pending(): number {
    return this.queued.reduce((sum, job) => sum + (job.cancelled ? 0 : job.bytes.byteLength - job.written), 0);
  }

  /** Writes up to the bound, oldest first, and says how many bytes it wrote. */
  flush(queue: GPUQueue, frame: number): number {
    let budget = this.bound;
    this.settle();
    while (budget > 0 && this.queued.length > 0) {
      const job = this.queued[0];
      const size = Math.min(job.bytes.byteLength - job.written, budget, CALL_LIMIT);
      queue.writeBuffer(job.target, job.written, job.bytes.buffer, job.bytes.byteOffset + job.written, size);
      this.record?.({ frame, face: job.face, kind: job.kind, offset: job.written, size });
      job.written += size;
      budget -= size;
      this.settle();
    }
    return this.bound - budget;
  }

  /**
   * Drops the jobs at the front that are cancelled or written, finishing the written ones, so a face is drawn in the
   * frame its last piece is written. Every size is a multiple of 4, so the budget stays one too.
   */
  private settle(): void {
    while (this.queued.length > 0) {
      const job = this.queued[0];
      if (!job.cancelled && job.written < job.bytes.byteLength) return;
      this.queued.shift();
      if (!job.cancelled) job.finished();
    }
  }
}
