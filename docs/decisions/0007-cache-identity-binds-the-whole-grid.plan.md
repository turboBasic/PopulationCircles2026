---
tags: [plan, code, popcircles]
created: 2026-08-15
---

# Implementation plan — ADR 0007, one attestation keyed on the whole grid

**Status: complete (2026-08-15).** Carries [ADR 0007](0007-cache-identity-binds-the-whole-grid.md) into
the tree, which is the whole of issue #45. It invalidates every cache and every ledger in existence, which
the ADR's Context prices and `README.md` already promises a release may do.

Measured on this tree before drafting: 253 Rust tests and 34 Python tests passing, `mise run ci` green in
21 s. The three modules this plan moves hold 16 tests (`table::cache`), 13 (`smallest::cache`) and 8
(`popcircles-cli --test commands`); `tests/test_lint_version_bumps.py` holds 10.

Eight facts settled here rather than met mid-task:

- **`Identity` already carries the whole geometry.** `table/cache.rs:145` holds a `Decimation`, which holds
  both grids, so no caller's side changes: the CLI's ten flags, `CachedTable::open` and every fixture that
  builds an `Identity` stay as they are. What narrows the geometry to three numbers is the two documents'
  field lists and their `check` bodies, which is all this plan moves.
- **A bumped version reports as a syntax error unless the version is parsed first.** Measured in a scratch
  crate outside this tree: a v1 header read into a `Header` carrying the four new fields fails with
  `missing field origin_lat`, which is `CacheError::HeaderSyntax` and never reaches `check`. A struct
  carrying `format_version` alone parses both a v1 and a v2 document. So the probe is part of 1.2 and 2.1
  rather than a follow-up — without it this plan's own bump is the failure issue #45 forbids.
- **The probe's leniency is a default, and it is already load-bearing on every read path.** A probe reads a
  real document, and every real document carries keys the probe does not declare, so `deny_unknown_fields` on
  one would fail every test that opens a cache rather than slipping through. Measured on 2026-08-15 that the
  attribute compiles beside `flatten` and refuses an unknown key, so the failure is loud rather than
  impossible. That is why 1.2 and 2.1 each name the property in a test instead of this being a register entry
  someone has to act on later.
- **`#[serde(flatten)]` emits in declaration order and keeps the digest exact.** Measured the same way:
  declaring `format_version`, then the flattened attestation, then `byte_order` produces
  `{"format_version":2,"digest":...,"width":...,...,"lat_step":...,"byte_order":"little"}` — v1's key order
  with the four geometry keys inserted before the last, so `the_header_leads_with_its_format_version` passes
  unchanged. A `u64` digest round-trips as `0xf17aa802a6890f0c`, and f64 round-trips bit-identically
  (1/120 as `0.008333333333333333`).
- **The four numbers are not equally reachable, so the fixtures differ per case.** `Grid::new` pins the
  origin latitude of a grid that runs pole to pole — ADR 0007's Context measures `--origin-lat 89.99999999`
  refused — so a tolerance case on latitude needs a sub-globe fixture, while the origin-longitude case works
  on the existing whole-globe `grid(4, 3)`. A task that reaches for `grid(w, h)` to test a latitude tolerance
  will be refused by the constructor before the cache is opened.
- **Both exit-code matches are exhaustive.** `main.rs:1065` and `:1118` have no `_` arm, so collapsing the
  four per-ground variants into one is a compile error in the CLI until it follows. Every geometry ground
  joins `EXIT_MISSING_DATA`, the class the four it replaces already carry.
- **The hook's trigger table pairs a block with the `FORMAT_VERSION` in its own file.**
  `scripts/lint_version_bumps.py:14` is `(path, names)` and `bumped(path, "FORMAT_VERSION")` reads the
  constant from that same path. A block shared by two formats has no expression in that shape, which is why
  3.1 exists: without it, a field added to the attestation would bump the table's constant and leave the
  ledger's untouched.
- **The wire format does not move.** No field of `report.rs` changes — only the note about what its `grid`
  is — so `SCHEMA_VERSION` stays 1 and none of the ten snapshots is rewritten.
- **The human layer needs nothing.** `README.md:213` already says any release may invalidate a cache or a
  ledger and that one it did not write is refused and rebuilt rather than migrated; `CONTRIBUTING.md:68`
  says the same to a maintainer.

## Ground rules

- **The geometry comparison and its tolerance exist once.** They live in the attestation, and both documents
  reach them through it. A second `BOUNDARY_TOLERANCE_DEG`, or a geometry comparison written into either
  `check` body, is the drift ADR 0007 decision 2 exists to prevent — and `grid.rs:17` says why the constant
  has one owner.
- **A task that moves a document's fields moves that document's `FORMAT_VERSION` in the same commit.** The
  `version-bumps` hook reads the index against HEAD, so a task deferring the bump cannot be committed at
  all. Do not reach for `SKIP=version-bumps`: this is the change the hook was written for.
- **A task that changes either error enum updates the CLI's exit-code match in the same commit.** The tree
  is red between the two, so they are one task and not two.
- **No fixture is the registry raster.** The geometry cases are built from `Synthetic` and from
  `commands.rs`'s in-process fixture at `:96`. The temptation is specific to this work — the defect is about
  the registry's own grid — and a test needing 428 MB of LFS content is a test CI cannot run.
- **`SCHEMA_VERSION` is not touched and no snapshot is accepted.** A rewritten snapshot in this plan means
  something changed that this plan did not decide, and is a finding to report rather than accept.

## Out of scope

- **Migrating a v1 cache or ledger.** ADR 0007 decision 5: a v1 document's fields are true and its silence
  is the defect, so there is nothing to migrate from.
- **Recording the source grid beside the coarser one.** Decision 1 — the factor and the coarser grid
  determine it, and the grid a query resolves against is the coarser one.
- **Keying the cache or ledger path on the geometry.** Weighed and lost in ADR 0007's alternatives: both
  paths are caller flags, so the convention is one a caller can decline.
- **A mask field in the attestation for #13.** No second caller yet, and `application.md` "Architecture"
  says not to build the extension point ahead of one. The point of one owner is that #13 adds it once.
- **Publishing the geometry as a new wire field.** What changes is the claim `report.rs` makes about the
  `grid` it already publishes. A new field would move `SCHEMA_VERSION` and all ten snapshots for a consumer
  nobody has asked for.
- **Tightening `BOUNDARY_TOLERANCE_DEG`, or taking it as a caller's argument.** The reader's rule is the
  point of decision 3, and a per-caller tolerance would be a second answer to one question. `FU-14` is where
  a finer grid forces the question.

## Phase 1 — the attestation, and the table's header on it

- [x] **1.1 An attestation exists in `table/cache.rs`, built from an `&Identity` and compared against one.**
  A `pub struct Attestation` carrying `digest`, `width`, `height`, `decimation`, `origin_lat`, `origin_lon`,
  `lon_step`, `lat_step`, deriving `Serialize`, `Deserialize`, `Debug`, `Clone`, `Copy` and `PartialEq`;
  `Attestation::new(&Identity)`; and `check(&self, wanted: &Identity) -> Result<(), Mismatch>` beside a
  `pub enum Mismatch` whose eight variants are the grounds, each carrying what was wanted and what was
  found. The digest, the dimensions and the factor compare exactly; the four geometry numbers compare within
  `BOUNDARY_TOLERANCE_DEG` imported from `grid`, longitude through `wrap_lon` — `raster/geotiff.rs:369` and
  `:386` are the shape to follow. Nothing embeds it yet, so no document and no version moves in this task.
  The latitude and step cases need a sub-globe fixture, per the fact list above.
  Verify: `cargo test -p popcircles --lib table::cache` runs 16 plus the new tests, green, among them one
  per ground and both sides of the tolerance — an origin 1.16e-11 away accepted, 1e-8 away refused as
  `Mismatch::OriginLat`; `rg -c 'const BOUNDARY_TOLERANCE_DEG' crates/popcircles/src` returns 1 and
  `rg -n '1e-9|1e-09' crates/popcircles/src/table/cache.rs` returns nothing, which is what makes the
  constant's owner the only owner.

- [x] **1.2 The cache header is the attestation, at `FORMAT_VERSION = 2`, and its version is read before the
  document.** `Header` declares `format_version`, then the attestation with `#[serde(flatten)]`, then
  `byte_order`; it loses its derived `Eq` and keeps `Copy`. `checked_header` parses a `format_version`-only
  struct first, compares it, and only then parses the `Header` — with a comment at the probe saying it rests
  on serde ignoring unknown fields by default, which is what makes it able to read a document of any version.
  `CacheError`'s `Digest`, `Width`, `Height` and `DecimationFactor` variants collapse into one carrying
  `Mismatch`, and `exit_code_for_cache_error` follows them into `EXIT_MISSING_DATA`. `FORMAT_VERSION` becomes
  2 in the same commit.

  **The refusal a person reads must not lose what those four variants said.** `Failure` at
  `popcircles-cli/src/main.rs:356` prints the error's `Display` to stderr, so the wrapper names the document
  and the ground names what differed and what was wanted — `Mismatch`'s messages are phrased to read in both
  documents, since 2.1 wraps the same enum in `LedgerError`. Nothing pins these strings today, which is why
  the verification below does.
  Verify: a new test writing the literal v1 document
  `{"format_version":1,"digest":…,"width":4,"height":3,"decimation":1,"byte_order":"little"}` asserts
  `CacheError::FormatVersion { expected: 2, found: 1 }` and **not** `HeaderSyntax`, which is the failure this
  task exists to prevent; a test asserts a cache built over `grid(4, 3)` is refused for the same width,
  height, factor and digest at an origin longitude a half turn away, naming `Mismatch::OriginLon`;
  `the_header_leads_with_its_format_version` passes with `{"format_version":2,`; a test named for the
  property the probe rests on asserts it reads the version out of a document carrying a key it does not
  declare, so `deny_unknown_fields` on the probe fails a test that says why rather than a dozen that do not;
  a test asserts the refusal's own text for a geometry ground and a dimension one names the cache header and
  both numbers, which is what the four collapsed variants used to say and nothing else now pins; with the
  change staged `prek run version-bumps` passes, and with `FORMAT_VERSION` reverted to 1 it fails naming the
  moved fields; `cargo test -p popcircles --lib table::cache` and `cargo test -p popcircles-cli` green.

## Phase 2 — the ledger on the same attestation

- [x] **2.1 The ledger document is the attestation, at its own `FORMAT_VERSION = 2`, and its version is read
  before the document.** `Document` declares `format_version`, the flattened attestation, then `radii`;
  `Document::check` is the attestation's check and nothing else; `open_or_empty` parses the version-only
  struct first, as 1.2 does. `LedgerError`'s four per-ground variants collapse into one carrying `Mismatch`,
  and `exit_code_for_ledger_error` follows. The ledger's own `FORMAT_VERSION` becomes 2 in the same commit —
  it is a second constant, not the table's.
  Verify: a test asserts a v1 ledger document is refused as `LedgerError::FormatVersion { expected: 2,
  found: 1 }`; a test asserts a ledger filled over `grid(4, 3)` and reopened for a grid differing **only** in
  origin longitude is refused naming `Mismatch::OriginLon`, where before this task it would mint each probe's
  `row`/`col` back onto the new grid at `smallest/cache.rs:234` and resume; the ledger's probe carries 1.2's
  leniency test in its own module, because the two probes are separate structs and a test of one says nothing
  about the other; a test asserts a refusal's text names the **ledger** and not the header, which is the half
  of 1.2's message shape a shared `Mismatch` could quietly drop; `cargo test -p popcircles --lib
  smallest::cache` runs 13 plus the new tests green; `prek run version-bumps` passes with the change staged
  and fails with the bump reverted.

## Phase 3 — the gate, the end-to-end refusal, and close-out

- [x] **3.1 The `version-bumps` hook requires both constants when the shared block moves.**
  `STRUCT_TRIGGERS` carries, per watched block, the files whose `FORMAT_VERSION` its shape governs:
  `Header` the table's, `Document` and `Probe` the ledger's, and `Attestation` **both**. `bumped` is asked
  of every constant a trigger names, and a finding names the constants that did not move.
  `tests/test_lint_version_bumps.py` follows the new tuple shape — `test_the_watched_blocks_are_the_ones_on_disk`
  and `test_the_watched_constants_are_the_ones_on_disk` both unpack it — and pins the attestation as a
  watched block against two constants, so a later plan cannot quietly drop the pairing.
  Verify: `uv run pytest tests/test_lint_version_bumps.py` runs 10 plus the new tests green; with a field
  staged into `Attestation` and neither constant moved, `prek run version-bumps` fails naming both files;
  with only `table/cache.rs`'s constant moved it still fails naming `smallest/cache.rs`; with both moved it
  passes, and the scratch edit reverts clean.

- [x] **3.2 A command declaring another grid is refused end to end.** A case in
  `crates/popcircles-cli/tests/commands.rs` modelled on
  `a_digest_naming_another_table_is_missing_data_and_prints_nothing`: the fixture's own cache, queried with
  every flag as built except an origin longitude a half turn away, exits `EXIT_MISSING_DATA`, prints nothing
  on stdout, and names the origin on stderr. This is the whole record in one assertion — before this plan the
  same invocation answered with a population.
  Verify: `cargo test -p popcircles-cli --test commands` runs 9 tests green; the new case's assertion on
  stdout is emptiness rather than a JSON document.

- [x] **3.3 `report.rs` says what is attested now.** The module note at `:24` and `Provenance`'s doc comment
  at `:118` stop naming `FU-11` as an open gap and say that the grid is attested by the cache that answered,
  in the same sense the digest and the factor are — the header binds it and opening one compares it.
  `docs/ai/application.md` needs nothing: its `table/cache.rs` clause names the header and the atomic
  publication rather than the header's fields.
  Verify: `rg -n 'FU-11' crates/popcircles/src` returns nothing; `rg -n 'declared'
  crates/popcircles/src/report.rs` no longer matches in a sentence about the cache's grid;
  `git diff --name-only crates/popcircles/src/snapshots/`
  is empty and `rg -n 'SCHEMA_VERSION: u32 = 1' crates/popcircles/src/report.rs` still matches, which is what
  proves this task changed a claim and not a format; `mise run lint:docs` clean.

- [x] **3.4 The register carries what this plan settled and what it left, #45's boxes are ticked, and this
  plan is closed.** `FU-11` closed in [`../follow-ups.md`](../follow-ups.md) with the date and what closed
  it, naming the departure from its Fix as written: the geometry went into the header alone, because
  `Identity` already carried it. One new entry, with a condition a sweep can answer: `FU-14`, a registry
  dataset whose grid step is close enough to `BOUNDARY_TOLERANCE_DEG` that comparing geometry within it stops
  being negligible — the sweep is the step figure in `data/README.md`'s registry row against the constant at
  `grid.rs:17`. The probe's leniency gets no entry: 1.2 and 2.1 pin it in CI, and an obligation a gate
  already discharges is the kind of stale entry the register's own preamble calls worse than none. Tick the
  boxes of #45 this plan discharged and the roadmap's `#45` box in #11, without closing either issue — the
  PR's `Closes #45` does that, per `platform.md` "Git". Then the status line above reads
  `**Status: complete (YYYY-MM-DD).**` and the Follow-ups section below holds the one identifier.
  Verify: `rg -n '^### FU-1[14]' docs/follow-ups.md` names two entries and `FU-11`'s Status line reads
  `closed` with a date; `gh issue view 45` shows its five boxes ticked and the issue still open;
  `gh issue view 11` shows the `#45` box ticked; `mise run ci` green.

## Follow-ups

Written by 3.4, in [`../follow-ups.md`](../follow-ups.md): `FU-14`.
