---
tags: [plan, code, popcircles]
created: 2026-08-13
---

# Implementation plan — ADR 0001, the CLI crate and the JSON contract

**Status: in progress (2026-08-13).** Carries the work
[ADR 0001](0001-cli-and-output-layer.md) decided. It lands the frame, not the command surface: the four
commands #8 names need #3 through #7, and #16's need #13 through #15. What is executable today is the
crate boundary, the wire format's owner, and one command surface thin enough to prove both against code
that already exists.

## Ground rules

These add to the normal task loop; they do not replace it.

- **No domain type gets a serde derive.** ADR 0001 decision 3 puts the wire format in `report`, and the
  shortcut this rules out is deriving `Serialize` on `LatLon` or `Grid` and skipping the DTO. It costs
  less today and makes every later rename of a field a change to a published format.
- **`clap` and `anyhow` never appear in `crates/popcircles/Cargo.toml`.** That table is the gate the
  split bought; a helper wanted in both crates goes in the library without them, or is duplicated.
- **stdout carries the JSON document and nothing else.** Every diagnostic, warning and progress line
  goes to stderr. clap's own `--help` and `--version` on stdout are not diagnostics and are correct
  there.
- **Do not restore clap's default features.** ADR 0001 took the loss of flag suggestions deliberately
  and says so in its costs; adding `color` or `suggestions` back is a decision to record, not a
  convenience to take.
- Two new `Cargo.toml` files land in this plan, so `mise run fmt` runs taplo over them before the
  commit. The `[lints] workspace = true` block is not optional in either — without it a new crate
  silently opts out of every lint the workspace sets.
- Every task ends green under `mise run ci`, and is its own commit.

## Out of scope

- **The progress sink of ADR 0001 decision 4.** Nothing in the library runs long enough to report
  progress from; #3 is its first caller. Declaring the trait now is the extension point ahead of a
  second caller that `application.md` "Architecture" rules against, and the ADR states the rule so the
  shape is settled when the caller arrives.
- **The `missing data` and `interrupted` exit-code classes #8 names.** There is no dataset to be
  missing until #2 and no long run to interrupt until #6. The mapping covers the errors that exist and
  grows with them.
- **The four commands #8 names** — `population-at`, `most-populous`, `smallest-for-share`, `sweep`.
  Each needs the search. This plan proves the frame holds with commands whose answers the library can
  already compute.
- **Provenance in the envelope** — which raster and which parameters produced a result. #8's second box
  wants it and #2 and #3 own the values; the envelope is a struct that gains fields.
- **`clap_complete` and `clap_mangen`.** Both are worth having and neither is worth generating against a
  command surface that #8 and #16 will replace.
- **`tracing`, `indicatif`.** ADR 0001 weighed and rejected both for now, with the condition under which
  they return.
- **Anything Python.** #9 and #17 consume the contract; nothing in this plan produces a renderer.

## Phase 1 — the crate boundary

**Model: Sonnet 5.** A file move, two manifests and a task. The judgment was ADR 0001's; what is left is
mechanical, and its verification is a `cargo tree` line rather than a reading.

- [x] **1.1 The binary is its own crate.** `crates/popcircles-cli/` exists with `src/main.rs` — the
      current three-line stub, moved, not rewritten — and a manifest inheriting `edition`,
      `rust-version`, `license`, `repository` and `authors` with `field.workspace = true`, carrying
      `[lints] workspace = true` and a path dependency on `popcircles`. `crates/popcircles/src/main.rs`
      is gone, so the library is lib-only. The root manifest needs no edit: `members = ["crates/*"]`
      already globs the new directory. A `[tasks.cli]` in `mise.toml` runs
      `cargo run -p popcircles-cli --`, so the invocation has one home rather than being repeated in
      prose. `Cargo.lock` is committed in the same commit.
      *Verify:* `cargo tree -p popcircles -e normal | rg 'clap|anyhow|serde_json'` returns nothing —
      the property the split exists to hold, and the one that stays checkable after every later task;
      `rg -n 'fn main' crates/popcircles/src` returns nothing; `mise run cli` prints
      `popcircles: not implemented yet`; `mise run ci` green.

## Phase 2 — the contract, on the library side

**Model: Opus 5.** The envelope's shape is what #8, #16, #9 and #17 all read, and the version field is
the only part of it that cannot be fixed additively later. Getting the DTO boundary wrong here is the
mistake that reaches four issues.

Both tasks land in `crates/popcircles/src/report.rs`, declared in `lib.rs`. Nothing here knows about
clap, a path, or a stream.

- [x] **2.1 The envelope and two payloads.** `report::SCHEMA_VERSION`, an `Envelope<T>` carrying the
      schema version, the tool name and the tool version from `CARGO_PKG_*`, and the payload under a
      `result` key. Two payload types, each `Serialize` and each built from domain values by an explicit
      conversion rather than a derive on the domain: a distance report (the two coordinates and the
      great-circle kilometres) and a grid summary (dimensions, origin, steps, whether the columns close,
      and the cell area at the grid's middle row). `serde` with `features = ["derive"]` joins
      `[workspace.dependencies]` and the library inherits it; `uv lock` is not involved and
      `Cargo.lock` is committed.
      *Verify:* `rg -n 'Serialize|Deserialize' crates/popcircles/src/geodesy.rs
      crates/popcircles/src/grid.rs` returns nothing, which is ADR 0001 decision 3 made checkable
      rather than remembered; `cargo tree -p popcircles -e normal | rg 'clap|anyhow'` still returns
      nothing; a unit test asserts the serialised envelope's first key is `schema_version` and its value
      is `1`.

- [x] **2.2 Snapshots pin the wire format.** `insta` with its `json` feature as a dev-dependency of the
      library, and one snapshot per payload type over a fixed input — the quarter-circumference pair
      (0, 0) to (0, 90) for the distance report, the 1° whole-globe grid for the summary. The snapshots
      live with the library because ADR 0001 puts the contract there: they must be able to fail without
      the argument parser existing.
      *Verify:* both `.snap` files are committed under `crates/popcircles/src/snapshots/`; each has
      `schema_version` as its first key, so the snapshot pins key *order* and not merely presence;
      `cargo test -p popcircles` passes with no `INSTA_FORCE_PASS` or `INSTA_UPDATE` set in
      `mise.toml`, `.github/workflows/` or the environment — a snapshot suite that rewrites itself
      under CI asserts nothing.

## Phase 3 — the command surface

**Model: Sonnet 5.** clap's derive and a JSON print, against an envelope and two payloads that already
exist and are already snapshotted. The one part with a trap is 3.2's exhaustive match, and the task says
what it must be.

Everything here lands in `crates/popcircles-cli/`.

- [x] **3.1 `distance`, end to end.** `clap` joins `[workspace.dependencies]` as
      `{ version = "4", default-features = false, features = ["std", "derive", "help", "usage",
      "error-context"] }` and `anyhow` as `"1"`, both inherited by the CLI crate only. A `Cli` struct
      with `#[command(name = "popcircles", version)]`, a `Command` enum, and a `distance` variant taking
      four coordinates. It builds the library's `LatLon` values, calls `great_circle_km`, wraps the
      payload in the envelope and writes it to stdout with `serde_json`. `main` returns
      `std::process::ExitCode`; `anyhow` carries context to the edge and nowhere else.
      *Verify:* `mise run cli -- distance 0 0 0 90 2>/dev/null | jq -e '.result.great_circle_km'`
      prints `10007.55722101796` — one ulp below the exact `R·π/2`, which is haversine's `atan2` path
      rather than a bug, and the value the 2.2 snapshot already pins; piping through `jq -e` with
      stderr discarded is what proves stdout carries a JSON document and nothing else; `mise run cli
      -- --help` lists `distance`; `cargo tree -p popcircles -e normal | rg 'clap|anyhow'` still
      returns nothing.

- [x] **3.2 `grid describe`, and errors that become exit codes.** A `grid` subcommand taking width,
      height, origin and steps as flags, constructing a `Grid` and emitting the summary payload. A pure
      function in the CLI crate maps `GridError` to an exit code by **exhaustive match** — no `_` arm —
      so a variant added to the library fails this crate's build rather than falling into a default.
      Bad input is one code, distinct from success; the classes #8 names for missing data and
      interrupted work are out of scope above.
      *Verify:* `mise run cli -- grid describe --width 43200 --height 21600 --origin-lat 90
      --origin-lon -180 --lon-step 0.008333333333333333 --lat-step -0.008333333333333333
      2>/dev/null | jq -e '.result.spans_full_turn'` prints `true`; the same command with
      `--height 21601` writes nothing to stdout, writes a message naming the south pole to stderr, and
      exits non-zero; `rg -n '_ =>' crates/popcircles-cli/src` returns nothing in the mapping's file;
      a unit test constructs every `GridError` variant and asserts its code.

## Phase 4 — close-out

**Model: Opus 5 for 4.1.** Two documents state the crate count as a fact and both are in the instruction
layer, where a correction that says more than the change owes is the repository's standing failure mode
and no gate catches it. 4.2 is a register entry and two `gh` comments.

- [x] **4.1 Documentation the split invalidated.** Two claims stop being true and both are about crate
      count, not about the CLI:
      - `docs/ai/application.md` line 71, "A module per subject inside the one crate, splitting into
        more crates only when a dependency forces it" — the split happened and a dependency is exactly
        what forced it, so the sentence's rule survives while its premise does not. Say there are two
        crates and which one may not grow dependencies. Lines 50–51's "one library crate" is still
        literally true and needs no edit; leave it.
      - `README.md`'s Layout table row `crates/popcircles/` reads "Rust library and binary — the
        search". One row per crate.

      `docs/ai/platform.md` "Structure" needs **no** change: `crates/` is already a root there and that
      section carries roots rather than an inventory. ADR 0001's own Consequences are not edited — an
      accepted record's prose stays as written.

      One coordination note, not an edit: `tmp/issue-2-geotiff-reader.plan.md` task 5.2 also rewrites
      this paragraph of "Approach", for the I/O purity claim. Whichever runs second reads the paragraph
      as it then stands rather than as its own plan quotes it.
      *Verify:* `rg -n 'library and binary'` and `rg -n 'inside the one crate'` both return nothing;
      `README.md`'s Layout table has a row for `crates/popcircles-cli/`; `prek run --all-files` green.

- [ ] **4.2 Close the plan.** `FU-03` in [`../follow-ups.md`](../follow-ups.md), in that file's format
      and meeting its bar: nothing couples a change to a `report` type to a bump of `SCHEMA_VERSION`,
      and the condition a sweep can evaluate is a commit that changes a file under
      `crates/popcircles/src/snapshots/` without changing `SCHEMA_VERSION`.

      Then four issue notes, because the frame reaches past the two CLI issues. **None of them edits a
      "Done when" list.** Those are what the roadmap discovered #8 and #16 must satisfy, and a box this
      frame half-satisfies is still unsatisfied; a comment records what a record has since decided
      without rewriting what the issue asked.
      - **#8** — name ADR 0001 and which *fragments* of its four boxes the frame satisfies: stable key
        ordering but not the synthetic fixture, version stamping but not provenance, a machine-readable
        stdout but no progress, bad input but neither missing data nor interruption. Its Goal, the four
        commands, is untouched.
      - **#16** — the envelope is the contract its second box bumps rather than one that box defines,
        and `SCHEMA_VERSION` is where "existing world-level output stays readable" gets decided.
      - **#3** — `serde` is already in the library, which its cache header needs; its bounded build is
        the first caller of the progress sink ADR 0001 decision 4 fixes the shape of; and `FU-03` is the
        pattern for binding a format version to the thing it versions.
      - **#9, and #11's body** — #11 reads "#9 needs only the schema from #8". After ADR 0001 the schema
        is the library's `report` module and #8 writes it rather than defining it. That sentence is the
        one body edit this task makes, because it names an owner that moved.

      Then this plan's status line reads `**Status: complete (YYYY-MM-DD).**` and the Follow-ups section
      below holds `FU-03`.
      *Verify:* `rg -n 'FU-03' docs/follow-ups.md` matches an entry with all three fields; #8, #16, #3
      and #9 each carry the comment and no "Done when" list in any of them has changed; #11's body no
      longer says #9 takes the schema from #8; this file's status line reads complete and its Follow-ups
      section names no candidate that is not a register entry.

## Follow-ups

One candidate, not an entry until 4.2 writes it:

- **Nothing couples a wire-format change to a version bump.** `SCHEMA_VERSION` is a constant a change to
  a `report` type is free to ignore, and the snapshots will happily record the new shape under the old
  number. The condition is checkable — a commit touching `crates/popcircles/src/snapshots/` without
  touching `SCHEMA_VERSION` — which is what makes it a register entry rather than a note here.
