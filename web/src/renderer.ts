// Draws the scene's faces: one indexed draw a face, recorded once into a render bundle and recorded again only when
// the set of faces drawn changes, so a frame costs the same however many faces there are. Each draw's instance is its
// face's place in the bundle, which reads the face's mark from a storage buffer written each frame: whether it is
// selected, highlighted, or both, drawn in a colour each. The edges selected or highlighted are drawn after, as lines
// through their samples, over the faces.

import type { Gpu } from "./gpu.ts";
import { VERTEX_LAYOUT } from "./layout.ts";
import type { Scene } from "./scene.ts";

/** A face's or an edge's mark: selected, highlighted, or both. */
export const SELECTED = 1;
export const HIGHLIGHTED = 2;

const COLOURS = /* wgsl */ `
fn marked(mark: u32, base: vec3f) -> vec3f {
  if ((mark & 2u) != 0u) {
    return vec3f(0.16, 0.66, 0.32);
  }
  if ((mark & 1u) != 0u) {
    return vec3f(0.96, 0.56, 0.12);
  }
  return base;
}
`;

const SHADER = /* wgsl */ `
struct Camera {
  view_projection: mat4x4f,
  light: vec4f,
}
@group(0) @binding(0) var<uniform> camera: Camera;
@group(0) @binding(1) var<storage, read> marks: array<u32>;

struct Varyings {
  @builtin(position) position: vec4f,
  @location(0) normal: vec3f,
  @location(1) @interpolate(flat) mark: u32,
}

@vertex
fn vs_main(@location(0) offset_u: vec4f, @location(1) normal_v: vec4f, @builtin(instance_index) face: u32) -> Varyings {
  var out: Varyings;
  out.position = camera.view_projection * vec4f(offset_u.xyz, 1.0);
  out.normal = normal_v.xyz;
  out.mark = marks[face];
  return out;
}
${COLOURS}
@fragment
fn fs_main(in: Varyings) -> @location(0) vec4f {
  let lit = abs(dot(normalize(in.normal), camera.light.xyz));
  return vec4f(marked(in.mark, vec3f(0.56, 0.62, 0.7)) * (0.3 + 0.7 * lit), 1.0);
}
`;

const LINES = /* wgsl */ `
struct Camera {
  view_projection: mat4x4f,
  light: vec4f,
}
@group(0) @binding(0) var<uniform> camera: Camera;

struct Varyings {
  @builtin(position) position: vec4f,
  @location(0) @interpolate(flat) mark: u32,
}

@vertex
fn vs_line(@location(0) offset_mark: vec4f) -> Varyings {
  var out: Varyings;
  out.position = camera.view_projection * vec4f(offset_mark.xyz, 1.0);
  out.mark = u32(offset_mark.w);
  return out;
}
${COLOURS}
@fragment
fn fs_line(in: Varyings) -> @location(0) vec4f {
  return vec4f(marked(in.mark, vec3f(0.1, 0.1, 0.1)), 1.0);
}
`;

/** Floats a line's vertex: its offset from the page's origin, then its mark. */
export const LINE_FLOATS = 4;

/** The camera's uniform: the view-projection, then the light's direction, as `f32`. */
export const CAMERA_FLOATS = 20;
const DEPTH: GPUTextureFormat = "depth24plus";

export class Renderer {
  readonly camera: GPUBuffer;
  /** Each face's mark, by its place in the bundle. */
  marks: GPUBuffer;
  /** The faces whose marks the marks buffer holds, in its order. */
  marked: readonly string[] = [];
  /** The lines drawn, `LINE_FLOATS` a vertex, two vertices a segment. */
  lines: GPUBuffer | null = null;
  lineVertices = 0;
  private readonly gpu: Gpu;
  private readonly usage: number;
  private readonly pipeline: GPURenderPipeline;
  private readonly linePipeline: GPURenderPipeline;
  private readonly lineGroup: GPUBindGroup;
  private bindGroup: GPUBindGroup;
  private written = new Uint32Array(0);
  private depth: GPUTexture | null = null;
  private bundle: GPURenderBundle | null = null;
  private bundled = -1;

  /** `usage` adds to the camera's, the marks' and the lines' buffers' usage, as the inspector's read-back needs. */
  constructor(gpu: Gpu, usage = 0) {
    this.gpu = gpu;
    this.usage = usage;
    const { device } = gpu;
    const depthStencil: GPUDepthStencilState = { format: DEPTH, depthWriteEnabled: true, depthCompare: "less" };
    const module = device.createShaderModule({ code: SHADER });
    this.pipeline = device.createRenderPipeline({
      layout: "auto",
      vertex: { module, entryPoint: "vs_main", buffers: [VERTEX_LAYOUT] },
      fragment: { module, entryPoint: "fs_main", targets: [{ format: gpu.format }] },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil,
    });
    const lines = device.createShaderModule({ code: LINES });
    const attributes: GPUVertexAttribute[] = [{ shaderLocation: 0, offset: 0, format: "float32x4" }];
    this.linePipeline = device.createRenderPipeline({
      layout: "auto",
      vertex: { module: lines, entryPoint: "vs_line", buffers: [{ arrayStride: LINE_FLOATS * 4, attributes }] },
      fragment: { module: lines, entryPoint: "fs_line", targets: [{ format: gpu.format }] },
      primitive: { topology: "line-list" },
      depthStencil: { ...depthStencil, depthWriteEnabled: false, depthCompare: "always" },
    });
    this.camera = device.createBuffer({
      size: CAMERA_FLOATS * 4,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST | usage,
    });
    this.marks = this.markBuffer(64);
    this.bindGroup = this.faceGroup();
    this.lineGroup = device.createBindGroup({
      layout: this.linePipeline.getBindGroupLayout(0),
      entries: [{ binding: 0, resource: { buffer: this.camera } }],
    });
  }

  private markBuffer(faces: number): GPUBuffer {
    const usage = GPUBufferUsage.STORAGE | GPUBufferUsage.COPY_DST | this.usage;
    return this.gpu.device.createBuffer({ size: Math.max(1, faces) * 4, usage });
  }

  private faceGroup(): GPUBindGroup {
    return this.gpu.device.createBindGroup({
      layout: this.pipeline.getBindGroupLayout(0),
      entries: [{ binding: 0, resource: { buffer: this.camera } }, { binding: 1, resource: { buffer: this.marks } }],
    });
  }

  private record(scene: Scene): GPURenderBundle {
    if (this.bundle && this.bundled === scene.version) return this.bundle;
    const encoder = this.gpu.device.createRenderBundleEncoder({
      colorFormats: [this.gpu.format],
      depthStencilFormat: DEPTH,
    });
    encoder.setPipeline(this.pipeline);
    encoder.setBindGroup(0, this.bindGroup);
    let place = 0;
    for (const face of scene.faces()) {
      if (face.count > 0) {
        encoder.setVertexBuffer(0, face.vertices);
        encoder.setIndexBuffer(face.indices, "uint32");
        encoder.drawIndexed(face.count, 1, 0, 0, place);
      }
      place += 1;
    }
    this.bundle = encoder.finish();
    this.bundled = scene.version;
    return this.bundle;
  }

  /** Writes each face's mark, as `mark` gives it, where it changed, growing the buffer, and so the bundle, if need be. */
  private mark(scene: Scene, mark: (id: string) => number): void {
    this.marked = [...scene.faces()].map((face) => face.block.id);
    const marks = Uint32Array.from(this.marked, mark);
    if (marks.byteLength > this.marks.size) {
      this.marks.destroy();
      this.marks = this.markBuffer(marks.length * 2);
      this.bindGroup = this.faceGroup();
      this.bundled = -1;
    }
    if (marks.length === this.written.length && marks.every((m, k) => m === this.written[k])) return;
    if (marks.length > 0) this.gpu.device.queue.writeBuffer(this.marks, 0, marks);
    this.written = marks;
  }

  /** Draws `vertices`, `LINE_FLOATS` a vertex, as lines from now on: none if it is empty. */
  outline(vertices: Float32Array<ArrayBuffer>): void {
    if (!this.lines || this.lines.size < vertices.byteLength) {
      this.lines?.destroy();
      const usage = GPUBufferUsage.VERTEX | GPUBufferUsage.COPY_DST | this.usage;
      this.lines = this.gpu.device.createBuffer({ size: Math.max(16, vertices.byteLength), usage });
    }
    if (vertices.length > 0) this.gpu.device.queue.writeBuffer(this.lines, 0, vertices);
    this.lineVertices = vertices.length / LINE_FLOATS;
  }

  private depthView(width: number, height: number): GPUTextureView {
    if (!this.depth || this.depth.width !== width || this.depth.height !== height) {
      this.depth?.destroy();
      this.depth = this.gpu.device.createTexture({
        size: [width, height],
        format: DEPTH,
        usage: GPUTextureUsage.RENDER_ATTACHMENT,
      });
    }
    return this.depth.createView();
  }

  /** Writes the camera and each face's mark, as `mark` gives it, and submits one frame of the scene and the lines. */
  draw(scene: Scene, camera: Float32Array, mark: (id: string) => number): void {
    const { device, context } = this.gpu;
    device.queue.writeBuffer(this.camera, 0, camera.buffer, camera.byteOffset, camera.byteLength);
    this.mark(scene, mark);
    const colour = context.getCurrentTexture();
    const encoder = device.createCommandEncoder();
    const pass = encoder.beginRenderPass({
      colorAttachments: [{
        view: colour.createView(),
        clearValue: { r: 0.95, g: 0.95, b: 0.96, a: 1 },
        loadOp: "clear",
        storeOp: "store",
      }],
      depthStencilAttachment: {
        view: this.depthView(colour.width, colour.height),
        depthClearValue: 1,
        depthLoadOp: "clear",
        depthStoreOp: "discard",
      },
    });
    pass.executeBundles([this.record(scene)]);
    if (this.lines && this.lineVertices > 0) {
      pass.setPipeline(this.linePipeline);
      pass.setBindGroup(0, this.lineGroup);
      pass.setVertexBuffer(0, this.lines);
      pass.draw(this.lineVertices);
    }
    pass.end();
    device.queue.submit([encoder.finish()]);
  }
}
