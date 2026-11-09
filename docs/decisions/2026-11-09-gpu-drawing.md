# GPU drawing is in scope, and a spike tests GPU-generated geometry

**Decided by the user, 2026-11-09:** "gpu display should be in scope", then "yes drawing in scope, and add the write-through
spike".

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- The renderer draws on the GPU, from the first release. `occam-mesh` still builds the mesh on the CPU, and that mesh is the
  authoritative one.
- Whether the GPU also generates display geometry is not decided. `proving-ground.p12` measures it: surfaces evaluated on the
  GPU and written straight to a vertex buffer, never read back, in a page with a render loop.
- `proving-ground.p12` may start before the milestone opens, on the same terms as
  [the second wave](2026-11-09-second-wave.md).
- GPU results stay out of anything the kernel hashes or reuses (L23), because they are 32-bit and differ between adapters.
- Which API the renderer uses, WebGPU or WebGL2, waits on which browsers the first release must support.
