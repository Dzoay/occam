// The kernel as a worker loads it: occam-wasm's module, fetched from beside the worker's own script and instantiated
// in the worker, never on the page's main thread.

import init from "occam-wasm";

let loading: Promise<unknown> | null = null;

/** Resolves once this worker's instance of the kernel is ready, loading it the first time. */
export function kernel(): Promise<unknown> {
  loading ??= init({ module_or_path: new URL("occam.wasm", import.meta.url) });
  return loading;
}

/** `error` as text: a refusal the kernel throws is its reports' JSON already. */
export function said(error: unknown): string {
  return error instanceof Error ? `${error.name}: ${error.message}` : String(error);
}
