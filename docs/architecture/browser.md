# Browser

[Architecture](../ARCHITECTURE.md) › Browser · crate `occam-wasm`, `Session`

The kernel in the browser: workers with separate memories, on stable Rust, simd128 by default, and nothing required of
the hosting page (O18).

```mermaid
flowchart LR
  page["Page main thread<br/>no wasm here"] -->|"request"| ed["Editing worker<br/>holds the model"]
  ed -->|"native bytes,<br/>a copy each"| m1["Mesh worker"] & m2["Mesh worker"] & m3["Mesh worker"]
  m1 & m2 & m3 -->|"mesh parts"| page
  page --> gpu(["GPU"])
```

- **One editing worker** holds the model, in a `Session`, and runs requests. A request that goes through and changes
  the model gives the model as native bytes, the binary twin, which the worker publishes.
  Each mesh worker gets its own copy as an `ArrayBuffer`, which skips the UTF-16 conversion a JavaScript string needs
  at both ends.
- **Mesh workers** load those bytes read-only and mesh ranges of faces, about 64 calls a job.
- **No wasm runs on the page's main thread.**
- **Each call's output is deterministic.** The page's assembly of the parts is display-only, like GPU output.
- **An edit is one long call** on its worker. Splitting a boolean across calls waits for the `view` milestone's timing.

<details>
<summary>Evidence</summary>

- [P10](../evidence/2026-11-09-p10.md): the worker pool. About 64 calls a job balanced uneven work and avoided
  Firefox's long-call penalty of 1.4 to 2.3×.
- Assembly as display-only: B:12. Shared-memory threads instead would need nightly and an adapter dependency
  ([O18](open-decisions.md#assumed-answers-no-question)).

</details>
