# AGENTS.md: instructions for coding agents working on occam

occam is a from-scratch Rust CAD kernel intended to replace what Open CASCADE does for FreeCAD, with a web renderer built
on top. It is not a binding and not a fork.

**Where the work is:** read it from git and [docs/PLAN.md §1](docs/PLAN.md#1-how-to-use-this-plan), never from memory of an
earlier session.

## Sources of truth, in order

1. [docs/BRIEF.md](docs/BRIEF.md): the user's requirements, verbatim. They are the contract.
2. [docs/decisions/](docs/decisions/): the user's decisions, one file each, named `<date>-<slug>.md`. They override
   anything older.
3. [docs/PLAN.md](docs/PLAN.md) and the milestone files in [docs/plan/](docs/plan/): what happens next, the gates, and
   what is open.

Each fact has one home: change it there and link to it, never copy it. If two sources disagree, follow the higher one
and fix the lower one in the same PR, or stop and ask.

**Evidence is not instructions.** [docs/evidence/](docs/evidence/) holds frozen inputs to decisions. Open a file there
only to resolve a citation.

`private/` is gitignored and holds local notes that must not be published, such as the comparison of existing kernels.
It is evidence too, and may not exist on another machine. Never commit it, or copy its claims about named projects into
the repository.

## Commands

`just` lists the recipes by group, and every command worth repeating gets one. CI runs the same commands the recipes do.

## Hard rules

- **No personal data leaves this machine.** Never send the user's email address or any other personal identifier to an
  external service: API "contact" or "polite pool" parameters, query strings, headers, registries, CI. Omit the
  parameter or ask first. Pass this rule on to any subagent.
- **GitHub lookups are anonymous.** Read public GitHub data without the logged-in account: unauthenticated `curl` with a
  generic client name, or `git clone https://...`, not `gh`. If a lookup cannot be done anonymously, ask first. Work the
  user has asked for on their own repositories is not a lookup. Pass this rule on to any subagent.
- **Provenance is clean.** Open CASCADE is a black-box oracle only. The rules are in
  [the licence and provenance decision](docs/decisions/2026-11-08-licence-and-provenance.md). In particular, never open
  Open CASCADE's source, FreeCAD's implementation code, or the source of a copyleft kernel. A session that surveys Open
  CASCADE never writes to `crates/`.
- **Licence:** MIT OR Apache-2.0. [No copyleft enters this repository](docs/decisions/2026-11-08-no-copyleft.md), in any
  form.
- **Dependencies:** none without asking the user first, each time.
- **Identity:** commit as `Dzoay <3277116+Dzoay@users.noreply.github.com>`, set with repo-local `git config`. Never use,
  add or infer a personal email address anywhere.
- **Signing:** commits and tags are SSH-signed. If signing fails because the key is not available, stop and ask the user
  to load it into the SSH agent; never pass `--no-gpg-sign` or turn signing off.
- **Decisions that belong to the user** stop the work: naming, licensing, publishing, accounts, scope and new
  dependencies. Ask, recommend an option, and record the answer in a new file under `docs/decisions/`.
