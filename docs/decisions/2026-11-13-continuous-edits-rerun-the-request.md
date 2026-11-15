# A continuous edit re-runs its request every frame, and meshes when it stops

**Decided by the user, 2026-11-13,** asked [O29](../PLAN.md#7-decisions-the-user-still-owns) while the
[`view`](../plan/view.md) milestone was drafted: what produces each frame's new surfaces during a
[continuous edit](2026-11-13-write-through-in-the-first-release.md). The answer was the recommended option.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- A continuous edit is a request whose number changes continuously, from a slider or from an agent's stream.
- Each frame, the editing worker re-runs that request from the snapshot taken before the edit. It commits the result
  with its checks, so every frame is a valid model, and it meshes nothing. The new surfaces of the touched faces go to
  the page, and the GPU draws them ([write-through](2026-11-13-write-through-in-the-first-release.md)).
- When the edit stops, the mesh workers mesh the touched faces, and those CPU meshes replace the GPU's output.
- No new operation moves control points. Moving one face's control points in the page would leave its neighbours off
  its edges in a solid.
- If one re-run takes longer than a frame, the page goes on submitting frames, and the display follows at the speed of
  the re-runs. `view.timing` records the editing worker's time per frame.
