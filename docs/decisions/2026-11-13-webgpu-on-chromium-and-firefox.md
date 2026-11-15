# The first release draws with WebGPU, on current Chromium and Firefox

**Decided by the user, 2026-11-13,** asked [O19](../PLAN.md#7-decisions-the-user-still-owns) while the
[`view`](../plan/view.md) milestone was drafted: which browsers the first release must support, which also picks the
renderer's API ([GPU drawing](2026-11-09-gpu-drawing.md)). The answer was "WebGPU, Chromium+Firefox", the recommended
option.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- The renderer uses WebGPU and nothing else. There is no WebGL2 path. The
  [renderer contract](../architecture/mesh-and-renderer.md#the-renderer-contract) needs a buffer that both a storage
  binding and a vertex fetch can read, and only WebGPU gives one.
- Supported means the stable desktop Chromium and Firefox current when a release is cut.
- Safari is not claimed. Before any release claims it, P12's measurements are repeated on Safari.
- [P12](../evidence/2026-11-09-p12.md) measured only Firefox, on two GPUs. In P9, headless Chromium reached only a
  software adapter. So `view.timing` measures Chromium on a real GPU, in a headed browser if a headless one cannot
  reach the GPU.
- CI has no GPU. Its browser tests run on headless Chromium's software adapter and assert buffers, not pixels.
- A browser without WebGPU gets a named report with its remedy, not a blank canvas.
