// The live demo's kit made ready for docs/evidence/: a copy, in the kit's evidence/, of the score, the calls, each
// prompt's transcript, what the page showed and the command lines, with nothing in it of the person's own setup or of
// this machine. A transcript's first event, Claude Code's init, keeps only what says what the agent was given; its
// lists of the skills, plugins, slash commands and agents installed, its paths and its account's settings are dropped,
// and so are the account's rate-limit events. Every path of this machine is written as `$KIT` (the kit), `$OCCAM` (the
// real command) or `~` (the home directory), and `-HOME` where Claude Code names a directory after one.
import { mkdir, readFile, realpath, writeFile } from "node:fs/promises";
import { homedir } from "node:os";
import { dirname, join } from "node:path";

/** The files copied, from the kit. */
export const KEPT = [
  "score.md",
  "score.json",
  "logs/calls.log",
  "logs/page.jsonl",
  "logs/command-lines.txt",
  "logs/prompt-1.jsonl",
  "logs/prompt-2.jsonl",
];
/** What the init event keeps: what says which model and tools the agent had, and under which permissions. */
export const INIT = ["type", "subtype", "session_id", "model", "tools", "permissionMode", "claude_code_version", "stand_in"];
/** Events dropped whole: the account's own state. */
const DROPPED = new Set(["rate_limit_event"]);

/** A transcript's line, its init event kept to `INIT` and an account's event dropped; the rest as it was. */
function line(text) {
  let event;
  try {
    event = JSON.parse(text);
  } catch {
    return text;
  }
  if (DROPPED.has(event?.type)) return null;
  if (event?.type !== "system" || event.subtype !== "init") return text;
  return JSON.stringify(Object.fromEntries(INIT.filter((key) => key in event).map((key) => [key, event[key]])));
}

/**
 * The paths of this machine the copies must not hold, each with what stands for it, longest first: the kit `root`, as
 * given and as resolved, the real command `occam`, and the home directory, also as Claude Code writes it in a
 * directory's name.
 */
async function swaps(root, occam) {
  const home = homedir();
  const pairs = [
    [root, "$KIT"],
    [await realpath(root).catch(() => root), "$KIT"],
    [occam, "$OCCAM"],
    [home, "~"],
    [slug(home), "-HOME"],
  ];
  return pairs.filter(([from]) => from.length > 1).sort(([a], [b]) => b.length - a.length);
}

/** A path as Claude Code writes it in a directory's name, each character but a letter or digit made a dash. */
const slug = (path) => path.replace(/[^A-Za-z0-9]/g, "-");

/** `text` with every path of `pairs` written as what stands for it. */
const scrubbed = (text, pairs) => pairs.reduce((done, [from, to]) => done.replaceAll(from, to), text);

/**
 * Writes the kit `root`'s scrubbed copy under `root/evidence/`, from the files of `KEPT` that are there; `occam` is the
 * real command the kit ran. Returns the files written, and any that still name the home directory, which should be
 * none.
 */
export async function evidence(root, occam) {
  const pairs = await swaps(root, occam);
  const written = [];
  const leaks = [];
  for (const name of KEPT) {
    const text = await readFile(join(root, name), "utf8").catch(() => null);
    if (text === null) continue;
    const lines = name.endsWith(".jsonl") ? text.split("\n").map((l) => (l ? line(l) : l)).filter((l) => l !== null) : [text];
    const copy = scrubbed(lines.join(name.endsWith(".jsonl") ? "\n" : ""), pairs);
    const to = join(root, "evidence", name);
    await mkdir(dirname(to), { recursive: true });
    await writeFile(to, copy);
    written.push(to);
    if (copy.includes(homedir()) || copy.includes(slug(homedir()))) leaks.push(to);
  }
  return { written, leaks };
}
