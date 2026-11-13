# The divergence register records no OCCT numbers while O6 is open

**Decided by the user, 2026-11-13,** asked on 2026-11-09, when the
[corpus inventory](../evidence/2026-11-09-corpus-inventory.md) found 26 generated cases where OCCT disagreed: while O6,
a lawyer's view of the provenance policy and of oracle output ([the plan](../PLAN.md#7-decisions-the-user-still-owns)),
is open, may
[`tools/oracle/divergences.json`](../../tools/oracle/divergences.json) hold OCCT's measured values? The recommendation
was to record each case, what was measured, the invariant that decides it and the verdict, without OCCT's numbers, and
to revisit after O6. The user chose the recommendation.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- The register's schema 2 drops the rule that an entry holds two values. `values` holds only sources other than OCCT,
  such as a closed form, a quadrature, occam or a second opinion, and may be empty. `just oracle register` refuses an
  `occt` value.
- An entry says in its verdict and note that OCCT disagreed, and how, in words ("its result has none", "reaches past
  the plane"). It does not copy OCCT's numbers.
- The 26 cases are recorded, one entry per case and measure, 37 entries in all. The raw oracle results stay in the
  machine's cache, as before.
- When O6 is answered, that answer decides whether OCCT's values may be added. Allowing them is one set in
  [`register.py`](../../tools/oracle/occam_oracle/register.py).
