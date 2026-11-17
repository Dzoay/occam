# Browser

[Architecture](../ARCHITECTURE.md) › Browser · crate `occam-wasm`, `Session` · page `web/`, its workers

The kernel in the browser: workers with separate memories, on stable Rust, simd128 by default, and nothing required of
the hosting page (O18).

```mermaid
flowchart LR
  page["Page main thread<br/>no wasm here"] -->|"request"| ed["Editing worker<br/>holds the model"]
  ed -->|"the twin, a copy<br/>for each mesh worker"| page
  page -->|"its copy, then<br/>ranges of faces"| m1["Mesh worker"] & m2["Mesh worker"] & m3["Mesh worker"]
  m1 & m2 & m3 -->|"flat arrays"| page
  page --> gpu(["GPU"])
```

- **One editing worker** holds the model, in a `Session`, and runs requests in the order they come. A request that
  goes through and changes the model gives the model as native bytes, the binary twin, which the worker publishes as
  one copy for each mesh worker. Every model it holds was checked, by the runs that made it or by its open, which
  measures and checks a model whole: the page's one check. A model [`occam view`](live-view.md) sends is opened and published the same way, with
  the places of the faces that changed since the one it opened last (`Session.again`), which alone are meshed again. Each copy is an `ArrayBuffer`, which skips the UTF-16 conversion a JavaScript string
  needs at both ends, and the page passes it on without copying it.
- **Mesh workers,** kept alive between requests, each open their own copy read-only, trusting the editing worker's
  check ([decision](../decisions/2026-11-16-mesh-workers-trust-the-editing-worker.md)): `Trusted.open` reads it,
  refusing bytes that do not read as a model, which the page names as `view.mesh-failed`, but measures and checks
  nothing. Only the editing worker makes a copy of the type the pool takes, `Checked`, so fetched bytes reach a mesh
  worker only through its open; and only the browser build has the open without the check, occam's `trusted`
  feature, so the `occam` command cannot open a file unchecked. A `Trusted` meshes and picks, and runs nothing, so
  what it holds never leaves it as a model. The workers mesh ranges of faces (`Trusted.mesh_part`). The page cuts a model's faces into about 64 ranges of one size and deals them out one at a
  time to whichever worker is free. Each range's arrays are numbered from their own start, and its edges are sampled
  as the whole mesh samples them, so the ranges laid end to end in id order are the buffers `occam mesh` gives, bit
  for bit. Each range is checked watertight as far as it reaches.
- **A pick is asked of one mesh worker,** the first that has not stopped, about the model it was given last. It meshes
  that model whole the first time, as `occam mesh` does, and keeps the mesh and its box trees for the next pick, so the
  page picks against the CPU mesh and maps no GPU buffer. The same worker gives the samples of the edges the page
  outlines.
- **No wasm runs on the page's main thread.** Only the workers fetch the kernel's module, `occam.wasm`, built with
  simd128.
- **Nothing is shared,** so the page needs no `SharedArrayBuffer`, and no COOP or COEP headers: bytes and arrays are
  transferred.
- **Each call's output is deterministic.** The page's assembly of the parts is display-only, like GPU output.
- **A face that will not mesh is named** in the page by the kernel's report, and the rest of the model is drawn. A
  call that throws instead, as when a worker's kernel will not load, is `view.mesh-failed`, naming the faces it held;
  every other face is drawn, and a reload meshes them again.
- **An edit is one long call** on its worker. The longest of `construction`'s requests there is timed in
  [the `view` milestone's timing](../evidence/2026-11-16-view-timing.md), which splitting a boolean across calls is
  weighed against.

| Message | From, to | What it carries |
|---|---|---|
| `run` | page, editing worker | the request, and how many copies of the twin to make |
| `ran` | editing worker, page | the run's one JSON form, each face's id, and the copies, if it changed |
| `open` | page, editing worker | a twin `occam view` sent, how many copies to make, and the limits |
| `opened` | editing worker, page | each face's id, the copies, and the faces to mesh again and what keeps its mesh |
| `load` | page, mesh worker | a model's number and its own copy of the twin the editing worker checked |
| `mesh` | page, mesh worker | a call's number, the model's, the limits, and the range of faces |
| `meshed` or `failed` | mesh worker, page | the range's arrays and faces' table, with the reports refusing any of it; or why it could not mesh at all |
| `pick` | page, a mesh worker | an ask's number, the model's, the limits, a click's ray in `f64`, and the edge tolerance a millimetre along it |
| `samples` | page, a mesh worker | an ask's number, the model's, the limits, and the edges selected or highlighted, to outline |
| `picked`, `sampled` or `unanswered` | mesh worker, page | `occam pick`'s JSON for the ray; each edge's samples; or why it could not answer |

[`web/src/protocol.ts`](../../web/src/protocol.ts) declares them.

<details>
<summary>Evidence</summary>

- [P10](../evidence/2026-11-09-p10.md): the worker pool. About 64 calls a job balanced uneven work and avoided
  Firefox's long-call penalty of 1.4 to 2.3×.
- [The view on large models](../evidence/2026-11-17-view-large-models.md): with the mesh workers trusting the editing
  worker's check, the pool's job on P9's 1,002-face model fell from 35 s to 0.5 s on six workers, and scales.
- Assembly as display-only: B:12. Shared-memory threads instead would need nightly and an adapter dependency
  ([O18](open-decisions.md#assumed-answers-no-question)).

</details>
