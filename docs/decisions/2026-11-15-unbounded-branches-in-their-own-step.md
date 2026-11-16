# A quadric pair's branches to infinity get their own step

**Decided by the user, 2026-11-15,** choosing "Defer to its own step" when `intersection.quadrics` was finished except
for the sections that run to infinity. Three classes do: two cones that share a ruling direction, a quartic with its
node at infinity, and a ruling with a cubic. `intersection.quadrics` detects and classifies all three, and reports them
as not implemented, never fitted.

The step had said to parametrize a branch to infinity by `u = u₀ + t²`. Building it found that this fits only the first
class, and only on a ruled member whose ruling parameter runs to infinity with the curve, which the step's cone does
not give. The other two are rational but unbounded: one point on each ruling, with a pole where the ruling's
coefficient vanishes. So the rational B-splines the plan names, with finite ranges and positive weights, cannot hold
them. Doing all three there was estimated at 500 to 700 more lines and a new unbounded curve family.

The other choices were to clip each branch to an extent the caller gives, in that step, or to build the unbounded forms
there.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- `intersection.quadrics` merges with the three classes reported as not implemented.
- A new step, [`intersection.unbounded`](../plan/intersection.md), after `intersection.quadrics` and before
  `intersection.timing`, answers them. It chooses their form, such as an unbounded family or bounded pieces up to an
  extent `booleans` gives, since `booleans` meets only bounded faces.
