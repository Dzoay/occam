# Licence and provenance

**Decided by the user, 2026-11-08:** "MIT OR Apache-2.0, with clean provenance and OCCT only as a black-box oracle."

## Why this differs from phasekit

Phasekit read CoolProp's C++ and translated it, which CoolProp's MIT licence allowed. Open CASCADE (OCCT) is LGPL-2.1
with an exception, and a translation would probably be a derivative work under that licence.

## What it means in practice

This section is Claude's working reading of the decision. The user can change it.

Nobody working on occam, human or agent, may:

- open OCCT's source, or translate, paraphrase or copy it;
- read FreeCAD's implementation code, or the source of a copyleft kernel such as `brepkit` or `ogeom`;
- name anything after OCCT's internals (`TopoDS`, `BRep_*`, `Geom_*`, `gp_*`, `BOPDS`, `Pave`, `CommonBlock` and the
  like).

Surveying and implementing are separate. A survey session may do the allowed things below and never writes to `crates/`.
Implementers work from `docs/PARITY.md` and published literature.

These are allowed:

- running a pinned OCCT binary outside the workspace and comparing what comes out;
- driving `arris` and `solidcore` the same way, through their documented interfaces only;
- in a survey session, reading OCCT's public documentation for what an operation does and accepts, but never its Boolean
  Operations specification or any other description of how an algorithm works internally;
- a script that fetches public bug reports and strips code blocks and linked patches, so that only the stripped text is
  read, for symptoms;
- a script, developed on synthetic fixtures, that extracts from FreeCAD the names of the OCCT classes it uses, with
  counts, so that only the resulting table is read.

Algorithms come from published papers and books, and each is cited where it is used and in `docs/SOURCES.md`.

## Still open

A lawyer's view of this policy and of the licensing of oracle outputs has not been taken.
