// A mesh worker, one of the page's pool, kept alive between requests. It loads its own copy of each model the editing
// worker publishes, read-only, trusting the editing worker's check: it reads the copy, refusing bytes that do not read
// as a model, but neither measures nor checks it again
// (docs/decisions/2026-11-16-mesh-workers-trust-the-editing-worker.md). It meshes the ranges of its faces the page
// deals it, one call at a time, transferring the flat arrays back with what refused any of them. It also picks against
// its model's whole CPU mesh, made the first time it is asked and kept, and gives the samples of the edges the page
// outlines.

import { Trusted } from "occam-wasm";

import type { Answer, Ask, FromMesher, KernelReport, Part, ToMesher } from "../protocol.ts";
import { kernelReport, own } from "../protocol.ts";
import { kernel, said } from "./kernel.ts";

const port = own<ToMesher, FromMesher>();
/** The model this worker holds, or why it could not read it. */
let held: { readonly model: number; readonly session: Trusted | null; readonly refused: string } | null = null;

type Mesh = Extract<ToMesher, { kind: "mesh" }>;

/** The reports a call's summary holds: each face's refusal, in id order, then the rest. */
function reports(summary: string): KernelReport[] {
  const { answers, reports: rest } = JSON.parse(summary) as {
    answers: { report?: Record<string, unknown> }[];
    reports: Record<string, unknown>[];
  };
  return [...answers.flatMap((answer) => (answer.report ? [answer.report] : [])), ...rest].map(kernelReport);
}

/** What holds model `model`, as a call or an ask names it. */
function holding(model: number): Trusted {
  if (!held || held.model !== model) throw new Error(`this worker holds no model ${model}`);
  if (!held.session) throw new Error(held.refused);
  return held.session;
}

function mesh(call: Mesh): Part {
  const meshed = holding(call.model).mesh_part(call.deflection, call.angle, call.first, call.end);
  try {
    return {
      positions: meshed.positions(),
      normals: meshed.normals(),
      uvs: meshed.uvs(),
      indices: meshed.indices(),
      table: meshed.faces(),
      reports: reports(meshed.summary()),
    };
  } finally {
    meshed.free();
  }
}

/**
 * What a ray hits, with edges within a few pixels at the depth of the face it hits first, or at `fallback` along it
 * where it hits none: one pick with no tolerance finds that depth, and a second picks with the tolerance there.
 */
function pick(ask: Extract<Ask, { kind: "pick" }>): Answer {
  const session = holding(ask.model);
  const ray = Float64Array.from(ask.ray);
  const first = JSON.parse(session.pick(ask.deflection, ask.angle, ray, 0)) as { distance?: number };
  const along = typeof first.distance === "number" ? first.distance : ask.fallback;
  return { kind: "picked", ask: ask.ask, answer: session.pick(ask.deflection, ask.angle, ray, ask.slope * along) };
}

function samples(ask: Extract<Ask, { kind: "samples" }>): Answer {
  const session = holding(ask.model);
  const edges = ask.edges.map((id) => ({ id, points: session.samples(ask.deflection, ask.angle, id) }));
  return { kind: "sampled", ask: ask.ask, edges };
}

async function answer(message: ToMesher): Promise<void> {
  if (message.kind === "pick" || message.kind === "samples") {
    try {
      await kernel();
      const answered = message.kind === "pick" ? pick(message) : samples(message);
      port.postMessage(answered, answered.kind === "sampled" ? answered.edges.map((edge) => edge.points.buffer) : []);
    } catch (error) {
      port.postMessage({ kind: "unanswered", ask: message.ask, reason: said(error) }, []);
    }
    return;
  }
  if (message.kind === "load") {
    held?.session?.free();
    held = null;
    try {
      await kernel();
      held = { model: message.model, session: Trusted.open(new Uint8Array(message.twin)), refused: "" };
    } catch (error) {
      held = { model: message.model, session: null, refused: said(error) };
    }
    return;
  }
  const { call, model } = message;
  try {
    await kernel();
    const part = mesh(message);
    const moved = [part.positions, part.normals, part.uvs, part.indices, part.table].map((array) => array.buffer);
    port.postMessage({ kind: "meshed", call, model, part }, moved);
  } catch (error) {
    port.postMessage({ kind: "failed", call, model, reason: said(error) }, []);
  }
}

// A load awaits the kernel only the first time, and a call after it awaits the same promise, so they keep their order.
port.onmessage = (event) => void answer(event.data);
