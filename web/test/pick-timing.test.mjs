// The GPU pick timing script refuses a run on a software adapter (docs/plan/view.md, `view.timing`): given the flags
// that ask headless Chromium for SwiftShader, and kept headless, it says which adapter it found and exits 3 before it
// opens any model. Run by `just web-test`, which gives it those flags.
import assert from "node:assert/strict";
import { execFile } from "node:child_process";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";
import { test } from "node:test";

const exec = promisify(execFile);
const SCRIPT = fileURLToPath(new URL("./pick-timing.mjs", import.meta.url));

test("the GPU pick timing refuses a software adapter, naming it, and opens no model", async () => {
  assert.ok(process.env.OCCAM_WEBGPU_FLAGS, "OCCAM_WEBGPU_FLAGS is not set: run the tests with `just web-test`");
  const refused = await exec("node", [SCRIPT, "no-such-model.occam", "chromium", "--headless-only"]).then(
    () => assert.fail("it ran on a software adapter"),
    (error) => error,
  );
  assert.equal(refused.code, 3, refused.stderr);
  const said = JSON.parse(refused.stdout);
  assert.equal(said.refused, "software-adapter");
  assert.equal(said.headless, true);
  assert.match(`${said.adapter?.vendor} ${said.adapter?.architecture} ${said.adapter?.description}`, /swiftshader/i);
});

test("the GPU pick timing names what it takes when asked wrongly", async () => {
  const refused = await exec("node", [SCRIPT]).catch((error) => error);
  assert.equal(refused.code, 2);
  assert.match(refused.stderr, /usage: pick-timing\.mjs <model file>/);
});
