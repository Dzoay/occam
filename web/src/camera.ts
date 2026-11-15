// The camera, in `f64`: it orbits a target, pans across the view and zooms towards the target. Its matrix is built
// relative to the page's origin, as the vertices are, and only then rounded to `f32`.

import { type Bounds, type Vec3, centre, radius } from "./mesh.ts";

/** The vertical field of view. */
export const FIELD_OF_VIEW = Math.PI / 4;
const PITCH_LIMIT = Math.PI / 2 - 1e-3;
/** How much room fitting leaves around the model's bounding sphere. */
const FIT_MARGIN = 1.05;
const Z_UP: Vec3 = [0, 0, 1];

const add = (a: Vec3, b: Vec3, scale = 1): Vec3 => [a[0] + b[0] * scale, a[1] + b[1] * scale, a[2] + b[2] * scale];
const dot = (a: Vec3, b: Vec3): number => a[0] * b[0] + a[1] * b[1] + a[2] * b[2];
const cross = (a: Vec3, b: Vec3): Vec3 => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const unit = (a: Vec3): Vec3 => {
  const length = Math.hypot(a[0], a[1], a[2]);
  return [a[0] / length, a[1] / length, a[2] / length];
};

/** `p · v`, both 4 by 4 and column-major. */
function multiply(p: Float64Array, v: Float64Array): Float64Array {
  const out = new Float64Array(16);
  for (let column = 0; column < 4; column += 1) {
    for (let row = 0; row < 4; row += 1) {
      let sum = 0;
      for (let k = 0; k < 4; k += 1) sum += p[k * 4 + row] * v[column * 4 + k];
      out[column * 4 + row] = sum;
    }
  }
  return out;
}

export class Camera {
  target: Vec3 = [0, 0, 0];
  distance = 10;
  /** About the z axis, from the x axis. */
  yaw = -Math.PI / 4;
  /** Above the xy plane. */
  pitch = 0.5;

  /** From the target towards the eye. */
  backward(): Vec3 {
    const flat = Math.cos(this.pitch);
    return [flat * Math.cos(this.yaw), flat * Math.sin(this.yaw), Math.sin(this.pitch)];
  }

  eye(): Vec3 {
    return add(this.target, this.backward(), this.distance);
  }

  /** The view's right and up, in the model's frame. */
  axes(): [Vec3, Vec3] {
    const forward = add([0, 0, 0], this.backward(), -1);
    const right = unit(cross(forward, Z_UP));
    return [right, cross(right, forward)];
  }

  /** Turns about the target, in radians. */
  orbit(yaw: number, pitch: number): void {
    this.yaw += yaw;
    this.pitch = Math.min(PITCH_LIMIT, Math.max(-PITCH_LIMIT, this.pitch + pitch));
  }

  /** Moves the target across the view, in fractions of the view's height at the target. */
  pan(right: number, up: number): void {
    const height = 2 * this.distance * Math.tan(FIELD_OF_VIEW / 2);
    const [x, y] = this.axes();
    this.target = add(add(this.target, x, right * height), y, up * height);
  }

  /** Scales the distance to the target. */
  zoom(factor: number): void {
    if (Number.isFinite(factor) && factor > 0) this.distance = Math.min(1e12, Math.max(1e-9, this.distance * factor));
  }

  /** Looks at the box's centre from far enough that its bounding sphere is inside the view, either way round. */
  fit(box: Bounds, aspect: number): void {
    const half = Math.min(FIELD_OF_VIEW / 2, Math.atan(Math.tan(FIELD_OF_VIEW / 2) * aspect));
    this.target = centre(box);
    this.distance = Math.max(radius(box), 1e-9) / Math.sin(half) * FIT_MARGIN;
  }

  centre(point: Vec3): void {
    this.target = point;
  }

  /** The column-major view-projection relative to `origin`, its near and far planes holding `box`. */
  viewProjection(origin: Vec3, aspect: number, box: Bounds | null): Float64Array {
    const eye = add(this.eye(), origin, -1);
    const forward = add([0, 0, 0], this.backward(), -1);
    const [right, up] = this.axes();
    const reach = box ? Math.hypot(...add(centre(box), this.eye(), -1)) : this.distance;
    const size = box ? radius(box) : this.distance / 2;
    const far = (reach + size) * 1.01;
    const near = Math.max((reach - size) * 0.99, far * 1e-5);
    const view = new Float64Array([
      right[0], up[0], -forward[0], 0,
      right[1], up[1], -forward[1], 0,
      right[2], up[2], -forward[2], 0,
      -dot(right, eye), -dot(up, eye), dot(forward, eye), 1,
    ]);
    const f = 1 / Math.tan(FIELD_OF_VIEW / 2);
    const projection = new Float64Array([
      f / aspect, 0, 0, 0,
      0, f, 0, 0,
      0, 0, far / (near - far), -1,
      0, 0, (near * far) / (near - far), 0,
    ]);
    return multiply(projection, view);
  }
}
