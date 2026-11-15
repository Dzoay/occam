# A description numbers and names the step that made an entity as `occam run` does

**Decided by the user, 2026-11-13,** asked after [the construction blind
run](../evidence/2026-11-13-construction-blind-run.md), whose c5 agent met the mismatch. `describe` said "made at step
N", counting the model's changing steps from 1 with no operation, while `run` numbers a request's steps from 0, measures
included, and names each one's operation. In c5 the cylinder was request step 4, while describe said step 3, and a
reader who took 3 as a request step answered `wedge`. A copy's entities also kept their source's provenance, so a copy
said it was made by the step that made the original. The recommendation was to fix it in `construction`, and the user
chose that: "Fix now, in construction".

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- **The model keeps a note of each step a request named:** its index in the request, from 0, as `run` numbers it,
  and its operation. An entity's provenance still records the model's step and the entity's part in it, and the note
  for that step says the rest. A description gives all four, in any later command too, and its words say "made by step
  4, cylinder".
- **A copy is the copy step's own.** Its bodies, faces and edges are made by the copy step, in the part `copy`, and
  the step's note names the body it copied, so a description of any of them says "made by step 5, copy, of body:2/1".
  Before, a copy carried its source's provenance, so its origin pointed at the step that made the original.
- **The native format holds the notes,** in JSON and the binary twin alike, after the step counter. No file has been
  released, so the format stays at version 1 with no reader for the layout before, as when
  [the checksum](2026-11-13-binary-twin-checksum.md) was added.
- **The digest covers the notes,** so two models that differ only in what their requests named their steps differ.
  A model built by edits that no request named, as the topology's own probes are, has none, and its digest is as
  before.
- **A step no request named** is described by the model's step alone, in words "made at the model's step N".
