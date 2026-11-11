# The architecture and its answers

**Decided by the user, 2026-11-11,** asked to approve the architecture and to answer the questions it left open: "I am
happy with the archtiecture for now (we will learn as we do) and I am happy with your proposals for all the outstanding
questions."

- **The architecture:** [approved](../ARCHITECTURE.md), which meets the exit gate of
  [`proving-ground`](../plan/proving-ground.md).
- **O11:** "cannot decide at this resolution" is a normal outcome, reported with its failure state.
- **O12:** a new model whose request gives no tolerance gets 1e-7 mm and 1e-9 rad.
- **O14:** occam owns `sin_cos` and `atan2` for the first release.
- **O16:** the determinism contract promises the same bits in browser and server only for functions `occam-math` owns,
  and makes that promise only when the feature is real.
- **O18:** the first release requires nothing of its hosting page.
- **O22:** topology is stored as records, read through per-kind views so that the layout is the table module's alone.
  This reverses the plan's earlier recommendation.
- **O23:** L29's first sentence becomes "Sort calls by family and degree. Evaluate one surface at many points in a
  batch. Use lanes per kernel class only where the vectorisation gate measures a gain. Group stored surfaces by family
  when memory asks." Its second, "Keep intersection marching scalar, but parallel across curves", stays.
- **O24:** a loop below the resolvable size is a reported result: "contact or loop smaller than `d`".
- **O25:** no spike for coincident surfaces. They become a step of `booleans`, written test-first from S1's 32 reports,
  and until then a reported failure with `move-apart`.
- **O26:** reports carry ids, described by one resolver as the result leaves the kernel, with the descriptions inside
  the report.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- "For now (we will learn as we do)": the architecture is where the first release starts, not a freeze. When a
  milestone's tests or measurements contradict a topic page, that milestone's PR changes the page and says so, and the
  plan's rule to stop and ask the user still applies. [Risks](../architecture/risks.md) lists what to watch.
- "All the outstanding questions" is read as the ten rows of the plan's §7 that `proving-ground.design` needed. The
  rows due later (O5 to O10, O13, O15, O17, O19, O20 and O27) stay open and come up at their milestones.
- The plan's §7 drops the answered rows and links here.
  [The architecture's decisions page](../architecture/open-decisions.md) keeps the reasoning for each.
- `proving-ground` closes, and the first release's milestones follow in [the plan's order](../PLAN.md#6-milestones).
