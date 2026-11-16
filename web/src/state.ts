// What the controls show, carried by signals so that a change redraws only the control that reads it, never the page.

import { signal } from "@preact/signals";

import type { KernelReport } from "./protocol.ts";
import type { Report } from "./report.ts";

export interface Status {
  readonly faces: number;
  readonly triangles: number;
  /** Bytes still to upload. */
  readonly pending: number;
  /** Whether the adapter is a software or otherwise limited one. */
  readonly fallback: boolean;
}

/** Why the page cannot draw, if it cannot. The canvas is hidden while there is one. */
export const failure = signal<Report | null>(null);
/** A report shown over the drawing, which goes on with the last good model, such as a malformed mesh's. */
export const notice = signal<Report | null>(null);
/** What refused any of the model shown, from the kernel: each face that did not mesh, by name, and what else it found. */
export const refusals = signal<readonly KernelReport[]>([]);
/** The selected ids, in the order they were selected. */
export const selection = signal<readonly string[]>([]);
export const status = signal<Status>({ faces: 0, triangles: 0, pending: 0, fallback: false });
