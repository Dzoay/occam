// Builds the viewer with esbuild into one directory: the page, its style, its bundle, the editing worker's and the mesh
// workers' bundles, each with its source map, and the kernel's module, `occam.wasm`, which only the workers load.
// esbuild strips the types without checking them; `tsc --noEmit` checks them (`just web-check`).
//
// The kernel is occam-wasm as wasm-bindgen writes it for the web: `just web-build` builds it and gives its directory
// as OCCAM_WASM_JS, and each worker's bundle takes its JavaScript in as "occam-wasm".
//
// Usage: OCCAM_WASM_JS=<wasm-bindgen's output> node web/build.mjs [output directory, by default web/dist]
import { copyFile, mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";

const here = fileURLToPath(new URL(".", import.meta.url));
const out = resolve(process.argv[2] ?? resolve(here, "dist"));
const kernel = process.env.OCCAM_WASM_JS;
if (!kernel) throw new Error("OCCAM_WASM_JS is not set: build the viewer with `just web-build`");
await mkdir(out, { recursive: true });
await build({
  entryPoints: {
    viewer: resolve(here, "src/main.tsx"),
    editing: resolve(here, "src/workers/editing.ts"),
    mesher: resolve(here, "src/workers/mesher.ts"),
  },
  outdir: out,
  alias: { "occam-wasm": resolve(kernel, "occam_wasm.js") },
  bundle: true,
  format: "esm",
  target: "es2023",
  minify: true,
  sourcemap: true,
  jsx: "automatic",
  jsxImportSource: "preact",
  logLevel: "warning",
});
for (const file of ["index.html", "viewer.css"]) await copyFile(resolve(here, "src", file), resolve(out, file));
await copyFile(resolve(kernel, "occam_wasm_bg.wasm"), resolve(out, "occam.wasm"));
