# No copyleft

**Decided by the user, 2026-11-08:** "DO NOT introduce copy left into this repo"

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

Nothing under a copyleft licence enters this repository in any form: not code, a dependency of any kind, a vendored or
translated file, a test fixture or a corpus case. Anything whose licence is unclear stays out. The licence allow-list in
`deny.toml` names only permissive licences.

Programs that run outside the workspace and are never committed are not in the repository, so the black-box oracles in
[the licence and provenance decision](2026-11-08-licence-and-provenance.md) stand. Whether oracle outputs carry licence
terms is the question already open as O6 in [the plan](../PLAN.md#7-decisions-the-user-still-owns).
