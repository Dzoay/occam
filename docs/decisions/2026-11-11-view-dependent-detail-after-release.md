# View-dependent detail joins the work after the first release

**Decided by the user, 2026-11-11,** asked whether view-dependent detail, a mesh only as fine as the screen needs,
should join the list of work after the first release: "yes, add it to the after-release list in #42 but it looks like
our approach will be sufficient".

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- [The plan](../PLAN.md#6-milestones) names view-dependent detail among the work after the first release. Like the rest
  of that list, it is planned when reached and needs the user's go-ahead. The first release meshes each face once.
- "Our approach" is to mesh each face again from its exact surface, as finely as its size on screen asks, rather than
  adopt a level-of-detail format that simplifies one fine mesh (3D Tiles, clustered meshes, splats). The pieces are
  already in the design: mesh workers that mesh ranges of faces, a vertex block and a range per face in the
  [renderer contract](../architecture/mesh-and-renderer.md#the-renderer-contract), and P12's GPU write-through.
- Two rules come with it. A shared edge is sampled as finely as the finer of its two faces needs, so neighbours do not
  crack. The view's mesh is display-only: picking, measurement, export and the watertight check keep the fixed mesh.
