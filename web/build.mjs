// Builds the viewer with esbuild into one directory: the page, its style, and one bundle with its source map.
// esbuild strips the types without checking them; `tsc --noEmit` checks them (`just web-check`).
//
// Usage: node web/build.mjs [output directory, by default web/dist]
import { copyFile, mkdir } from "node:fs/promises";
import { resolve } from "node:path";
import { fileURLToPath } from "node:url";
import { build } from "esbuild";

const here = fileURLToPath(new URL(".", import.meta.url));
const out = resolve(process.argv[2] ?? resolve(here, "dist"));
await mkdir(out, { recursive: true });
await build({
  entryPoints: [resolve(here, "src/main.tsx")],
  outfile: resolve(out, "viewer.js"),
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
