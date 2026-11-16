// A stand-in for `claude -p` in the live demo's dry run (`just agent-demo --dry-run`): it takes the live run's command
// line, makes the scripted demo's calls through the `occam` first on its PATH, the wrapper, as the agent would, held to
// the tools and rules that command line gives (`--tools`, `--allowedTools`), and answers in Claude Code's stream-json
// form, a JSON event a line, with a session id that `--resume` gives back. No model is asked anything.
import { randomUUID } from "node:crypto";

import { heldAgent, scripted, sleep } from "./harness.mjs";

const argv = process.argv.slice(2);
const given = (flag) => (argv.includes(flag) ? argv[argv.indexOf(flag) + 1] : null);
/** The values a flag that takes many is given: those after it, up to the next flag. */
const many = (flag) => {
  const rest = argv.includes(flag) ? argv.slice(argv.indexOf(flag) + 1) : [];
  const end = rest.findIndex((value) => value.startsWith("--"));
  return end < 0 ? rest : rest.slice(0, end);
};
const permissions = { tools: (given("--tools") ?? "").split(",").filter(Boolean), allowed: many("--allowedTools") };
const resumed = given("--resume");
const session = resumed ?? randomUUID();
const say = (event) => process.stdout.write(`${JSON.stringify({ ...event, session_id: session })}\n`);

say({ type: "system", subtype: "init", model: given("--model"), cwd: process.cwd(), stand_in: true, ...permissions });
let turns = 0;
const held = heldAgent(process.cwd(), process.env, permissions);
const agent = {
  async bash(command) {
    turns += 1;
    say({ type: "assistant", tool: "Bash", command });
    return held.bash(command);
  },
  async write(name, text) {
    turns += 1;
    say({ type: "assistant", tool: "Write", file: name });
    return held.write(name, text);
  },
};
// As a model would, the stand-in takes a moment between the highlight and the run, so the page shows the highlight.
const result = await (resumed ? scripted.act(agent, () => sleep(1000)) : scripted.build(agent)).then(
  () => ({ subtype: "success", is_error: false, result: `The stand-in made the scripted demo's calls for: ${given("-p")}` }),
  (error) => ({ subtype: "error_during_execution", is_error: true, result: String(error?.message ?? error) }),
);
say({ type: "result", ...result, num_turns: turns, total_cost_usd: 0 });
