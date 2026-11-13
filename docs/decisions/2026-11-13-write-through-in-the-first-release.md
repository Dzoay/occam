# The first release serves continuous edits of very large faces, by GPU write-through

**Decided by the user, 2026-11-13,** asked [O20](../PLAN.md#7-decisions-the-user-still-owns) while the
[`view`](../plan/view.md) milestone was drafted: whether the first release must serve continuous whole-surface editing
of very large faces ([P12](../evidence/2026-11-09-p12.md)). The recommendation was no: draw the CPU mesh with local
updates, and add write-through if a real workload asks for it. **The user chose against it: "Yes, in first release".**

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- During a continuous edit, the GPU writes the display of each face the edit changes. This is P12's compute
  write-through: a compute shader writes a `STORAGE | VERTEX` buffer that is drawn and never read back. P12 rejected
  vertex pulling, which costs 4 to 5 times a stored draw's GPU time every frame and cannot update locally.
- The reason is P12's measurement. When a whole face is re-evaluated every frame, the CPU held 60 Hz at 399,424 points
  and fell to 30 Hz at 1,000,000, limited by `writeBuffer` on the main thread. Write-through held 60 Hz at 4,000,000
  points with 0.2 ms of main-thread time.
- It stays display-only ([GPU drawing](2026-11-09-gpu-drawing.md), L23). When the edit settles, the touched faces are
  meshed again on the CPU, and those meshes replace what the GPU drew. Nothing the GPU makes reaches a digest, a mesh,
  picking, measurement, export or the watertight check, and nothing is read back outside a test.
- The shader evaluates each face at its CPU mesh's own `(u, v)`. Trims and triangles therefore hold, and the index
  buffer stays the same while the topology does. P12 evaluated only an untrimmed rectangle, so this part is new.
- Each shared edge is evaluated once a frame, and both faces read it. In P12 that left 0 of 501 positions different.
  Control points are offsets from the page's `f64` origin.
- The shader evaluates rational B-splines. Families with an exact B-spline form reach it through `to_bspline` over the
  face's range. The other families update on the CPU.
- What produces each frame's new surfaces is a question still open, [O29](../PLAN.md#7-decisions-the-user-still-owns).
  Whether the 60 Hz figures are a gate or a goal is [O30](../PLAN.md#7-decisions-the-user-still-owns).
