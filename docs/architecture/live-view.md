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
| `occam selection --model part.occam` | Describes what the person has selected, as `describe` does |
| `occam highlight --model part.occam edge:41/1 ...` | Shows the agent's ids in the page; `--clear` removes them |
| `occam skill --install` | Copies the occam skill into this project's `.claude/skills/` |

## The server

- **The agent starts it** ([decision](../decisions/2026-11-14-the-agent-starts-the-viewer.md)). `occam view` starts
  the server as a process of its own, waits until it listens, prints the address (with the token under `--json`), and
  exits. A state file beside the model, `part.occam.view`, names the process, port and token, so a second `occam view`
  prints the running server's address instead of starting another; a state file whose process is gone is replaced.
- **It runs until stopped:** by `occam view --stop`, or by itself when no page has been open for an hour.
- **axum on tokio,** in `occam-cli` only ([decision](../decisions/2026-11-14-axum-and-tokio-for-the-viewer.md)). The
  page's built files are in the binary, so `occam view` needs nothing beside it. A recipe builds them; a plain
  `cargo build` embeds a stub page that names the recipe.
- **This machine only.** It listens on `127.0.0.1`, on a free port unless `--port` names one, and every route wants the
  token that is in the URL it opens. Another page on the machine cannot read the model or post a selection.
- **Change by polling.** It looks at the model and highlight files' size and modification time several times a second.
  When one changes and reads whole, it sends the page an event naming the model's digest; a half-written file is read
  again on the next look.
- **Routes:** the page; the model as [the binary twin](files.md), whichever form the file is in; a stream of
  server-sent events (model changed, highlight changed, selection changed); and a post of the selection.

## The page

- **What [`view`](../plan/view.md) builds.** The [editing worker](browser.md) loads the twin, the mesh workers mesh it,
  and [WebGPU](mesh-and-renderer.md) draws it. An event makes it fetch the model again; only the faces that changed are
  meshed again.
- **View controls** move the camera over the same mesh: orbit, pan, zoom, centre on the selection, fit the model to the
  window.
- **Selection.** A click picks against the CPU mesh (an edge wins over its faces within a few pixels) and selects what
  it hit; shift-click adds or removes. The page posts the ids with the digest of the model it shows.
- **Highlight** draws the agent's ids in a second colour, apart from the selection.

## The selection file

- **JSON beside the model:** the model's digest and a list of ids, in the order they were selected. Each writer writes a
  temporary file and renames it over the old one, so a reader never sees half a file.
- **`occam selection`** describes each id against the model. If the file's digest is not the model's, the selection is
  stale: it says so, with the remedy of selecting again, and describes nothing.
- **Carried over by `occam run`.** After a run commits, it maps each selected id through the run's
  [changes](operations.md): an entity changed keeps its id, one replaced becomes those that replaced it, one deleted
  drops out. It writes the file again with the new digest, and its result says what became of each id. It does this
  only if the file's digest was the model's before the run, so a stale selection is never made to look fresh.
- **A post for an older model is refused,** so the page fetches the new model and the person selects again.
- **The highlight file** has the same form. `occam run` clears it, since it named what the agent was about to change.

## What waits

- **An in-memory session server** that the commands talk to, which a web service with the prompt in the browser would
  grow from, and **the in-page chat**: after the first release
  ([decision](../decisions/2026-11-14-prompt-driven-building-shown-live.md)).
- **Editing in the page,** write-through included: with the editing tools
  ([decision](../decisions/2026-11-14-write-through-after-the-first-release.md)).
