# Live view

[Architecture](../ARCHITECTURE.md) › Live view · crates `occam-cli`, `occam-mesh`, `occam-wasm`; `web/`

A person prompts an agent in the agent tool they already use. The agent builds with [the `occam` command](command-line.md),
and a page shows the model as each run saves it ([decision](../decisions/2026-11-14-prompt-driven-building-shown-live.md)).
The model file is where the two meet: nothing holds the model in memory between commands.

```mermaid
flowchart LR
  person(["Person"]) -->|prompt| agent["Agent tool"]
  agent -->|"starts"| view["occam view"]
  agent -->|"occam run"| model[("part.occam")]
  view["occam view"] -->|watches| model
  view -->|"model, then each change"| page["Page"]
  person -->|"clicks, orbits"| page
  page -->|selection| view
  view -->|writes| sel[("part.occam.selection")]
  agent -->|"occam selection"| sel
  agent -->|"occam highlight"| hl[("part.occam.highlight")]
  view -->|watches| hl
```

## The commands

| Command | What it does |
|---|---|
| `occam view --model part.occam` | Starts the model's server if none runs, prints the page's address, opens it, and returns |
| `occam view --model part.occam --status`, `--stop` | Says whether the model's server runs and where; stops it |
| `occam pick --model part.occam --from 45,20,5 --towards 30,20,5` | The id a ray hits first, as a click in the page picks |
| `occam selection --model part.occam` | Describes what the person has selected, as `describe` does |
| `occam highlight --model part.occam edge:41/1 ...` | Shows the agent's ids in the page; `--clear` removes them |
| `occam skill --install` | Copies the occam skill into this project's `.claude/skills/occam/`; `occam skill` prints it |

## The server

- **The agent starts it** ([decision](../decisions/2026-11-14-the-agent-starts-the-viewer.md)). `occam view` starts
  the server, the same program run again as a process of its own, waits until it listens and has read the model once,
  prints the address (with the token and the model's digest under `--json`), opens the page unless `--no-open`, and
  exits. It opens the page with the program `BROWSER` names, or the system's opener, and only if no page is open.
- **It waits as long as the server opens the model** ([decision](../decisions/2026-11-17-occam-view-waits-while-its-server-opens.md)),
  saying on standard error every five seconds that it is still opening. A server that ends first answers in its own
  words, its report, or `view.not-started` if it said none. An interrupt stops `occam view` alone; the server, finding
  no one to tell that it listens, stops once it has opened the model and removes its state file.
- **One server per model.** A state file beside the model, `part.occam.view`, names the process, port and token as
  `{"schema", "pid", "port", "token"}`, written whole and readable by its owner alone. A second `occam view` asks that
  server what it watches and prints its address instead of starting another; a state file whose server does not answer
  is stale, and is replaced. A server whose state file comes to name another server stops.
- **It runs until stopped:** by `occam view --stop`, or by itself when no page has been open for an hour.
  `--status` says whether it runs and where, with how many pages are open and why the model does not read.
- **axum on tokio,** in `occam-cli` only ([decision](../decisions/2026-11-14-axum-and-tokio-for-the-viewer.md)), on one
  thread. A model is opened on a blocking thread, never on the async one.
- **The page is in the binary.** `just web-build` builds it into `crates/occam-cli/page/`, which the next build of
  `occam` embeds; a plain `cargo build` embeds a stub page that names the recipe, so `occam view` needs nothing beside
  it either way. The page it serves has its `<html>` marked `data-live`.
- **This machine only.** It listens on `127.0.0.1`, on a free port unless `--port` names one (`view.port-taken` if it
  is taken), and every address begins with the token: `http://127.0.0.1:<port>/<token>/`. Anything else is refused, so
  another page on the machine cannot read the model or post a selection.
- **Change by polling.** It looks at the model file's size and modification time five times a second. When they
  change and the file reads whole, it sends the page an event naming the model's digest. A file that does not read
  may be half-written, so it is read again at the next look, and named only once a look finds it unchanged: the
  kernel's reports for a file that is not a model, and `view.model-missing` for one that is not there, in the page and
  on the command line, while the server keeps watching and the page keeps drawing the last model that read.
- **Routes,** each under the token: the page and its files; `model`, the last model that read whole as [the binary
  twin](files.md), whichever form the file is in, naming its digest in `x-occam-digest`; `events`, a stream of
  server-sent events, `model` when another model reads whole and `refused` when the file does not, each carrying what
  the server says of the model (its digest, its reports and the pages open); `status`, the same, for the command line;
  a post to `stop`; and a post to `selection`, the page's selection, which it writes as the selection file, or refuses
  with 409 and `view.selection-stale` if it was made on another model than the one the server shows. It also sends
  `selection` and `highlight` events, each carrying its file's JSON, or `{"schema", "digest": null, "ids": []}` where
  there is none, when a page opens and each time the file changes.

## The page

- **What [`view`](../plan/view.md) builds.** The [editing worker](browser.md) loads the twin, the mesh workers mesh it,
  and [WebGPU](mesh-and-renderer.md) draws it. An event makes it fetch the model again. The editing worker opens it and
  says which faces changed since the model drawn (`occam_mesh::again`, from the two models alone: the faces drawn from
  anything written since, and those along an edge whose samples changed), and only those are meshed again; every other
  face keeps its block and what was reported of it. One model is fetched and meshed at a time.
- **View controls** move the camera over the same mesh: orbit, pan, zoom, centre on the selection, fit the model to the
  window.
- **Selection.** A click, the left button let go where it went down, picks against the CPU mesh and selects what it
  hit; shift-click adds or removes it, and a click on nothing clears the selection. The page builds the ray in `f64`
  from the camera, from the eye through the pixel, and asks one mesh worker, which meshes its model whole the first
  time and keeps the mesh and its box trees; no GPU buffer is read. An edge wins over its faces within 4 pixels at the
  depth of the face the ray hits, or at the camera's target where it hits none: the worker picks once with no
  tolerance to find that depth, then with the tolerance there, and the answer is `occam pick`'s for that ray and
  tolerance. The page posts the ids with the digest of the model they were picked on, shows them so that a person
  can copy them, and draws the faces selected in a colour of their own and the edges as lines through their samples.
- **Highlight** draws the agent's ids in a second colour, apart from the selection, once the model the highlight was
  made on is drawn. Each face's mark, selected or highlighted, is a number in a storage buffer that its draw reads.

## The selection file

- **JSON beside the model,** `part.occam.selection`: the model's digest and a list of ids, in the order they were
  selected, each once, as `{"schema": 2, "digest": "0123456789abcdef", "ids": ["face:9/1", "edge:12/1"]}`, the digest
  as the server writes it, sixteen hexadecimal digits. Each writer writes a temporary file and renames it over the
  old one, so a reader never sees half a file.
- **`occam selection`** describes each id against the model, in `describe`'s JSON form; with no file, it answers no
  ids. If the file's digest is not the model's, the selection is stale: it is refused with `view.selection-stale`,
  whose remedy is `select-again`, and describes nothing.
- **Carried over by `occam run`.** After a run commits, it maps each selected id through the run's
  [changes](operations.md), step by step: an entity changed keeps its id, one replaced becomes those that replaced it,
  one deleted drops out. Replaced means deleted with a lineage of `split`, `trimmed` or `merged`; what a `copied` or
  `swept` lineage names is new, so a body copied stays selected and a face a sweep took drops out. It writes the file
  again with the new digest, and its result says what became of each id, as `"selection": [{"entity", "now"}]`, `now`
  empty for one dropped, and in words, a line each. It does this only if the file's digest was the model's before the
  run, so a stale selection is never made to look fresh.
- **A post for an older model is refused,** so the page fetches the new model and the person selects again: the page
  shows the report and drops its selection.
- **The highlight file,** `part.occam.highlight`, has the same form. `occam highlight` writes the ids that name
  something live and refuses the rest alone; `occam run` removes the file once it changes the model, since it named
  what the agent was about to change, and its result says `"highlight-cleared": true`. A run that changes nothing,
  such as a measure, clears nothing.

## The skill and the demo

- **The skill,** `skills/occam/SKILL.md`, in Claude Code's form, says what the command's guide does not: run each
  `occam` command alone and write requests with the file tool, since a project may let only `occam` run in the shell;
  start the viewer and say where to look, build in runs that can be checked, read `occam selection` for "the selected", find a
  flat face's centre as the middle of its box, highlight before a change, measure to confirm, and stop the viewer at
  the end. A test runs every example in it, in order, the person's click posted to the viewer before it reads the
  selection.
- **`occam skill`** prints it, as the binary carries it. `--install` writes `.claude/skills/occam/SKILL.md` under the
  current directory and says where; one there already that is this build's is left unchanged, and another is refused
  as `cli.skill-present` unless `--force` replaces it. A project it cannot write into is `cli.skill-not-written`.
- **The demo, scripted,** runs in `just web-test` (`web/test/demo.test.mjs`): the agent's calls, through the
  logging wrapper the live test gives its agent, start `occam view`, whose page headless Chromium opens as the
  person's browser, and build a plate with a hole; a click selects its top face; the agent reads the selection,
  highlights it and stands a cylinder of radius 5 and height 20 on its centre. The score holds the cylinder's base to
  the face's plane and centre within the band, clear of the hole, the page to drawing it, and the calls to the
  skill's order. A cylinder stood on the plane's origin instead fails it.
- **The live agent test,** `just agent-demo`, runs by hand on the person's own key: headless Claude Code on Haiku,
  pinned, with the skill installed in a directory of its own and `occam` behind the wrapper, the shell allowed
  `occam` alone and the file tools, capped in calls a prompt and in spend, resumed for the second prompt. The
  scripted demo's agent is held to the same permissions. It is scored as the scripted demo is and keeps the kit, the
  transcripts and the score in `~/.cache/occam-agent-demo/`, with a copy in `evidence/` for `docs/evidence/` that
  drops the person's installed skills, plugins and commands, their account's state and this machine's paths.
  `--dry-run` has a stand-in play the agent, held to the permissions its command line gives. `--score KIT --why
  REASON` scores a kit again from its logs with the scorer as it is now, and its score says it was re-scored, why,
  and what the scoring before gave. A call for help is no step in the order the score checks.

## What waits

- **An in-memory session server** that the commands talk to, which a web service with the prompt in the browser would
  grow from, and **the in-page chat**: after the first release
  ([decision](../decisions/2026-11-14-prompt-driven-building-shown-live.md)).
- **Editing in the page,** write-through included: with the editing tools
  ([decision](../decisions/2026-11-14-write-through-after-the-first-release.md)).
