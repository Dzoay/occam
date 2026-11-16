// Serves a built viewer on 127.0.0.1 only, on a free port: a loopback address is a secure context, which WebGPU needs.
import { readFile } from "node:fs/promises";
import { createServer } from "node:http";
import { extname, join, normalize } from "node:path";

const TYPES = {
  ".html": "text/html",
  ".js": "text/javascript",
  ".css": "text/css",
  ".map": "application/json",
};

/** Serves `root`; resolves to the server and its address. */
export async function serve(root) {
  const server = createServer(async (request, response) => {
    const path = new URL(request.url, "http://localhost").pathname;
    const file = path === "/" ? "index.html" : normalize(path).replace(/^([/\\]|\.\.)+/, "");
    try {
      const body = await readFile(join(root, file));
      response.writeHead(200, { "content-type": TYPES[extname(file)] ?? "application/octet-stream" }).end(body);
    } catch {
      response.writeHead(404).end();
    }
  });
  await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
  return { server, url: `http://127.0.0.1:${server.address().port}` };
}
