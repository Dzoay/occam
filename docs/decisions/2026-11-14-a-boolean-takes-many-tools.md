# A boolean takes many tools in one operation

**Decided by the user, 2026-11-14,** asked [O32](../PLAN.md#7-decisions-the-user-still-owns) while the
[`booleans`](../plan/booleans.md) milestone was drafted. The question was whether, in the first release, a boolean
(fuse, cut, common) takes one object and one tool body, or a list of tools at once. The recommendation was one tool:
many tools would be a sequence of booleans, because P2 and S1 measured only the two-operand case. **The user chose
against the recommendation: "Many tools".** One boolean takes a list of tools, as a multi-cut does, at the cost of
more splitting and classifying work with nothing yet measured for it.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- A boolean takes an object and one or more tools. One tool is the case with one entry, not a separate operation.
- The tools are split against the object and against each other in one pass, and every piece is classified against
  every operand, so the result does not depend on the order the tools are listed in.
- The milestone's exit gates gain multi-tool cases: results equal to the same booleans run as a sequence, wherever a
  sequence is defined, and the same result for every order of the tools.
- Nothing yet measures more than two operands, so the milestone measures it before its speed goals bind.
