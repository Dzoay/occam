# The dependencies of occam-xtask

**Decided by the user, 2026-11-08:** `occam-xtask` may depend on `serde` and `serde_json`, `syn` and `proc-macro2`, and
`toml`. In the user's words: "You don't need to write it std-only you can just ask me permission giving a reason. I don't
recomend cutting your own serialization for example."

| Crates | Why |
|---|---|
| `serde`, `serde_json` | JSON for `--json` on every gate, and reading clippy's JSON in the lint probes |
| `syn`, `proc-macro2` | Parsing Rust for the complexity gate and the forbidden-identifier scan, including inside macros |
| `toml` | Reading the manifests, `Cargo.lock`, `REUSE.toml` and `deny.toml`, so the licence allow-list has one home |

Together they add 17 crates, all permissive, so [the no-copyleft decision](2026-11-08-no-copyleft.md) holds.

`Cargo.lock` also names `equivalent`, `hashbrown` and `indexmap`, all MIT or Apache-2.0. They belong to an optional feature of
`toml` that nothing enables, so they are never compiled. Cargo records them anyway. The user chose to list them in
`ci/dependencies.txt` and keep the dependency gate strict about everything in the lockfile.

This covers `occam-xtask` alone, which is never published. Kernel crates stay std-only unless the user agrees otherwise,
asked when the step arrives, as O5 in [the plan](../PLAN.md#7-decisions-the-user-still-owns) says.
