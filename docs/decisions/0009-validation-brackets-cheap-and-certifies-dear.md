---
tags: [adr, code, popcircles]
created: 2026-08-15
decided: 2026-08-15
supersedes: null
superseded_by: null
---

# ADR 0009 - Validation brackets on a decimated table and certifies at full resolution, and a benchmark is a std-timed target whose figures live in a record

## Status

Accepted - 2026-08-15. It supersedes nothing.

It closes the question [ADR 0003](0003-summation-table-cache.md)'s plan deferred — "**Benchmarks.** #10's.
ADR 0003's figures come from a scratch crate outside this tree, and this plan commits no harness that
reproduces them" — by committing that harness, and it corrects one of that record's expectations with a
measurement rather than overturning a ruling, so 0003 is not edited.

It also answers the condition [ADR 0004](0004-diagnostics-through-log.md) named for reopening itself. That
record's plan lists "**Per-operation durations reported rather than derived, and benchmarks**" as belonging
to issue #10, and 0004 names benchmarks as one thing that might force a subscriber. It does not: decision 1
below measures outside the process's diagnostics, so nothing here asks `log` for a duration and that record
is left standing as written.

## Context

Issue #10 asks for four things, and the two that shape this record are a comparison against the published
prior art and a benchmark harness. Nothing in the tree is either: `rg -n criterion Cargo.toml` is empty,
`crates/popcircles/benches/` did not exist, and the only timings this repository has ever quoted are
`docs/decisions/0003-summation-table-cache.plan.md`'s, measured in a scratch crate on another tree and
labelled as borrowed.

Everything below was measured on one machine on 2026-08-15 — Apple M2 Pro, 10 cores, 16 GiB, internal SSD,
rust 1.97.1, the release profile this workspace declares (`codegen-units = 1`, thin LTO). The order of
magnitude and the ratio between two figures are what they are for; the third digit is this machine's.

### The full-resolution search is not compute, it is page faults

One fixed-radius search over the 30 arcsec table, at 3300 km and an initial spacing of 2048:

| Figure | Value |
| --- | --- |
| wall clock | 207.24 s |
| CPU inside it | 13.4 s, so **6.5% of the run** |
| disk during it | ~7 000 transfers/s, 115–135 MB/s, 15–19 KB per transfer |
| blocks examined / pruned | 3 934 / 3 010 (76.5%) |
| circles evaluated | 924 |
| kernels built | 460 |

A 3300 km circle spans 7 130 rows of that grid, so 924 circle evaluations are about 6.59 million
four-corner queries: **31 µs each**. ADR 0003's plan records 18.6 ns for the same query against a warm page
cache. The three orders of magnitude between those two numbers are the whole cost of the run, and they are
not arithmetic — a kernel walks rows in order and consecutive table rows are 345 KB apart, so one circle
evaluation touches roughly 111 MB of scattered pages in a 7.47 GB payload on a machine holding 16 GiB.

Pre-faulting the payload does not fix it. Reading all 7.47 GB sequentially takes 2.25 s at 3.2 GB/s, after
which `vm_stat` reports 4.5 GB of file-backed pages — the machine will not hold the table, and the search
that followed the warm-up ran *longer* than the cold one rather than shorter.

**This is what makes a benchmark of the resident case alone a description of a run nobody has.** The thread
on issue #10 says so about the borrowed figures, and the measurement above is the same statement against
this tree. `bench:circle` reports both, and its two figures bracket the search's own:

| Table | ns per four-corner query | ms per circle |
| --- | --- | --- |
| resident, 5 arcmin, 200 km | 48.6 | 0.002 |
| resident, 5 arcmin, 3 300 km | 16.5 | 0.011 |
| mapped, 30 arcsec, 200 km | 136 487 | 55.2 |
| mapped, 30 arcsec, 3 300 km | 100 062 | 646.7 |

The resident figure agrees with the 18.6 ns borrowed from the scratch crate. The mapped one is **three times
worse than the 31 µs the real search pays**, and the difference is the point rather than a discrepancy: this
benchmark samples centres evenly over the whole globe, so no two circles share pages, while a search
concentrates on the regions that survive pruning and re-touches what it has already faulted. So 100 µs is
the pessimistic bound and 31 µs is the realised cost, and both belong in a record — a benchmark that
reported only the second would be describing the search's luck rather than the query.

### The build's costs are the other way round from what ADR 0003 expected

The full-resolution build, from the registry raster through the CLI to a published 7 465 478 408-byte
payload:

| Figure | Value |
| --- | --- |
| wall clock | 17.76 s |
| user CPU | 14.34 s |
| system CPU | 1.54 s |
| peak resident set | 5.05 MiB |

`bench:table` then separates the three things inside that, by streaming a generated raster of the registry's
own mix of nodata, zero and counts — so the same code path, without the decoder:

| What runs | Seconds | Share of the CLI's 17.76 s |
| --- | --- | --- |
| the compensated build, rows discarded | 10.90 | 61% |
| the same, through the cache writer | 14.58 | the write is **3.68 s**, 21% |
| the CLI, over LZW strips | 17.76 | the decode is the remaining **~3.2 s**, 18% |

At 933 120 000 cells that is 85.6 million cells a second, and a 5 arcmin source runs at 70.0 million — the
smaller shape being slower per cell, which is the fixed cost of a row amortised over a shorter one.

ADR 0003's plan expected that "decode and the 7.5 GB write were not measured and are expected to dominate".
Measured, they do not: together they are 39% of the run and the compensated arithmetic is the other 61%.
The 5.05 MiB peak is the other half of that record's claim holding exactly — the build's memory is the
grid's width and not its area, for a table 1 400 times larger than the process that wrote it.

### Kernel construction is not the cost, and it is the only trigonometry

`bench:kernel`, over kernels evenly spaced across each shape:

| Shape | 200 km | 800 km | 3 300 km | 8 000 km |
| --- | --- | --- | --- | --- |
| 4320 × 2160 | 13.2 µs | 32.5 µs | 72.5 µs | 156.8 µs |
| 43200 × 21600 | 55.0 µs | 194.3 µs | 667.2 µs | 1 275.4 µs |

The rate is flat at 9 to 11 million kernel rows a second once a kernel spans enough rows to amortise its
own setup, which is what says the per-row half width is the whole cost and nothing else in there scales.

Against the search that motivates it: the full-resolution run at 3 360 km built 465 kernels, so it spent
about **0.31 s of a 247 s run** — a tenth of a percent — computing every geodesic distance it used. Step 2
of `application.md` "Approach" calls kernel construction the only step that computes geodesic distance, and
the figure that matters about it is how little of the run it is.

### Half the world, and where the published 3300 km went

Over the 5 arcmin table, a search for the smallest circle holding half the raster's own total settles in
0.86 s and 24 probed radii:

| Figure | Value |
| --- | --- |
| answer | **3 360 km** |
| centre | 28.7917 N, 100.6250 E — western Yunnan |
| population | 3 879 165 388.02 of a target of 3 878 991 299.66 |
| share achieved | 0.500022 |
| radius below | 3 359 km, holding 3 878 869 485.42 — short by 121 814 persons |
| predicate slack | 0.0120 persons |
| ambiguity | none reported |

The certification at full resolution is decision 3's, and it is three fixed-radius runs rather than a
search: 3 358, 3 359 and 3 360 km over the 30 arcsec table, at an initial spacing of 2 048.

| Radius | Population held | Margin against the target | Reaches half | Wall clock |
| --- | --- | --- | --- | --- |
| 3 358 km | 3 878 227 953.17 | −763 346.49 | no | 248 s |
| 3 359 km | 3 878 915 902.28 | −75 397.39 | no | 292 s |
| 3 360 km | 3 879 646 779.21 | **+655 479.55** | **yes** | 247 s |

**So the full-resolution answer is 3 360 km, and the decimated table had it exactly right.** The bracket is
certified where the search proves it — 3 360 reaches, 3 359 does not — and the grid term the list below
allows for turns out to be worth 0 km rather than the 1 km it was budgeted at. The centre moves 6 km, from
28.7917 N 100.6250 E to 28.8375 N 100.6625 E, which is under one 5 arcmin cell.

That is decision 3's whole justification, measured: the cheap grid can be trusted to bracket, because the
answer is an integer kilometre and a 9.3 km mesh does not move it. The margin at 3 359 km is 75 397 persons
against a summation slack of 0.20, six orders of magnitude, so no ambiguity is reported and none should be.

The comparison the issue asks for is against Danny Quah's ~3 300 km and the Valeriepieris circle before it,
and **the 60 km of divergence is explained rather than tuned away.** Four sources account for it, none of
them a defect:

- **The dataset is a different year.** This repository's raster is GPWv4.11 UN WPP-adjusted for **2020**
  (`data/README.md`), 7.757 9 billion persons. Quah's figure was computed on an earlier revision at an
  earlier year, and world population grew about 4% between 2015 and 2020. A circle sized to hold half of a
  larger, more urbanised distribution is not the same circle.
- **Half of *this* raster is not half of the world.** The target is `share × total` of the dataset's own
  cells, which is `application.md`'s "Population totals are dataset properties". A published figure quoting
  half of a different total is answering a slightly different question by construction.
- **The earth model.** Every distance here is a great-circle arc on a sphere of 6 371.0088 km
  (`crates/popcircles/src/geodesy.rs`), published in every document's `earth_model`. A prior result on
  another radius or on an ellipsoid is measuring in different units at the fourth digit.
- **The grid.** The answer is the maximum over cell centres, and a 5 arcmin cell is 9.3 km at the equator.
  The certification above is what removes this term, and it turned out to be worth 0 km: both grids answer
  3 360.

What matters for validation is that none of the four is a tolerance the search chose: the reported
`tolerance_persons` is 0.0, and the answer is a bracket the search proved — 3 360 km reaches the target and
3 359 km does not, both measured.

### The initial spacing's curve says something the register did not expect

Issue #10's thread proposes measuring `FU-08`'s spacing sweep here, on the ground that "32 on the k=10 shape
prunes 86.9% of blocks at a 200 km radius" is one point and not a curve, and that the analytic ceiling
`slack_km` documents sits "≈ 2 136 cells at a 200 km radius ... against a useful 32", two orders of
magnitude away.

Swept over the 5 arcmin table at four radii, the pruning *fraction* does fall with spacing, exactly as that
reasoning predicts — and the wall clock falls with it, monotonically, in every row:

| Spacing | 200 km | 800 km | 3 300 km | 8 000 km | Pruned at 3 300 km |
| --- | --- | --- | --- | --- | --- |
| 8 | 0.21 s | 0.18 s | 0.81 s | 2.77 s | 97.3% |
| 16 | 0.04 s | 0.07 s | 0.34 s | 0.88 s | 95.7% |
| 32 | 0.03 s | 0.04 s | 0.15 s | 0.31 s | 92.9% |
| 64 | 0.02 s | 0.03 s | 0.14 s | 0.15 s | 87.8% |
| 128 | 0.02 s | 0.03 s | 0.05 s | 0.09 s | 81.3% |
| 256 | 0.02 s | 0.02 s | 0.04 s | 0.08 s | 77.2% |

Past 256 it is flat: at 3 300 km, spacings of 512, 1 024, 2 048 and 4 319 examine 1 521, 1 495, 1 498 and
1 420 blocks and evaluate 370, 371, 374 and 379 circles. The plateau, not a knee, is where the curve ends.

**So the pruning fraction is the wrong figure to tune on, and a spacing chosen to maximise it is chosen to
be slow.** A coarse first level examines few blocks and prunes a smaller share of them; a fine one prunes
almost everything but pays for the tiling. What the search costs is circles evaluated, and that falls with
spacing until the top of the tree stops mattering. `FU-08`'s ceiling is therefore not "two orders of
magnitude away from the answer" — it is the neighbourhood of the answer, and the derivation that entry
wants is a clamp against it rather than a fitted constant inside it.

That entry stays open, and this record does not implement it: its Fix is a derivation in `search` beside the
level loop, which changes what every caller of `most_populous` gets and is its own change. What this record
supplies is the curve it said it needed.

## Decision

**1. A benchmark is a `harness = false` bench target in the library crate, timing with
`std::time::Instant`, and it adds no dependency.** Three targets, one per subject issue #10 names:
`benches/table_build.rs`, `benches/kernel.rs`, `benches/circle.rs`. Each is `test = false` so `cargo test`
never runs one, and `cargo clippy --all-targets` lints all three. Each has a mise task, and none is in
`lint`, `test` or `ci`: a benchmark asserts nothing, so there is no result for CI to be red about, and a
figure measured on a shared runner would be noise quoted to three digits.

criterion is what this refuses, and the reason is not weight. Its warm-up is definitional, and the figure
that describes a full-resolution run is the one a warm-up destroys — 31 µs against 18.6 ns. A harness that
cannot express the measurement is not the right harness however good its statistics are.

**2. A benchmark reports the mapped figure beside the resident one, or says out loud that it skipped it.**
`benches/circle.rs` measures a table it builds and then the cache under `out/` when a full-resolution table
is there, and prints a skip naming what would produce one when it is not. No benchmark in this repository
reports a resident figure alone.

**3. Validation brackets on a decimated table and certifies at full resolution.** The gated end-to-end test
is 5 arcmin, where a whole search is 0.86 s; the full-resolution answer is certified by fixed-radius runs at
the bracket's ends, and recorded in this record. A full-resolution search over radius is 24 probes at 207 s
each and would be a test nobody runs — and it would prove nothing the two ends do not, because a bracket is
what the answer already is.

**4. A measured figure is recorded in the record that measured it, and nowhere else in the instruction
layer.** No `docs/benchmarks.md`. A record is dated and frozen, so a figure in one is a historical claim
that cannot rot; the harness is how a later reader gets today's number instead. `README.md` may carry a
headline result with its date, under the licence `docs/ai-instructions.md` already grants the human layer.

**5. The accuracy note is `report`'s module documentation.** What a published number is accurate to is what
a consumer of the wire format needs to know, and `application.md` "Approach" already makes `report` the
owner of that. It composes the three sources rather than measuring a fourth: ADR 0003's 4 ulp per rectangle
query, issue #6's `tolerance_persons` of exactly zero, and `smallest`'s `predicate_slack_persons` over the
rows a circle spans. No second file states the composition.

## Consequences

**Positive**

- Issue #10's four boxes close against measurements rather than assertions, and the one figure that was
  borrowed from another tree — 18.6 ns — now has this tree's counterpart beside it.
- ADR 0003's two open expectations are answered: the write does not dominate, and the build's memory really
  is the grid's width.
- `FU-08` gets the curve it was waiting for, and gets it with its premise corrected. Whoever picks it up
  clamps against the ceiling instead of fitting a constant under it.
- The search's real bottleneck is now a recorded number rather than a suspicion. 6.5% CPU is an invitation
  to a different traversal order, and one nobody could have justified without this.
- Nothing new is installed. The benchmark story survives a dependency audit because there is no dependency.

**Negative / costs**

- `std::time::Instant` gives one sample. There is no variance, no outlier rejection and no saved baseline,
  so a 10% regression is invisible here and a 2× one is obvious. This repository has no performance gate,
  and after this it still has none.
- The figures in this record are one machine's, and a reader on another will find every absolute number
  wrong. Only the ratios travel, and nothing enforces that a later reader reads them that way.
- `benches/circle.rs` names a cache path and a digest as literals, so a table built anywhere else is
  invisible to it and reports as absent. That is the same trade `tests/registry_raster.rs` makes.
- The full-resolution certification is three radii, not a search. It certifies the bracket the 5 arcmin
  search found; it does not independently discover that bracket, and a defect that moved the answer by more
  than the band the gated test allows would need the 90-minute run to catch.
- `bench:table` writes 7.5 GB to measure what writing 7.5 GB costs. It is out of the `bench` aggregate for
  that reason, which means the aggregate does not cover every benchmark — a caller has to know to ask.

## Alternatives considered

- **criterion in `benches/`.** The conventional answer, and it was the first option put to the user. Lost on
  decision 1's reason: its warm-up cannot produce the non-resident figure, so the mapped measurement would
  have needed a second hand-rolled harness beside it, and then criterion is 30 crates carrying half the
  story.
- **A committed `docs/benchmarks.md` of current figures.** Lost to decision 4. It is a new root in the
  structure tree whose whole content is stale the moment anything changes, and nothing in `mise run ci`
  could tell that it had.
- **A full-resolution search over radius as the gated test.** Lost on 207 s × 24 probes. Kept as a recorded
  run rather than dropped: the ledger makes it resumable, so it remains available to anyone who wants the
  published document rather than the certified bracket.
- **Simulating the cold case by writing a table larger than RAM in the benchmark.** Genuinely arguable, and
  it would need no dataset. Lost because the real search already measures it on the real table, and a
  synthesised 20 GB file measures this machine's page cache rather than this program.
- **Tuning the answer toward 3 300 km** — by another earth radius, another year's raster, or a tolerance
  that lets 3 300 km reach half. Lost because issue #10 asks for the divergence to be explained rather than
  removed, and every knob that would close the gap is a claim about the dataset that the dataset does not
  make.
- **Closing `FU-08` here**, with the derived spacing landing beside the level loop. Lost on scope: it
  changes what every caller of `most_populous` receives, including the CLI's required `--spacing` flag whose
  no-default ruling lives in `SearchArgs`' own doc comment. The measurement is separable from the derivation
  and only the measurement needed this issue's data.
