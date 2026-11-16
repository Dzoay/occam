# The page can show its mesh

**Asked for by the user, 2026-11-16,** on seeing the live demo: "yes add the show mesh toggle after agent merges", in
answer to whether the page should be able to show the mesh it draws. Until then the page drew each face shaded, and
edges only where they were selected or highlighted, so the triangles `view` meshes could be read back as numbers
(`occam mesh --json`, or the page's `?inspect` hook) but not seen.

## What it means in practice

This section is Claude's working reading of the request. The user can change it.

- A control in the page turns on a "show mesh" overlay: each face's triangle edges drawn over its shading, from the
  index buffers the page already holds, as a second line pass. It is off by default, and does not change what is meshed,
  picked or selected.
- It is display only, like everything the GPU draws ([decision](2026-11-09-gpu-drawing.md)).
- It is not a plan step: it merges in its own small PR, with no `Plan-Step` footer.
