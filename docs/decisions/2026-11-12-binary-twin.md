# The native format gets a binary twin

**Decided by the user, 2026-11-12,** asked during [`construction`](../plan/construction.md) once
`construction.native` had measured loading. Parsing occam-topo's plate of 100,350 faces from the native JSON took
1.445 s, written compact (191.9 MB), over the one second at which [the files risk](../architecture/risks.md) says
JSON as the only native format is shown wrong. Measuring every edge and checking the whole model took 0.83 s more on
eight threads, and 3.1 s on one. The question was how loading should get faster. There were three options:

- A binary twin of the format, encoded with the `postcard` crate (MIT OR Apache-2.0, built on `serde`, which is
  [already allowed](2026-11-09-serde.md)). The readable JSON stays.
- A binary encoding written in occam itself, with no new crate.
- JSON alone for the first release, with the miss recorded against the risk.

The recommendation was the first, and the user said yes to it: "postcard binary twin (Recommended)".

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

- **One format in two encodings.** Both encodings hold the same records under the same version (1) and are read by the
  same reader. JSON stays the native format and its readable dump (PARITY 2.17, L8). The binary twin is the same file
  in fewer bytes, for loading fast. It is not a second format with fields of its own.
- **What a twin file is.** `save` writes JSON as before, and `save_binary` writes the twin. The twin begins with a
  10-byte magic, `\x89occam\r\n\x1a\n`: a first byte no text begins with, the short name, then both kinds of line end
  and an end-of-file mark, which show a file mangled as text in transit. Next comes the version as four bytes,
  little-endian, and then the records in postcard's encoding. Each link is written as its kind, slot and generation,
  and each number as its eight bytes, so a NaN keeps its payload.
- **Reading either.** `parse` and `load` read both encodings and tell them apart by the magic. They refuse a twin for
  everything they refuse a JSON file for, with the same codes: a link of the wrong kind, a version they don't read
  (`io.other-version`, read from the header alone), a family's refusal, or a check that fails. A binary file has no
  lines, so `io.not-readable` gives line 1 and, as its column, the byte where reading stopped, counted from 1.
- **The dependency.** `postcard` is named in `occam-io` only, with no features. It is pinned exactly, because a twin's
  bytes are postcard's, so a new version is taken on purpose and with the format's tests. It brings in `cobs` and,
  through `cobs`, `thiserror` and `thiserror-impl`, all MIT OR Apache-2.0. Each is in `ci/dependencies.txt` under this
  decision. On 2026-11-13 the user accepted those three as well: "I am ok with the extra crates".
- **The risk now watches the twin.** If parsing 10⁵ faces from the twin takes over a second, measured apart from the
  checking, the twin is shown too slow too.
