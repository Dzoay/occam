// Picking in the page (docs/architecture/live-view.md#the-page): a click asks a mesh worker what the ray through it
// hits, against the CPU mesh and never against anything the GPU drew, and selects it; shift-click adds or removes it.
// The ray is built in `f64` from the camera, and an edge is hit within a few pixels at the depth of what the ray hits.
// Served by `occam view`, the page posts the selection with the digest of the model it was picked on. The edges selected
// or highlighted are outlined through their samples, which the same worker gives.

import { effect } from "@preact/signals";

import type { Limits, Pool } from "./pool.ts";
import { highlight, selection } from "./state.ts";
import type { Viewer } from "./viewer.ts";

/** How near an edge a click must land to pick it, in pixels. */
export const PIXELS = 4;

/** What a pick answered, as the page reads its one JSON form. */
export interface Picked {
  /** The ray as the page built it, its start and its direction, which `occam pick` takes as they are. */
  readonly ray: readonly number[];
  readonly digest: string | null;
  readonly entity: string | null;
  readonly tolerance: number;
  /** Its one JSON form, as `occam pick --json` gives it. */
  readonly json: string;
}

/** Where a click posts the selection: `occam view`, given the digest of the model it was made on. */
export type Post = (digest: string, ids: readonly string[]) => void;

/** `ids` with `id` added at the end, or taken out if it is there. */
function toggled(ids: readonly string[], id: string): string[] {
  return ids.includes(id) ? ids.filter((each) => each !== id) : [...ids, id];
}

export class Picking {
  /** The last pick's answer, for the inspector. */
  last: Picked | null = null;
  /** The samples of the edges outlined, by id, of the model the pool was given last. */
  private samples = new Map<string, Float64Array>();
  private model = -1;

  constructor(
    private readonly viewer: Viewer,
    private readonly pool: Pool,
    private readonly limits: Limits,
    private readonly post: Post | null,
  ) {
    effect(() => {
      void this.outline([...selection.value, ...highlight.value]);
    });
  }

  /** Picks at the point `(x, y)` of the window and selects what the ray hits; with `add`, adds or removes it. */
  async click(x: number, y: number, add: boolean): Promise<Picked | null> {
    const { ray, slope, fallback } = this.viewer.rayAt(x, y, PIXELS);
    const { deflection, angle } = this.limits;
    const answer = await this.pool.ask({ kind: "pick", deflection, angle, ray, slope, fallback });
    if (answer.kind !== "picked") return null;
    const said = JSON.parse(answer.answer) as { digest: string | null; entity: string | null; tolerance: number };
    const picked = { ray, digest: said.digest, entity: said.entity, tolerance: said.tolerance, json: answer.answer };
    this.last = picked;
    const before = selection.peek();
    const now = picked.entity === null ? (add ? before : []) : add ? toggled(before, picked.entity) : [picked.entity];
    selection.value = now;
    if (picked.digest !== null && this.post) this.post(picked.digest, now);
    return picked;
  }

  /** Outlines the edges selected and highlighted again, as after a model is drawn. */
  refresh(): void {
    void this.outline([...selection.peek(), ...highlight.peek()]);
  }

  /** Outlines the edges among `ids`, asking for the samples of those not yet held. */
  async outline(ids: readonly string[]): Promise<void> {
    if (this.pool.current !== this.model) [this.samples, this.model] = [new Map(), this.pool.current];
    const model = this.model;
    const edges = [...new Set(ids.filter((id) => id.startsWith("edge:")))];
    const missing = edges.filter((id) => !this.samples.has(id));
    if (missing.length > 0 && model > 0) {
      const { deflection, angle } = this.limits;
      const answer = await this.pool.ask({ kind: "samples", deflection, angle, edges: missing });
      if (answer.kind === "sampled" && model === this.model) answer.edges.forEach((e) => this.samples.set(e.id, e.points));
    }
    this.viewer.outline(edges.flatMap((id) => {
      const points = this.samples.get(id);
      return points && points.length >= 6 ? [[id, points] as const] : [];
    }));
  }
}
