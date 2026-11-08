# Sources

Where the kernel's algorithms come from. Every file in a kernel crate starts with `//! Source: <key>`, using a key from
this table, or `//! Source: original` when nothing was taken from elsewhere. `gates provenance` holds the two together.

Algorithms come from published papers and books, under
[the provenance rules](decisions/2026-11-08-licence-and-provenance.md). A row of kind `code` is a port from another
project: its file must also be declared in `REUSE.toml`, and its licence must be on the allow-list in `deny.toml`
([no copyleft](decisions/2026-11-08-no-copyleft.md)).

| Key | Kind | Licence | Citation |
|---|---|---|---|

[A deliberately broken link](no-such-file.md)
