// A mesh worker, one of the page's pool, kept alive between requests. It loads its own copy of each model the editing
// worker publishes, read-only, and meshes the ranges of its faces the page deals it, one call at a time, transferring
// the flat arrays back with what refused any of them.

import { Session } from "occam-wasm";

import type { FromMesher, KernelReport, Part, ToMesher } from "../protocol.ts";
import { own } from "../protocol.ts";
import { kernel, said } from "./kernel.ts";

const port = own<ToMesher, FromMesher>();
/** The model this worker holds, or why it could not open it. */
let held: { readonly model: number; readonly session: Session | null; readonly refused: string } | null = null;

type Mesh = Extract<ToMesher, { kind: "mesh" }>;

/** What the page shows of a report in occam's JSON form. */
function shown(report: Record<string, unknown>): KernelReport {
  const entities = Array.isArray(report.entities) ? report.entities.map(String) : [];
  const reason = typeof report.reason === "string" ? { reason: report.reason } : {};
  return { code: String(report.code), message: String(report.message), entities, ...reason };
}

/** The reports a call's summary holds: each face's refusal, in id order, then the rest. */
function reports(summary: string): KernelReport[] {
  const { answers, reports: rest } = JSON.parse(summary) as {
    answers: { report?: Record<string, unknown> }[];
    reports: Record<string, unknown>[];
  };
  return [...answers.flatMap((answer) => (answer.report ? [answer.report] : [])), ...rest].map(shown);
}

function mesh(call: Mesh): Part {
  if (!held || held.model !== call.model) throw new Error(`this worker holds no model ${call.model}`);
  if (!held.session) throw new Error(held.refused);
  const meshed = held.session.mesh_part(call.deflection, call.angle, call.first, call.end);
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

async function answer(message: ToMesher): Promise<void> {
  if (message.kind === "load") {
    held?.session?.free();
    held = null;
    try {
      await kernel();
      held = { model: message.model, session: Session.open(new Uint8Array(message.twin)), refused: "" };
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
