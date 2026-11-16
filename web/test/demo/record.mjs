// What the live demo keeps of a run in its kit (live.mjs), and the run scored again from what it kept. score.json holds
// the run: the prompts and how each ended, how the page was found, the face selected, what the page showed at the end,
// and the score; score.md says it in words, a table as the construction blind run's; logs/page.jsonl is what the page
// showed as it changed. A re-score reads the kit's logs, the models copied after each prompt and the wrapper's calls,
// with the scorer as it is now, asks no agent anything, and says in the score that it was re-scored, why, and what the
// scoring before it gave. Either way the scrubbed copy for docs/evidence/ is written again (evidence.mjs).
import { readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";

import { evidence } from "./evidence.mjs";
import { MODEL as FILE, answer, callsIn, digestOf, facts, score } from "./harness.mjs";

/** The checks of `scored` that failed, by name. */
const failed = (scored) => (scored?.checks ?? []).filter((check) => !check.pass).map((check) => check.name);

/** The score in words, a table as the blind run's, for the evidence. */
export function words(kept) {
  const rows = (kept.scored?.checks ?? []).map(
    (c) => `| ${c.name} | ${c.pass ? "pass" : "FAIL"}${c.required ? "" : " (noted)"}${c.carried ? " (as observed in the run)" : ""} |`,
  );
  const prompts = kept.prompts.map(
    (p, k) => `| ${k + 1} | ${p.text} | ${p.calls} | ${p.ended?.num_turns ?? "?"} | ${p.ended?.total_cost_usd ?? "?"} |`,
  );
  const rescored = (kept.rescored ?? []).map((again) => {
    const before = again.was.pass ? "Pass" : `Fail, failing: ${again.was.failed.join("; ") || "none"}`;
    return `Re-scored ${again.at} from the kit's logs, no agent run again: ${again.why} The scoring before it: ${before}.`;
  });
  return [
    `# The live agent test, ${kept.dry ? "dry run (a stand-in played the agent)" : kept.model}`,
    "",
    `Run ${kept.started}, the agent's model ${kept.prompts[0]?.model ?? "unknown"}, the page found by ${kept.opened ?? "nothing"}.`,
    ...rescored.flatMap((line) => ["", line]),
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

/**
 * Writes the run `kept` into the kit `root`: score.json, score.md and, if `seen` is given, logs/page.jsonl; then the
 * scrubbed copy, `occam` being the real command. Returns the copies that still name the home directory.
 */
export async function record(root, kept, occam, seen = null) {
  await writeFile(join(root, "score.json"), `${JSON.stringify(kept, null, 2)}\n`);
  await writeFile(join(root, "score.md"), words(kept));
  if (seen) await writeFile(join(root, "logs", "page.jsonl"), seen.map((now) => `${JSON.stringify(now)}\n`).join(""));
  return (await evidence(root, occam)).leaks;
}

/**
 * The kit `root` scored again from its logs with `occam`, for the reason `why`: its score.json, score.md and
 * evidence/ rewritten, and what was kept returned. A kit that kept no record of the faces the page drew at the end
 * carries that check's result over from the scoring before, marked as observed in the run.
 */
export async function rescore(root, occam, why) {
  const kept = JSON.parse(await readFile(join(root, "score.json"), "utf8"));
  const logs = join(root, "logs");
  const kit = { root, logs, case: join(root, "case"), model: join(root, "case", FILE), occam };
  const [first, second] = [join(logs, "first.occam"), join(logs, "second.occam")];
  const before = await facts(kit, "first", first);
  const after = await facts(kit, "second", second);
  const id = kept.selected?.id ?? JSON.parse(await readFile(`${kit.model}.selection`, "utf8")).ids[0];
  const description =
    kept.selected?.description ?? (await answer(occam, ["describe", "--model", first, id])).answers[0].description;
  const seen = (await readFile(join(logs, "page.jsonl"), "utf8")).split("\n").filter(Boolean).map((l) => JSON.parse(l));
  const digest = await digestOf(occam, second);
  const calls = callsIn(await readFile(join(logs, "calls.log"), "utf8"));
  const scored = score({ before, after, selected: { id, description }, seen, shown: kept.shown ?? null, digest, calls });
  if (!kept.shown) {
    const name = "the page shows the cylinder";
    const was = kept.scored?.checks?.find((check) => check.name === name);
    scored.checks = scored.checks.map((check) =>
      check.name === name && was ? { ...check, pass: was.pass, carried: true, detail: was.detail } : check,
    );
    scored.pass = scored.checks.every((check) => check.pass || !check.required);
  }
  const at = new Date().toISOString();
  kept.rescored = [...(kept.rescored ?? []), { at, why, was: { pass: Boolean(kept.scored?.pass), failed: failed(kept.scored) } }];
  kept.selected ??= { id, description };
  kept.scored = scored;
  kept.leaks = await record(root, kept, occam);
  return kept;
}
