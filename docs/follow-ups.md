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

- **Status** — `closed` (2026-08-14): `scripts/lint_docs.py` implements the fix below, wired into
  `mise run lint:docs` (in `lint`) and the `doc-pointers` prek hook.
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

- **Status** — `closed` (2026-08-15): the `version-bumps` prek hook, `scripts/lint_version_bumps.py`, is
  the tripwire the Fix below asks for, and it carries `FU-06`'s two further pairs in the one hook that
  entry prescribes. Closed while still dormant — the sweep is clean, four commits touch the directory and
  every one is an addition — with three departures from the Fix as written, each measured; see the note
  beneath it.
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

  **Three departures from that Fix, all measured on 2026-08-15.** What fires the snapshot half is a JSON
  key HEAD's snapshot published and the staged one does not, rather than any modification: `report.rs`
  rules the format additive, so a new field rewrites an existing snapshot while owing no bump, and a
  tripwire firing on that would send an author with nothing to bump to `--no-verify` — the hole `FU-05`
  closed. A renamed or removed field always drops a key, and a deleted snapshot drops all of them. Second,
  `always_run: true` rather than `files:`: on prek 0.4.13 a `files:`-gated hook is skipped when the only
  staged change is a deletion, so a withdrawn payload type would pass unseen. Third, the bump is read as
  the constant's value in HEAD against its value in the index rather than as a diff naming the constant,
  because editing the comment above it names it too.

  Two limits, since neither is visible from a green commit. The escape for a change no reader can misread
  is `SKIP=version-bumps`, which prek honours per-hook, rather than `--no-verify`. And there is no
  `mise run lint:` half: the check reads the index against HEAD, so on a CI checkout nothing is staged and
  an `--all-files` run would report a gate that had looked at nothing. The hook is the whole of it, bar the
  trigger names, which `tests/test_lint_version_bumps.py` pins against the tree — so a watched block
  renamed out from under the check fails in CI even though the check itself never runs there.

### FU-04 - Diagnostics have no facade

- **Status** — `closed` (2026-08-14): [ADR 0004](decisions/0004-diagnostics-through-log.md) is the record
  the Fix below demanded, and its [plan](decisions/0004-diagnostics-through-log.plan.md) is the
  implementation. Two departures from that Fix as written, both on measurements in that record's Context:
  `log` on the emitting side rather than `tracing`, and a hand-written `log::Log` in the CLI rather than
  `tracing-subscriber`. The cost table below is left as it was measured — ADR 0004 carries its correction.
- **Condition** — either signal that ad-hoc printing has outgrown itself. `rg -n 'print!|println!|eprintln!' crates/popcircles/src`
  matches anything: the library is printing, which is an `application.md` "Architecture" violation
  before it is a logging question. Or the CLI grows the flag shape that hand-rolls level filtering out of
  two booleans — a `--verbose` or `--quiet` long name, a field of either name in an args struct, or
  `short = 'v'` or `short = 'q'` aliasing one:
  `rg -n "^\s*(verbose|quiet)\s*:|--verbose|--quiet|short = '[vq]'" crates/popcircles-cli/src`. The two
  letters are spelled out rather than matching `short =` at large, because a condition firing on any short
  alias the CLI ever grows would ban a mechanism instead of a flag. Progress reporting is **not** this
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

### FU-06 - Nothing couples a cache header change to a format version bump

- **Status** — `closed` (2026-08-15): the `version-bumps` hook is `FU-03`'s hook carrying this entry's two
  further pairs, which is the one hook the Fix below asks for. Closed while still dormant: the sweep is
  clean, one commit touches each cache file and each adds it with its constant. `FU-03` holds the
  three departures common to both halves; two more are this entry's, in the note beneath the Fix.
- **Condition** — a commit changes the fields of `Header` in `crates/popcircles/src/table/cache.rs`
  without changing `FORMAT_VERSION` in the same file. The sweep is
  `git log --format=%H -- crates/popcircles/src/table/cache.rs`, and for each commit it names,
  `git show <sha> -- crates/popcircles/src/table/cache.rs` carrying an added or removed field line inside
  the `struct Header` block while no line of that diff names `FORMAT_VERSION`. An **added** field fires it
  too, which is where this differs from `FU-03`: serde ignores keys it does not know, so a build reading a
  header from a later one accepts the document and then maps a payload whose layout it has no reason to
  doubt. The wire format may grow a field additively; a cache header cannot.
- **Fix** — `FU-03`'s hook with two more pairs, so one `repo: local` hook discharges all of them: a table of
  trigger and constant — files under `crates/popcircles/src/snapshots/` with `SCHEMA_VERSION`, the
  `struct Header` block in `crates/popcircles/src/table/cache.rs` with `FORMAT_VERSION`, and the
  `struct Document` and `struct Probe` blocks in `crates/popcircles/src/smallest/cache.rs` with that file's
  own `FORMAT_VERSION` — failing when `git diff --cached` touches a trigger while carrying no line naming
  that trigger's constant. Run against this tree on 2026-08-14: two commits touch `table/cache.rs`, the
  first adding the file with the constant in the same diff and the second touching neither the block nor the
  constant, and one commit adds `smallest/cache.rs` with its constant in the same diff, so the sweep is
  clean. Like `FU-03` it cannot judge whether a change was breaking, which makes it a tripwire of the same
  kind as `geo-data-lfs` rather than a lint of its own.

  The radius ledger is the same kind of file as the table header rather than the same kind as a snapshot,
  which is why it joins this entry: a run that resumes reads the document *back*, so a field added to it
  without a bump leaves an older build resuming from radii it has half understood.

  **Two departures beyond `FU-03`'s three, measured the same day.** The trigger is the whole field set of
  each named block in HEAD against its field set in the index, rather than a field line read out of a diff
  hunk — the same answer where a diff is readable, and no `struct Header {` block boundary to find inside
  one. And a watched block that is not there under that name at all fires the check too, for
  `single-unsafe-allow`'s reason: a rename would otherwise leave the tripwire watching nothing and saying so
  nowhere.

### FU-07 - A radius in kilometres is a bare f64 in more than one signature

- **Status** — `closed` (2026-08-14): `RadiusKm` in `crates/popcircles/src/geodesy.rs` is the fix below,
  and `Kernel::new` takes one, so the sweep now names no signature at all rather than one. The second
  caller was #6's search over candidate centres, not #7's binary search this entry expected: the search
  builds a kernel per candidate row and a second per widened bound radius, so it takes a radius by value
  one issue earlier than predicted.
- **Condition** — more than one **public** library signature takes a radius in kilometres as a bare
  `f64`. The sweep is `rg -n 'pub fn [a-z_]+\([^)]*radius_km: f64' crates/popcircles/src`, which names
  exactly one on 2026-08-14: `Kernel::new`. Three near misses the wording excludes deliberately —
  `Cap::over` beside it is private, `Kernel::radius_km` returns a radius rather than taking one, and
  `report.rs`'s `great_circle_km` parameter is a distance and not a radius. #5's circle evaluation adds
  none of its own: it takes a kernel, which carries the radius it was built for, so the radius reaches it
  through a type rather than through a second parameter. The second signature is expected to be #7's
  binary search over radius, which is where this fires rather than on a refactor.
- **Fix** — a `RadiusKm` newtype in `geodesy`, beside the radius and the conversion it would wrap, whose
  constructor holds what `Kernel::new` checks inline today — finite, not negative — so
  `KernelError::RadiusNotFinite` and `KernelError::RadiusNegative` move into it and no later caller
  revalidates. That is `application.md` "Architecture": prefer a type whose invalid states do not
  construct over a check repeated at every use. It waits for the second caller because introducing it for
  one trades one inconsistency for another — no scalar in this crate is wrapped, and `great_circle_km`,
  `cell_area_km2` and the CLI's `distance` all pass kilometres as `f64` — and with two callers the
  newtype is the cheaper side of that trade rather than merely the more principled one.

### FU-08 - Nothing couples the search's initial spacing to a measured figure

- **Status** — `dormant`.
- **Condition** — a caller outside `search.rs` itself **chooses** the search's initial spacing. Choosing is
  what fires it, not calling: a caller that takes a spacing as a parameter and forwards it has made no
  choice, and the number it passes on is still its own caller's. The sweep is therefore two-part —
  `rg -n 'most_populous\(' crates/popcircles-cli/src crates/popcircles/src --glob '!search.rs'` for the call
  sites, and for each one, whether the spacing it passes is a literal or a constant rather than something it
  was given.

  On 2026-08-14 that names exactly one call site, `crates/popcircles/src/smallest.rs`, and it forwards: the
  parameter arrives from its own caller untouched, and `smallest.rs` declares no spacing of its own —
  `rg -n 'NonZeroU32::new' crates/popcircles/src/smallest.rs` matches only inside its tests. So #7 came and
  went without firing this. #8 is where it fires, when a command surface has to put a number in a flag's
  default or in a help string.

  **#8 came and went without firing it either** (2026-08-14). The three commands that drive a search take
  `--spacing` as a **required** flag with no default, and its help string names no figure — checkable as
  `cargo run -p popcircles-cli -- most-populous --help` carrying no digit on the `--spacing` line. A command
  that takes a spacing and forwards it has made no choice, which is the distinction this Condition draws, so
  the entry is recorded as still dormant rather than left to read as an oversight. The note is here for the
  reason the #7 one is: silence in a register is ambiguous, and a later reader should be able to tell "the
  surface arrived and did not choose" from "nobody looked".

  `crates/popcircles/tests/decimated_search.rs` is deliberately outside the sweep. It picks 32, but a
  deselected fixture choosing a spacing to exercise pruning is a fixture and not a default, and widening the
  sweep to catch it would leave this entry permanently `due` with nothing to do about it. The unit fixtures in
  `smallest.rs` are outside it for the same reason.
  `crates/popcircles/tests/registry_validation.rs` joins that exclusion (2026-08-15). It picks 256, on the
  plateau ADR 0009 measured, and it is a deselected fixture for the same reason the two above are.

  **The curve this entry was waiting for exists, and it corrects the Condition's own arithmetic**
  (2026-08-15, [ADR 0009](decisions/0009-validation-brackets-cheap-and-certifies-dear.md)). Swept over the
  5 arcmin table at 200, 800, 3 300 and 8 000 km, the wall clock falls **monotonically** with spacing in
  every row and flattens from about 256 — a sixteenth of the grid's width — rather than peaking at a knee.
  The pruning *fraction* falls with it, from 97.3% at spacing 8 to 77.2% at 256, so the figure the Condition
  reasons from is the one that must not be maximised: a fine first level prunes almost every block it makes
  and pays for making them. What the search costs is circles evaluated, and at 3 300 km that is 4 447 at
  spacing 8 against 390 at 256.

  So "the ceiling is a sanity limit, two orders of magnitude away from the answer" is wrong, and the Fix
  below is right for a reason it did not claim: the ceiling is the neighbourhood of the answer, and clamping
  against it is the derivation rather than a guard on one. Nothing else about the entry changes.

- **Fix** — a derivation of the initial spacing from the radius and the grid, in `search` beside the loop
  that consumes it, bounded above by the ceiling `slack_km` already documents: once `radius + slack` reaches
  half the circumference the widened circle is the whole sphere, every bound equals the raster's total and
  the level prunes nothing, so a spacing past that point is strictly wasted work. The figure inside that
  ceiling is #10's to measure — 32 on the k=10 shape prunes 86.9% of blocks at a 200 km radius, which is one
  point and not a curve. The entry exists so the first caller does not silently become the default, and it
  cannot be discharged by a benchmark alone: what it wants is a function of the two inputs, not a constant
  that happened to measure well once.

### FU-09 - The predicate slack is reported and nothing acts on it

- **Status** — `closed` (2026-08-14): [ADR 0005](decisions/0005-ambiguous-minimality-is-reported.md) is the
  record and [its plan](decisions/0005-ambiguous-minimality-is-reported.plan.md) the implementation. Closed
  with one departure from the Fix as written, measured rather than preferred — see the note beneath it. The
  condition keeps standing and stays checkable: the field is still published, and now something acts on it.
- **Condition** — a surface publishes `predicate_slack_persons` while the search still answers an ambiguous
  comparison with a single radius. The sweep is `rg -n 'predicate_slack_persons'
  crates/popcircles/src/report.rs crates/popcircles-cli/src scripts` — widened to `report.rs` because that
  is where the field is published from, the CLI reaching it through `SmallestReport::new` rather than by
  naming it. Empty on 2026-08-14 before #8 and matching after it. What makes it a real obligation rather
  than a tidy-up is the two pieces of work that can land on a target inside it: #10 validates against the
  published 3300 km result, where the interesting shares sit on a plateau of ocean, and #18 sweeps every
  country, where a small country's own total is close to one.
- **Fix** — report the two radii around an ambiguous comparison rather than one: where a probe's population is
  within the slack of the target, the honest answer is the bracket `[short, reaching]` and a statement that
  the arithmetic cannot separate them, which is a wider bracket and not a tolerance. `smallest` already
  carries the pieces — the slack, the answer and the radius below it — so the change is a comparison and a
  field rather than a new search. It cannot be discharged by shrinking the slack: `mise run test:fold`
  measures the fold's real error at a world's magnitude as **exactly zero** against a bound of 0.0218 persons
  on 2026-08-14, so the bound is conservative by orders of magnitude and tightening it would be a claim about
  cancellation rather than about the arithmetic. Nor by a compensated fold in `circle::population`: that
  changes the answer's bits, which `search`'s determinism tests pin, and is a record's call.

  The two surfaces are the ones #8 added, and the figure has to be on them: #9 puts it on a map and #10
  validates against a share sitting on an ocean plateau. So publishing it was right and the bracket is what
  is owed — its own PR, because it changes `smallest`'s result shape and every document carrying it.

  **What landed reports a wider span than `[short, reaching]`, because that pair is not where the ambiguity
  is.** Measured against the 5 arcmin table at a share of one on 2026-08-14: nine of the run's 28 probed
  radii sit inside the slack, spanning 14 960 to 16 384 km, where the pair this Fix names is 2 km wide. So
  the field is accumulated over every radius the search probed rather than derived from the final pair, and
  it is published as a floor on the ambiguity — the climb doubles, so the radii between two probes were
  never measured and the true interval runs past both ends. ADR 0005's Context holds the measurement and
  decision 2 the reason it is not read back from the ledger.

### FU-10 - Nothing checks rustdoc

- **Status** — `closed` (2026-08-14): `mise run lint:rustdoc` implements the fix below and `lint` depends on
  it, so CI runs it. Closed with two departures from the Fix as written, both measured rather than
  preferred — see the note beneath it.
- **Condition** — no quality gate runs rustdoc, so a doc comment naming an item that has gone survives the
  whole of `mise run ci`. The sweep is `rg -n 'cargo doc' mise.toml .github/workflows/` coming back empty,
  which is what makes this `due` the day it is written. The evidence that it bites rather than merely could:
  on the tree before #8, `cargo doc -p popcircles --no-deps` emitted seven warnings while `mise run ci` was
  green, and one of them was `search.rs`'s `# Errors` line promising a `SearchError::Radius` that went away
  when `RadiusKm::widened_by` became total under `FU-07`. It was found by a human reading the classifier
  beside it, not by a check. The other six were public docs linking to private items, and one redundant
  explicit link target. #8 cleared all seven, so the sweep above is the condition rather than a warning
  count — a clean tree with no gate is exactly the state that lets the next one through.
- **Fix** — `cargo doc -p popcircles --no-deps` as a `lint:rustdoc` task with `lint` depending on it, the
  shape `lint:rust` already uses. It needs `RUSTDOCFLAGS="-D warnings"` or an equivalent, because rustdoc
  warns and exits 0: measured on the tree before #8, the command exited 0 with seven warnings, so a task
  reading only the exit status would gate nothing. Measured again after #8: zero warnings, so the task comes
  back clean the day it is added and gates from then on.

  **What landed takes two more flags than that, because the command as written gates less than it looks
  like it does.** Both were measured on 2026-08-14 by injecting a broken link and checking the exit status.
  `--document-private-items`: without it rustdoc never reads a private item's doc comment, so a broken link
  inside `report.rs`'s own helpers exits 0 and reports nothing — and this crate keeps most of its reasoning
  on private items. `--workspace` rather than `-p popcircles`: the CLI crate has doc comments too, and its
  own intra-doc links were outside the prescribed command. The flag also retires the lint that produced six
  of the seven warnings above — "public documentation links to private item" cannot fire once private items
  are documented — which is the right trade: that lint is about doc visibility, while the defect this entry
  exists to catch is a link naming something gone. The regression test is the original defect itself,
  `search.rs`'s `[`SearchError::Radius`]`, which the task reports as "the enum `SearchError` has no variant
  or associated item named `Radius`".

### FU-11 - The cache binds no grid geometry

- **Status** — `closed` (2026-08-15): [ADR 0007](decisions/0007-cache-identity-binds-the-whole-grid.md) is
  the record the Fix below asks for, and its plan carried it into the tree. Both documents now embed one
  flattened attestation over the digest, the dimensions, the factor and the whole geometry, at
  `FORMAT_VERSION = 2` each. One departure from the Fix as written and one addition to it, plus a correction
  to the Condition's own illustration — all three in the note beneath the Fix.
- **Condition** — `struct Header` in `crates/popcircles/src/table/cache.rs` carries no origin and no step
  field while a command resolves a coordinate against a grid taken from flags. Two greps: `rg -n 'origin|step'
  crates/popcircles/src/table/cache.rs` naming nothing inside the `struct Header` block, and
  `rg -n 'origin_lat' crates/popcircles-cli/src/main.rs` matching. The header binds `format_version`,
  `digest`, `width`, `height`, `decimation` and `byte_order`, and `Identity` is the digest and the
  decimation — so the origin and the two steps are six numbers nothing checks against the table. Build over
  the registry raster's grid, query the same digest with `--origin-lat 0`, and the cache opens cleanly while
  every coordinate resolves to the wrong cell. The digest cannot catch it: it is over cells, and it is itself
  a flag the caller copies across. #8's documents publish the **declared** grid beside the **attested**
  digest and say which is which, which makes the gap visible rather than closed.
- **Fix** — grid geometry in the header and in `Identity`, so opening a cache compares the whole geometry
  rather than three of its numbers. It **takes a record**: ADR 0003 decision 3 ruled what sits in the header
  as against inside the digest, and this reopens that ruling. It also bumps `FORMAT_VERSION` and invalidates
  every existing cache, which is a cost the record has to weigh rather than something a fix decides in
  passing — at full resolution a rebuild is the raster read again. The entry names the record it needs and
  stops there deliberately: prescribing the change here would settle in a register what `docs/decisions/`
  owns.

  **The geometry went into the header alone.** `Identity` holds a `Decimation`, which holds both grids, so
  half of "in the header and in `Identity`" was already there and what narrowed the geometry to three
  numbers was the header's field list and its `check` body. No caller's side moved.

  **The radius ledger was in scope, which this entry does not say and issue #45 excluded.**
  `crates/popcircles/src/smallest/cache.rs` spelled the same three numbers itself rather than embedding
  `Identity`, so it inherited nothing from the header's fix, and it mints each probe's row and column back
  onto the caller's declared grid: with the dimensions agreeing the mint succeeded and a resumed run
  published a centre whose population was measured somewhere else. Its own `FORMAT_VERSION` moved too, and
  the `version-bumps` hook gained what it could not express — a watched block whose shape governs two
  constants.

  **The Condition's illustration does not reproduce, and the count is four rather than six.** ADR 0007's
  Context measures it: a grid 21600 rows deep at 1/120° spans exactly 180°, which pins its origin latitude
  to the pole within the boundary tolerance, so `--origin-lat 0` is refused by `Grid::new` before any cache
  is opened. Six is the count of `GridArgs`' flags; the header bound two of them and four went unchecked.
  What was reachable was the origin's longitude, freely, and each step downward — and the reachable case is
  not a typo but a half-turn shift of every column over identical width, height and steps.

### FU-12 - No gate compiles this for Apple silicon

- **Status** — `due` (2026-08-15): met the moment `.github/workflows/release.yml` landed. **`due` here is a
  standing accepted gap and not queued work**, which is the one place this register's statuses can mislead —
  see the note under the Fix.
- **Condition** — a release job builds a macOS artifact while no gate ever compiles for that target. Two
  greps: `rg -n 'macos' .github/workflows/release.yml` matching, and `rg -n 'runs-on|macos'
  .github/workflows/ci.yml` naming `ubuntu-latest` and nothing else. So the first time this code is compiled
  for `aarch64-apple-darwin` is on a pushed tag, including the `unsafe` mmap site ADR 0003 decision 5
  reviewed and the `#[allow(unsafe_code)]` the `single-unsafe-allow` hook guards. A macOS-only break
  therefore surfaces when the tag already exists, which is the half of ADR 0006's cost that CONTRIBUTING's
  Releasing section has to give a recovery for rather than prevent.
- **Fix** — `macos-latest` in `ci.yml`'s job as a matrix beside `ubuntu-latest`. ADR 0006 weighed this and
  did not take it: the cost is a second runner on every pull request rather than on every tag, which is why
  this is an entry and not a task. What would move it is evidence the gap bites — a release leg failing on
  macOS where the Linux leg passed, which is a fact a run either shows or does not.

  **The Fix above is declined rather than pending, and #50 is where the evidence comes from instead**
  (2026-08-15, the repository owner's ruling). `ci.yml` stays `ubuntu-latest` and macOS is compiled at
  release time only: a second runner on every pull request buys less than the same evidence asked for on
  demand. So this entry is not waiting for someone to notice it, and a later reader proposing `macos-latest`
  is reopening a decision rather than discharging an obligation.

  What #50 changes about the Condition is nothing — it is a command a person runs, not a gate, so the two
  greps keep matching and this entry keeps standing. What it changes is the cost of the gap: the first macOS
  compile of a tree stops being the tag someone is trying to ship. The evidence the paragraph above names
  becomes available before a release rather than during one, which is the whole of the mitigation.

### FU-13 - A published binary carries no Developer ID, and a user is told to clear an attribute by hand

- **Status** — `dormant` (2026-08-15): the condition needs a release to exist, and none does. The workflow
  has been fired once, against a pre-release tag since deleted along with its Release.
- **Condition** — a Release exists while no artifact carries a Developer ID signature. The sweep is
  `gh release list` returning at least one release, together with
  `rg -n 'codesign|notarytool|notarize' .github/workflows/release.yml` returning nothing. Not "unsigned":
  the dry run measured `codesign -dv` on the published macOS asset as `Signature=adhoc` with
  `TeamIdentifier=not set`, which is the linker's default and is what lets it run on Apple silicon at all.
  What is missing is the identity Gatekeeper will accept. The consequence is written down in `README.md`'s
  Releases section, which tells a macOS user to clear `com.apple.quarantine` off a browser download —
  measured too: quarantined, the binary is killed with "Apple could not verify"; with the attribute gone it
  runs. That is a documented workaround for a missing identity rather than a property of the tool, and the
  entry exists because such an instruction reads as normal once it has sat in a README for a while.
- **Fix** — sign and notarize the macOS artifact in the publish job, which drops the README line rather
  than explaining it better. ADR 0006 put it out of scope for a reason that is a prerequisite and not a
  preference: it needs a paid Apple identity and a certificate in CI secrets, so this cannot be closed by
  anyone who does not hold the account. Until then the honest form is the documented attribute, which is
  why the entry's condition is about a release existing rather than about the README's wording.

### FU-14 - A dataset's grid step approaches the tolerance two grids are one within

- **Status** — `dormant` (2026-08-15): the registry holds one dataset, at 0.008333° — six orders of
  magnitude above the constant.
- **Condition** — a registry row in [`data/README.md`](../data/README.md) names a grid resolution within
  three orders of magnitude of `BOUNDARY_TOLERANCE_DEG` in `crates/popcircles/src/grid.rs`. The sweep is the
  resolution figure in each row of that registry against the constant: 0.008333° against 1e-9° today, and
  what fires it is a dataset finer than about 1e-6°. Two grids within the tolerance are one table — ADR 0007
  decision 3 compares the geometry within it while the dimensions compare exactly — so a cell small enough
  to land in that gap makes a stale cache indistinguishable from the right one, which is the failure that
  record exists to catch.
- **Fix** — a record, not a smaller number chosen in passing. The constant is shared with the raster reader
  by design (`grid.rs` says why a second copy would be two answers to one question), so tightening it for
  the cache alone reintroduces exactly what that comment forbids, and tightening it for both changes what
  rasters the reader accepts. What a record has to weigh is that 1e-9° is scaled to a measured rounding in
  the registry raster's own geotransform, and a dataset fine enough to fire this would bring a measurement
  of its own to scale it to. ADR 0007's alternatives already rule out the two shortcuts: exact bit equality
  on the four numbers, and a per-caller tolerance.

### FU-15 - cartopy is built from source because no cp314 wheel exists

- **Status** — `dormant` (2026-08-15): measured in a clean environment on the day ADR 0008 was written.
  `uv pip install --only-binary :all: --python-version 3.14 cartopy` answers "all versions of cartopy have
  no usable wheels", and a cold `uv sync --locked --reinstall-package cartopy` prints
  `Building cartopy==0.25.0` and takes 25.55 s here.
- **Condition** — that same command resolves. It is a one-line sweep with a yes-or-no answer, and it fires
  the day cartopy publishes a cp314 wheel: nothing about this repository has to change for it to become
  true, which is why the entry exists rather than a task. `uv pip install --only-binary :all:
  --python-version 3.14 pydantic` is the contrast — that one resolves today, so the cost is cartopy's
  alone.
- **Fix** — drop the `~/.cache/uv` cache step from `.github/workflows/ci.yml` and the comment beside it.
  The cache is there for exactly one reason, stated in that comment, and it is the kind of step that
  outlives its reason silently: a reader a year from now finds a cache keyed on `uv.lock` and no way to
  tell whether removing it costs 25 seconds a job or nothing at all. ADR 0008's Consequences carry the
  same figure, so the entry firing is what licenses removing the step rather than guessing at it.

### FU-16 - The CC BY citation is a Python constant rather than the registry's own text

- **Status** — `dormant` (2026-08-15): the registry holds one dataset, so one citation is the whole
  mapping and a constant is indistinguishable from one.
- **Condition** — [`data/README.md`](../data/README.md) carries a second dataset row. The sweep is the
  count of registry entries in that file against the number of citations `scripts/render_map.py` holds:
  one and one today. A second dataset makes `CITATION` the citation of whichever raster happens to have
  been first, and a figure rendered from the other one then credits the wrong source while
  `tests/test_render_map.py` still passes — the test asserts the constant appears in the registry, which
  stays true of the wrong entry.
- **Fix** — key the citation by dataset rather than holding one, and publish enough in the document to
  choose between them. That second half is the reason this is an entry and not a task: `report`'s
  `provenance` names the table a document was answered from by digest and grid, not the dataset the raster
  came from, so there is nothing in the wire format a renderer could select a citation with today. Whoever
  fires this adds that field first, additively, and the entry is where the two halves are recorded
  together.

### FU-17 - The full-resolution search is page faults, not arithmetic

- **Status** — `dormant` (2026-08-15): measured in
  [ADR 0009](decisions/0009-validation-brackets-cheap-and-certifies-dear.md), and dormant rather than due
  because nothing in the tree asks for the runs that make it hurt. One radius at 30 arcsec is 207 s of which
  **13.4 s is CPU**, with `iostat` reporting ~7 000 transfers a second against the 7.5 GB payload. Every
  answer this repository has published came from a decimated table or from three certifying radii, and both
  are affordable at that rate.
- **Condition** — a command that needs many full-resolution radii becomes something this repository asks
  for: `smallest-for-share` or `sweep` at `--decimate 1` named in a mise task, in `README.md`'s Usage, or in
  an open issue's acceptance. Two dozen probes at 207 s is 90 minutes, and issue #18's per-country sweep is
  the first plausible caller — ninety-plus countries at full resolution is not that multiplied by one.
  The figure is re-measurable in one line, which is what keeps this checkable rather than remembered:
  `/usr/bin/time -l` around a `most-populous --decimate 1` run, and the entry has fired as long as user plus
  system time stays under a fifth of real.
- **Fix** — a record, because the candidates are algorithm changes rather than tuning. What the measurement
  says is that locality is the cost: `circle::population` walks a kernel's rows for one centre, and
  consecutive table rows are 345 KB apart, so one evaluation touches ~111 MB of scattered pages and the next
  centre re-walks the same stride one column over. Inverting that loop — one row band, every candidate column
  in it, before moving on — reuses each page across a whole level instead of once, and is the change ADR 0009
  says nobody could have justified without this figure. It is not free: it changes the order the fold adds
  in, which `search`'s determinism tests pin and `application.md` "Determinism" makes a stated rule, so a
  record has to weigh a changed answer's bits against the wall clock. Prefetching or `madvise` are the
  cheaper half-measures a record should rule on beside it, and neither touches the order.
