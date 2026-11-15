# The viewer's npm packages

**Decided by the user, 2026-11-15,** asked before [`view.renderer`](../plan/view.md) began, so that it could start
beside `view.faces`. [Preact with signals](2026-11-13-preact-with-signals.md) and [TypeScript typed against
WebGPU](2026-11-13-typescript-with-webgpu-types.md) were decided on 2026-11-13. The plan asks again for each package,
at a pinned version, under the dependency rule. The latest release of each was read from the npm registry that day,
and each was approved as asked:

| Package | Version | Licence | Kind | What it brings |
|---|---|---|---|---|
| `preact` | 11.2.0 | MIT | runtime | nothing |
| `@preact/signals` | 2.11.4 | MIT | runtime | `@preact/signals-core` 1.14.5, MIT, nothing further |
| `esbuild` | 0.28.2 | MIT | build | one prebuilt binary for the platform, of 26 optional packages |
| `typescript` | 7.0.2 | Apache-2.0 | build | the native compiler: one platform package, Apache-2.0, of 20 |
| `@webgpu/types` | 0.1.74 | BSD-3-Clause | build | nothing |
| `@types/web` | 0.0.366 | Apache-2.0 | build | nothing |

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- **`web/` pins each exactly** in its `package.json`, with a lock file, as `tools/browser-perf` pins Playwright. The
  build packages are development dependencies, and none of their code reaches the page.
- **Only these:** any other package, a new major version, or a package one of these starts to bring is asked again.
- **Tests use the Playwright** `gates perf` already runs, at the same pinned version, listed in `web/package.json` as
  well as in `tools/browser-perf`'s.
- **`@types/web` was asked for later the same day,** once `view.renderer` was built: TypeScript 7.0.2's own DOM types
  declare only part of WebGPU and clash with `@webgpu/types`, whose README recommends `@types/web` in their place. The
  user chose it over checking with `skipLibCheck`, so the declaration files are checked too.
