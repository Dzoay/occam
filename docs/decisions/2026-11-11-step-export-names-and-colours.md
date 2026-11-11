# STEP export writes names and colours back

**Decided by the user, 2026-11-11,** asked whether STEP export should write back the names and colours that STEP
import keeps: "yes to both, add them to #42". No parity row asked for it, so the `exchange` round trip would have
reported them lost.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- Parity row 2.9 now covers export as well as import. The row keeps its place in [the approved list](../PARITY.md).
- Export writes what [the table of names and colours](../architecture/topology.md#names-and-colours) holds for the
  entities it writes. Anything it cannot write is reported, as for every other loss (PARITY 1.9).
- The `exchange` exit's round trip through the OCCT oracle checks names and colours too.
