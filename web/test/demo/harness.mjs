// The end-to-end demo (docs/plan/view.md, `view.agent`): an agent starts `occam view` and builds a plate with a hole
// while headless Chromium follows the page; a person selects the plate's top face there; the agent reads the
// selection, highlights it and stands a cylinder on the face's centre. What the scripted test (../demo.test.mjs) and the
// live agent test (live.mjs) share is here: the kit the agent works in, the page following the model, the person's
// click, the scripted agent's calls, the wrapper's log read back, and the score.
import { execFile } from "node:child_process";
import { chmod, copyFile, mkdir, readFile, writeFile } from "node:fs/promises";
import { join } from "node:path";
import { fileURLToPath } from "node:url";
import { promisify } from "node:util";

const exec = promisify(execFile);
const HERE = fileURLToPath(new URL(".", import.meta.url));

/** The model's file, in the agent's directory. */
export const MODEL = "part.occam";
/** What a person asks for first: a plate with a hole. */
export const FIRST =
  "Using occam, make a plate 80 mm long, 60 mm wide and 10 mm thick, lying flat on the xy plane, with a round hole " +
  "12 mm across through it, its centre 20 mm from one short end and midway across. Keep it in part.occam.";
/** What they ask once they have selected the plate's top face in the page, as the plan words it. */
export const SECOND = "stand a cylinder of radius 5 and height 20 on the centre of the selected face";
/** What the prompts ask for, which the score holds the models to. */
export const ASKED = { plate: { size: [80, 60, 10], hole: 6 }, cylinder: { radius: 5, height: 20 } };

export const sleep = (ms) => new Promise((resolve) => setTimeout(resolve, ms));
export const request = (steps) => JSON.stringify({ schema: 1, units: { length: "mm", angle: "deg" }, steps });

/**
 * The agent's kit under `root`: `case/`, the directory it works in; `bin/occam`, the logging wrapper it finds first on
 * its PATH, allowing `calls` a prompt and logging them to `log`; `bin/browser`, which `occam view` opens the page with
 * and which notes the address; and `logs/`. `occam` is the real command. `env(prompt)` is the environment the agent
 * runs in.
 */
export async function kit(root, occam, calls = 12) {
  const dirs = { root, case: join(root, "case"), bin: join(root, "bin"), logs: join(root, "logs") };
  for (const dir of [dirs.case, dirs.bin, dirs.logs]) await mkdir(dir, { recursive: true });
  for (const [from, to] of [["occam.sh", "occam"], ["browser.sh", "browser"]]) {
    await copyFile(join(HERE, from), join(dirs.bin, to));
    await chmod(join(dirs.bin, to), 0o755);
  }
  const files = { log: join(dirs.logs, "calls.log"), opened: join(dirs.logs, "opened.txt"), model: join(dirs.case, MODEL) };
  const added = (prompt) => ({
    PATH: `${dirs.bin}:${process.env.PATH ?? ""}`,
    BROWSER: join(dirs.bin, "browser"),
    OCCAM_DEMO_REAL: occam,
    OCCAM_DEMO_LOG: files.log,
    OCCAM_DEMO_OPENED: files.opened,
    OCCAM_DEMO_PROMPT: String(prompt),
    OCCAM_DEMO_CALLS: String(calls),
  });
  return { ...dirs, ...files, occam, allowed: calls, env: (prompt) => ({ ...process.env, ...added(prompt) }) };
}

/** What the real `occam` answers `args` in JSON, whatever its exit status: the harness's own calls, never logged. */
export async function answer(occam, args) {
  const { stdout } = await exec(occam, [...args, "--json"], { maxBuffer: 1 << 28 }).catch((refused) => refused);
  return JSON.parse(stdout);
}

/** The digest of the model in `file` as it is now, as the viewer and its page name it, or null if it does not read. */
export async function digestOf(occam, file) {
  const picked = await answer(occam, ["pick", "--model", file, "--from", "0,0,1e9", "--direction", "0,0,1"]);
  return picked.digest ?? null;
}

/** What `check` gives once it gives something, within `wait` ms, or null. */
export async function until(check, wait = 30_000) {
  for (const start = Date.now(); Date.now() - start < wait; await sleep(100)) {
    const found = await check();
    if (found) return found;
  }
  return null;
}

/**
 * The address of the page the agent's `occam view` opened through `bin/browser`, and by which it was found. A viewer
 * that runs with no page opened, as `--no-open` starts one, gives its address by `--status` after five seconds, or once
 * `done()`; until `done()` says to stop, it waits.
 */
export async function opened(kit, done) {
  for (let since = null; ; await sleep(250)) {
    const noted = (await readFile(kit.opened, "utf8").catch(() => "")).split("\n").find(Boolean);
    if (noted) return { url: noted, by: "browser" };
    const status = await answer(kit.occam, ["view", "--model", kit.model, "--status"]).catch(() => null);
    since = status?.url ? (since ?? Date.now()) : null;
    if (status?.url && (Date.now() - since > 5000 || done())) return { url: status.url, by: "status" };
    if (done()) return null;
  }
}

/**
 * Headless Chromium on the page at `url`, as the person watches it, with `?inspect` for the harness to read it and
 * two mesh workers. What it shows is noted on `seen` as it changes, every 100 ms: the digest of the model drawn, the
 * selection and the highlight. `stop()` stops noting.
 */
export async function follow(browser, url) {
  const page = await browser.newPage();
  page.setDefaultTimeout(120_000);
  const errors = [];
  page.on("pageerror", (error) => errors.push(String(error)));
  await page.goto(`${url}?inspect&workers=2`);
  await page.waitForFunction(() => window.occamViewer);
  const seen = [];
  let watching = true;
  const watched = (async () => {
    for (let last = ""; watching; await sleep(100)) {
      const now = await page.evaluate(() => {
        const viewer = window.occamViewer;
        return { digest: viewer.live()?.digest ?? null, selection: [...viewer.selection()], highlight: [...viewer.highlight()] };
      }).catch(() => null);
      const key = JSON.stringify(now);
      if (now && key !== last) seen.push({ at: new Date().toISOString(), ...now });
      last = key;
    }
  })();
  const stop = async () => {
    watching = false;
    await watched;
  };
  return { page, seen, errors, stop };
}

/** Once the page has drawn the model `file` holds now, and its uploads are done: that model's digest. */
export async function drawn(follower, occam, file) {
  const digest = await digestOf(occam, file);
  await follower.page.waitForFunction((d) => window.occamViewer.live()?.digest === d, digest);
  await follower.page.evaluate(() => window.occamViewer.settled());
  return digest;
}

/** `m · (x, y, z, 1)`, `m` column-major. */
function project(m, [x, y, z]) {
  return [0, 1, 2, 3].map((row) => m[row] * x + m[4 + row] * y + m[8 + row] * z + m[12 + row]);
}

/** Where in the window the model's point `point` is drawn, through the camera the page drew with. */
async function pixelOf(page, point) {
  const origin = await page.evaluate(() => window.occamViewer.origin());
  const matrix = await page.evaluate(() => window.occamViewer.readCamera());
  const [x, y, , w] = project(matrix, point.map((p, axis) => p - origin[axis]));
  const box = await page.locator("#view").boundingBox();
  return [box.x + ((x / w + 1) / 2) * box.width, box.y + ((1 - y / w) / 2) * box.height];
}

/** The triangles of face `face` of `mesh`, each as its three points. */
function triangles(mesh, face) {
  const { first, count } = face.triangles;
  const point = (k) => [0, 1, 2].map((axis) => mesh.positions[(face.vertices.first + k) * 3 + axis]);
  return Array.from({ length: count }, (_, t) => [0, 1, 2].map((corner) => point(mesh.indices[(first + t) * 3 + corner])));
}

const minus = (a, b) => a.map((x, k) => x - b[k]);
const dot = (a, b) => a.reduce((sum, x, k) => sum + x * b[k], 0);
const cross = (a, b) => [a[1] * b[2] - a[2] * b[1], a[2] * b[0] - a[0] * b[2], a[0] * b[1] - a[1] * b[0]];
const area = ([a, b, c]) => Math.hypot(...cross(minus(b, a), minus(c, a))) / 2;

/** The face of `mesh` on top: of those whose every vertex is at the greatest height, the largest. */
function topFace(mesh) {
  const heights = mesh.faces.map((face) => triangles(mesh, face).flat().map((p) => p[2]));
  const highest = Math.max(...heights.flat());
  const near = 1e-9 * Math.max(1, Math.abs(highest));
  const flat = mesh.faces.filter((_, k) => heights[k].every((z) => highest - z <= near));
  const size = (face) => triangles(mesh, face).reduce((sum, t) => sum + area(t), 0);
  return flat.reduce((best, face) => (size(face) > size(best) ? face : best));
}

/**
 * The person's click on the top face of what the page draws, at the centre of its largest triangle; once the page has
 * selected it and `occam selection` describes it, its id and its description.
 */
export async function selectTop(follower, kit) {
  const { page } = follower;
  const mesh = await page.evaluate(async () => {
    await window.occamViewer.settled();
    return window.occamViewer.mesh();
  });
  const face = topFace(mesh);
  const largest = triangles(mesh, face).reduce((best, t) => (area(t) > area(best) ? t : best));
  const [x, y] = await pixelOf(page, [0, 1, 2].map((axis) => largest.reduce((sum, p) => sum + p[axis], 0) / 3));
  await page.mouse.click(x, y);
  await page.waitForFunction((id) => window.occamViewer.selection().join() === id, face.id);
  const description = await until(async () => {
    const said = await answer(kit.occam, ["selection", "--model", kit.model]);
    return said.answers?.length === 1 && said.answers[0].entity === face.id && said.answers[0].description;
  });
  if (!description) throw new Error(`the page selected ${face.id}, and occam selection never described it`);
  return { id: face.id, description };
}

/** The value named `name` of a description or a measure. */
export const valueOf = (values, name) => values.find((value) => value.name === name)?.value;
/** The middle of a face's box, from its description. */
export const middleOf = (description) => {
  const [low, high] = ["low", "high"].map((end) => valueOf(description.values, end));
  return low.map((x, axis) => (x + high[axis]) / 2);
};

/** The plate the first prompt asks for, as a request: its outline with the hole, drawn on z = 0 and extruded up. */
export const PLATE = request([
  {
    op: "face",
    label: "outline",
    outer: { points: [[0, 0, 0], [80, 0, 0], [80, 60, 0], [0, 60, 0]], closed: true },
    holes: [{ edges: [{ circle: { centre: [20, 30, 0], radius: 6, normal: [0, 0, 1] } }] }],
  },
  { op: "extrude", label: "plate", face: "outline", vector: [0, 0, 10] },
]);

/**
 * What the agent may do: the live run gives it to Claude Code as `--tools` and `--allowedTools` under `--permission-mode
 * dontAsk`, where what is not allowed is refused, and the scripted agent is held to it. Its tools, and the rules that
 * let it use them: the shell for `occam` alone, and the file tools.
 */
export const PERMISSIONS = {
  tools: ["Bash", "Read", "Write", "Edit", "Skill"],
  allowed: ["Bash(occam *)", "Read", "Write", "Edit", "Skill"],
};

const literal = (text) => text.replace(/[.*+?^${}()|[\]\\]/g, "\\$&");

/**
 * Whether `permissions` let the agent use `tool`, and for Bash run `command`: one command, with no operator,
 * redirection or substitution, that a `Bash(...)` rule matches, `*` standing for anything. A model of Claude Code's
 * check, stricter than it, and not Claude Code itself; it refuses what the live run's agent was refused, a request
 * written by `cat` and a heredoc.
 */
export function permits(permissions, tool, command = "") {
  if (!permissions.tools.includes(tool)) return false;
  if (tool !== "Bash") return permissions.allowed.includes(tool);
  if (/[;&|<>`$\n\\]/.test(command)) return false;
  const rules = permissions.allowed.map((rule) => /^Bash\((.*)\)$/.exec(rule)?.[1]).filter(Boolean);
  return rules.some((rule) => new RegExp(`^${rule.split("*").map(literal).join(".*")}$`).test(command));
}

/**
 * The scripted agent: the calls an agent following the skill makes, through `agent.bash(command)`, which runs a shell
 * command where it works and gives its standard output, and `agent.write(name, text)`, its file tool.
 */
export const scripted = {
  /** The first prompt: start the viewer, then build the plate in one run. `started()` is awaited between them. */
  async build(agent, started = async () => {}) {
    await agent.bash(`occam view --model ${MODEL}`);
    await started();
    await agent.write("plate.json", PLATE);
    await agent.bash(`occam run --model ${MODEL} plate.json`);
  },
  /**
   * The second: read the selection, highlight it, stand the cylinder on its centre along its normal, and measure what
   * the run made. `shown(id)` is awaited between the highlight and the run; `place(description)` gives where the
   * cylinder stands, the middle of the face's box as the skill has it unless a test says otherwise.
   */
  async act(agent, shown = async () => {}, place = middleOf) {
    const { entity, description } = JSON.parse(await agent.bash(`occam selection --model ${MODEL} --json`)).answers[0];
    await agent.bash(`occam highlight --model ${MODEL} ${entity}`);
    await shown(entity);
    const frame = { origin: place(description), z: valueOf(description.values, "normal") };
    await agent.write("cylinder.json", request([{ op: "cylinder", label: "boss", radius: 5, height: 20, frame }]));
    const ran = JSON.parse(await agent.bash(`occam run --model ${MODEL} cylinder.json --json`));
    await agent.write("check.json", request([{ op: "measure", target: ran.steps[0].made, what: ["volume", "bounds"] }]));
    await agent.bash(`occam run --model ${MODEL} check.json`);
  },
};

/**
 * An agent for `scripted`, working in `dir` with the environment `env`, held to `permissions` as Claude Code holds the
 * live run's agent: a shell command or a file write they do not permit is refused, and throws.
 */
export function heldAgent(dir, env, permissions = PERMISSIONS) {
  return {
    async bash(command) {
      if (!permits(permissions, "Bash", command)) throw new Error(`refused, as the live run's agent would be: ${command}`);
      return (await exec("sh", ["-c", command], { cwd: dir, env, maxBuffer: 1 << 28 }).catch((refused) => refused)).stdout;
    },
    async write(name, text) {
      if (!permits(permissions, "Write")) throw new Error(`refused, as the live run's agent would be: Write ${name}`);
      await writeFile(join(dir, name), text);
    },
  };
}

/** The agent for `scripted` that works in `kit` on prompt `prompt`, held to the live run's permissions. */
export const scriptedAgent = (kit, prompt) => heldAgent(kit.case, kit.env(prompt));

/** The wrapper's log read back: each call's number, prompt, arguments, request files, standard output and status. */
export function callsIn(log) {
  return log
    .split(/^=== /m)
    .filter((block) => block.startsWith("call ") || block.startsWith("refused "))
    .map((block) => {
      const head = block.slice(0, block.indexOf("\n")).split(" ");
      const argv = (/^argv:(.*)$/m.exec(block)?.[1] ?? "").match(/(?:\\.|[^ \\])+/g) ?? [];
      const files = [...block.matchAll(/^--- file (.*)\n([\s\S]*?)\n(?=--- )/gm)].map(([, name, text]) => ({ name, text }));
      const stdout = /^--- stdout\n([\s\S]*?)\n--- stderr$/m.exec(block)?.[1] ?? "";
      const exit = Number(/^--- exit (\d+)$/m.exec(block)?.[1] ?? -1);
      const prompt = Number(head[0] === "call" ? head[3] : head[2]);
      return { refused: head[0] === "refused", prompt, argv: argv.map((a) => a.replace(/\\(.)/g, "$1")), files, stdout, exit };
    });
}

/**
 * Everything the score reads of the model in `kit` now, or of the model file `from`, from a copy named `name` in the
 * logs, which a re-score reads again.
 */
export async function facts(kit, name, from = kit.model) {
  const copy = join(kit.logs, `${name}.occam`);
  if (from !== copy) await copyFile(from, copy);
  const listed = await answer(kit.occam, ["list", "--model", copy, "--kind", "body,face"]);
  const [bodies, faces] = (listed.answers ?? []).map((kind) => kind.entities);
  const described = await answer(kit.occam, ["describe", "--model", copy, ...(faces ?? [])]);
  const measure = join(kit.logs, `${name}.measure.json`);
  const what = ["volume", "centre-of-mass", "bounds"];
  await writeFile(measure, request((bodies ?? []).map((target) => ({ op: "measure", target, what }))));
  const ran = await answer(kit.occam, ["run", "--model", copy, measure]);
  return {
    bodies: bodies ?? [],
    faces: Object.fromEntries((described.answers ?? []).map((a) => [a.entity, a.description])),
    measured: Object.fromEntries((ran.steps ?? []).map((step) => [step.measured.target, step.measured.values])),
    band: ran.tolerance?.length ?? 1e-7,
  };
}

const near = (got, want, within) => typeof got === "number" && Math.abs(got - want) <= within;
const nearAll = (got, want, within) => got?.length === want.length && got.every((x, k) => near(x, want[k], within));
const unit = (v) => v.map((x) => x / Math.hypot(...v));

/** The plate after the first prompt: one body, of the volume and size asked, its top face holed. */
function plateChecks({ before, selected }) {
  const { size, hole } = ASKED.plate;
  const [body] = before.bodies;
  const values = before.measured[body] ?? [];
  const volume = size[0] * size[1] * size[2] - Math.PI * hole * hole * size[2];
  const [low, high] = ["bounds-low", "bounds-high"].map((name) => valueOf(values, name) ?? []);
  const extents = low.map((x, axis) => high[axis] - x).sort((a, b) => b - a);
  return [
    ["the plate is one body", before.bodies.length === 1, before.bodies],
    ["of the volume asked", near(valueOf(values, "volume"), volume, 1e-7 * volume), valueOf(values, "volume")],
    ["and the size", nearAll(extents, size, 1e-6), extents],
    ["the face selected is its top, with the hole through it", valueOf(selected.description.values, "loops") === 2, selected.id],
  ];
}

/** The cylinder after the second prompt, against the face selected before it. */
function cylinderChecks({ before, after, selected }) {
  const { radius, height } = ASKED.cylinder;
  const made = after.bodies.filter((body) => !before.bodies.includes(body));
  const faces = Object.keys(after.faces).filter((id) => !(id in before.faces)).map((id) => ({ id, ...after.faces[id] }));
  const values = after.measured[made[0]] ?? [];
  const band = after.band;
  const face = selected.description;
  const centre = middleOf(face);
  const normal = unit(valueOf(face.values, "normal"));
  const plate = valueOf(before.measured[before.bodies[0]] ?? [], "centre-of-mass") ?? centre;
  const out = dot(minus(plate, centre), normal) < 0 ? normal : normal.map((x) => -x);
  const along = (point) => dot(minus(point, centre), out);
  const across = (point) => Math.hypot(...minus(minus(point, centre), out.map((x) => x * along(point))));
  const planes = faces.filter((f) => f.form === "plane");
  const radii = faces.filter((f) => f.form === "cylinder").map((f) => valueOf(f.values, "radius"));
  const base = planes.find((f) => near(along(middleOf(f)), 0, band));
  const holes = Object.values(before.faces).filter((f) => f.form === "cylinder");
  const clear = holes.every((f) => across(valueOf(f.values, "origin")) >= valueOf(f.values, "radius") + radius - band);
  const [low, high] = ["low", "high"].map((end) => valueOf(face.values, end));
  const inside = base && ["low", "high"].every((end) => valueOf(base.values, end).every((x, k) => x >= low[k] - band && x <= high[k] + band));
  const volume = Math.PI * radius * radius * height;
  const mass = valueOf(values, "centre-of-mass");
  return [
    ["one body more", made.length === 1, made],
    ["a cylinder of the volume asked", near(valueOf(values, "volume"), volume, 1e-7 * volume), valueOf(values, "volume")],
    ["of the radius asked", radii.length > 0 && radii.every((r) => near(r, radius, band)), radii],
    ["and the height", planes.length === 2 && planes.some((f) => near(along(middleOf(f)), height, band)), planes.map((f) => along(middleOf(f)))],
    ["its base lies in the selected face's plane", Boolean(base), base?.id ?? null],
    ["centred on the face, within the band", base && near(across(middleOf(base)), 0, band), base && across(middleOf(base))],
    ["within the face, clear of its hole", Boolean(inside) && clear, { inside: Boolean(inside), clear }],
    ["standing out of the plate", mass && near(along(mass), height / 2, band) && near(across(mass), 0, band), mass],
  ];
}

/** The page after the second prompt: it drew the model the file holds, the new faces among those it drew. */
function pageChecks({ before, after, seen, shown, digest }) {
  const made = Object.keys(after.faces).filter((id) => !(id in before.faces));
  const digests = [...new Set(seen.map((now) => now.digest).filter(Boolean))];
  const drawn = shown?.digest === digest && made.length > 0 && made.every((id) => shown.faces.includes(id));
  return [
    ["the page drew each model, the plate and then the cylinder beside it", digests.length >= 2 && digests.at(-1) === digest, digests],
    ["the page shows the cylinder", drawn, made],
  ];
}

/** What the page shows now: the digest of the model drawn, and the faces it draws. */
export const shownBy = (follower) =>
  follower.page.evaluate(() => ({ digest: window.occamViewer.live()?.digest, faces: window.occamViewer.faces().map((f) => f.id) }));

/** Whether a call is one of the agent's steps: not refused, and not a call for help, which changes nothing. */
const step = (call) => !call.refused && call.argv[0] !== "help" && !call.argv.some((arg) => arg === "--help" || arg === "-h");

/**
 * What the agent did, from the wrapper's log: the viewer started first, and the selection read and highlighted before
 * the run that changed it. Calls for help are no steps, and are skipped. Rows of `[name, pass, detail, required]`.
 */
export function callChecks({ calls, selected }) {
  const command = (call) => call.argv[0];
  const first = calls.filter((call) => call.prompt === 1 && step(call));
  const viewed = first.findIndex((call) => command(call) === "view" && !call.argv.includes("--stop") && !call.argv.includes("--status"));
  const ran = first.findIndex((call) => command(call) === "run");
  const second = calls.filter((call) => call.prompt === 2 && step(call));
  const read = second.findIndex((call) => command(call) === "selection");
  const lit = second.findIndex((call, k) => k > read && command(call) === "highlight" && call.argv.includes(selected.id) && call.exit === 0);
  const made = second.findIndex((call) => command(call) === "run" && call.exit === 0 && call.files.some((f) => f.text.includes('"cylinder"')));
  const measured = second.findIndex((call, k) => k > made && command(call) === "run" && call.files.some((f) => f.text.includes('"measure"')));
  return [
    ["the agent started the viewer before its first run", viewed >= 0 && (ran < 0 || viewed < ran), { viewed, ran }],
    ["it read the selection, highlighted it, then ran", read >= 0 && lit > read && made > lit, { read, lit, made }],
    ["it measured what it made", measured > made && made >= 0, { measured }, false],
    ["no call was refused for the cap", calls.every((call) => !call.refused), calls.filter((call) => call.refused).length, false],
  ];
}

/**
 * The demo scored, as both tests score it, and from a kit's logs again. `before` and `after` are the facts after each
 * prompt, `selected` what the person selected, `seen` what the page showed as it changed and `shown` what it showed at
 * the end (`shownBy`), `digest` the model's digest at the end and `calls` the wrapper's log read back. A check marked
 * `required: false` is noted and does not decide the pass.
 */
export function score({ before, after, selected, seen, shown, digest, calls }) {
  const rows = [
    ...plateChecks({ before, selected }),
    ...cylinderChecks({ before, after, selected }),
    ...pageChecks({ before, after, seen, shown, digest }),
    ...callChecks({ calls, selected }),
  ];
  const checks = rows.map(([name, pass, detail, required = true]) => ({ name, pass: Boolean(pass), required, detail }));
  return { pass: checks.every((check) => check.pass || !check.required), checks };
}
