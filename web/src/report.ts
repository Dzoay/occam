// What the page tells a person when it cannot draw: a named report with its remedy, never a blank canvas. The codes,
// wording and remedies are occam-report's catalogue's (`just errors` lists it), and a test holds this file to it.

export type Code = "view.webgpu-missing" | "view.no-adapter" | "view.no-device" | "view.device-lost"
  | "view.mesh-malformed" | "view.mesh-failed";

export type RemedyKind = "use-webgpu-browser" | "turn-on-webgpu" | "reload-page";

export interface Remedy {
  readonly kind: RemedyKind;
  readonly text: string;
}

export interface Report {
  readonly code: Code;
  readonly message: string;
  readonly remedy: Remedy | null;
  /** The entities it names, all of them, where its wording names only the first few. */
  readonly entities?: readonly string[];
}

/**
 * Each code's wording, with `{reason}` for what the browser, the mesh or a worker said and `{faces}` for the faces it
 * names, and its kind of fix, if it has one.
 */
export const TEMPLATES: Readonly<Record<Code, readonly [string, RemedyKind | null]>> = {
  "view.webgpu-missing": ["This browser has no WebGPU, which the viewer draws with.", "use-webgpu-browser"],
  "view.no-adapter": [
    "The browser has WebGPU but gave the page no GPU adapter, so the viewer cannot draw.",
    "turn-on-webgpu",
  ],
  "view.no-device": ["The GPU adapter would not give the page a device: {reason}", "reload-page"],
  "view.device-lost": ["The GPU device was lost, so the page has stopped drawing: {reason}", "reload-page"],
  "view.mesh-malformed": [
    "The page was given a mesh that breaks the renderer contract, which is a bug in occam: {reason}. It still draws the last good model.",
    null,
  ],
  "view.mesh-failed": [
    "A mesh worker could not mesh {faces}, so the page draws the model without them: {reason}",
    "reload-page",
  ],
};

export const REMEDIES: Readonly<Record<RemedyKind, string>> = {
  "use-webgpu-browser": "Open the page in a current Chromium or Firefox, which draw with WebGPU.",
  "turn-on-webgpu":
    "Turn on WebGPU and hardware acceleration in the browser's settings; in Chromium, chrome://gpu says why WebGPU is off.",
  "reload-page": "Reload the page.",
};

/** How many faces a report's wording names before it counts the rest. */
const NAMED = 12;

/** `faces` as a report's wording names them: the first few, then how many more. */
function named(faces: readonly string[]): string {
  const shown = faces.slice(0, NAMED).join(", ");
  return faces.length > NAMED ? `${shown} and ${faces.length - NAMED} more faces` : shown;
}

/** The report for `code`, its wording given `reason` and the `faces` it names. */
export function report(code: Code, reason = "", faces: readonly string[] = []): Report {
  const [template, kind] = TEMPLATES[code];
  return {
    code,
    // Replaced through functions, so a `$` in what a worker or the browser said is kept as it is.
    message: template.replace("{faces}", () => named(faces)).replace("{reason}", () => reason),
    remedy: kind ? { kind, text: REMEDIES[kind] } : null,
    ...(faces.length > 0 ? { entities: faces } : {}),
  };
}
