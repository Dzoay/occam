# GPU write-through moves after the first release, with the editing tools

**Decided by the user, 2026-11-14,** once [building became prompt-driven](2026-11-14-prompt-driven-building-shown-live.md).
It reverses [O20's answer of 2026-11-13](2026-11-13-write-through-in-the-first-release.md), which put continuous
whole-surface editing of very large faces, by GPU write-through, in the first release. Asked whether write-through should
move to the later editing milestone, since an agent's runs are separate steps, each meshed whole, the user answered:

> we will need standard view controls to view the object from different faces. Rotatate, drag, center, fit to view port
> etc. if that is *not* write-through then yes delay that until we add property based or other edit actions in the UX.

View controls move the camera over the same mesh, which is not write-through, so write-through moves.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- `view.write-through` leaves [the view milestone](../plan/view.md), and so do its exit gate and its timing goals. It
  returns, with its design as written in [mesh and renderer](../architecture/mesh-and-renderer.md), when the editing
  tools are planned after the first release: property edits, sliders and any other edit a person makes in the page.
- [O29's answer](2026-11-13-continuous-edits-rerun-the-request.md), that a continuous edit re-runs its request every
  frame, goes with it, and so do the write-through figures in [O30's answer](2026-11-13-gpu-figures-are-goals.md). The
  pick figure stays a goal in `view`.
- Orbit, pan, zoom, centring on the selection and fitting the model to the window stay in `view.renderer`.
- `view.buffers` keeps each vertex's `(u, v)` on its face, which costs little and is what write-through will evaluate
  at, so the buffers do not change shape when it returns.
