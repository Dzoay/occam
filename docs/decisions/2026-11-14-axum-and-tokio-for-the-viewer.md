# `occam view` serves its page with axum on tokio

**Decided by the user, 2026-11-14,** asked while [the live view](../architecture/live-view.md) was designed. `occam
view` serves the page on this machine, sends the model and each change to it, and takes the person's selection back.
The user had said "A small http server crate is what I think we should use. Rust http servers are fast and scale very
well." Three crates were checked that day:

- `axum` 0.8 with `tokio`: 37 crates in all (`tokio`, `hyper`, `tower` and their helpers), MIT. Maintained; server-sent
  events built in; what [an in-memory server and a web service](2026-11-14-prompt-driven-building-shown-live.md) would
  be built on.
- `tiny_http` 0.12: 6 crates, MIT OR Apache-2.0, blocking. Last released in October 2022; a stream of events would be
  written by hand.
- `hyper` 1 with `tokio`: 18 crates, MIT. axum's engine without its routing, so more of the server would be ours.

The recommendation was the first, and the user chose it: **"axum + tokio (Recommended)".**

Later the same day, a builder's read found that server-sent events need a stream type, and telling every open page of
a change needs one of tokio's channels. The user was asked whether `occam-cli` may name `futures-util` directly
(already in the tree through axum, so no crate is added) and turn on tokio's `sync` feature, take `tokio-stream`
instead (one more crate), or ask again when `view.live` is built: **"futures-util + tokio sync (Recommended)".**

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- **Only `occam-cli` names them,** with `futures-util`, for `occam view`. No kernel crate, and nothing built for wasm,
  depends on any of them.
- **Features as few as serve:** `axum` without default features but `http1` and `tokio`, `tokio` with `rt`, `net`,
  `time`, `sync` and `macros` only as the step needs them, and `futures-util` without default features. Each crate the
  tree gains is listed in `ci/dependencies.txt` under this decision, with its licence, in `view.live`'s commit; a crate
  whose licence is not MIT or Apache-2.0 compatible is refused there, as always.
- **The kernel stays synchronous.** The server reads files and streams events; anything that loads a model runs off the
  async threads.
- **Anything more**, such as `tower-http` for static files, is asked for again under the dependency rule. The page's
  files are built into the binary instead, so none is needed.
