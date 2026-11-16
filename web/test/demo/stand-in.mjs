// A stand-in for `claude -p` in the live demo's dry run (`just agent-demo --dry-run`): it takes the live run's command
// line, makes the scripted demo's calls through the `occam` first on its PATH, the wrapper, as the agent would, and
// answers in Claude Code's stream-json form, a JSON event a line, with a session id that `--resume` gives back. No
// model is asked anything.
import { execFile } from "node:child_process";
import { randomUUID } from "node:crypto";
import { writeFile } from "node:fs/promises";
import { promisify } from "node:util";

import { scripted, sleep } from "./harness.mjs";

const exec = promisify(execFile);
const argv = process.argv.slice(2);
const given = (flag) => (argv.includes(flag) ? argv[argv.indexOf(flag) + 1] : null);
const resumed = given("--resume");
const session = resumed ?? randomUUID();
const say = (event) => process.stdout.write(`${JSON.stringify({ ...event, session_id: session })}\n`);

say({ type: "system", subtype: "init", model: given("--model"), cwd: process.cwd(), stand_in: true });
let turns = 0;
const agent = {
  async occam(args) {
    turns += 1;
    say({ type: "assistant", tool: "Bash", command: ["occam", ...args].join(" ") });
    return (await exec("occam", args, { maxBuffer: 1 << 28 }).catch((refused) => refused)).stdout;
  },
  async write(name, text) {
    turns += 1;
    say({ type: "assistant", tool: "Write", file: name });
    await writeFile(name, text);
  },
};
// As a model would, the stand-in takes a moment between the highlight and the run, so the page shows the highlight.
await (resumed ? scripted.act(agent, () => sleep(1000)) : scripted.build(agent));
const result = `The stand-in made the scripted demo's calls for: ${given("-p")}`;
say({ type: "result", subtype: "success", is_error: false, num_turns: turns, result, total_cost_usd: 0 });
