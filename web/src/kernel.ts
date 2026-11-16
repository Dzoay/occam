// The kernel as the page reaches it: an editing worker that holds the model and runs requests, and a pool of mesh
// workers that mesh each model it publishes. The page's main thread only routes messages and draws: it never loads the
// kernel's module, and it copies nothing, since the editing worker makes each mesh worker's copy of the twin.

import { type Limits, type Meshing, Pool } from "./pool.ts";
import type { FromEditing, Run } from "./protocol.ts";

/** What a request gave: the run's one JSON form, and the model it made, meshed, if it changed the model. */
export interface Ran {
  readonly ran: string;
  /** Resolves with the mesh, or with null if a later model came first; null itself if the model did not change. */
  readonly meshing: Promise<Meshing | null> | null;
}

export class Kernel {
  readonly pool: Pool;
  private readonly editing: Worker;
  private next = 0;
  private readonly waiting = new Map<number, (answer: FromEditing) => void>();

  /** An editing worker and `workers` mesh workers, their scripts beside the page's. */
  constructor(workers: number, base: URL) {
    this.editing = new Worker(new URL("editing.js", base), { type: "module", name: "editing" });
    this.editing.onmessage = (event: MessageEvent<FromEditing>) => {
      this.waiting.get(event.data.id)?.(event.data);
      this.waiting.delete(event.data.id);
    };
    this.pool = new Pool(workers, new URL("mesher.js", base));
  }

  /** Runs `request` in the editing worker and, if it changed the model, meshes the model to `limits`. */
  async run(request: string, limits: Limits): Promise<Ran> {
    const id = (this.next += 1);
    const answer = await new Promise<FromEditing>((resolve) => {
      this.waiting.set(id, resolve);
      const message: Run = { kind: "run", id, request, copies: this.pool.size };
      this.editing.postMessage(message);
    });
    if (answer.kind === "failed") throw new Error(`the editing worker could not run the request: ${answer.reason}`);
    const meshing = answer.twins.length > 0 ? this.pool.mesh(answer.twins, answer.faces, limits) : null;
    return { ran: answer.ran, meshing };
  }
}
