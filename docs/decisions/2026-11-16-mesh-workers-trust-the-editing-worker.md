# Mesh workers trust the model the editing worker has checked

**Decided by the user, 2026-11-16,** choosing "Trust the editing worker" when asked whether the page's mesh workers
should trust the model the editing worker has already opened and checked. The other choices were keeping every worker's
own check, or deciding later. [The view's timing](../evidence/2026-11-16-view-timing.md) found that each mesh worker
re-opened and re-checked its own copy, so on spline models more workers made it slower: a warm job took 30 s with one
worker and 87 s with six, against about 2 s of meshing. Save to redraw on P9's 1,163-face model took about 2 minutes.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- A model is checked once, by the editing worker, when it is opened, whether from a file, from `occam view` or after a
  request. The binary twin it publishes carries that checked state, and a mesh worker opens it without checking again.
- A mesh worker still refuses bytes that do not read as a model, as a `view.mesh-failed` naming them. Trusting the
  check is not trusting arbitrary bytes.
- Nothing changes natively: `occam` opens and checks a file as before.
