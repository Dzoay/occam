# Command line

[Architecture](../ARCHITECTURE.md) › Command line · crate `occam-cli`

The bracket test's agent has only the `occam` command and its reports. Every command takes `--json`, takes many ids,
and gives one result per id.

| Command | What it does |
|---|---|
| `occam run --model part.occam ops.json` | Runs a [request](operations.md) and writes the model back |
| `occam list --kind edge` | Lists every entity of a kind |
| `occam adjacent edge:41/1` | Lists an entity's neighbours |
| `occam describe edge:41/1 face:5/1` | Describes entities, with the [one resolver](reports.md#how-an-agent-learns-what-an-entity-is) |
| `occam check` | Checks the whole model |
| `occam mesh` | Meshes the model |
| `occam export --step` | Writes STEP |

A step's outcome lists what it created, changed and deleted, by id.
