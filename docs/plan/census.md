# Milestone: census

What "replacement" means, measured from outside. Runs alongside `bedrock`. Everything here obeys
[the provenance rules](../decisions/2026-11-08-licence-and-provenance.md). A session working on this milestone is a
survey session and never writes to `crates/`. Output goes in `docs/evidence/`, except `docs/PARITY.md`, which is a living
document, and the harness in `tools/oracle/`.

**Exit (gate):** the user has approved `docs/PARITY.md`, and the oracle harness reproduces its own results on a second
run.

**PR grouping:** one PR each.

**Still to do from `census.oracle`:** the `solidcore` second opinion is not wired up yet.

## Steps

Every step has merged, and [the exit gate above is met](../decisions/2026-11-09-parity-approved.md).
