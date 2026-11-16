// The page as `occam view` serves it (docs/architecture/live-view.md#the-page). It follows the model file through the
// server's events: each time the model's digest changes it fetches the model again, the editing worker opens it and
// says which faces changed since the model drawn, and the mesh workers mesh only those, every other face keeping its
// block and what was reported of it. A file that does not read is the server's named report over the drawing, which
// goes on with the last model that did. One model is fetched and meshed at a time; a change that comes meanwhile is
// followed once it is done. The page posts what the person selects, and shows the selection and the highlight the
// server sends from the files beside the model once the model they were made on is drawn.

import type { Kernel } from "./kernel.ts";
import type { Mesh } from "./mesh.ts";
import { type Limits, type Meshing, kept } from "./pool.ts";
import { type KernelReport, type Opened, type Part, kernelReport } from "./protocol.ts";
import { report } from "./report.ts";
import { highlight, notice, refusals, refusedSelection, selection, watched } from "./state.ts";
import type { Viewer } from "./viewer.ts";

/** What the page drew last: the model's digest, its mesh, what refused any of it, and the faces no call could mesh. */
interface Drawn {
  readonly digest: string;
  readonly mesh: Mesh;
  readonly reports: readonly KernelReport[];
  readonly failed: ReadonlySet<string>;
}

/** What an event of the server carries: what it says of the model it watches. */
interface Watching {
  readonly digest: string | null;
  readonly reports: readonly Record<string, unknown>[];
}

/** A selection or a highlight, as its file beside the model holds it: ids of the model of `digest`. */
interface Marked {
  readonly digest: string | null;
  readonly ids: readonly string[];
}

export class Live {
  /** The model drawn last. */
  drawn: Drawn | null = null;
  /** How many models have been drawn. */
  models = 0;
  /** The digest of the model the server says reads whole last. */
  private wanted: string | null = null;
  private busy = false;
  /** The selection and the highlight beside the model, as the server last sent them. */
  private marks: { selection: Marked; highlight: Marked } = {
    selection: { digest: null, ids: [] },
    highlight: { digest: null, ids: [] },
  };

  constructor(
    private readonly kernel: Kernel,
    private readonly viewer: Viewer,
    private readonly limits: Limits,
    /** Told each time a model is drawn. */
    private readonly drew: () => void = () => {},
  ) {}

  /** Follows the server's events, beside the page's address. */
  start(): void {
    const events = new EventSource("events");
    const heard = (event: MessageEvent<string>) => {
      const said = JSON.parse(event.data) as Watching;
      watched.value = said.reports.map(kernelReport);
      this.wanted = said.digest;
      void this.follow();
    };
    events.addEventListener("model", heard);
    events.addEventListener("refused", heard);
    for (const name of ["selection", "highlight"] as const) {
      events.addEventListener(name, (event: MessageEvent<string>) => {
        this.marks[name] = JSON.parse(event.data) as Marked;
        this.mark(name);
      });
    }
  }

  /**
   * Shows the selection or the highlight the server sent, if it was made on the model drawn: the selection as what is
   * selected, and the highlight as what the agent is about to change. One made on another model shows nothing of the
   * highlight, and leaves the selection to the person, until the model it was made on is drawn.
   */
  private mark(name: "selection" | "highlight"): void {
    const { digest, ids } = this.marks[name];
    const shown = digest !== null && digest === this.drawn?.digest;
    if (name === "highlight") highlight.value = shown ? ids : [];
    else if (shown) selection.value = ids;
  }

  /**
   * Posts the selection `ids`, made on the model of `digest`, to `occam view`, which writes it beside the model; one
   * for an older model is refused, and the page shows why, drops it and the person selects again on the model drawn.
   */
  async post(digest: string, ids: readonly string[]): Promise<void> {
    const body = JSON.stringify({ schema: 2, digest, ids });
    const answer = await fetch("selection", { method: "POST", body, headers: { "content-type": "application/json" } });
    if (answer.status === 409) {
      refusedSelection.value = kernelReport((await answer.json()) as Record<string, unknown>);
      selection.value = [];
    } else if (answer.ok) {
      refusedSelection.value = null;
    }
  }

  /** Draws the model the server says reads whole, again after each change that comes meanwhile. */
  private async follow(): Promise<void> {
    if (this.busy) return;
    this.busy = true;
    try {
      while (this.wanted !== null && this.wanted !== this.drawn?.digest && (await this.draw()));
    } finally {
      this.busy = false;
    }
  }

  /** Fetches the model and draws it, meshing only what changed since the one drawn; whether it could. */
  private async draw(): Promise<boolean> {
    const response = await fetch("model", { cache: "no-store" });
    const digest = response.headers.get("x-occam-digest");
    if (!response.ok || digest === null) return false;
    const opened = await this.kernel.open(await response.arrayBuffer(), this.limits);
    const [only, keeping] = this.again(opened);
    const meshed = await this.kernel.pool.mesh(opened.twins, opened.faces, this.limits, only, keeping);
    if (!meshed) return false;
    this.show(digest, meshed, opened.again === null ? [] : opened.again.kept);
    return true;
  }

  /**
   * The places of the faces to mesh, null for all of them, and the blocks the others keep: what the editing worker
   * says changed, with every face no call could mesh last time.
   */
  private again(opened: Opened): [readonly number[] | null, (readonly [number, Part])[]] {
    const drawn = this.drawn;
    if (!drawn || !opened.again) return [null, []];
    const blocks = new Map(drawn.mesh.faces.map((face) => [face.id, face]));
    const meshed = new Set(opened.again.faces);
    const keeping: (readonly [number, Part])[] = [];
    for (let place = 0; 2 * place < opened.faces.length; place += 1) {
      const ids = [opened.faces[2 * place], opened.faces[2 * place + 1]] as const;
      const id = `face:${ids[0]}/${ids[1]}`;
      const block = blocks.get(id);
      if (drawn.failed.has(id)) meshed.add(place);
      else if (!meshed.has(place) && block) keeping.push([place, kept(drawn.mesh, block, ids)]);
    }
    return [[...meshed].sort((a, b) => a - b), keeping];
  }

  /** Shows `meshed`, the model of `digest`, with what was reported of what keeps its mesh, the ids `keptIds`. */
  private show(digest: string, meshed: Meshing, keptIds: readonly string[]): void {
    const keptSet = new Set(keptIds);
    const carried = (this.drawn?.reports ?? []).filter((each) => keptSet.has(each.entities[0] ?? ""));
    const reports = [...meshed.reports, ...carried];
    this.viewer.show(meshed.mesh);
    refusals.value = reports;
    const failed = meshed.failed.flatMap((call) => call.faces);
    const [first] = meshed.failed;
    if (first && !notice.peek()) notice.value = report("view.mesh-failed", first.reason, failed);
    this.drawn = { digest, mesh: meshed.mesh, reports, failed: new Set(failed) };
    this.models += 1;
    this.mark("selection");
    this.mark("highlight");
    this.drew();
  }
}
