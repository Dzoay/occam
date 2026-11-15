# The construction blind run: can a stranger build and inspect a solid with `occam` alone?

The goal that [design B](2026-11-10-design-b.md) set for every milestone from `construction` on: an
[S6](2026-11-09-s6.md)-style blind run on the real kernel. It is a goal, not a gate. It tests the
[risk](../architecture/risks.md) that descriptions inside reports do no better than S6's.

**Sample size: eight cases, each run once, by one model (Haiku), eight agent runs in all. This is a rough indication and
no more.** No case was repeated, so how much one agent varies from run to run is not measured.

## Question

Given only the release `occam` command (construction at `075cd48`) and a task in words, can an agent that reads no
source build the solid asked for, inspect a model it is given, and repair a failing request from its report?

## What was measured

**The protocol.** One fresh Haiku agent per case. Its prompt named its case directory and nothing else. That directory
held `TASK.md`, a wrapper `./occam` and any input files. It forbade reading anything outside the directory, any source
and the web. The wrapper logged every call (arguments, files passed, output and exit status) outside the directory, and
refused a 13th call. Call counts below are the log's, not the agents'. The pass rules and expected answers (all closed
forms) were written before any run, and a reference solution of every case was played through the same wrappers first.
Every model an agent built was re-measured from its file by the scorer.

**Limits of the setup.** The ban on reading outside the directory is on the honour system. During the run, the answers
were moved out of the kit's tree. The repair cases gave the agent the failing request and its report, numbers visible,
so they are S6's baseline and not its blind control.

| case | what it asks |
|---|---|
| c1 plate with holes | a 120 × 80 × 6 plate with two holes of radius 5: volume, area, face count |
| c2 angle bar | an L-section drawn in xz, extruded 200 along y: volume, area, centre |
| c3 stepped shaft | one solid of two diameters, so a revolve, not two stacked cylinders |
| c4 two pins | a pin, and a copy of it turned onto +x and moved to x = 100: the copy's centre and box |
| c5 inspect | a given model: counts, a face's surface and radius, its neighbours, check, and which request step made a face |
| c6 measure | a torus, a sphere and a frustum in a given model, which must stay byte-identical |
| c7 repair a request | `"centimetres"` for the unit and a misspelt `radious`; choosing `"mm"` changes the size tenfold |
| c8 repair a plate | an extrude vector lying in the face's own plane, refused as `construct.vector-in-plane` |

## Results

| case | calls of 12 | outcome | what cost calls |
|---|---|---|---|
| c1 | 8 | pass | `--help` for `list` and `describe` before using them |
| c2 | 6 | pass | `list body` refused, since `list` takes `--kind` |
| c3 | 6 | pass | the same `list` mistake |
| c4 | 6 | pass | a second, measure-only run, because the first run's 75 KB of JSON buried the measurements |
| c5 | 12 | pass | five `--help` calls; face neighbours found edge by edge |
| c6 | 11 | pass | four `--help` calls |
| c7 | 4 | pass | one run to save the output, since the first's 93 KB was hard to read |
| c8 | 4 | pass | none |

**8 of 8 passed.** Every built model matched its closed form to 1e-7 relative and passed `check`. Both repairs ran first
time, from the report and the request alone. c7 kept the numbers and chose `"cm"`; c8 changed only the vector.

## What misled the agents

- **The step number in `describe`.** "Made at step N" counts the model's changing steps from 1 and names no operation,
  while `run` numbers a request's steps from 0, measures included. c5 answered right with a wrong theory ("skips the
  measure steps").
- **Help spread over subcommands.** A full reading takes five `--help` calls, a large share of 12. `run --help` shows no
  example of a wire written in place, or of a revolve, so c1 guessed the form (right) and c2 wrote a separate wire step.
- **`list` takes `--kind`.** A positional kind is refused by the argument parser, with no hint of the flag.
- **Output size.** A successful run with `--json` printed 75 to 145 KB, almost all of it the description of every entity
  made. c8's failing report was 36.7 KB, and all but 0.5 KB of it described the two steps that succeeded before the
  failure.
- **Nothing says** that labels do not carry from one request to the next, that a measure-only run does not write the
  model, or that a box's origin is its corner.
- **`adjacent` on a face** gives its edges and vertices, not neighbouring faces.
- **c7's report.** `"centimetres"` came back as `input.unknown-name` with the allowed names, but the agent read it as a
  wrong value, not a wrong spelling. `radious` gave `input.unknown-field`, with no list of the fields allowed, and then
  `input.missing` for `radius`, which the agent took for a repeat of the same mistake.
- **A loose box.** In c8, `describe` gave the solid a box reaching z −13 to 18 for a plate from z 0 to 5. `measure`'s
  box was right. The agent saw it and went on. This is a fault: each slanted side face carried a box far larger than the
  face.
- **Seams as edges** did not mislead. c1 counted 10 faces, not 8, and said why: each hole's wall is two faces.

## Against S6

S6's baseline, with the numbers visible, handled 9 of 10 failing booleans. Its blind control handled 4 of 6 on S1's
reports and 6 of 6 enriched. Here, with the numbers visible, both repairs passed in one run each. The reports carried
what S6 asked for: a code per invalid input, the parameter, the value given and, for names, the values allowed. The
fault class was there too (`invalid-input`), along with the band in force. This run cannot speak to S6's blind control:
no case withheld the numbers, and construction has no boolean.

## What this does not show

- **Anything statistical.** Eight cases, one run each, one model.
- **Failures deep in a history, or in booleans.** Every case is small, and the two repairs are input errors found before
  any geometry or at the step that made it.
- **That the agents obeyed the ban on reading outside their directory.** The wrapper's log sees calls, not file reads.
- **That Haiku stands for an agent driving occam.** A stronger model might read less help; a weaker one might not
  finish.

## What it decides

Nothing. It is input for the follow-ups the user may choose: help in one place with examples, the `--kind` hint, a
smaller default output, `describe`'s step numbering, and an allowed-fields list on `input.unknown-field`. The loose box
is a fault, and is taken up with this milestone.
