// What the workers use of occam-wasm, as wasm-bindgen declares it in the `occam_wasm.d.ts` it writes beside its
// JavaScript (`just web-build` makes both, and esbuild bundles that JavaScript into each worker as "occam-wasm"). The
// type check runs without that build, so this declares the part the workers call; the browser tests run it.

declare module "occam-wasm" {
  /** A model's mesh: flat arrays, each face a vertex block whose indices count from its start, and its summary. */
  export class Meshed {
    private constructor();
    free(): void;
    /** Six numbers a face that meshed, in id order: slot, generation, first vertex, vertices, first triangle, triangles. */
    faces(): Uint32Array<ArrayBuffer>;
    indices(): Uint32Array<ArrayBuffer>;
    normals(): Float64Array<ArrayBuffer>;
    positions(): Float64Array<ArrayBuffer>;
    /** One answer per face and the reports, in the one JSON form `occam mesh --json` gives. */
    summary(): string;
    uvs(): Float64Array<ArrayBuffer>;
  }

  /** A model, as the editing worker holds it. */
  export class Session {
    constructor();
    free(): void;
    /** What to mesh again to draw this model after `before`'s, in its one JSON form; throws the limits' refusal. */
    again(before: Session, deflection: number, angle: number): string;
    face_count(): number;
    /** Each face's slot and generation, in id order. */
    face_ids(): Uint32Array<ArrayBuffer>;
    /** The model `native` holds, measured and checked whole; throws the reports refusing it, as JSON text. */
    static open(native: Uint8Array): Session;
    run(request: string): string;
    take_saved(): Uint8Array<ArrayBuffer> | undefined;
  }

  /** A model as a mesh worker holds it: the twin the editing worker checked, opened without checking it again. */
  export class Trusted {
    private constructor();
    free(): void;
    /** Throws the reports refusing the limits, as JSON text. */
    mesh_part(deflection: number, angle: number, first: number, end: number): Meshed;
    /** The model `twin` holds, read but not measured or checked; throws the reports refusing bytes that do not read. */
    static open(twin: Uint8Array): Trusted;
    /**
     * What a ray of six numbers, its start and its direction, hits first in the model's mesh, edges within `tolerance`,
     * in the one JSON form `occam pick --json` gives; throws the reports refusing the limits or the ray, as JSON text.
     */
    pick(deflection: number, angle: number, ray: Float64Array, tolerance: number): string;
    /** The samples of the edge `edge`, three numbers a point; none for an id that is not an edge a face uses. */
    samples(deflection: number, angle: number, edge: string): Float64Array<ArrayBuffer>;
  }

  /** Fetches and instantiates the module at `module_or_path`. */
  export default function init(options: { module_or_path: URL }): Promise<unknown>;
}
