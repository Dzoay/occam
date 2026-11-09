// Times the workload in a real browser, with and without simd128, alternating the two in one run so that the machine and its
// load cancel out (L30). Prints one line of JSON. Serves only on localhost.
//
// Usage: node run.mjs <scalar js directory> <simd128 js directory>
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, normalize } from "node:path";
import { chromium } from "playwright";

const [scalarDir, simdDir] = process.argv.slice(2);
const roots = { scalar: scalarDir, simd128: simdDir };
const types = { ".js": "text/javascript", ".wasm": "application/wasm", ".html": "text/html" };
const GRID = 1500;
const ROUNDS = 9;

const server = createServer(async (request, response) => {
  if (request.url === "/") {
    response.writeHead(200, { "content-type": "text/html" }).end("<!doctype html><title>occam</title>");
    return;
  }
  const [, build, ...rest] = request.url.split("/");
  try {
    const file = await readFile(join(roots[build], normalize(rest.join("/"))));
    response.writeHead(200, { "content-type": types[extname(request.url)] ?? "application/octet-stream" }).end(file);
  } catch {
    response.writeHead(404).end();
  }
});
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));

const browser = await chromium.launch({ headless: true });
const page = await browser.newPage();
await page.goto(`http://127.0.0.1:${server.address().port}/`);
const timed = (build) =>
  page.evaluate(
    async ({ build, grid }) => {
      const module = await import(`/${build}/perf.js`);
      await module.default();
      const start = performance.now();
      const result = module.run(grid);
      return { ms: performance.now() - start, result };
    },
    { build, grid: GRID },
  );

const best = { scalar: Infinity, simd128: Infinity };
const results = {};
for (let round = 0; round < ROUNDS; round += 1) {
  for (const build of ["scalar", "simd128"]) {
    const { ms, result } = await timed(build);
    best[build] = Math.min(best[build], ms);
    results[build] = result;
  }
}
await browser.close();
server.close();
console.log(
  JSON.stringify({
    scalar_ms: best.scalar,
    simd128_ms: best.simd128,
    scalar_result: results.scalar,
    simd128_result: results.simd128,
  }),
);
