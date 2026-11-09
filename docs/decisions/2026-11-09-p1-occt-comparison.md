# P1's comparison with OCCT stays off `main` for now

**Decided by the user, 2026-11-09:** "keep them out for now."

## What it means in practice

The finding of `proving-ground.p1` on `main` leaves out its comparison of whole-boolean time with OCCT: the family medians
of the time ratio and the counts of cases where the prototype was faster. Those figures are derived from oracle outputs,
and whether oracle outputs carry licence terms is still open as O6 in
[the plan](../PLAN.md#7-decisions-the-user-still-owns).

The question comes back when O6 is answered, as O27 in the plan. The full text is kept in `private/`, so
`proving-ground.design` can delete the `spikes` branch without losing it.
