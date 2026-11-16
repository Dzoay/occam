// The mesh workers, kept alive between requests (P10). Each model the editing worker publishes reaches every worker as
// its own copy, and its faces are cut into about 64 ranges, dealt out one at a time to whichever worker comes free, so
// uneven faces balance and no call runs long. The parts come back as transferred flat arrays and are laid end to end in
// id order, each numbered from its own start, so no index is rewritten. That assembly is display-only: each call's
// output is deterministic, and the parts laid end to end are the buffers `occam mesh` gives the same model.

import type { FaceRange, Mesh } from "./mesh.ts";
import type { FromMesher, KernelReport, Part, ToMesher } from "./protocol.ts";

/** About how many calls a job is cut into (P10). */
export const CALLS = 64;

/** What a mesh is made to: a chordal deflection in millimetres and an angle in radians. */
export interface Limits {
  readonly deflection: number;
  readonly angle: number;
}

/** A range of faces, counted in id order, and the worker that meshed it. */
export interface Call {
  readonly first: number;
  readonly end: number;
  worker: number | null;
}

/** A call that threw instead of meshing its range: the faces it held, by id, and what it said. */
export interface Failed {
  readonly faces: readonly string[];
  readonly reason: string;
}

/** A model meshed: the parts laid end to end, what refused any of it, and how it was dealt out. */
export interface Meshing {
  readonly model: number;
  readonly mesh: Mesh;
  readonly reports: readonly KernelReport[];
  readonly calls: readonly Call[];
  /** Each call that could not mesh its range at all, in id order: a fault in the page or a worker, never a face refused. */
  readonly failed: readonly Failed[];
}

/** `faces` faces cut into at most `calls` ranges of one size, the last perhaps shorter. */
export function cut(faces: number, calls = CALLS): Call[] {
  const size = Math.max(1, Math.ceil(faces / calls));
  return Array.from({ length: Math.ceil(faces / size) }, (_, k) => ({
    first: k * size,
    end: Math.min(faces, (k + 1) * size),
    worker: null,
  }));
}

function joined<T extends Float64Array<ArrayBuffer> | Uint32Array<ArrayBuffer>>(arrays: T[], make: (n: number) => T): T {
  const out = make(arrays.reduce((sum, array) => sum + array.length, 0));
  arrays.reduce((at, array) => (out.set(array, at), at + array.length), 0);
  return out;
}

/** `parts`, in id order, laid end to end: each face's block and triangles moved past the parts before it. */
export function assemble(parts: readonly Part[]): { mesh: Mesh; reports: KernelReport[] } {
  const faces: FaceRange[] = [];
  let [vertices, triangles] = [0, 0];
  for (const part of parts) {
    for (let row = 0; row + 6 <= part.table.length; row += 6) {
      const [slot, generation, first, count, start, many] = part.table.subarray(row, row + 6);
      faces.push({
        id: `face:${slot}/${generation}`,
        vertices: { first: vertices + first, count },
        triangles: { first: triangles + start, count: many },
      });
    }
    [vertices, triangles] = [vertices + part.positions.length / 3, triangles + part.indices.length / 3];
  }
  const each = <K extends "positions" | "normals" | "uvs">(key: K) => parts.map((part) => part[key]);
  const mesh: Mesh = {
    positions: joined(each("positions"), (n) => new Float64Array(n)),
    normals: joined(each("normals"), (n) => new Float64Array(n)),
    uvs: joined(each("uvs"), (n) => new Float64Array(n)),
    indices: joined(parts.map((part) => part.indices), (n) => new Uint32Array(n)),
    faces,
  };
  // An edge two parts' faces use is refused in both: keep the first.
  const seen = new Set<string>();
  const reports = parts.flatMap((part) => part.reports).filter((report) => {
    const key = JSON.stringify(report);
    return !seen.has(key) && (seen.add(key), true);
  });
  return { mesh, reports };
}

/** One model's faces being meshed. */
class Job {
  readonly parts: (Part | null)[];
  /** Why each call threw, if it did. */
  readonly failures: (string | null)[];
  next = 0;
  left: number;

  constructor(
    readonly model: number,
    /** Each face's slot and generation, in id order. */
    readonly ids: Uint32Array,
    readonly calls: Call[],
    readonly limits: Limits,
    readonly finish: (meshing: Meshing | null) => void,
  ) {
    this.parts = calls.map(() => null);
    this.failures = calls.map(() => null);
    this.left = calls.length;
  }

  /** The calls that threw, in id order, each with its faces named. */
  failed(): Failed[] {
    const name = (k: number) => `face:${this.ids[2 * k]}/${this.ids[2 * k + 1]}`;
    return this.calls.flatMap(({ first, end }, call) => {
      const reason = this.failures[call];
      if (reason === null) return [];
      return [{ faces: Array.from({ length: end - first }, (_, k) => name(first + k)), reason }];
    });
  }
}

export class Pool {
  /** The calls the last job was cut into, and which worker meshed each, for the inspector. */
  last: readonly Call[] = [];
  private readonly workers: Worker[];
  /** The model and call each worker is meshing, or null when it is free. */
  private readonly busy: (readonly [model: number, call: number] | null)[];
  /** Why each worker stopped, if it did: a script that did not load, say. It is dealt nothing more. */
  private readonly stopped: (string | null)[];
  private model = 0;
  private job: Job | null = null;

  constructor(size: number, script: URL) {
    this.workers = Array.from({ length: size }, (_, k) => {
      const worker = new Worker(script, { type: "module", name: `mesh ${k}` });
      worker.onmessage = (event: MessageEvent<FromMesher>) => this.answered(k, event.data);
      worker.onerror = (event) => this.stop(k, event.message || "the worker stopped");
      return worker;
    });
    this.busy = this.workers.map(() => null);
    this.stopped = this.workers.map(() => null);
  }

  get size(): number {
    return this.workers.length;
  }

  /**
   * Gives each worker its own copy of a model, `twins` one each, and meshes its faces, whose slots and generations are
   * `ids`, to `limits`. Resolves with the parts laid end to end, or with null if another model came first.
   */
  mesh(twins: readonly ArrayBuffer[], ids: Uint32Array, limits: Limits): Promise<Meshing | null> {
    if (twins.length !== this.size) throw new Error(`${twins.length} copies of the model for ${this.size} workers`);
    this.model += 1;
    this.job?.finish(null);
    this.workers.forEach((worker, k) => this.post(worker, { kind: "load", model: this.model, twin: twins[k] }, [twins[k]]));
    return new Promise((resolve) => {
      const job = new Job(this.model, ids, cut(ids.length / 2), limits, resolve);
      this.job = job;
      this.last = job.calls;
      if (job.left === 0) this.done(job);
      this.workers.forEach((_, k) => this.busy[k] === null && this.deal(k));
    });
  }

  private post(worker: Worker, message: ToMesher, transfer: Transferable[] = []): void {
    worker.postMessage(message, transfer);
  }

  /** Gives worker `k` the job's next range, if any is left; if every worker has stopped, fails what is left. */
  private deal(k: number): void {
    const job = this.job;
    if (!job || job.next >= job.calls.length) return;
    if (this.stopped.every((reason) => reason !== null)) {
      for (; job.next < job.calls.length; job.next += 1) this.fail(job, job.next, this.stopped[k] ?? "");
      return;
    }
    if (this.stopped[k] !== null) return;
    const call = job.next;
    job.next += 1;
    const { first, end } = job.calls[call];
    job.calls[call].worker = k;
    this.busy[k] = [job.model, call];
    const { deflection, angle } = job.limits;
    this.post(this.workers[k], { kind: "mesh", call, model: job.model, deflection, angle, first, end });
  }

  private answered(k: number, message: FromMesher): void {
    this.busy[k] = null;
    const job = this.job;
    if (job && message.model === job.model) {
      if (message.kind === "meshed") {
        job.parts[message.call] = message.part;
        this.arrived(job);
      } else {
        this.fail(job, message.call, message.reason);
      }
    }
    this.deal(k);
  }

  /** Worker `k` stopped, for `reason`: the call it was meshing fails, and the rest go to the others. */
  private stop(k: number, reason: string): void {
    this.stopped[k] = reason;
    const [job, busy] = [this.job, this.busy[k]];
    this.busy[k] = null;
    if (job && busy && busy[0] === job.model) this.fail(job, busy[1], reason);
    this.workers.forEach((_, other) => this.busy[other] === null && this.deal(other));
  }

  private fail(job: Job, call: number, reason: string): void {
    job.failures[call] = reason;
    this.arrived(job);
  }

  private arrived(job: Job): void {
    job.left -= 1;
    if (job.left === 0) this.done(job);
  }

  private done(job: Job): void {
    if (this.job === job) this.job = null;
    const parts = job.parts.filter((part): part is Part => part !== null);
    job.finish({ model: job.model, ...assemble(parts), calls: job.calls, failed: job.failed() });
  }
}
