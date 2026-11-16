# Command line

[Architecture](../ARCHITECTURE.md) › Command line · crate `occam-cli`

The bracket test's agent has only the `occam` command and its reports. Every command takes `--json`, takes many ids,
and gives one result per id. `occam --help`, and `--help` after any command, say what each takes
([clap](../decisions/2026-11-13-clap.md) writes them).

| Command | What it does |
|---|---|
| `occam run --model part.occam ops.json` | Runs a [request](operations.md) and writes the model back |
| `occam list --model part.occam --kind edge,face` | Lists every live entity of each kind, in slot order |
| `occam adjacent --model part.occam edge:41/1` | Lists each entity's neighbours |
| `occam describe --model part.occam edge:41/1 face:5/1` | Describes entities, with the [one resolver](reports.md#how-an-agent-learns-what-an-entity-is) |
| `occam check --model part.occam` | Checks the whole model |
| `occam mesh --model part.occam --deflection 0.1` | Meshes the model, writes its [buffers](mesh-and-renderer.md#the-buffers) beside it, and says how each face meshed |
| `occam pick --model part.occam --from 45,20,5 --towards 30,20,5` | The face or edge a ray hits first, by [picking](mesh-and-renderer.md#picking) the mesh the viewer's page draws |
| `occam view --model part.occam` | Starts the model's [live view](live-view.md) if none runs, prints the page's address and returns; `--status`, `--stop` |
| `occam selection --model part.occam` | Describes what the person selected in the [live view](live-view.md#the-selection-file) |
| `occam highlight --model part.occam face:9/1` | Shows ids in the [live view](live-view.md#the-selection-file) in a colour of their own; `--clear` |
| `occam skill --install` | Copies the occam skill into this project's `.claude/skills/`, for the [live view](live-view.md) (`view`) |
| `occam export --step` | Writes STEP (`exchange`) |

```mermaid
flowchart LR
  file[("part.occam<br/>JSON or binary twin")] -->|"open, check_all"| ses["Session"]
  req(["ops.json"]) --> ses
  ses -->|"whole request went through"| file
  ses --> out(["One JSON form, or words"])
```

- **A step's outcome** lists what it created, changed and deleted, by id.
- **`run` is the whole request or nothing.** The model is written back only if every step went through, so a request
  that stopped short can be mended and run again on the same file. A missing file is a new model. A model keeps its
  encoding; a new one is the readable JSON, or the [binary twin](files.md) with `--binary`. A model the format cannot
  write yet is not written, and the run says so with `io.not-saved`.
- **`check` opens the model,** which measures every edge and checks the whole model as `commit` does, so a model
  that opens is sound.
- **One answer per id.** An id that names nothing is `topo.stale-id` for that id alone, and the others are answered.
- **The JSON form** of `list`, `adjacent`, `describe` and `check` is `{"schema", "ok", "answers", "reports"}`: an answer
  per id or kind, each with the report refusing it if it has none, and the reports that refused the command whole or
  that the check found. `run` gives the run's own form. `mesh` adds the limits, the buffers' counts and the file it
  wrote; each face's answer is its vertex block and its triangles, and its `reports` refuse edges or say where the
  watertight check found the mesh open. A face or edge refused offers `use-deflection`, which `--deflection` takes.
  `view` gives `{"schema", "ok", "model", "running", "started", "stopped", "reports"}`, and where a viewer runs its
  `url`, `token`, `port`, `pid`, open `pages`, the model's `digest` and whether it `opened` the page; its `reports` say
  why the command was refused or why the model does not read. `pick` takes `--from`, and `--direction` or
  `--towards`, an edge `--tolerance` (0 unless given), and the viewer's page's `--deflection` 0.1 and `--angle` 0.35
  unless given; it gives `{"schema", "ok", "digest", "from", "direction", "tolerance", "entity", "point",
  "distance", "reports"}`, the direction of length one, and `entity` null with `mesh.ray-missed` first in `reports`
  for a ray that hits nothing. `selection` and `highlight` give `describe`'s form for the ids they read or write. A
  `run` that carries the selection over adds `selection`, and one that clears the highlight `highlight-cleared`.
- **In words,** answers go to standard output and refusals to standard error.
- **The exit status** is 0 when everything asked was answered, 1 when anything was refused, and 2 when the command line
  is wrong (`cli.bad-argument`, naming the argument, what was given and what was wanted).
