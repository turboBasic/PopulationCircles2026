---
tags: [adr, code, popcircles]
created: 2026-08-14
decided: 2026-08-14
supersedes: null
superseded_by: null
---

# ADR 0003 - The summation table is built compensated and read by mmap, at the cost of one unsafe

## Status

Accepted - 2026-08-14. It supersedes nothing.

It extends [ADR 0001](0001-cli-and-output-layer.md) along the axis that record opened. 0001 held the
library's dependencies at `serde` and argued that the serde the cache header needs "arrives in the
library on #3's schedule, not on this one"; this is that schedule, and it adds three crates 0001 did
not weigh.
It also declares the progress sink 0001 decision 4 fixed the shape of and deliberately left
undeclared — that record's plan put it Out of scope naming #3 as its first caller.

[ADR 0002](0002-no-system-gdal.md)'s Consequences record that `unsafe_code = "forbid"` holds across
the reader. Decision 5 below demotes that lint to `deny` for the whole workspace. The substance of
0002's claim survives — there is no `unsafe` in the reader, and none anywhere but the single site named
below — while the literal level does not. 0002 itself is not edited: an accepted record's Consequences
stay as written even where a later decision moves the ground under them, and this paragraph is where a
reader of 0002 is meant to find that out.

## Context

Issue #3 is the first step that writes a file, and the first whose output every later step reads
billions of times. Both halves of that force choices no later step can revisit cheaply: a cache
format is a compatibility surface, and the access mechanism decides whether the search is minutes or
hours.

The seam it consumes exists. `crates/popcircles/src/raster.rs:188` declares `RasterSource`, handing
out one row at a time with nodata already zeroed; `RasterRow.values` is `&[f32]` (line 179), and
`Synthetic` (line 242) is the in-memory implementation every test here is written against. Nothing
above `raster/geotiff.rs` names a path or a decoder, and this step is the first caller of any of it.

The scale is `data/README.md`'s registry row: 933 120 000 cells, 7 757 982 599.32 persons measured
with a compensated sum, largest cell 602 380.375. At 8 bytes a cell the table is ~7.5 GB, on
development machines with 16 GiB.

`Cargo.toml:35` sets `unsafe_code = "forbid"` for the workspace. `forbid` cannot be overridden by an
`#[allow]` — that is the difference between it and `deny` — so any mmap at all is a change to that
line, not a local exception to it. This was not apparent when the mmap requirement was written into
issue #3's Goal.

### What the upstream project did

[`ai/application.md`](../ai/application.md) "Provenance and the copying rule" permits consulting
`alexmijo/PopulationCircles` for facts. It was consulted on 2026-08-14 and no expression crossed over;
what follows is what it settles about the published maps' actual table, from a local checkout:

- `MediumStuff/summationTableMaker.cpp:212` builds the table with the four-term recurrence
  `data[y][x] += data[y][x-1] + data[y-1][x] - data[y-1][x-1]` (line 225), uncompensated, subtracting
  two numbers of order 1e9 at every cell.
- Its cache format (lines 256–257) is two host-endian `int`s and then the payload: no version, no
  checksum, no declared endianness.
- `rasterCircleFinder.cpp:978` reads it back with `new double[numCols]` per row — the whole 7.5 GB
  resident.
- The later rewrite in `SpringCleaning/src/SumTable.cpp:15` converged on a padded `(H+1)×(W+1)` table
  with a zero row and column, which removes the edge branches its predecessor needed. Its own query
  transposes two axes, so the convention is the useful part and the code is not.
- `rasterCircleFinder.cpp:265` handles a wrapped rectangle by splitting it in the caller, with a
  separate branch when a kernel row spans the full width (line 292), and a `TODO` at line 173 worrying
  about double-counting a column in that case.

So every box issue #3 asks for is a repair of something in that file, and the one convention worth
taking is the padding.

### Measurements

All three tables come from a scratch crate outside this tree (`rust 1.97.1`, 16 GiB, internal SSD),
on **synthetic** cells, not on the registry raster: ~19.5% populated, populated mean ~42.6 persons,
four orders of magnitude of spread, full f32 mantissas. At 933 120 000 cells that reaches a total of
7 752 675 585.72 persons, the same regime as the registry's 7 757 982 599.32. The reference is exact,
in `i128` units of 2⁻⁴⁰.

Construction, maximum absolute error over every one of the 933 120 000 cells, and over 20 000 random
rectangle queries whose widest spans 7.27e9 persons:

| construction | max cell error | max query error |
| --- | --- | --- |
| four-term recurrence (upstream's shape) | 1.0e-2 | 1.4e-2 |
| separable — row prefix, then column accumulation — uncompensated | 1.2e-4 | 1.2e-4 |
| separable, Neumaier compensation in both passes | 4.8e-7 | 1.9e-6 |

One ulp at that magnitude is 9.5e-7, so the third row is 0.5 ulp per cell and 2 ulp per query: the
cell figure is correctly rounded and not improvable in f64, and the query figure is the four-corner
subtraction. Building the whole grid cost 698 ms with the compensation against 102 ms without,
single-threaded, net of cell generation.

Access, 2 000 000 random four-corner queries over a 2 GiB payload:

| access | ns/query |
| --- | --- |
| mmap, page cache warm, `bytemuck` checked cast to `&[f64]` | 18.6 |
| mmap, page cache warm, hand-rolled `f64::from_le_bytes` per corner | 92.9 |
| `pread` per corner (`FileExt::read_at`), page cache warm | 2324 |
| mmap, cold mapping, first touch of each page | ~17 000 |

The last row is the one that is not about the mechanism: on a random pattern over a payload larger
than RAM the cost is the disk, and `pread` cold measures the same order. It is recorded here because
it makes the decimated table a working necessity rather than a convenience, and because it moves the
question of traversal order into #6 where it belongs.

Digest throughput over the registry raster's 408.6 MiB, and the dependency cost of each candidate,
counted with `cargo tree -e normal` excluding the probe crate:

| digest | throughput | crates added |
| --- | --- | --- |
| FNV-1a over u64 words, hand-rolled | 6.00 GiB/s | 0 |
| FNV-1a over f32 words, hand-rolled | 2.52 GiB/s | 0 |
| `blake3` | not measured | 5 |
| `sha2` | 0.34 GiB/s | 9 |

`memmap2` costs 2 crates including `libc`; `bytemuck` costs 1 and has no dependencies of its own.

One more measured fact decides where decimation lives. A 10 × 10 block of the registry's largest cell
summed in an `f32` accumulator **loses 29.5 persons** — f32's exact-integer ceiling is 2²⁴ = 16 777 216
and such a block reaches 6.0e7.

And one fact that bounds what a rejected cache costs, this one measured on the registry raster with
this repository's own reader rather than on synthetic data: `mise run test:raster` completes a full
decode-and-traverse pass over all 933 120 000 cells in **9.5 s** in release. With the compensated
prefix pass at 0.7 s and 7.5 GB written at the 1.5–1.8 GB/s this machine sustains, **a full-resolution
table is rebuilt from the raster in about 15 seconds.** Every refusal below is priced against that,
and none of them is expensive.

## Decision

**1. The table is two modules: `table.rs` computes and `table/cache.rs` does the I/O.** The split
`raster.rs` and `raster/geotiff.rs` already have, for the same reason: `application.md` "Architecture"
rules that the domain "does not read, write, print, or format", and a `Table::open(path)` convenience
on the pure type is how that stops being true. `table.rs` names no path, no file and nothing that serialises.

The query type borrows rather than owning: `Table<'a>` over a `&'a [f64]`, with no storage generic and
no trait. A `Vec<f64>` and a mapping are both a slice by the time the query sees one, so the generic
`Table<S: AsRef<[f64]>>` this record started from bought nothing and cost the thing that matters — an
`AsRef` implementation over a mapping cannot return a `Result`, so the checked cast either moves into an
accessor that can panic or is skipped. Borrowing puts the cast at `open`, once, where it can fail
properly.

**2. The table is built separably, with Neumaier compensation in both passes.** A row prefix along
each row, then a column accumulation carrying a per-column correction — never the four-term
recurrence, which subtracts two ~1e9 numbers per cell for no gain. The measured consequence is the
tolerance this repository now holds itself to, and it is a test's assertion rather than a hope: **every
cell within 1 ulp, and every rectangle query within 4 ulp, of the exact sum, at full-resolution
magnitudes.** `application.md` "Correctness invariants" already requires f64 throughout the table;
this fixes what f64 is required to achieve with it.

That is two claims and they are tested apart. On values whose partial sums are exact in f64 — small
integers, or dyadic rationals inside a bounded exponent range — the table agrees with a direct sum
**exactly**, and that is the test of the traversal: the indexing, the wrapping, the padding offset.
The ulp budget is about arithmetic and is tested at full magnitude, where no direct sum exists to
compare against and the reference is exact integer arithmetic instead. Asserting "equals brute force"
over unconstrained f32 values conflates the two and produces a test that fails for the wrong reason.

The build streams. One f64 accumulator row, one f64 correction row and the borrowed input row is
~900 KB at full width, each completed row written out as it is produced, so the table is never
resident during construction either.

**3. A table's identity is a digest of the decoded cells, computed in the build pass.** FNV-1a,
hand-rolled, folded into the traversal that is happening anyway. Not the file's checksum: the seam
hands out decoded rows rather than file bytes, so a file digest means opening the file a second time
beside the reader, and it answers the wrong question — what must not be silently reused is a table
built from different *data*, which is also the only form of the question a decimated or masked source
can answer at all. `data/README.md`'s SHA-256 stays what it is, provenance for the file.

Its definition is part of the ruling, because a digest whose input order or element width is left to
the implementation is not a digest — it is a number that happens to match today: **FNV-1a, 64-bit, with
the standard offset basis and prime, over the sanitised cells in row-major order, each cell's
`f32::to_bits()` widened to `u64`.** Sanitised is what makes those bits canonical. `sanitise_row` writes
`0.0` over every sentinel, negative and non-positive cell, so no `-0.0` and no NaN payload reaches the
digest, and two copies of a raster that differ only in which spelling of a sentinel they carry hash
alike — which is right, because they hold the same population. The dimensions and the decimation factor
sit beside the digest as header fields rather than inside it, so a mismatch in either is reported as
itself instead of as an unexplained digest failure.

**4. The cache is two files — a JSON header and a payload of nothing but f64 — published atomically and
never mutated in place.** The header carries a format version, the digest, the dimensions, the
decimation factor and the byte order, and `open` rejects a mismatch in any of them rather than reusing
the payload. The payload is the padded `(height+1) × (width+1)` table starting at offset 0, which is
what makes it page-aligned by construction, so the alignment half of the checked cast cannot fail. The
header being JSON rather than a packed struct is deliberate: a stale cache is something a person
debugs, and `serde` is already in the library.

**The payload is in the host's byte order, and the header records which.** `open` rejects a payload
whose recorded order is not the running host's. The obvious alternative — fix the format as
little-endian and swap on load — cannot be had at the same time as the mmap: swapping means
materialising up to 7.5 GB, which is the thing decision 5 exists to avoid, and `bytemuck` performs a
native-endian cast with no swap of its own. Declaring the format little-endian while casting natively
would be silently wrong on a big-endian host, which is what this clause replaces. A cache is a local
derived artefact and never travels, so the cost of native order is that a host which cannot read one
rebuilds it.

**Publication is: payload to a temporary name, flushed and synced, renamed into place; then the header,
the same way.** The header is the commit record, so a reader that finds a header finds a complete
payload behind it, and an interrupted build leaves at most an orphan temporary under a deterministic
name that the next build removes. Nothing is ever written into a file already in place. That ordering
is not tidiness: decision 5's soundness rests on it.

The version constant is exposed the way `SCHEMA_VERSION` is, and inherits the same gap: nothing
couples a change of the header's shape to a bump of its number. That is a follow-up register entry
rather than a claim this record makes.

**5. The payload is read by mmap, and `unsafe_code` goes from `forbid` to `deny` to allow it.**
`memmap2::Mmap::map` is an `unsafe fn` and cannot be otherwise — another process truncating the file
invalidates bytes already borrowed, which no signature prevents. It gets **one** `unsafe` block, in
`table/cache.rs`, with a `// SAFETY:` comment naming the invariant it rests on, and **one**
`#[allow(unsafe_code)]` on that item. The byte-to-f64 view is `bytemuck::try_cast_slice`, which is
safe and checked, so no second `unsafe` follows it.

**What makes the mapping sound is decision 4's publication rule, not the fact that this crate wrote the
file.** "We wrote it and opened it read-only" is not an invariant: it says nothing about what any
process does next. Immutable publication is the invariant. A rename replaces a directory entry rather
than an inode, so a mapping already established keeps the bytes it mapped even while a fresh build
publishes a replacement over the same path, and because no payload is ever written into a file already
in place, no writer this project contains can shrink or rewrite one that is mapped. The residual is a
third party truncating the cache directory by hand, and the comment states it rather than dissolving
it: mmap has no defence, the failure is a fault on access rather than a wrong number accepted as right,
and the payload is a rebuildable derived artefact.

Ownership follows from that. The mapped type owns the `Mmap` and the cell count `open` validated; it
does not keep the `File`, because a mapping outlives the descriptor that created it. It hands out
`&[f64]` borrowed from the mapping, and `Table<'a>` borrows that slice — so the lifetime of the query
is tied to the mapping by the compiler rather than by a rule someone has to remember.

Demoting the lint is the cost, and it is paid workspace-wide for a single site. What replaces the
guarantee `forbid` was making is a prek hook asserting that the count of `#[allow(unsafe_code)]` in the
tree is exactly one and that it is in that file — the property `forbid` bought, in the one form that
admits the exception. A gate, not an intention.

**6. Decimation is a fold in the builder, not a `RasterSource` adapter.** The adapter is the shape the
seam invites and the f32 measurement above forbids it: `RasterRow.values` is `&[f32]`, so a decimating
source would round every block sum before the table saw it. Blocks fold into the builder's f64
accumulators, the factor must divide both grid dimensions, and it is recorded in the header. k = 10
gives 5 arcmin, 2160 × 4320, 75 MB — small enough to stay resident, which is why it is the iteration
path and not only a fixture.

## Consequences

**Positive**

- The accuracy claim is a number a test can fail on, at the magnitude the real raster reaches, instead
  of a construction someone believes in. Two orders of magnitude better than the separable naive form
  and four better than the shape the published maps were computed with.
- The compensation is free at this scale: 0.6 s over the whole grid, against a build that also decodes
  408.6 MiB of LZW and writes 7.5 GB.
- Rectangle queries cost 18.6 ns warm, so the search's cost is its traversal order rather than its
  access mechanism — which is a problem #6 can reason about, unlike a per-corner syscall it cannot
  amortise.
- The cache refuses a stale payload on five independent grounds, and a person can read the header
  without a hex editor or a tool.
- The digest costs no extra pass and no crate, and keeps meaning something for a decimated or masked
  source, which is where #13 and #14 will need it.
- `table.rs` stays testable without a filesystem, so the property tests over wrapping and full-turn
  spans run on hand-built tables and on `Synthetic`, never on 428 MB of fetched raster.
- The decimated table makes every later step's iteration loop fit in RAM, which the cold-mapping
  measurement says is not a comfort but a requirement.

**Negative / costs**

- **`unsafe_code = "forbid"` is gone from this workspace, and it will not come back.** Every future
  crate and module inherits `deny`, so the protection is now a hook and a review habit rather than
  something the compiler makes impossible. The hook can be deleted in one line by anyone who finds it
  inconvenient; `forbid` could not be.
- One `unsafe` is one more than zero, and it is the kind whose failure mode is memory unsafety under a
  condition — a file truncated by another process — that no test in this repository will ever produce.
- Two new direct dependencies and one transitive — `memmap2` and `bytemuck`, with `libc` beneath the
  first — into a library whose direct list was three. Two of the three exist to serve a performance
  property measured on synthetic data on one machine.
- A JSON header beside a binary payload is two files to keep together, and nothing stops someone
  moving or copying one without the other. The failure is loud rather than silent, but it is a failure
  a single self-describing file would not have.
- Atomic publication needs a temporary file, so the cache directory transiently holds two payloads and
  the build needs ~15 GB free where the table itself is 7.5 GB. The `fsync` before each rename is a
  cost paid on every build for a durability property only a crash needs.
- The payload does not cross the little/big-endian boundary. Within one byte order it travels freely —
  every current target anyone would run this on is little-endian, so a cache built on x86_64 Linux
  loads on aarch64 macOS — but a host of the other order is told to rebuild rather than offered a
  conversion, and the mmap forecloses adding one later without a version bump. Cheap today at the
  measured 15 seconds, and the kind of cheap that stops being so the first time someone wants to
  publish a table rather than build one.
- `Table<'a>` borrowing its payload means every caller holds the mapping alive for as long as it holds
  the table, and the two-step `open` then `Table::new` is one more step than a constructor taking a
  path — which is the step the module split requires and callers will want to wrap.
- The format version has a constant and no coupling, exactly the gap `FU-03` records for the wire
  format. This record ships the gap knowingly and hands it to the register.
- The padded `(H+1)×(W+1)` payload spends ~520 KB at full resolution on a row and column of zeros, and
  the off-by-one between a padded payload index and a grid index is now a permanent hazard in the
  hottest function in the program.
- The tolerance is pinned against synthetic cells. The distribution was chosen to reach the registry's
  magnitude, and it is not the registry's distribution; the number that would settle it comes from
  #10's gated run, after this decision has already been built on.
- Decimation in the builder means a decimated table cannot be derived from a full one that already
  exists on disk — it is a second build from the raster. Deriving it from the payload would be four
  lookups per output cell and is not what this decides.

## Alternatives considered

- **`pread` per corner, keeping `forbid` intact.** The only option that needs no `unsafe` at all, and
  the one to beat. It lost on 2324 ns against 18.6: two orders of magnitude on the operation the search
  performs billions of times, to preserve a lint level rather than a property, since the property is
  recoverable by a hook.
- **Hold the whole table resident, as upstream does.** No mmap, no `unsafe`, no cache format worth
  arguing about. It lost on 7.5 GB against 16 GiB, which is also why issue #3's fourth box forbids it.
- **Quarantine the `unsafe` in a third crate** whose manifest sets `deny` while the workspace keeps
  `forbid`. Genuinely the tightest containment: a crate whose whole surface is "map this file, hand
  back a checked `&[f64]`". It lost because Cargo does not merge a crate's `[lints]` table with the
  workspace's — the new crate would restate the entire clippy block, creating exactly the drift site
  the workspace table exists to prevent — and because ADR 0001 set the bar for a crate boundary at a
  dependency that forces it, which a lint level is not.
- **A crate exposing a safe mmap wrapper**, so `forbid` holds untouched. Attractive and rejected on
  principle: routing around a lint by moving the `unsafe` into a less-reviewed dependency is worse
  than writing one block and gating its count, and no such crate was identified as well-maintained
  enough to carry a correctness-critical read path.
- **Hand-rolled `f64::from_le_bytes` at the four corners instead of `bytemuck`.** This was the
  recommendation until it was measured: no dependency, and endianness explicit at the point of use. It
  lost at 92.9 ns against 18.6 — five times the cost of the operation the whole `unsafe` was accepted
  to make fast, for one crate with no dependencies of its own.
- **The registry's SHA-256 as the cache's identity**, matching what `data/README.md` already records.
  It lost on all three counts: 9 crates, 0.34 GiB/s against 6.00, and a second read of the file beside
  the reader that is already streaming it — for an answer to "is this the file from SEDAC" when the
  question the cache asks is "was this table built from these cells". It survives as an optional
  provenance field, additive, whenever a caller has the value.
- **`blake3` as a middle path** — 5 crates, cryptographic, fast. It lost to FNV for the same reason
  `sha2` did: the cache is defending against a different raster, not against a forged one, and 0 crates
  beats 5 when nothing needs collision resistance.
- **The four-term recurrence, as upstream.** One accumulator instead of two rows, and the textbook
  statement of the technique. It lost at 1.0e-2 persons against 4.8e-7, from subtracting two large
  numbers at every cell — a cost with no compensating benefit once the separable form is written.
- **A decimating `RasterSource` adapter**, so the builder never learns about decimation. The cleaner
  seam, and the design this record started from. It lost to the f32 measurement: 29.5 persons per
  block of dense cells, silently, because the trait hands out `&[f32]` and population is additive
  faster than f32 can represent.
- **A fixed little-endian payload, swapped when the host disagrees.** The portable format, and what
  this record said before the interaction with `bytemuck` was checked: it casts natively and swaps
  nothing, so declaring the format little-endian while casting natively was silently wrong on a
  big-endian host. Two ways to make it right, and both lost. Swapping the payload **on load** means a
  7.5 GB copy, which is the mmap surrendered outright. Swapping **per corner** is the viable one and
  costs little to build — it is the `f64::from_le_bytes` accessor measured above, so a little-endian
  host would still pay nothing and only a big-endian host would pay, at 92.9 ns against 18.6. It lost
  on what the payment buys: moving a 7.5 GB table takes 60 s over gigabit ethernet and ~21 minutes over
  a 50 Mbps uplink, against the 15 s to rebuild it, and anyone able to run this tool already holds the
  raster because everything else needs it. So the artefact is cheaper to regenerate than to transfer,
  and the only case the byte order would gate is a fast local link between a little-endian machine and
  a mainframe.
- **A single self-describing file, header and payload together.** One file to move, one to delete, no
  way to separate the two. It lost to alignment: a header of any size that is not a multiple of the
  page size leaves the payload unaligned in the mapping, so it would need padding to a page boundary —
  a magic number in the format whose only job is to make a cast succeed, in exchange for a JSON
  sidecar a person can read with `cat`.
- **`Table<S: AsRef<[f64]>>`, generic over storage.** The shape this record's first draft carried, and
  the one that reads as obviously right. It lost because `AsRef::as_ref` returns a slice and not a
  `Result`: the checked cast over a mapping has nowhere to fail, so it either moves into an accessor
  that panics on a path no test can reach or is dropped entirely. Borrowing needs no generic and puts
  the fallible step in `open`, which is where a caller can act on it.
- **Widening `RasterRow.values` to `&[f64]`** so the adapter above becomes safe. It lost because it
  doubles the streaming cost of every consumer to serve one, and because the file's cells are f32:
  the widening buys precision only for sums, which is what the builder is for.
