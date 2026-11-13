# The first release's viewer picks faces and edges, and hands back their ids

**Decided by the user, 2026-11-13,** asked [O28](../PLAN.md#7-decisions-the-user-still-owns) while the
[`view`](../plan/view.md) milestone was drafted: whether the first release's viewer picks a face or edge, to hand its
id to an agent. The recommendation was no: the bracket test does not need it, so add it with the full web
application. **The user chose against it: "Yes, in first release".** The rule that came with the question still
holds: pick against the CPU mesh, never against GPU output.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- A pick answers with the model's own id, such as `face:12/3` or `edge:41/1`. An agent can pass that id to
  `occam describe` or to a request. The answer also gives the point hit, in model coordinates.
- A pick is a ray, built in `f64` from the page's origin. It is tested against the CPU mesh: the triangles for faces,
  and the edge samples, within a tolerance, for edges. GPU output is never read for a pick
  ([GPU drawing](2026-11-09-gpu-drawing.md), L23).
- Picking is deterministic. The same ray gives the same id at every thread count, on x86-64 and on wasm32. Ties go to
  the smaller id.
- The same code runs natively as `occam pick`, so an agent without a browser gets the same answer. In the page, the
  pick runs in a mesh worker and returns JSON. The page also shows the id, so a person can copy it.
- A ray that hits nothing returns no id, with the reason. During a
  [continuous edit](2026-11-13-write-through-in-the-first-release.md), a pick answers against the last CPU mesh and
  names the snapshot it used.
