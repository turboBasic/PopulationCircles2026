---
tags: [plan, code, popcircles]
created: 2026-08-15
---

# Implementation plan — ADR 0009, validation against the published result and the benchmark harness

**Status: in progress (2026-08-15).** Carries issue #10, the tenth and last step of roadmap #11: the
comparison against the published prior art, the gated end-to-end run against the real raster, the
benchmarks, and the accuracy note. It is the sibling of
[ADR 0009](0009-validation-brackets-cheap-and-certifies-dear.md), whose five rulings the tasks below
implement.

The record's Context holds every figure this work rests on, measured on one machine on 2026-08-15. Three of
them decide the shape of the tasks and are repeated here because a reader deciding whether a task is sized
right needs them at hand:

| Measured | Result | What it decides |
| --- | --- | --- |
| one fixed-radius search at 30 arcsec | 207 s wall, 13.4 s CPU | 2.1 is a 5 arcmin test, not a full-resolution one |
| the same query resident | 18.6 ns against 31 µs mapped | 2.2's harness reports both or says it skipped |
| wall clock against initial spacing | falls monotonically, flat past 256 | `FU-08`'s premise is corrected, not discharged |

## Ground rules

- **No figure lands in a live document.** ADR 0009 decision 4: a measured number goes in the record that
  measured it. A task that wants to publish one puts it in the ADR, or in `README.md` with its date.
- **A benchmark asserts nothing.** No task below adds one to `lint`, `test` or `ci`, and none makes a test
  depend on a benchmark's output.
- **The divergence from 3 300 km is explained, never closed.** No task changes an earth radius, a dataset or
  a comparison to move the answer toward a published figure.
- **Nothing in `test` or `ci` may need the raster.** The gated test skips with a message; `mise run ci` stays
  green on a clone holding pointers.

## Out of scope

- **`FU-08`'s derivation.** ADR 0009's last alternative: the entry's Fix is a change in `search` beside the
  level loop, which changes what every caller of `most_populous` receives including the CLI's required
  `--spacing`. This plan measures the curve and leaves the entry open with its premise corrected.
- **A performance gate.** Decision 1's cost, stated there: one `Instant` sample, no baseline, no variance. A
  regression check needs a stored baseline and a machine to compare against, and this repository has
  neither.
- **The full-resolution search over radius.** Decision 3. It stays available and resumable through the
  ledger; what is recorded is the certified bracket, not a 90-minute run in a task.
- **A traversal that faults fewer pages.** The 6.5% CPU figure invites one, and it is an algorithm change
  with its own record. Raised as a follow-up rather than built.

## Phase 1 — the record

**Model: Opus 5.** The rulings are what every later task cites, and decision 3 in particular is a claim
about what may be left unmeasured.

- [x] **1.1** `docs/decisions/0009-validation-brackets-cheap-and-certifies-dear.md` is an accepted record,
  and this file sits beside it under the same number. It rules five things, each with a measurement behind
  it: a benchmark is a `harness = false` target timing with `std::time::Instant` and adds no dependency;
  it reports the mapped figure beside the resident one or says it skipped it; validation brackets on a
  decimated table and certifies at full resolution; a measured figure lives in the record that measured it;
  and the accuracy note is `report`'s module documentation. Its Context carries the table build, the search,
  the half-the-world answer, the spacing curve and the four sources of the divergence from 3 300 km.
  Written with the `write-adr` skill.
  Verify: `mise run lint:docs` and `mise run lint:markdown` pass with both files in the tree, and
  `rg -l 'ADR 0009' docs/ crates/` names the record, this plan and `report.rs` — nothing else claims the
  number.

## Phase 2 — the harness, the test and the note

**Model: Opus 5.** 2.2 is the one task here that can fail by passing: a validation test whose band is wide
enough to admit a wrong answer looks exactly like one that works.

- [x] **2.1** Three `harness = false` bench targets under `crates/popcircles/benches/`, one per subject
  issue #10 names, with their `[[bench]]` entries in the library manifest and one mise task each plus a
  `bench` aggregate. `table_build.rs` streams a generated raster at the registry's own mix of nodata, zero
  and counts — so the build is measured on a plausible input and needs no LFS object — at three shapes, and
  then once more through the cache writer so the payload write is isolated from the arithmetic.
  `kernel.rs` builds kernels evenly spaced over two shapes at four radii, and prints the sample size rather
  than implying the whole grid. `circle.rs` reports the resident figure from a table it builds and the
  mapped figure from a full-resolution cache under `out/`, skipping the second with a message naming what
  would produce one. `bench:table` is out of the aggregate because it writes 7.5 GB.
  Verify: `mise run bench` prints a resident and a mapped line, or a resident line and a skip naming
  `table build`; `cargo test --all-features` runs no benchmark; `mise run lint:rust` passes with
  `--all-targets` compiling all three; `rg -n 'criterion' Cargo.toml crates/*/Cargo.toml` is empty.

- [x] **2.2** `crates/popcircles/tests/registry_validation.rs` is the gated end-to-end run, with a
  `test:validate` task. It streams the registry raster into a 5 arcmin table, checks the world total against
  the registry row, searches for the smallest circle holding half of it, and asserts the radius inside a
  40 km band, the centre inside a degree of Yunnan, the bracket the search proved, that the answer reaches
  the target, and that no ambiguity was reported. Skipped with a message naming `mise run data:pull` when
  the raster is an unfetched pointer — box 2 of the issue — which is an early return, because a `#[test]`
  cannot skip.
  Verify: `mise run test:validate` passes on this machine with the raster fetched, and the same task on a
  clone with pointers prints the skip and exits 0; `mise run test` does not select it.

- [x] **2.3** `report.rs`'s module documentation gains the accuracy note, between the documents table and
  Growth. It composes rather than measures: 4 ulp per rectangle query, the predicate slack over the rows a
  circle spans, `tolerance_persons` of zero and what that zero asserts, the radius as a proved bracket in
  whole kilometres, and the centre as a cell centre rather than a point on the continuum. No second file
  states the composition.
  Verify: `mise run lint:rustdoc` passes; `rg -ln '4 ulp' crates/ docs/ai/` names `circle.rs`, `report.rs`,
  `search.rs`, `smallest.rs` and `application.md`, every one of them citing ADR 0003 for the figure rather
  than stating one of its own.

## Phase 3 — the documentation and the close

- [ ] **3.1** The human layer stops claiming this work is outstanding. `README.md`'s opening sentence and
  its Usage section both say validation against the published result is a later step; both become the
  result, with its date and the four sources of the divergence named in one sentence each, sending a reader
  to ADR 0009 for the measurements. The new tasks appear where the other tasks are documented.
  Verify: `rg -n 'is a later step|What is left is validation' README.md` is empty; `mise run lint:docs` and
  `mise run lint:markdown` pass.

- [ ] **3.2** `docs/follow-ups.md` records what this work produced and corrects what it touched. `FU-08`
  gains a dated note: the curve it was waiting for exists, its premise that the ceiling is two orders of
  magnitude from the answer is wrong, and the entry stays `dormant` because no caller outside `search.rs`
  has chosen a spacing — the new gated test picks one, and it is a deselected fixture, which that entry
  already excludes for `decimated_search.rs`. A new entry carries the traversal: the search spends 6.5% of
  its wall clock on CPU at full resolution, and the condition names the figure a later reader can re-measure.
  Verify: `rg -n 'FU-08' docs/` shows the note dated 2026-08-15 and the status still `dormant`; the new
  entry is the last in the register and names a command.

- [ ] **3.3** The plan is closed: the status line reads `**Status: complete (2026-08-15).**`, Follow-ups
  holds the identifiers 3.2 wrote, and the four boxes in issue #10's body are ticked along with #10's box in
  roadmap #11. The issue is left open — the PR's `Closes #10` closes it, per `platform.md` "Git". The
  spacing finding is a comment on #10 rather than a silent correction, because it contradicts a proposal
  made in that thread.
  Verify: `gh issue view 10` shows four ticked boxes and state OPEN; `gh issue view 11` shows #10 ticked;
  `mise run ci` is green.

## Follow-ups

Written by 3.2, in [`../follow-ups.md`](../follow-ups.md): `FU-17`.
