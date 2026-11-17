// The editing worker: it holds the model in a session and runs the page's requests on it, one at a time, in the order
// they came. A run that goes through and changes the model gives the binary twin, which it publishes to the page as
// one copy for each mesh worker, every copy transferred, so the page's main thread copies nothing. When `occam view`
// sends the page a model, the worker opens it, measuring and checking it whole, publishes it the same way, and says
// which faces must be meshed again after the model it opened last, which it keeps until the next. Its check is the
// page's only one: the mesh workers trust it.

import { Session } from "occam-wasm";

import { type Checked, type FromEditing, type Open, type Opened, type Run, own } from "../protocol.ts";
import { kernel, said } from "./kernel.ts";

const port = own<Run | Open, FromEditing>();
let session: Session | null = null;
/** The model `occam view` sent last, which the page draws. */
let shown: Session | null = null;

/**
 * `twin` and `count - 1` copies of it, each with its own buffer, for the mesh workers, which open them without checking
 * them again: called only with a twin this worker has just opened or made, and so checked, and the only place a
 * `Checked` is made.
 */
function copies(twin: Uint8Array<ArrayBuffer>, count: number): Checked[] {
  if (count < 1) return [];
  const first = twin.byteOffset === 0 && twin.byteLength === twin.buffer.byteLength ? twin.buffer : twin.slice().buffer;
  return [first, ...Array.from({ length: count - 1 }, () => twin.slice().buffer)].map((copy) => copy as Checked);
}

function run({ id, request, copies: count }: Run): FromEditing {
  session ??= new Session();
  const ran = session.run(request);
  const twin = session.take_saved();
  const twins = twin ? copies(twin, count) : [];
  return { kind: "ran", id, ran, faces: session.face_ids(), twins };
}

function open({ id, twin, copies: count, deflection, angle }: Open): Opened {
  const next = Session.open(new Uint8Array(twin));
  try {
    const again = shown ? (JSON.parse(next.again(shown, deflection, angle)) as Opened["again"]) : null;
    shown?.free();
    shown = next;
    return { kind: "opened", id, faces: next.face_ids(), twins: copies(new Uint8Array(twin), count), again };
  } catch (error) {
    next.free();
    throw error;
  }
}

async function answer(message: Run | Open): Promise<void> {
  try {
    await kernel();
    const answered = message.kind === "run" ? run(message) : open(message);
    const moved = answered.kind === "failed" ? [] : [answered.faces.buffer, ...answered.twins];
    port.postMessage(answered, moved);
  } catch (error) {
    port.postMessage({ kind: "failed", id: message.id, reason: said(error) }, []);
  }
}

// Each answer waits for the kernel, then runs without awaiting anything, so requests keep the order they came in.
port.onmessage = (event) => void answer(event.data);
