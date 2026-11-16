// The messages between the page and its workers (docs/architecture/browser.md). No wasm runs on the page's main thread:
// the editing worker holds the model and runs requests, or opens each model `occam view` sends, and each mesh worker
// meshes ranges of faces from its own copy of the binary twin. Bytes and arrays move by transfer, never through shared memory, so the page needs no
// SharedArrayBuffer and its server sends no COOP or COEP headers.

/** What the page asks the editing worker: run `request` on the model, and give `copies` copies of the model it makes. */
export interface Run {
  readonly kind: "run";
  readonly id: number;
  readonly request: string;
  readonly copies: number;
}

/**
 * What the page asks the editing worker when `occam view` sends it a model: open `twin`, give `copies` copies of it,
 * and say which faces must be meshed again, to `deflection` and `angle`, after the model it last opened.
 */
export interface Open {
  readonly kind: "open";
  readonly id: number;
  readonly twin: ArrayBuffer;
  readonly copies: number;
  readonly deflection: number;
  readonly angle: number;
}

/** What the editing worker answers an open: the model's faces and copies, and what to mesh again after the last. */
export interface Opened {
  readonly kind: "opened";
  readonly id: number;
  /** Each face's id, in id order, as its slot and generation. */
  readonly faces: Uint32Array<ArrayBuffer>;
  readonly twins: readonly ArrayBuffer[];
  /**
   * The places, among the faces in id order, of those to mesh again, and the ids of what keeps its mesh from the model
   * opened last; null for the first model, every face of which is meshed.
   */
  readonly again: { readonly faces: readonly number[]; readonly kept: readonly string[] } | null;
}

/** What the editing worker answers a run. */
export type FromEditing =
  | {
    readonly kind: "ran";
    readonly id: number;
    /** The run's one JSON form, as `occam run --json` gives it. */
    readonly ran: string;
    /** Each face's id, in id order, as its slot and generation. */
    readonly faces: Uint32Array<ArrayBuffer>;
    /** The binary twin, one copy for each mesh worker, if the run changed the model; none if it did not. */
    readonly twins: readonly ArrayBuffer[];
  }
  | Opened
  | { readonly kind: "failed"; readonly id: number; readonly reason: string };

/**
 * What the page asks a mesh worker of the model it holds, meshed whole to `deflection` and `angle` the first time and
 * kept: what a ray hits, or the samples of some edges. Picking is against this CPU mesh, never the GPU's.
 */
export type Ask =
  | {
    readonly kind: "pick";
    readonly ask: number;
    readonly model: number;
    readonly deflection: number;
    readonly angle: number;
    /** Where the ray starts and its direction, in millimetres in the model's frame. */
    readonly ray: readonly number[];
    /** The tolerance for edges a millimetre along the ray: a few pixels, at the depth of what the ray hits. */
    readonly slope: number;
    /** How far along the ray to take the tolerance at where it hits no face. */
    readonly fallback: number;
  }
  | {
    readonly kind: "samples";
    readonly ask: number;
    readonly model: number;
    readonly deflection: number;
    readonly angle: number;
    readonly edges: readonly string[];
  };

/** What the page asks a mesh worker: hold model `model`, the twin given, mesh a range of its faces, or an ask. */
export type ToMesher =
  | { readonly kind: "load"; readonly model: number; readonly twin: ArrayBuffer }
  | {
    readonly kind: "mesh";
    readonly call: number;
    readonly model: number;
    readonly deflection: number;
    readonly angle: number;
    /** The faces from `first` up to `end`, counted in id order. */
    readonly first: number;
    readonly end: number;
  }
  | Ask;

/** A report as occam's one JSON form gives it, with what the page shows of it. */
export interface KernelReport {
  readonly code: string;
  readonly message: string;
  /** The entities it names, the first the one it is about. */
  readonly entities: readonly string[];
  readonly reason?: string;
}

/** What the page shows of `report`, a report in occam's one JSON form. */
export function kernelReport(report: Record<string, unknown>): KernelReport {
  const entities = Array.isArray(report.entities) ? report.entities.map(String) : [];
  const reason = typeof report.reason === "string" ? { reason: report.reason } : {};
  return { code: String(report.code), message: String(report.message), entities, ...reason };
}

/** What one call meshed: its arrays, numbered from their own start, and what refused any of it. */
export interface Part {
  readonly positions: Float64Array<ArrayBuffer>;
  readonly normals: Float64Array<ArrayBuffer>;
  readonly uvs: Float64Array<ArrayBuffer>;
  readonly indices: Uint32Array<ArrayBuffer>;
  /** Six numbers a face that meshed: its slot and generation, its first vertex and how many, its first triangle and how many. */
  readonly table: Uint32Array<ArrayBuffer>;
  /** The reports refusing its faces, then those refusing their edges and what the watertight check found. */
  readonly reports: readonly KernelReport[];
}

/** An edge's samples, three numbers a point, in millimetres. */
export interface Samples {
  readonly id: string;
  readonly points: Float64Array<ArrayBuffer>;
}

/** What a mesh worker answers an ask. */
export type Answer =
  /** The pick's one JSON form, as `occam pick --json` gives it. */
  | { readonly kind: "picked"; readonly ask: number; readonly answer: string }
  | { readonly kind: "sampled"; readonly ask: number; readonly edges: readonly Samples[] }
  | { readonly kind: "unanswered"; readonly ask: number; readonly reason: string };

/** What a mesh worker answers a call or an ask. */
export type FromMesher =
  | { readonly kind: "meshed"; readonly call: number; readonly model: number; readonly part: Part }
  | { readonly kind: "failed"; readonly call: number; readonly model: number; readonly reason: string }
  | Answer;

/** A worker's own end of its messages: the dom library declares no worker's global scope. */
export interface Own<In, Out> {
  onmessage: ((event: MessageEvent<In>) => void) | null;
  postMessage(message: Out, transfer: Transferable[]): void;
}

/** The worker this runs in, as its own end of its messages. */
export function own<In, Out>(): Own<In, Out> {
  return self as unknown as Own<In, Out>;
}
