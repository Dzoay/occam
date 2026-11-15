# The viewer's page is built with Preact and signals, and the full web application grows from it

**Decided by the user, 2026-11-13,** asked [O8](../PLAN.md#7-decisions-the-user-still-owns) while the
[`view`](../plan/view.md) milestone was drafted: which framework the viewer's page uses. The recommendation was no
framework for the viewer, and to decide again for the full web application. **The user chose to pick one now: Preact
with signals** (`preact` and `@preact/signals`), over SolidJS, Svelte 5 and React.

The option, as it was offered: about 4 KB, MIT, and no dependencies of its own. JSX is built by plain esbuild, so
there is no Babel and no compiler. A signal updates one control without a re-render, which suits the main thread's
budget of under 1 ms a frame. `preact/compat` opens React's component libraries to the full application later.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- The full web application grows from this choice. It is not decided again later.
- Nothing is added to any `package.json` now. The packages are added, at pinned versions, by the step that first
  needs them, `view.renderer`.
- esbuild as the bundler was part of the option as described. The dependency rule still applies, so `view.renderer`'s
  PR names esbuild and asks for it again, along with the two packages.
- No Rust dependency follows from this choice.
- The renderer's frame loop and GPU work stay outside the component tree. Preact draws the controls, and signals carry
  state such as a picked id to them, without re-rendering the page.
