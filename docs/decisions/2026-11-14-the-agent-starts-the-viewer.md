# The agent starts the viewer, a recipe builds its page, and the live agent test runs by hand

**Decided by the user, 2026-11-14,** after a builder's read of [the live view](../architecture/live-view.md) asked
four things the design had left open. Each came with a recommendation; the user took three and chose otherwise on the
third:

- **Where the live agent test runs.** A `just` recipe run once on the milestone PR, its log kept as evidence; a CI job
  each night; or every PR in CI. The last two need an API key stored as a repository secret. **"Local recipe
  (Recommended)".**
- **How an ordinary build of the `occam` command gets its page.** A recipe builds the real page and a plain
  `cargo build` serves a stub that says how; the built page, wasm included, committed to the repository; or a build
  script that runs a nested cargo, wasm-bindgen and npm on every compile. **"Recipe builds it (Recommended)".**
- **Who starts `occam view`, and how long it lives.** The person, in a terminal, in the foreground; the agent, which
  starts it detached and leaves it running, with a status and a stop; or both. The recommendation was the person.
  **"Agent starts it".**
- **Which agent tools the first skill serves, and how it reaches them.** Claude Code first, copied into a project by
  `occam skill --install`; a published Claude Code plugin; or a tool-neutral instructions file. **"Claude Code, install
  cmd (Recommended)".**

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- **`occam view` returns.** It starts the server as a process of its own, waits until it listens, prints the page's
  address (with the token, under `--json`), opens the page unless `--no-open` is given, and exits. The agent tells the
  person where to look. Starting a process apart from its parent needs only the standard library.
- **One server per model.** A small state file beside the model names the running server's process, port and token. A
  second `occam view` on the same model prints the running one's address and starts nothing; a state file whose process
  is gone is stale, and is replaced.
- **It lives until it is stopped.** `occam view --stop` stops it and `occam view --status` says whether one runs and
  where. The skill tells the agent to stop it when the person is done. So that a forgotten server does not run for
  days, it also stops itself when no page has been open for an hour; a later `occam view` starts it again.
- **The page is built by a recipe.** The recipe builds `occam-wasm` and bundles the page with esbuild into the
  directory the binary embeds. A plain `cargo build` embeds a stub page that names the recipe, so CI's compiles need no
  node. Release builds always carry the real page, and the page's own tests run against it.
- **The skill lives in the repository and in the binary.** `skills/occam/SKILL.md` is the source; `occam skill
  --install` copies it into the current project's `.claude/skills/occam/`, and says where it wrote. Nothing is
  published to a marketplace; that stays a later decision of the user's.
- **The live agent test is a recipe, like the blind runs.** It drives headless Claude Code (`claude -p`, resumed for
  the second prompt) with the model pinned and a cap on calls, needs no new dependency, and runs on the person's own
  key. It runs once on `view`'s first PR, and its log is kept under `docs/evidence/`. Everything it checks that needs
  no model, the page, the selection and the highlight, is scripted and runs in CI on every PR.
