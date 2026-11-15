# The viewer's page is written in TypeScript, typed against WebGPU

**Decided by the user, 2026-11-13,** when the dependency list was reviewed after
[Preact and signals](2026-11-13-preact-with-signals.md) were chosen. esbuild strips TypeScript's types without checking
them, so a typed page needs the compiler to check it. The user said: "yes, use typescript with @webgpu/types".

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- `web/` is written in TypeScript. `typescript` and `@webgpu/types` are development dependencies, used only to check
  types. Nothing from them ships in the page.
- esbuild still builds the page. The compiler only checks it, with `tsc --noEmit` in strict mode, and that check is
  part of the gates, so CI runs it too.
- Like Preact, signals and esbuild, the two packages are added at pinned versions by `view.renderer`, the step that
  first needs them, and that step's PR names them under the dependency rule.
- No Rust dependency follows from this choice.
