# The live agent test: does a cheap agent, given the occam skill, build a part and act on a face a person selects?

The agent test of [`view`'s exit gate](../plan/view.md): headless Claude Code on the cheapest model the
[construction blind run](2026-11-13-construction-blind-run.md) used builds a plate with a hole while a page follows
it live, then stands a cylinder on the face a person selected there. It ran by `just agent-demo` on the person's own
key and account, as [the decision](../decisions/2026-11-14-the-agent-starts-the-viewer.md) has it.

**Sample size: one scored run of two prompts, by one model (`claude-haiku-5-5`), after one run that failed on a
permission and built nothing. It shows that the demo can work end to end, and no more.**

## Question

Given only the occam skill installed in its project, the `occam` command, and a person's two prompts, does the agent
start the viewer, build the part asked for, read what the person selected in the page, highlight it, stand a cylinder
on its centre, and say what it did, with the page showing each change?

## What was measured

**The protocol.** Claude Code 2.1.325, headless (`claude -p`), on `claude-haiku-5-5` pinned by its id, in a directory
of its own outside the repository. `occam skill --install` put the skill in its `.claude/skills/occam/` first. Its
`occam` was the blind run's logging wrapper, first on its PATH, allowing 12 calls a prompt; its tools were Bash, Read,
Write, Edit and Skill, and under `--permission-mode dontAsk` the shell was allowed `occam` alone. Each prompt could
spend at most $0.50. The first prompt:

> Using occam, make a plate 80 mm long, 60 mm wide and 10 mm thick, lying flat on the xy plane, with a round hole 12 mm
> across through it, its centre 20 mm from one short end and midway across. Keep it in part.occam.

Headless Chromium opened the page that the agent's `occam view` opened, and followed each run. When the first prompt
ended, the harness clicked the plate's top face, at the centre of its largest triangle, as a person would, and resumed
the session (`--resume`) with the second prompt, the plan's words:

> stand a cylinder of radius 5 and height 20 on the centre of the selected face

**The score** is the scripted demo's (`web/test/demo/harness.mjs`, run in CI on every PR), against closed forms fixed
before the run. The plate is one body of volume 80 · 60 · 10 − π · 6² · 10 = 48000 − 360π = 46869.0266 mm³ and
extents 80, 60 and 10, with its top face holed. The cylinder is one body more, of volume π · 5² · 20 = 500π =
1570.7963 mm³, radius 5 and height 20, its base in the selected face's plane and centred on the face's centre (the
middle of its box) within the band, 1e-7 mm, inside the face and clear of its hole, and standing out of the plate. The
page drew each model and shows the cylinder's faces. The wrapper's log shows the viewer started before the first run,
and the selection read, then highlighted, before the run that made the cylinder. Every model was measured again by
the scorer from a copy.

**Limits of the setup.** The permission check and the agent's tools are Claude Code's; the scripted demo only models
them. `--setting-sources project` kept the person's own settings and the plugins they enabled out of the agent's
session. Claude Code's own skills and plugins, and skills synced from the person's account, were still listed to it:
no flag of `claude --help` keeps them out while keeping the project's skill and a subscription login, and the user
accepted the isolation as it is. The harness's click is a script's, not a person's.

## Results

| run | prompt | turns | occam calls of 12 | cost (USD) | time | outcome |
|---|---|---|---|---|---|---|
| first, 07:23 UTC | 1 | 6 | 3 | 0.0040 | | built nothing |
| second, 07:43 UTC | 1 | 10 | 5 | 0.0043 | 90 s | the plate |
| second | 2 | 7 | 4 | 0.0066 | 67 s | the cylinder |

**The first run** loaded the skill and read three commands' help, then wrote its request with `cat` and a heredoc,
which the permissions refused, as only `occam` may run in the shell. It wrote the file with Write, but took the
refusal for one of `occam` and stopped without starting the viewer or running anything. The skill now says first to
run each `occam` command alone and write requests with the file tool, and that a refused shell command says nothing
about `occam`; the scripted demo is held to the same permissions. The second prompt was not run, and its logs are not
kept here.

**The second run passed:** 16 of 16 required checks, and the 2 noted ones too. Its calls, from the wrapper's log:

| call | prompt | command | exit |
|---|---|---|---|
| 1 | 1 | `occam run --help` | 0 |
| 2 | 1 | `occam view --model part.occam`, at the same moment as 1 | 1, no model yet, as the skill says |
| 3 | 1 | `occam run --model part.occam plate.json` | 0 |
| 4 | 1 | `occam run --model part.occam check.json`, before it wrote the file | 1, not read |
| 5 | 1 | `occam run --model part.occam check.json`, a measure | 0 |
| 6 | 2 | `occam selection --model part.occam` | 0 |
| 7 | 2 | `occam highlight --model part.occam face:7/1` | 0 |
| 8 | 2 | `occam run --model part.occam cylinder.json` | 0 |
| 9 | 2 | `occam run --model part.occam check_post.json`, a measure | 0 |

- **The plate,** `body:0/2`, one face with its hole written in place and extruded 10 up, as the guide's own example
  writes it: volume 46869.02664470765 mm³, bounds (0, 0, 0) to (80, 60, 10) within 3e-13 mm. Its top face, `face:7/1`,
  has two loops.
- **The cylinder,** `body:1/1`: volume 1570.7963267948962 mm³, bounds (35, 25, 10) to (45, 35, 30) within 4e-13 mm.
  Its base, `face:11/1`, lies in the top face's plane, 1.3e-14 mm from the face's centre (40, 30, 10). The agent said
  why it was there: the middle of the face's box, clear of the hole at x = 20.
- **The page** drew the plate (digest `e158e545cd685c90`), showed the selection, showed the highlight 13 s before the
  run that changed the model, and drew the plate with the cylinder (`45313d7f152803b9`), the selection carried over.
- **What it said.** It gave the person the page's address, built the hole into the outline, checked both bodies'
  volumes against their closed forms, said that the cylinder is a body of its own because this build has no boolean,
  and left the viewer running, offering to stop it.

**Re-scored.** The first scoring failed one check, "the agent started the viewer before its first run": the scorer
took call 1, a call for help made at the same moment as call 2, for the first run. A call for help is no step now, and
the kit was scored again from its logs with `just agent-demo --score`, asking no agent anything: pass. The run kept no
record of the faces the page drew at the end, so "the page shows the cylinder" is carried over from the first scoring,
which observed it in the page. Both scorings are in [score.md](2026-11-16-view-agent-live-run/score.md).

## What misled the agent

- **A refused shell command,** in the first run, taken for `occam` refused. The skill's first section now covers it.
- **Its own order,** once: it ran a measure before writing its request, which cost a call (4).
- **`occam view` on a model not yet made exits 1.** The agent went on, as the skill says to, but a stricter agent might
  not.

## What this does not show

- **Anything statistical.** One scored run, one model.
- **Other parts or prompts:** an edge selected, a face not square to the axes, a part needing a boolean.
- **That the skill was needed.** No run went without it.
- **That the cap was needed.** The run used 5 and 4 calls of 12, and $0.011 of $1.
- **That it was blind.** The agent could read files outside its directory with Read, and listed skills it did not use.

## What it decides

Nothing by itself. It is the run the exit gate's agent test asks for, its log kept.

## Files

[2026-11-16-view-agent-live-run/](2026-11-16-view-agent-live-run/) holds the kit's scrubbed copy, as the recipe wrote
it: `score.md` and `score.json`; and in `logs/`, `calls.log` (the wrapper's log, the blind run's form),
`prompt-1.jsonl` and `prompt-2.jsonl` (each prompt's stream of events from Claude Code), `page.jsonl` (what the page
showed as it changed) and `command-lines.txt` (the command lines run). Each transcript's first event keeps only its
type, session, model, tools, permission mode and version; the lists of skills, plugins, slash commands and agents,
the paths and the account's state are dropped, and every path of the machine is written as `$KIT` (the kit), `$OCCAM`
or `~`.
