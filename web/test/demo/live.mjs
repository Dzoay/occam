// The live agent test (docs/plan/view.md, `view.agent`), which `just agent-demo` runs by hand on the person's own key
// and account, never in CI (docs/decisions/2026-11-14-the-agent-starts-the-viewer.md). Headless Claude Code (`claude
// -p`), on the cheapest model the construction blind run used, pinned by its id, works in a directory of its own with
// the occam skill installed there and `occam` behind the blind run's logging wrapper. Given the first prompt it is to
// start the viewer and build a plate with a hole, while headless Chromium follows the page it opened; the harness then
// selects the plate's top face there, as a person would, and resumes the session with the second prompt. It is scored
// as the scripted demo is (../demo.test.mjs), and the kit is kept, with the transcripts, the calls, what the page
// showed and the score, in the blind run's form, and a copy of them scrubbed for docs/evidence/ (evidence.mjs).
//
//   just agent-demo                the live run, which prints where it keeps everything
//   just agent-demo --dry-run      everything but the model: the command lines are written down, and a stand-in
//                                  (stand-in.mjs) plays the agent with the scripted demo's calls
//   --budget 0.50 --calls 12       the most each prompt may spend, in US dollars, and the occam calls it may make
//   --out DIR                      where to keep the kit, rather than ~/.cache/occam-agent-demo/<time>/
import { execFile, spawn } from "node:child_process";
import { once } from "node:events";
import { createWriteStream } from "node:fs";
import { readFile, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { parseArgs, promisify } from "node:util";
import { chromium } from "playwright";

import {
  FIRST,
  MODEL as FILE,
  PERMISSIONS,
  SECOND,
  answer,
  callsIn,
  digestOf,
  drawn,
  facts,
  follow,
  kit,
  opened,
  score,
  selectTop,
} from "./harness.mjs";
import { evidence } from "./evidence.mjs";

/** The cheapest model the construction blind run's agents ran on, pinned by its full id. */
const MODEL = "claude-haiku-5-5";
/** How long a prompt may run before it is stopped. */
const LIMIT_MS = 15 * 60_000;
const STAND_IN = fileURLToPath(new URL("stand-in.mjs", import.meta.url));
const exec = promisify(execFile);

const { values: options } = parseArgs({
  options: {
    "dry-run": { type: "boolean", default: false },
    budget: { type: "string", default: "0.50" },
    calls: { type: "string", default: "12" },
    out: { type: "string" },
  },
});
const dry = options["dry-run"];
const OCCAM = process.env.OCCAM ?? "";
const WEBGPU = (process.env.OCCAM_WEBGPU_FLAGS ?? "").split(/\s+/).filter(Boolean);
if (!OCCAM || WEBGPU.length === 0) throw new Error("OCCAM and OCCAM_WEBGPU_FLAGS are not set: run it with `just agent-demo`");

/** The command line of `claude` for `prompt`, resuming the session `resume` if one is given. */
function claude(prompt, resume) {
  return [
    "-p",
    prompt,
    "--model",
    MODEL,
    "--output-format",
    "stream-json",
    "--verbose",
    "--max-budget-usd",
    options.budget,
    "--setting-sources",
    "project",
    "--strict-mcp-config",
    "--no-chrome",
    "--tools",
    PERMISSIONS.tools.join(","),
    "--permission-mode",
    "dontAsk",
    ...(resume ? ["--resume", resume] : []),
    "--allowedTools",
    ...PERMISSIONS.allowed,
  ];
}

/** `args` as a shell would take them back. */
const quoted = (args) => args.map((a) => (/^[\w@%+=:,./-]+$/.test(a) ? a : `'${a.replaceAll("'", `'\\''`)}'`)).join(" ");

/**
 * The agent given prompt `n` by the command line `args`, in its directory and environment: `claude`, or in a dry run
 * the stand-in. Its stream of events is kept as `logs/prompt-<n>.jsonl`; what it ended with is returned.
 */
async function prompted(demo, n, args) {
  const [program, argv] = dry ? [process.execPath, [STAND_IN, ...args]] : ["claude", args];
  const child = spawn(program, argv, { cwd: demo.case, env: demo.env(n), stdio: ["ignore", "pipe", "pipe"] });
  const transcript = join(demo.logs, `prompt-${n}.jsonl`);
  const streams = [[child.stdout, transcript], [child.stderr, join(demo.logs, `prompt-${n}.stderr`)]].map(([from, to]) => {
    const file = createWriteStream(to);
    from.pipe(file);
    return once(file, "finish");
  });
  const timer = setTimeout(() => child.kill("SIGTERM"), LIMIT_MS);
  const [code, signal] = await once(child, "close");
  clearTimeout(timer);
  await Promise.all(streams);
  const events = (await readFile(transcript, "utf8")).split("\n").flatMap((line) => {
    try {
      return line ? [JSON.parse(line)] : [];
    } catch {
      return [];
    }
  });
  const init = events.find((event) => event.type === "system" && event.subtype === "init");
  const result = events.findLast((event) => event.type === "result");
  const ended = result && {
    subtype: result.subtype,
    is_error: result.is_error,
    num_turns: result.num_turns,
    total_cost_usd: result.total_cost_usd,
    result: result.result,
  };
  return { code, signal, session: result?.session_id ?? init?.session_id ?? null, model: init?.model ?? null, ended };
}

/** The score in words, a table as the blind run's, for the evidence. */
function words(kept) {
  const rows = (kept.scored?.checks ?? []).map((c) => `| ${c.name} | ${c.pass ? "pass" : "FAIL"}${c.required ? "" : " (noted)"} |`);
  const prompts = kept.prompts.map((p, k) => `| ${k + 1} | ${p.text} | ${p.calls} | ${p.ended?.num_turns ?? "?"} | ${p.ended?.total_cost_usd ?? "?"} |`);
  return [
    `# The live agent test, ${kept.dry ? "dry run (a stand-in played the agent)" : MODEL}`,
    "",
    `Run ${kept.started}, the agent's model ${kept.prompts[0]?.model ?? "unknown"}, the page found by ${kept.opened ?? "nothing"}.`,
    "",
    "| prompt | text | occam calls | turns | cost (USD) |",
    "|---|---|---|---|---|",
    ...prompts,
    "",
    "| check | result |",
    "|---|---|",
    ...rows,
    "",
    kept.error ? `Stopped: ${kept.error}` : `**${kept.scored?.pass ? "Pass" : "Fail"}.**`,
    "",
  ].join("\n");
}

/** The kit after the first prompt: the page, the plate drawn, and the top face selected there; or why not. */
async function selected(demo, follower) {
  if (!follower) throw new Error("the agent started no viewer, so no page followed it");
  const answered = await answer(OCCAM, ["check", "--model", demo.model]).catch(() => null);
  if (!answered?.ok) throw new Error(`the agent made no model that reads at ${FILE}`);
  await drawn(follower, OCCAM, demo.model);
  const before = await facts(demo, "first");
  return { before, selected: await selectTop(follower, demo) };
}

const stamp = new Date().toISOString().replace(/[:.]/g, "-");
const cache = process.env.XDG_CACHE_HOME ?? join(homedir(), ".cache");
const root = options.out ?? join(cache, "occam-agent-demo", `${stamp}${dry ? "-dry-run" : ""}`);
const demo = await kit(root, OCCAM, Number(options.calls));
await exec(OCCAM, ["skill", "--install"], { cwd: demo.case });
// The command lines, written with the kit and the real occam as variables, so that no path of this machine is in them.
await writeFile(
  join(demo.logs, "command-lines.txt"),
  [
    "# KIT is the directory that holds this logs/, OCCAM the occam `just agent-demo` built, and SESSION the",
    "# session id the first prompt's result gives. Each runs in $KIT/case.",
    'cd "$KIT/case"',
    'export PATH="$KIT/bin:$PATH" BROWSER="$KIT/bin/browser" OCCAM_DEMO_REAL="$OCCAM" OCCAM_DEMO_LOG="$KIT/logs/calls.log"',
    `export OCCAM_DEMO_OPENED="$KIT/logs/opened.txt" OCCAM_DEMO_CALLS=${demo.allowed}`,
    `OCCAM_DEMO_PROMPT=1 claude ${quoted(claude(FIRST))}`,
    `OCCAM_DEMO_PROMPT=2 claude ${quoted(claude(SECOND, "SESSION")).replace("--resume SESSION", '--resume "$SESSION"')}`,
    "",
  ].join("\n"),
);
const kept = { dry, started: stamp, model: MODEL, prompts: [], opened: null, scored: null, error: null };
const browser = await chromium.launch({ headless: true, args: WEBGPU });
let follower = null;
try {
  let done = false;
  const following = opened(demo, () => done).then(async (page) => {
    kept.opened = page?.by ?? null;
    follower = page && (await follow(browser, page.url));
  });
  const first = await prompted(demo, 1, claude(FIRST));
  done = true;
  await following;
  const count = async (n) => callsIn(await readFile(demo.log, "utf8").catch(() => "")).filter((c) => c.prompt === n).length;
  kept.prompts.push({ text: FIRST, ...first, calls: await count(1) });
  const { before, selected: face } = await selected(demo, follower);
  if (!first.session) throw new Error("the first prompt gave no session to resume");
  const second = await prompted(demo, 2, claude(SECOND, first.session));
  kept.prompts.push({ text: SECOND, ...second, calls: await count(2) });
  // A page that does not draw the last model, say because the agent stopped the viewer, fails the score's page checks.
  const digest = await drawn(follower, OCCAM, demo.model).catch(() => digestOf(OCCAM, demo.model));
  const after = await facts(demo, "second");
  const calls = callsIn(await readFile(demo.log, "utf8"));
  kept.scored = await score({ before, after, selected: face, follower, digest, calls });
} catch (error) {
  kept.error = String(error?.message ?? error);
} finally {
  await follower?.stop();
  await writeFile(join(demo.logs, "page.jsonl"), (follower?.seen ?? []).map((s) => `${JSON.stringify(s)}\n`).join(""));
  await answer(OCCAM, ["view", "--model", demo.model, "--stop"]).catch(() => null);
  await browser.close();
}
await writeFile(join(root, "score.json"), `${JSON.stringify(kept, null, 2)}\n`);
await writeFile(join(root, "score.md"), words(kept));
const { leaks } = await evidence(root, OCCAM);
process.stdout.write(`${words(kept)}\nKept in ${root}: score.md and score.json; the agent's directory case/; logs/ holds\n`);
process.stdout.write("the calls (calls.log), each prompt's transcript (prompt-1.jsonl, prompt-2.jsonl), what the page\n");
process.stdout.write("showed (page.jsonl) and the command lines run (command-lines.txt); and evidence/ holds the same,\n");
process.stdout.write("scrubbed of this machine's paths and the person's own setup, ready for docs/evidence/.\n");
if (leaks.length > 0) process.stdout.write(`These still name the home directory, so are not ready: ${leaks.join(", ")}\n`);
process.exitCode = kept.scored?.pass && !kept.error && leaks.length === 0 ? 0 : 1;
