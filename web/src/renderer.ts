// Draws the scene's faces: one indexed draw a face, recorded once into a render bundle and recorded again only when
// the set of faces drawn changes, so a frame costs the same however many faces there are.

import type { Gpu } from "./gpu.ts";
import { VERTEX_LAYOUT } from "./layout.ts";
import type { Scene } from "./scene.ts";

const SHADER = /* wgsl */ `
struct Camera {
  view_projection: mat4x4f,
  light: vec4f,
}
@group(0) @binding(0) var<uniform> camera: Camera;

struct Varyings {
  @builtin(position) position: vec4f,
  @location(0) normal: vec3f,
}

@vertex
fn vs_main(@location(0) offset_u: vec4f, @location(1) normal_v: vec4f) -> Varyings {
  var out: Varyings;
  out.position = camera.view_projection * vec4f(offset_u.xyz, 1.0);
  out.normal = normal_v.xyz;
  return out;
}

@fragment
fn fs_main(in: Varyings) -> @location(0) vec4f {
  let lit = abs(dot(normalize(in.normal), camera.light.xyz));
  return vec4f(vec3f(0.56, 0.62, 0.7) * (0.3 + 0.7 * lit), 1.0);
}
`;

/** The camera's uniform: the view-projection, then the light's direction, as `f32`. */
export const CAMERA_FLOATS = 20;
const DEPTH: GPUTextureFormat = "depth24plus";

export class Renderer {
  readonly camera: GPUBuffer;
  private readonly gpu: Gpu;
  private readonly pipeline: GPURenderPipeline;
  private readonly bindGroup: GPUBindGroup;
  private depth: GPUTexture | null = null;
  private bundle: GPURenderBundle | null = null;
  private bundled = -1;

  /** `usage` adds to the camera buffer's usage, as the inspector's read-back needs. */
  constructor(gpu: Gpu, usage = 0) {
    this.gpu = gpu;
    const { device } = gpu;
    const module = device.createShaderModule({ code: SHADER });
    this.pipeline = device.createRenderPipeline({
      layout: "auto",
      vertex: { module, entryPoint: "vs_main", buffers: [VERTEX_LAYOUT] },
      fragment: { module, entryPoint: "fs_main", targets: [{ format: gpu.format }] },
      primitive: { topology: "triangle-list", cullMode: "none" },
      depthStencil: { format: DEPTH, depthWriteEnabled: true, depthCompare: "less" },
    });
    this.camera = device.createBuffer({
      size: CAMERA_FLOATS * 4,
      usage: GPUBufferUsage.UNIFORM | GPUBufferUsage.COPY_DST | usage,
    });
    this.bindGroup = device.createBindGroup({
      layout: this.pipeline.getBindGroupLayout(0),
      entries: [{ binding: 0, resource: { buffer: this.camera } }],
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
    for (const face of scene.faces()) {
      if (face.count === 0) continue;
      encoder.setVertexBuffer(0, face.vertices);
      encoder.setIndexBuffer(face.indices, "uint32");
      encoder.drawIndexed(face.count);
    }
    this.bundle = encoder.finish();
    this.bundled = scene.version;
    return this.bundle;
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

  /** Writes the camera and submits one frame of the scene. */
  draw(scene: Scene, camera: Float32Array): void {
    const { device, context } = this.gpu;
    device.queue.writeBuffer(this.camera, 0, camera.buffer, camera.byteOffset, camera.byteLength);
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
    pass.end();
    device.queue.submit([encoder.finish()]);
  }
}
