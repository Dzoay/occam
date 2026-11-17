# `occam view` waits while its server opens the model

**Decided by the user, 2026-11-17,** choosing "Wait as long as it takes" over raising the limit or keeping it.
[The view's timing](../evidence/2026-11-16-view-timing.md) found that `occam view` gave its server 30 s to open and
check a model and say it listens, and P9's 10,001-face model takes longer, so `occam view` could not serve it.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- While the server it started is still running and opening the model, `occam view` keeps waiting, with no fixed limit,
  and says every few seconds that it is still opening.
- It stops waiting when the server says it listens, when the server's process exits, which is reported with its own
  words, or when the person interrupts it.
