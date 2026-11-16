// The editing worker: it holds the model in a session and runs the page's requests on it, one at a time, in the order
// they came. A run that goes through and changes the model gives the binary twin, which it publishes to the page as
// one copy for each mesh worker, every copy transferred, so the page's main thread copies nothing.

import { Session } from "occam-wasm";

import { type FromEditing, type Run, own } from "../protocol.ts";
import { kernel, said } from "./kernel.ts";

const port = own<Run, FromEditing>();
let session: Session | null = null;

/** `twin` and `count - 1` copies of it, each with its own buffer. */
function copies(twin: Uint8Array<ArrayBuffer>, count: number): ArrayBuffer[] {
  if (count < 1) return [];
  const first = twin.byteOffset === 0 && twin.byteLength === twin.buffer.byteLength ? twin.buffer : twin.slice().buffer;
  return [first, ...Array.from({ length: count - 1 }, () => twin.slice().buffer)];
}

async function answer({ id, request, copies: count }: Run): Promise<void> {
  try {
    await kernel();
    session ??= new Session();
    const ran = session.run(request);
    const twin = session.take_saved();
    const twins = twin ? copies(twin, count) : [];
    const faces = session.face_ids();
    port.postMessage({ kind: "ran", id, ran, faces, twins }, [faces.buffer, ...twins]);
  } catch (error) {
    port.postMessage({ kind: "failed", id, reason: said(error) }, []);
  }
}

// Each answer waits for the kernel, then runs without awaiting anything, so runs keep the order their requests came in.
port.onmessage = (event) => void answer(event.data);
