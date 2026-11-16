// The demo made a test (docs/plan/view.md, `view.agent`), with the agent's calls scripted: the steps the live agent
// test (demo/live.mjs) runs with a model, run here in CI through the same harness, wrapper and score. The agent starts
// `occam view`, whose page headless Chromium opens as the person's browser would, and builds a plate with a hole
// through `occam run` while the page follows; the person's click selects the plate's top face; the agent reads the
// selection, highlights it and runs a cylinder of radius 5 and height 20 standing on its centre; the cylinder's base
// lies on that face within the band, and the page shows it.
//
// Run by `just web-test`, which builds the page and then `occam`, which embeds it.
import assert from "node:assert/strict";
import { mkdir, mkdtemp, readFile, rm, writeFile } from "node:fs/promises";
import { homedir, tmpdir } from "node:os";
import { join } from "node:path";
import { after, before, test } from "node:test";
import { chromium } from "playwright";

import { INIT, evidence } from "./demo/evidence.mjs";
import {
  PERMISSIONS,
  answer,
  callsIn,
  drawn,
  facts,
  follow,
  kit,
  middleOf,
  opened,
  permits,
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

test("the live run's permissions let the agent run every command in the skill and write its requests, and refuse a shell write", async () => {
  const skill = await readFile(new URL("../../skills/occam/SKILL.md", import.meta.url), "utf8");
  const commands = [...skill.matchAll(/```sh\n([\s\S]*?)```/g)].flatMap(([, block]) => block.split("\n").filter(Boolean));
  assert.equal(commands.length, 7);
  for (const command of commands) assert.ok(permits(PERMISSIONS, "Bash", command), command);
  assert.ok(["Write", "Read", "Skill"].every((tool) => permits(PERMISSIONS, tool)), "its file tools and its skill");
  assert.match(skill, /write each request file with your file-writing tool \(Write\), never with `cat`/);
  // What the live run's agent tried first, and Claude Code refused: a request written by the shell.
  const tried = "cat > plate.json <<'EOF'\n{\"schema\": 1}\nEOF";
  assert.equal(permits(PERMISSIONS, "Bash", tried), false);
  for (const other of ["python3 -c 1", "rm part.occam", "occam run --model part.occam plate.json && rm plate.json"]) {
    assert.equal(permits(PERMISSIONS, "Bash", other), false, other);
  }
  assert.equal(permits({ ...PERMISSIONS, allowed: ["Bash(occam *)"] }, "Write"), false, "a tool needs its rule");
});

test("the evidence keeps the agent's transcript without the person's setup, their account's state or this machine's paths", async () => {
  const root = await mkdtemp(join(tmpdir(), "occam-evidence-"));
  try {
    const occam = join(homedir(), ".cache", "target", "debug", "occam");
    const init = {
      type: "system",
      subtype: "init",
      session_id: "2b72e98f",
      model: "claude-haiku-5-5",
      tools: PERMISSIONS.tools,
      permissionMode: "dontAsk",
      claude_code_version: "2.1.325",
      cwd: join(root, "case"),
      original_cwd: join(root, "case"),
      memory_paths: { auto: join(homedir(), ".claude", "projects", `${homedir().replace(/[^A-Za-z0-9]/g, "-")}-x`) },
      skills: ["occam", "theirs"],
      slash_commands: ["occam", "theirs"],
      plugins: [{ name: "theirs", path: "builtin" }],
      agents: ["theirs"],
      apiKeySource: "none",
    };
    const wrote = { type: "assistant", message: { content: [{ type: "tool_use", name: "Write", input: { file_path: join(root, "case", "plate.json") } }] } };
    const limits = { type: "rate_limit_event", rate_limit_info: { status: "allowed" } };
    const result = { type: "result", subtype: "success", session_id: "2b72e98f", num_turns: 6, total_cost_usd: 0.004 };
    await mkdir(join(root, "logs"));
    const transcript = [init, wrote, limits, result].map((event) => JSON.stringify(event)).join("\n");
    await writeFile(join(root, "logs", "prompt-1.jsonl"), `${transcript}\n`);
    await writeFile(join(root, "logs", "calls.log"), `=== call 1 prompt 1 now\ncwd: ${join(root, "case")}\nargv: view\n--- exit 0\n`);
    await writeFile(join(root, "score.md"), `Run by ${occam}\n`);
    const { written, leaks } = await evidence(root, occam);
    assert.deepEqual(written.map((file) => file.slice(root.length)), ["/evidence/score.md", "/evidence/logs/calls.log", "/evidence/logs/prompt-1.jsonl"]);
    assert.deepEqual(leaks, []);
    const events = (await readFile(join(root, "evidence", "logs", "prompt-1.jsonl"), "utf8")).split("\n").filter(Boolean).map((l) => JSON.parse(l));
    assert.deepEqual(Object.keys(events[0]), INIT.filter((key) => key in init), "the init event's lists and paths dropped");
    assert.deepEqual(events.map((event) => event.type), ["system", "assistant", "result"], "and the account's rate limits");
    assert.equal(events[1].message.content[0].input.file_path, "$KIT/case/plate.json");
    assert.equal(await readFile(join(root, "evidence", "logs", "calls.log"), "utf8"), "=== call 1 prompt 1 now\ncwd: $KIT/case\nargv: view\n--- exit 0\n");
    assert.equal(await readFile(join(root, "evidence", "score.md"), "utf8"), "Run by $OCCAM\n");
  } finally {
    await rm(root, { recursive: true, force: true });
  }
});
