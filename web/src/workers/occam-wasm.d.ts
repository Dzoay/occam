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

  /** A model, as a worker holds it. */
  export class Session {
    constructor();
    free(): void;
    face_count(): number;
    /** Throws the reports refusing the limits, as JSON text. */
    mesh_part(deflection: number, angle: number, first: number, end: number): Meshed;
    /** Throws the reports refusing the bytes, as JSON text. */
    static open(native: Uint8Array): Session;
    run(request: string): string;
    take_saved(): Uint8Array<ArrayBuffer> | undefined;
  }

  /** Fetches and instantiates the module at `module_or_path`. */
  export default function init(options: { module_or_path: URL }): Promise<unknown>;
}
