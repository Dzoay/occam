# Milestone: proving-ground

Answer the questions that decide the data structures, using code that is thrown away (L51). Needs `census.oracle` and
`census.corpus`, apart from the investigations that start early: [the first six](../decisions/2026-11-09-early-spikes.md)
and [the second wave](../decisions/2026-11-09-second-wave.md).

**Rules for spikes:** spike code lives on a long-lived `spikes` branch that is never merged, so later spikes can reuse
earlier ones. Each spike states its question and measure first. Its PR to `main` holds only its written finding, in
`docs/evidence/`. `proving-ground.design` deletes the branch. Nothing from it is copied into a crate; anything that
survives is rewritten test-first. The provenance rules still apply, so run `gates provenance` over the branch before each
findings PR.

**Exit (gate):** every spike has a written answer in `docs/evidence/`, `docs/ARCHITECTURE.md` exists, and the user has
approved it.

**PR grouping:** one findings PR per step. Investigations that share an experiment may share a PR.

## Spikes

Every spike has merged its finding, in `docs/evidence/2026-11-09-<id>.md`. The questions they left for the user are in
[§7 of the plan](../PLAN.md#7-decisions-the-user-still-owns).

## Closing the milestone

`proving-ground.design` made [ARCHITECTURE.md](../ARCHITECTURE.md) from two proposals and an adversarial critic on each,
with [a compiled type sketch](../sketch/src/lib.rs) of the seams. [§4 of the plan](../PLAN.md#4-layers) and
[the first-release outline](first-release.md) follow it. The exit gate is met when the user approves it.
