# Follow-up register

Every follow-up this repository has recorded, in one place. A plan's Follow-ups section names the
identifiers it produced and points here; work with no plan of its own writes its entries here
directly. This is a live document: entries change status as the repository moves under them, and an
entry whose status is stale is worse than no entry at all.

**Every entry states a condition the repository can answer.** A file, a command's output, a count, a
hook's result — something a sweep can evaluate without asking the user what they have experienced.
Prose that cannot meet that bar is not a follow-up and does not belong here.

## Statuses

- `dormant` — the condition is checkable and has not been met.
- `due` — the condition has fired and the fix has not landed.
- `closed` — the obligation is resolved; the entry says when and what closed it, which need not be
  the fix.
- `retired` — the entry can never fire; the entry says why.

Closed and retired entries stay in the register, so a reader can tell "never fired" from "cannot
fire".

## Entry format

An entry is a level-three heading, `FU-NN - <title>`, followed by three fields:

- **Status** — one of the four above, carrying the date when `closed`, the reason when `retired`.
- **Condition** — what has to become true, worded so a later reader can evaluate it against this
  repository rather than against the author's memory.
- **Fix** — what becomes correct once the condition holds. Checked against the tree it would run on,
  so the entry does not prescribe something untested.

Identifiers are flat, sequential and never reused.

## Entries

### FU-02 - Nothing checks that a pointer resolves

- **Status** — `due`.
- **Condition** — any pointer in a live Markdown document fails to resolve, in any of its four forms:
  a relative Markdown link, a **backticked repository-relative path**, an `ADR NNNN` reference, or an
  `@` import line in `CLAUDE.md`; and for a pointer naming a section, the quoted heading does not
  exist in the file it names. The scope is the scope of the housekeeping sweep's duplication check —
  the instruction layer, the human layer, and the live documents in `.github/`.
- **Fix** — a documentation lint that resolves all four pointer forms, asserts every quoted section
  heading exists in the file it names, asserts every `ADR NNNN` reference names a record in
  `docs/decisions/`, asserts the `@` import set in `CLAUDE.md` covers `docs/ai/` exactly, and asserts
  the roots in `platform.md` "Structure" cover the tree — every listed path present on disk, every
  root on disk listed; wired into `lint` and a hook. Both set comparisons are a few lines against a
  directory listing, and the import one has no fallback: a file added to `docs/ai/` without an import
  is unloaded for every session until someone runs a sweep, and nothing says so. Two constraints the
  lint must respect:
  - **`docs/decisions/` is history and is exempt from the resolve check.** A record may cite a path
    deliberately in the past tense, as the state before a migration, and cannot be edited to satisfy
    a linter.
  - **A quoted phrase beside a filename is not always a section pointer.** `docs/ai/code.md` reads
    ``application.md "Correctness invariants" owns what "safe" means``, where the first quote is a real
    section and the second is ordinary emphasis. The rule needs the heading to exist *or* the quote to
    be recognisable as prose, which is why this cannot be a one-line grep.

  It has no host yet: `crates/popcircles/` is a library about spherical geometry, there is no Python
  package, and a shell script would be a fourth place hooks are configured.

### FU-03 - Nothing couples a wire-format change to a version bump

- **Status** — `dormant`.
- **Condition** — a commit **modifies** an existing file under `crates/popcircles/src/snapshots/`
  without changing `SCHEMA_VERSION` in `crates/popcircles/src/report.rs`. The sweep is
  `git log --diff-filter=M --format=%H -- crates/popcircles/src/snapshots/`, and for each commit it
  names, `git show <sha> -- crates/popcircles/src/report.rs | rg SCHEMA_VERSION` coming back empty.
  Modification and not addition is the filter, because a new payload type is additive and owes no bump;
  a *changed* shape under an unchanged number is what leaves an old document unreadable without saying
  so.
- **Fix** — a `repo: local` hook beside `geo-data-lfs`, `files: ^crates/popcircles/src/snapshots/` and
  `pass_filenames: false`, failing when `git diff --cached --diff-filter=M --name-only` over that
  directory is non-empty while `git diff --cached -U0 -- crates/popcircles/src/report.rs` carries no
  `SCHEMA_VERSION` line. Both halves were run against this tree on 2026-08-13: the sole commit touching
  the directory adds the snapshots, so the sweep is clean, and a staged edit to one of them fires the
  check. It cannot judge whether a shape change was breaking, which makes it a tripwire of the same
  kind as `geo-data-lfs` rather than a lint of its own.

### FU-04 - Diagnostics have no facade

- **Status** — `dormant`.
- **Condition** — either signal that ad-hoc printing has outgrown itself. `rg -n 'print!|println!|eprintln!' crates/popcircles/src`
  matches anything: the library is printing, which is an `application.md` "Architecture" violation
  before it is a logging question. Or `rg -n 'verbose|quiet' crates/popcircles-cli/src` matches: the CLI
  is hand-rolling the level filtering `EnvFilter` exists for. Progress reporting is **not** this
  condition — ADR 0001 decision 4 routes progress through a sink the caller supplies, and a sink is not
  a log.
- **Fix** — a record **extending** ADR 0001's `tracing` clause rather than a fresh decision. That clause
  is a live ruling, and `write-adr` requires a record reopening a settled question to say which of the
  two it does. Then `tracing` on the emitting side and `tracing-subscriber` with `json` and `env-filter`
  on the consuming side, which is what makes it the analogue of structured logging in Python: fields on
  events, and spans carrying context down a nesting the search already has — per radius, per latitude
  band, per candidate.

  The cost is what the record has to argue, measured 2026-08-13 with `cargo tree -e normal` in a scratch
  project outside this tree:

  | Addition | Crates |
  | --- | --- |
  | `log` facade in a library | 1, and it has no dependencies of its own |
  | `tracing` facade in a library | 13 |
  | `log` + `env_logger` in a binary | 25 |
  | `tracing` + `tracing-subscriber` with `json`, `env-filter` | 41 |

  Forty-one is nearly triple the trimmed clap tree ADR 0001 accepted, and the 13 lands on the library
  whose dependency surface that record fought to hold at serde. So the cheaper shape has to be ruled out
  rather than skipped: `log` in the library, bridged into the binary's subscriber by `tracing-log`, costs
  one crate and buys no spans. If the nesting turns out shallow, that is the better answer and the record
  should say so.

### FU-05 - Formatting is enforced by hooks and by nothing else

- **Status** — `closed` (2026-08-14): `lint:format` runs `prek run --all-files cargo-fmt taplo-fmt
  ruff-format` and `lint` now depends on it, per the fix below.
- **Condition** — `mise run ci` passes on a tree a formatter would rewrite. Both halves are checkable:
  `rg -n -- '--check' mise.toml` names no formatter task, which is what makes this `due` the day it is
  written, and on such a checkout `cargo fmt --all --check`, `taplo fmt --check` or
  `uv run ruff format --check .` exits non-zero while `mise run lint` stays green, which is how a sweep
  shows it has bitten. A commit made with `--no-verify`, or from a clone where `prek install` never ran,
  is the way it happens: `lint` runs clippy, ruff's linter, actionlint and the three LFS hooks, and no
  formatter at all.
- **Fix** — a `lint:format` task selecting the formatting hooks by id, `prek run --all-files cargo-fmt
  taplo-fmt ruff-format`, with `lint` depending on it — the shape `lint:lfs` already uses, which is what
  puts hooks in CI without a second copy of the rule. Those hooks rewrite rather than check, so a CI
  failure reads "files were modified by this hook" rather than naming the diff; that is the same report
  `lint:lfs` gives and is enough to stop the merge. Measured on the tree that added the taplo hook: all
  three come back clean, and `ruff check --show-files` names only `pyproject.toml`, so the Python half
  gates nothing yet but needs none of the deferral `typecheck:python` carries.
