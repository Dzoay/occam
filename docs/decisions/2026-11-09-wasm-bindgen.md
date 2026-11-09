# wasm-bindgen for the browser build

**Decided by the user, 2026-11-09:** "yes to wasm-bindgen and gungraun". `occam-wasm` may depend on `wasm-bindgen`.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- Only `occam-wasm`, the thin browser facade, depends on it. The kernel stays free of it, and `gates layers` keeps that crate
  from reaching anything but `occam`.
- The crate is pinned exactly, to the version of `wasm-bindgen-cli` in `scripts/toolchain.sh`, because the two must match.
- Its transitive crates are listed in `ci/dependencies.txt` against this decision. All are MIT or Apache-2.0, so
  [the no-copyleft decision](2026-11-08-no-copyleft.md) holds.
