---
tags: [plan, code, popcircles]
created: 2026-08-14
---

# Implementation plan — ADR 0003, the summation table and its cache

**Status: in progress (2026-08-14).** Carries the work
[ADR 0003](0003-summation-table-cache.md) decided, which is the whole of issue #3: the table, its
accuracy, its cache and the one `unsafe` that reads it. Nothing that consumes the table is here — #4's
kernels and #5's circle evaluation are the first callers, and both are behind this.

## Ground rules

These add to the normal task loop; they do not replace it.

- **`table.rs` names no path, no file and nothing that serialises.** ADR 0003 decision 1. The shortcut this rules
  out is a `Table::open(path)` convenience on the pure type — the same shortcut that would have left
  `raster.rs` owning `raster/geotiff.rs`'s job.
- **One `unsafe` block and one `#[allow(unsafe_code)]`, both in `table/cache.rs`.** A second of either,
  or an allow at module or crate scope where it covers code nobody reviewed as unsafe, is a decision to
  record rather than a convenience to take. Task 2.2 lands the hook that holds this after the plan ends.
- **Every fold accumulates in f64.** The measured trap is 29.5 persons per dense 10 × 10 block. The
  `f32` arriving from `RasterRow` widens on the way in and is never an accumulator.
- **The digest is computed in the pass that is already reading the cells.** Reopening the file to hash
  it is the shape decision 3 rejected, and it would work — which is what makes it worth ruling out
  here rather than trusting nobody reaches for it.
- **A test needing release-mode time is `#[ignore]`d with a task of its own.** `platform.md` "Testing"
  defers fetched rasters, network and credentials; slowness is not on that list, and a thirty-second
  `cargo test` is how a suite stops being run. Only task 1.3 has one.
- Every task ends green under `mise run ci`, and is its own commit.

## Out of scope

- **The search's traversal order.** #6's. ADR 0003's cold-mapping measurement is why it matters, and
  nothing here decides how a scan walks the table.
- **A parallel build.** The compensation costs 0.6 s single-threaded over the whole grid. Parallelising
  a prefix sum needs a scan decomposition and a second correctness argument, for a build whose cost is
  LZW decode and a 7.5 GB write.
- **A masked or cropped table.** #13 and #14. The header takes an extent or a mask identity additively
  when a caller holds one; adding the field now is the extension point ahead of a second caller that
  `application.md` "Architecture" rules against.
- **The registry SHA-256 in the header.** Decision 3 keeps it as provenance for the file. Carrying it
  as an optional field waits for a caller that has the value.
- **Wiring progress to a stream.** #8's, per ADR 0001 decision 4. This plan declares the sink and
  passes it; nothing here writes to a terminal.
- **Benchmarks.** #10's. ADR 0003's figures come from a scratch crate outside this tree, and this plan
  commits no harness that reproduces them.
- **Reading the registry raster.** #10's gated test is the end-to-end run. Every fixture here is
  `raster::Synthetic` or built in code.

## Phase 1 — the pure table

**Model: Opus 5.** Three of these four tasks fail by producing a plausible wrong number rather than an
error, which is the failure mode `application.md` "Correctness invariants" is written against. Nothing
in this phase touches a file or adds a crate.

- [x] **1.1 The progress sink exists.** `crates/popcircles/src/progress.rs`, declared in `lib.rs`: a
      trait with one method, `fn advance(&mut self, done: u64, total: u64)`, and an implementation for
      `()` that does nothing, so a caller wanting no reporting passes `()` and the builder's signature
      does not fork. ADR 0001 decision 4 fixed this shape and left it undeclared until a caller existed;
      1.3 is that caller.
      *Verify:* `rg -n 'Write|Stdout|Stderr|io::' crates/popcircles/src/progress.rs` returns nothing —
      the sink names no stream, which is the whole of what makes it the caller's choice;
      `rg -n 'print!|println!|eprintln!' crates/popcircles/src` still returns nothing, so `FU-04`'s
      first condition stays unfired by the module that could most easily fire it; a unit test drives a
      counting implementation and asserts the final `(done, total)` pair; `cargo tree -p popcircles -e
      normal` lists no crate it did not before.

- [x] **1.2 The rectangle query exists, over a borrowed payload.** `crates/popcircles/src/table.rs`,
      declared in `lib.rs`: `Table<'a>` over a `&'a [f64]` — no storage generic and no trait, per ADR
      0003 decision 1 — holding the padded `(height + 1) × (width + 1)` layout decision 4 fixes, with a
      constructor that rejects a payload whose length is not that product. The query takes a row band and
      a longitude span, and the span is a type whose full-turn case is a **variant** rather than
      `west == east`, which is what makes the double-count upstream's `rasterCircleFinder.cpp:173` `TODO`
      worries about impossible to write instead of guarded against.
      *Verify:* unit tests over a hand-written 3 × 4 payload assert an interior rectangle, one wrapped
      across the antimeridian, and the full turn — and the full-turn case asserts equality with the row
      band's own total, not with a sum of two rectangles, which is the assertion that would have caught
      the double count; a test asserts a payload one element short and one element long are both
      rejected at construction; `rg -n 'std::fs|Path|serde|unsafe' crates/popcircles/src/table.rs`
      returns nothing.

- [x] **1.3 The builder streams a raster into table rows, compensated.** In `table.rs`: it consumes an
      `impl RasterSource` and a `&mut impl Progress`, emits each completed padded row to a sink the
      caller supplies, and returns the digest, the `CellTallies` the source finishes with, and the
      compensated total. The digest is decision 3's exactly: FNV-1a 64-bit, standard offset basis and
      prime, over the sanitised cells in row-major order, each cell's `f32::to_bits()` widened to `u64` —
      a unit test pins it against a hand-computed value over a three-cell fixture, because a digest
      nothing pins is a number that happens to match today. Neumaier compensation in the row prefix and
      in a per-column correction array — decision 2 — so the resident state is one accumulator row, one
      correction row and the borrowed input row, ~900 KB at full width, which is the answer issue #3's
      fourth box asks for and the task states as a figure.
      *Verify:* a proptest over random small grids asserts every axis-aligned rectangle sum equals a
      direct sum **exactly**, wrapped and full-width rectangles included — issue #3's first box, with the
      generator restricted to values whose partial sums are exact in f64 (integers below 2²⁰ will do), so
      the assertion is about the traversal and cannot fail for a rounding reason, which is decision 2's
      second paragraph; the digest test above; an `#[ignore]`d test at
      the full 21600 × 43200 shape, against an exact `i128` reference in units of 2⁻⁴⁰, asserts max cell
      error ≤ 1 ulp and max query error ≤ 4 ulp, and is run by a new `[tasks."test:slow"]` whose comment
      says why it is not in `test` — a debug build spends minutes on 933 120 000 cells, the reason
      `test:raster` already gives for `--release`, whose comment gains a clause saying the deselected set
      now also holds a test that needs time rather than data; the tolerance carries a comment recording
      that the uncompensated form measures 1.2e-4 at that shape, so a reader can delete the correction
      and watch the test fail, which is what makes the assertion about the construction rather than
      about f64.

- [x] **1.4 Decimation folds in the builder.** A factor that must divide both grid dimensions, rejected
      at construction otherwise; blocks accumulate in f64 before the prefix pass; the factor travels
      with the built table so 2.1's header can record it. Decision 6, and the reason it is here rather
      than behind the seam is the 29.5-person measurement.
      *Verify:* a test asserts the decimated table's sum over `[r1, r2) × [c1, c2)` equals the full
      table's over `[k·r1, k·r2) × [k·c1, k·c2)` within the 4 ulp budget, on a synthetic grid k divides —
      issue #3's third box as an equality that can fail, rather than as a description; a test asserts a
      factor dividing neither dimension is rejected, and one that divides only the width likewise;
      `rg -n 'f32' crates/popcircles/src/table.rs` shows `f32` only where a value arrives from
      `RasterRow`, never as an accumulator.

## Phase 2 — the cache

**Model: Sonnet 5 for 2.1, Opus 5 for 2.2.** 2.1 is a serde struct, a write, a read and five rejection
tests against a shape ADR 0003 fixed. 2.2 is the `unsafe`, the lint demotion and the gate that bounds
it, where the `// SAFETY:` argument is the part worth an expensive reading.

- [x] **2.1 The cache writes and reads, without mmap.** `crates/popcircles/src/table/cache.rs`,
      declared as a submodule of `table`: a `serde`-derived header carrying a format version constant,
      the digest, the dimensions, the decimation factor and the byte order — the **host's**, per decision
      4, so `open` rejects a payload written by a host of the other order rather than reinterpreting it;
      a write path publishing atomically, payload to a temporary name then flushed, synced and renamed,
      then the header the same way, so the header is the commit record and no file is ever written into
      in place; and a read path that loads the payload with `std::fs::read` and views it through
      `bytemuck::try_cast_slice`. Doing the safe read first is what puts every rejection under test
      before any `unsafe` exists. `bytemuck` joins `[workspace.dependencies]` and the library's;
      `Cargo.lock` is committed in the same commit.
      *Verify:* nine tests, one per rejection ground — digest, width, height, decimation factor, format
      version, byte order, a payload truncated mid-element, a payload carrying trailing bytes, and a
      header that is not valid JSON — each asserting a **distinct** error variant rather than one
      catch-all, which is issue #3's second box and the difference between a refusal a caller can act on
      and one it cannot; a test writes a header whose dimensions disagree with the payload's length and
      asserts the mismatch is refused, so neither file is trusted to describe the other; a test asserts a
      payload published while a header from an earlier build is present is not readable as the earlier
      table — the ordering claim, checked rather than asserted in prose; a test interrupts publication
      after the payload rename and before the header's, and asserts the next `open` reports a missing
      cache rather than a corrupt one and the next build removes the orphan; a round-trip test asserts
      every rectangle sum over the reloaded table is bit-identical to the same query over the table as
      built; a test asserts the header's first serialised key is the format version, so the field a reader
      needs first is where they will look; `rg -n 'unsafe' crates/popcircles/src` returns nothing, which
      is what makes this task's greenness independent of the lint change; `cargo tree -p popcircles -e
      normal | rg 'memmap|libc'` returns nothing.

- [x] **2.2 The mmap read path, its lint exception, and the gate that bounds it.** One commit, because
      none of the three is green alone: `unsafe_code` in `Cargo.toml` goes from `forbid` to `deny` with a
      comment naming ADR 0003 as the reason and this hook as the replacement guarantee; `memmap2` joins
      `[workspace.dependencies]` and the library's; `table/cache.rs` gains one `unsafe` block on
      `Mmap::map` carrying a `// SAFETY:` comment that names the invariant decision 5 settles — 2.1's
      immutable publication, so that a rename replaces a directory entry rather than an inode and no
      writer this project contains can shrink a mapped payload, with the residual third-party truncation
      stated rather than dissolved. "We wrote it and opened it read-only" is not the invariant and the
      comment may not claim it is. The mapped type owns the `Mmap` and the cell count `open` validated
      and does not keep the `File`; it hands out `&[f64]`, and `Table<'a>` borrows that. Then one
      `#[allow(unsafe_code)]` on that item, and a `repo: local` prek hook beside
      `geo-data-lfs` asserts the count of `#[allow(unsafe_code)]` under `crates/` is exactly one and that
      it is in `crates/popcircles/src/table/cache.rs`.
      *Verify:* `rg -c --no-filename 'allow\(unsafe_code\)' -g '*.rs' crates` prints `1`, and
      `rg -l 'allow\(unsafe_code\)' -g '*.rs' crates` prints only that one path;
      `rg -n 'unsafe \{' crates` returns exactly one line, whose preceding line begins `// SAFETY:`; the
      hook fails when a second allow is added to any file under `crates/` and passes on the tree as
      committed — demonstrated by adding one, running `prek run --all-files` and reverting, because a
      tripwire nobody has seen fire is a tripwire nobody knows is wired; deleting the `#[allow]` makes
      `mise run lint:rust` fail, which is what proves it load-bearing rather than decorative; a test
      asserts the mmap read path and 2.1's safe one give identical sums over the same pair of files.

## Phase 3 — close-out

**Model: Opus 5 for 3.1, Sonnet 5 for 3.2.** Both edited documents in 3.1 are the instruction layer,
where saying more than the change owes is this repository's standing failure mode and no gate catches
it. 3.2 is a register entry and `gh` edits.

- [ ] **3.1 Documentation the step invalidated.** Three claims stop being true, and each is corrected
      to state the present rather than to narrate the change — `git log` and this record own what moved:
      - `docs/ai/application.md` line 50, "Steps 1 to 5 are targets rather than existing code", and the
        inventory that follows it. Step 1 is code; the inventory gains `table` and `progress`; "Step 1 is
        the trait's first caller" is no longer a forward-looking sentence.
      - `docs/ai/application.md` line 57, that geodesy, grid and raster are "pure computation with no
        I/O; the file, the decoder and the tag validation are `crates/popcircles/src/raster/geotiff.rs`,
        and nothing above it names either". Still true of the raster, and no longer the crate's only
        I/O — `table/cache.rs` belongs beside `raster/geotiff.rs` in that sentence.
      - `README.md` line 9, "Grid geometry, spherical geodesy and raster ingest are implemented and
        tested". The summation table joins the list.

      `docs/ai/platform.md` "Structure" needs **no** change: `crates/` is already a root there and that
      section carries roots rather than an inventory. ADR 0001's and 0002's own prose is not edited —
      ADR 0003's Status is where 0002's `forbid` consequence is answered.
      *Verify:* `rg -n 'Steps 1 to 5'` and `rg -n 'raster ingest are implemented'` both return nothing;
      `rg -n 'progress' docs/ai/application.md` names the module; `rg -n 'used to|now|no longer'
      docs/ai/application.md README.md` surfaces no before-and-after narration introduced by this task;
      `mise run lint:docs` green, which is the check that would catch a path named here and absent on
      disk; `prek run --all-files` green.

- [ ] **3.2 Close the plan.** `FU-06` in [`../follow-ups.md`](../follow-ups.md), in that file's format
      and meeting its bar: nothing couples a change of the cache header's shape to a bump of its format
      version, with a condition worded against this tree and a fix that notes it is `FU-03`'s shape for a
      second constant, so one hook can discharge both.

      Then issue #3's six "Done when" boxes and #11's `#3` line are ticked. The issue is **not** closed
      here — the PR's `Closes #3` does that, per `platform.md` "Git", and closing it by hand is the
      ambiguity that left #2 closed hours before the PR carrying its work existed.

      Then this plan's status line reads `**Status: complete (YYYY-MM-DD).**` and the Follow-ups section
      below holds `FU-06`.
      *Verify:* `rg -n 'FU-06' docs/follow-ups.md` matches an entry with all three fields;
      `gh issue view 3 --json body --jq .body | rg -c '^- \[x\]'` prints `6` and
      `gh issue view 11 --json body --jq .body | rg '#3 '` shows `- [x]`;
      `gh issue view 3 --json state --jq .state` reads `OPEN`; this file's status line reads complete and
      its Follow-ups section names no identifier that is not an entry in the register.

## Follow-ups

`FU-06` in [`../follow-ups.md`](../follow-ups.md).
