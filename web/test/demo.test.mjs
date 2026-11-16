// The demo made a test (docs/plan/view.md, `view.agent`), with the agent's calls scripted: the steps the live agent
// test (demo/live.mjs) runs with a model, run here in CI through the same harness, wrapper and score. The agent starts
// `occam view`, whose page headless Chromium opens as the person's browser would, and builds a plate with a hole
// through `occam run` while the page follows; the person's click selects the plate's top face; the agent reads the
// selection, highlights it and runs a cylinder of radius 5 and height 20 standing on its centre; the cylinder's base
// lies on that face within the band, and the page shows it.
//
// Run by `just web-test`, which builds the page and then `occam`, which embeds it.
import assert from "node:assert/strict";
import { mkdtemp, readFile, rm } from "node:fs/promises";
import { tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { chromium } from "playwright";

import {
  answer,
  callsIn,
  drawn,
  facts,
  follow,
  kit,
  middleOf,
  opened,
  score,
  scripted,
  scriptedAgent,
  selectTop,
  valueOf,
} from "./demo/harness.mjs";

/** The flags that give headless Chromium its software WebGPU adapter: `just web-test` gives them, and says why. */
const WEBGPU = (process.env.OCCAM_WEBGPU_FLAGS ?? "").split(/\s+/).filter(Boolean);
/** The native `occam` command, which embeds the page: `just web-test` builds it. */
const OCCAM = process.env.OCCAM ?? "";

let browser;

before(async () => {
  assert.ok(WEBGPU.length > 0, "OCCAM_WEBGPU_FLAGS is not set: run the tests with `just web-test`");
  assert.ok(OCCAM, "OCCAM is not set: run the tests with `just web-test`");
  browser = await chromium.launch({ headless: true, args: WEBGPU });
});

after(async () => {
  await browser?.close();
});

/**
 * The demo played through with the agent's calls scripted, the cylinder standing where `place(description)` says on
 * the face selected: the score, what the page saw and the calls logged. The viewer is stopped and the kit removed after.
 */
async function played(place) {
  const root = await mkdtemp(join(tmpdir(), "occam-demo-"));
  const demo = await kit(root, OCCAM);
  let follower;
  try {
    const started = async () => {
      const page = await opened(demo, () => false);
      assert.equal(page.by, "browser", "the page the agent's occam view opened");
      follower = await follow(browser, page.url);
    };
    await scripted.build(scriptedAgent(demo, 1), started);
    const plate = await drawn(follower, OCCAM, demo.model);
    const first = await facts(demo, "first");
    const selected = await selectTop(follower, demo);
    const shown = (id) => follower.page.waitForFunction((ids) => window.occamViewer.highlight().join() === ids, id);
    await scripted.act(scriptedAgent(demo, 2), shown, place);
    const digest = await drawn(follower, OCCAM, demo.model);
    const second = await facts(demo, "second");
    const calls = callsIn(await readFile(demo.log, "utf8"));
    const scored = await score({ before: first, after: second, selected, follower, digest, calls });
    await follower.stop();
    assert.deepEqual(follower.errors, []);
    await follower.page.close();
    return { scored, calls, seen: follower.seen, selected, digests: [plate, digest] };
  } finally {
    await follower?.stop();
    await answer(OCCAM, ["view", "--model", demo.model, "--stop"]);
    await rm(root, { recursive: true, force: true });
  }
}

test("the demo, its agent scripted: a plate built as the page follows, its top face clicked, a cylinder stood on its centre", async () => {
  const { scored, calls, seen, selected, digests } = await played(middleOf);
  assert.deepEqual(scored.checks.filter((check) => !check.pass), [], "every check passes, those noted only too");
  assert.equal(scored.pass, true);
  assert.equal(scored.checks.length, 18);
  assert.notEqual(digests[0], digests[1]);
  const commands = calls.map((call) => call.argv[0]);
  assert.deepEqual(commands, ["view", "run", "selection", "highlight", "run", "run"], "the agent's calls, each logged");
  assert.deepEqual(calls.map((call) => call.prompt), [1, 1, 2, 2, 2, 2]);
  assert.match(calls[1].files[0].text, /"extrude"/, "with the request it ran");
  assert.ok(seen.some((now) => now.highlight.join() === selected.id), "the page showed the highlight");
  assert.deepEqual(seen.at(-1).selection, [selected.id], "and keeps the selection, carried over by the run");
});

test("a cylinder stood on the plane's origin, the face's corner, fails the score where it misses the centre", async () => {
  const { scored } = await played((description) => valueOf(description.values, "origin"));
  const failed = scored.checks.filter((check) => !check.pass).map((check) => check.name);
  assert.deepEqual(failed, ["centred on the face, within the band", "within the face, clear of its hole", "standing out of the plate"]);
  assert.equal(scored.pass, false);
});
