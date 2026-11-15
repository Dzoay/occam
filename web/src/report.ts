// What the page tells a person when it cannot draw: a named report with its remedy, never a blank canvas. The codes,
// wording and remedies are occam-report's catalogue's (`just errors` lists it), and a test holds this file to it.

export type Code = "view.webgpu-missing" | "view.no-adapter" | "view.no-device" | "view.device-lost"
  | "view.mesh-malformed";

export type RemedyKind = "use-webgpu-browser" | "turn-on-webgpu" | "reload-page";

export interface Remedy {
  readonly kind: RemedyKind;
  readonly text: string;
}

export interface Report {
  readonly code: Code;
  readonly message: string;
  readonly remedy: Remedy | null;
}

/** Each code's wording, with `{reason}` for what the browser or the mesh said, and its kind of fix, if it has one. */
export const TEMPLATES: Readonly<Record<Code, readonly [string, RemedyKind | null]>> = {
  "view.webgpu-missing": ["This browser has no WebGPU, which the viewer draws with.", "use-webgpu-browser"],
  "view.no-adapter": [
    "The browser has WebGPU but gave the page no GPU adapter, so the viewer cannot draw.",
    "turn-on-webgpu",
  ],
  "view.no-device": ["The GPU adapter would not give the page a device: {reason}", "reload-page"],
  "view.device-lost": ["The GPU device was lost, so the page has stopped drawing: {reason}", "reload-page"],
  "view.mesh-malformed": [
    "The mesh the page was given does not have the renderer contract's layout: {reason}. Mesh the model again with the `occam` that serves this page.",
    null,
  ],
};

export const REMEDIES: Readonly<Record<RemedyKind, string>> = {
  "use-webgpu-browser": "Open the page in a current Chromium or Firefox, which draw with WebGPU.",
  "turn-on-webgpu":
    "Turn on WebGPU and hardware acceleration in the browser's settings; in Chromium, chrome://gpu says why WebGPU is off.",
  "reload-page": "Reload the page.",
};

/** The report for `code`, its wording given `reason`. */
export function report(code: Code, reason = ""): Report {
  const [template, kind] = TEMPLATES[code];
  return {
    code,
    message: template.replace("{reason}", reason),
    remedy: kind ? { kind, text: REMEDIES[kind] } : null,
  };
}
