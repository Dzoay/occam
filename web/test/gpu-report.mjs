// What headless Chromium gives the viewer's tests, for a log: the WebGPU adapter; Chromium's GPU feature status and
// what it composites with; whether a read-back works after a canvas frame, which is what fails when Chromium composites
// through GL; and Chromium's own log lines about the GPU. It fails when there is no adapter or no read-back, which
// would fail most of the tests, and some only after a long wait.
//
// Run by `just web-gpu`, which gives it the flags `just web-test` uses.
import { createServer } from "node:http";

const flags = (process.env.OCCAM_WEBGPU_FLAGS ?? "").split(/\s+/).filter(Boolean);
// Playwright writes the browser's own output to stderr under `pw:browser`; keep those lines rather than print them all.
const logged = [];
const write = process.stderr.write.bind(process.stderr);
process.stderr.write = (chunk, ...rest) =>
  String(chunk).includes("pw:browser") ? (logged.push(...String(chunk).split("\n")), true) : write(chunk, ...rest);
process.env.DEBUG = "pw:browser";
const { chromium } = await import("playwright");
const server = createServer((_, response) =>
  response.writeHead(200, { "content-type": "text/html" }).end("<!doctype html><canvas width=64 height=64></canvas>"));
await new Promise((resolve) => server.listen(0, "127.0.0.1", resolve));
const browser = await chromium.launch({
  headless: true,
  args: [...flags, "--enable-logging=stderr", "--v=0"],
});
const { gpu } = await (await browser.newBrowserCDPSession()).send("SystemInfo.getInfo");
const page = await browser.newPage();
await page.goto(`http://127.0.0.1:${server.address().port}/`);
const probe = await page.evaluate(async () => {
  const adapter = await navigator.gpu?.requestAdapter();
  if (!adapter) return { adapter: null };
  const { vendor, architecture, description, isFallbackAdapter } = adapter.info;
  const device = await adapter.requestDevice();
  const context = document.querySelector("canvas").getContext("webgpu");
  context.configure({ device, format: navigator.gpu.getPreferredCanvasFormat() });
  const encoder = device.createCommandEncoder();
  const view = context.getCurrentTexture().createView();
  encoder.beginRenderPass({ colorAttachments: [{ view, loadOp: "clear", storeOp: "store" }] }).end();
  device.queue.submit([encoder.finish()]);
  const buffer = device.createBuffer({ size: 4, usage: GPUBufferUsage.MAP_READ });
  const readBack = await buffer.mapAsync(GPUMapMode.READ).then(() => "works", (error) => `fails: ${error}`);
  return { adapter: { vendor, architecture, description, isFallbackAdapter }, readBackAfterACanvasFrame: readBack };
}).catch((error) => ({ error: String(error) }));
await browser.close();
server.close();
const composited = Object.entries(gpu.auxAttributes).filter(([key]) => /vulkan|skia|gl_renderer|gl_implementation/i.test(key));
console.log(JSON.stringify({
  flags,
  chromium: browser.version(),
  probe,
  featureStatus: gpu.featureStatus,
  composited: Object.fromEntries(composited),
  devices: gpu.devices.map((device) => `${device.vendorString} ${device.deviceString}`.trim()),
}, null, 2));
const about = logged.filter((text) => /gpu|vulkan|dawn|webgpu|angle|swiftshader|skia|egl|\bgl/i.test(text));
console.log(`Chromium's log, ${about.length} of its ${logged.length} lines, those about the GPU:`);
for (const line of about) console.log(line.replace(/^.*pw:browser /, ""));
if (probe.readBackAfterACanvasFrame !== "works") {
  console.error("The viewer's tests need a WebGPU adapter whose buffers read back after a canvas frame, and this has none.");
  process.exitCode = 1;
}
