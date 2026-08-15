---
tags: [adr, code, popcircles]
created: 2026-08-15
decided: 2026-08-15
supersedes: null
superseded_by: null
---

# ADR 0007 - A cached table and its ledger are keyed on the whole grid, through one attestation

## Status

Accepted - 2026-08-15. It supersedes nothing.

It extends [ADR 0003](0003-summation-table-cache.md) decision 3 into the case that ruling did not
reach. Decision 3 settled what sits beside the digest as against inside it, and named the dimensions and
the decimation factor as the fields that sit beside it; the origin and the two steps are neither inside
the digest nor beside it, and the record does not say why. This applies decision 3's own principle to
them rather than overturning it, so 0003 is not edited and its Decision stands as written.

It is the first record to rule on the radius ledger's document. That file arrived under
[ADR 0005](0005-ambiguous-minimality-is-reported.md)'s plan following the cache's shape by analogy —
`crates/popcircles/src/smallest/cache.rs:1` says so — and inherited the gap below along with the shape.

## Context

`Header` (`crates/popcircles/src/table/cache.rs:167`) binds `format_version`, `digest`, `width`,
`height`, `decimation` and `byte_order`. A `Grid` is six numbers (`grid.rs:108`), and the header binds
two of them. So four go unchecked: the origin's latitude and longitude, and the two steps. `FU-11`'s
sentence can be read as six unchecked; six is the count of `GridArgs`' flags (`popcircles-cli/src/main.rs:250`),
of which the header binds two.

The digest cannot cover them and is not meant to. Decision 3 defines it over the sanitised cells in
row-major order, folded into the build traversal, so two callers declaring different geometries over one
raster compute the same digest — the cells are the same bytes and no geometry enters them. It is also
itself a flag the caller copies from a build's output (`--digest`), which is what makes it an attestation
of the cells and nothing more.

`Identity` (`table/cache.rs:145`) is not where the gap is. It holds a `Decimation`, which holds the source
`Grid` and the coarser one, so a caller asking for a table already hands over the whole geometry; only the
header's field list and `Header::check` (`:191`) narrow it to three numbers. `FU-11`'s Fix asks for
geometry "in the header and in `Identity`", and half of that is already there.

### The ledger has the same key and a worse consequence

`Document` in `crates/popcircles/src/smallest/cache.rs:118` spells the same four fields — `format_version`,
`digest`, `width`, `height`, `decimation` — and `Document::check` (`:141`) compares the same three numbers.
It does not embed `Identity`, so it inherits nothing automatically from a fix to the header.

What it does with the fields it trusts is the part that makes this worse than a wrong sum. A probe is
stored as `km`, `population`, `row`, `col` (`:99`), and `open_or_empty` mints each pair back into cells of
the caller's declared grid (`:234`). A ledger filled over one geometry and resumed under another therefore
hands the search a maximum whose centre is re-interpreted: the population is the number a circle at one
coordinate contained, and the coordinate published beside it is a different place. The dimensions match, so
the mint succeeds and `CentreOffGrid` never fires.

Issue #45 records the ledger as out of scope on the ground that it "keys on the table's identity, so it
inherits whatever this decides". The code above is what makes that not so.

### Which of the four a caller can actually reach

Measured on this tree on 2026-08-15 with the debug CLI, because `Grid::new`'s two bounds (`grid.rs:149`
and `:157`) constrain some of the four and not others:

| declared over the registry raster's shape | result |
| --- | --- |
| `--origin-lat 0`, `FU-11`'s own example | refused: `grid rows reach -180, past the south pole` |
| `--origin-lat 89` | refused: `grid rows reach -91` |
| `--origin-lat 89.99999999` | refused: `grid rows reach -90.00000001` |
| `--origin-lon 0` | accepted, reported as `{"lat":90.0,"lon":0.0}` |
| `--lon-step` doubled | refused: `grid columns span 720 degrees, past a full turn` |
| `--lon-step` halved | accepted, `spans_full_turn: false` |

So `FU-11`'s illustration does not reproduce: a grid 21600 rows deep at 1/120° spans exactly 180°, which
pins its origin latitude to 90 within the boundary tolerance, and the constructor refuses the rest before
any cache is opened. Three of the four are reachable today — the origin's longitude, freely, and each step
downward — and the reachable case is not a typo but a half-turn shift of every column over identical
width, height and steps. The fourth becomes reachable with the first grid that does not run pole to pole,
which is #13's country work.

### What a comparison of those numbers has to allow

The raster reader already compares a file's geotransform against a declared grid, and does it within
`BOUNDARY_TOLERANCE_DEG` = 1e-9, longitude through `wrap_lon` (`raster/geotiff.rs:369` and `:386`). That
constant is `pub(crate)` with a comment (`grid.rs:17`) saying a second copy of it would be two answers to
one question, and it is scaled to a measured fact about the registry raster: its step is 1/120 + 5.4e-16
and its origin latitude 90 + 1.16e-11.

Exact equality is nonetheless available, which a comparison of floats read from JSON cannot be assumed to
have. Measured in a scratch crate outside this tree: `serde_json` round-trips 1/120 as
`0.008333333333333333` and 90 + 1.16e-11 as `90.0000000000116`, both bit-identical on the way back
(`0x3f81111111111111`).

### A bumped version is a syntax error, not a version refusal

The same probe measured what a v1 header meets when the reader has grown four fields. A v1 document
read into the widened `Header` fails with `missing field origin_lat` — `CacheError::HeaderSyntax`,
"the cache header at … is not the JSON document this format is", raised before `check` runs and therefore
before `format_version` is looked at. The version-first ordering inside `check` (`table/cache.rs:191`) is
real but unreachable: it orders comparisons within a document that already parsed.

The same document read into a struct carrying `format_version` alone parses, because serde ignores
unknown fields unless told otherwise and neither document sets `deny_unknown_fields`. Only the forward
direction works today — a v1 reader meeting a v2 header parses it and refuses on the version — and that is
the direction nobody is in.

### The cost of invalidating, and why now

A full-resolution table is rebuilt from the raster in about 15 seconds, which is ADR 0003's measurement and
is cited here as that record's rather than re-run. A ledger is the expensive half: each row is a search over
the globe at one radius, and there is no migration to offer, because the reason to refuse a v1 ledger is
precisely that nothing recorded the geometry its populations were measured over.

`gh release list` is empty on 2026-08-15, so every cache and ledger in existence is local to a machine that
holds the raster. ADR 0006 shipped the release workflow, which turns both `FORMAT_VERSION` constants into
promises across installs the first time a tag is published.

## Decision

**1. A cached table's identity is the whole grid geometry, and it sits beside the digest rather than inside
it.** The four numbers a `Grid` carries past its dimensions — the origin's latitude and longitude, and the
two steps — are recorded and compared, and a mismatch in any one of them is reported as itself. That is
decision 3's rule, applied to the fields it left out.

The geometry recorded is the table's own grid, not the source's. Given the coarser grid and the factor the
source's is determined — its dimensions are the product, its origin the same, its steps the quotient — so
recording both would be one fact twice, and the fact a query resolves coordinates against is the coarser
one.

**2. One attestation, serialised into both documents.** A single type in `table/cache.rs` carries the digest,
the dimensions, the factor and the geometry, is built from an `&Identity`, and holds the one comparison
against an `&Identity`, returning a mismatch enum whose variants are the eight grounds. `CacheError` and
`LedgerError` each carry that enum as one variant of their own rather than restating the grounds, so a
refusal still names which of them fired and there is one place a ground is added. Both documents embed the
attestation with `#[serde(flatten)]`, so each stays the flat object a person can read with `cat` and neither
grows a nesting level.

Two fields stay out of it. `format_version` is per document and per constant — the table's header and the
ledger are separately versioned and this record bumps both, which is not the same as merging them. And
`byte_order` is the header's alone: it describes a payload of raw f64, and a ledger's numbers are JSON text
with no order to disagree about.

The duplication this replaces is not hypothetical. The two `check` bodies are already copies of one another,
and the copy is how a hole opened in one reached the other; the next field a table's identity needs — a
country mask, at #13 — would otherwise be added twice again. `Identity` itself is not what gets serialised:
it holds `Grid`s whose fields are private behind a fallible constructor, so deriving `Deserialize` on it
would mint grids that `Grid::new` never saw.

**3. The geometry is compared per field within `BOUNDARY_TOLERANCE_DEG`, longitude through `wrap_lon`** —
the reader's own rule, from the reader's own constant, with no second copy of either. A tolerance rather
than the bit equality that is demonstrably available: the reader accepts a file whose origin is
90 + 1.16e-11 against a declared 90, so an exact comparison here would refuse a cache built over a raster
the reader had accepted, which is two answers to one question. And it costs nothing against the failure this
record exists to catch, which is a half-turn of longitude, six orders of magnitude above the tolerance and
five above the finest cell this program has.

**4. A document's version is read before the document.** Both readers parse a struct carrying
`format_version` alone, compare it, and only then parse the full document. Without that, this record's
own bump reports as "not the JSON document this format is", which is the failure issue #45's third box
forbids — and every later shape change reports the same way, so the constant would be decoration. The probe
depends on serde ignoring unknown fields, which is a default rather than a declaration, so it is stated at
the probe and pinned by a test of its own: the property the mechanism rests on is named where it can fail
rather than left for a reader to know.

**5. Both format versions go to 2, and every cache and ledger in existence is refused.** Not migrated: a v1
document's fields are true and its silence is what is wrong with it. The refusal names the version, per
decision 4. The `version-bumps` hook already couples the header's fields and the ledger's document to their
constants (`FU-03`, `FU-06`), so the bump cannot be dropped from the commit that moves the fields.

## Consequences

**Positive**

- A cache answers "is this the table over this grid" rather than three of that question's numbers. The
  failure it closes is the kind this repository treats as worst: a plausible coordinate rather than an error.
- The ledger is closed with it, so a resumed run cannot publish a centre whose population was measured
  somewhere else.
- One attestation and one comparison, so #13's mask is added once and both documents inherit it.
- `FORMAT_VERSION` becomes a gate for every future shape change rather than for the ones that happen to
  parse — a property both constants were written to have and neither had.
- `report.rs`'s provenance stops publishing a documented gap: the grid it names is attested by the cache
  that answered, in the same sense the digest and the factor already were.
- The invalidation lands while no release exists, so the caches destroyed are ones whose owners hold the
  raster and can rebuild in about 15 seconds.

**Negative / costs**

- **Every ledger anyone holds is refused, and a ledger is not cheap.** Each row cost a search over the globe
  at one radius, the file cannot be migrated for the reason it is refused, and at full resolution a rerun is
  hours rather than the table's 15 seconds. Anyone mid-search across the change pays for it twice.
- Two format constants move in one commit, and a reader of either file now parses the same bytes twice. The
  cost is nothing at these sizes, and the leniency the first parse rests on is serde's default rather than
  anything declared — but not silently so: `deny_unknown_fields` on a probe would fail every test that opens
  a cache, because a probe reads a real document and every real document carries keys the probe does not
  declare. Measured on 2026-08-15, that attribute compiles beside `flatten` and refuses an unknown key, so
  the way it goes wrong is loud rather than unavailable.
- `Header` loses its derived `Eq` — f64 fields do not have one — so nothing compares two headers by
  equality any more, and the test module's `float_cmp` exemption now covers a comparison that matters
  rather than only a round trip.
- The four per-ground variants each error enum spells today collapse into one wrapping the shared enum, so a
  caller that wants to tell a digest mismatch from a moved payload matches one level deeper than it does now.
  What it does not lose is the ability: the mismatch enum is public and its variants are the grounds, so
  rebuilding on a digest miss while refusing a geometry miss is one nested pattern rather than a distinction
  this forecloses. The cost that is real is the wording — each document's refusal used to name itself in the
  variant's own message, and the noun now comes from the wrapper while the ground says what differed, so a
  message that reads well in both documents is a constraint on how the grounds are phrased.
- **Two grids within 1e-9° are one table.** That is six orders below the finest cell here, so no coordinate
  this program resolves can land in the gap, but it is a tolerance where the dimensions get an exact
  comparison, and nothing checks that a future grid's cell stays above it.
- `#[serde(flatten)]` buffers both documents through serde's intermediate representation on the way in, and
  forecloses `deny_unknown_fields` on either — which is the same default decision 4's probe rests on, now
  load-bearing in two places for opposite reasons.
- The attestation is a type in `table/cache.rs` that the ledger imports, so a module about a summation
  table's I/O now owns a shape the search over radius depends on. The alternative was a third module for one
  struct.
- This grows issue #45 past the scope its Notes state, so the issue's scope moves before the work lands
  rather than after.

## Alternatives considered

- **Fold the geometry into the digest**, which is one reading of `FU-11`'s Fix. One field, no header growth,
  no comparison to write. It lost to what it would cost the digest: decision 3 defines it over decoded cells
  so that it keeps meaning something for a decimated or masked source, and a geometry mixed into it would
  report a half-turn shift as an unexplained digest failure — the outcome decision 3 named and rejected.
- **A geometry digest as a seventh header field**, hashing the four numbers into one `u64`. Smaller than four
  fields and it keeps the cells' digest intact. It lost on both halves of decision 3's principle at once: it
  reports a 90-degree origin error and a 1e-11 rounding identically, and hashing bits forecloses the
  tolerance the reader already grants.
- **Exact bit equality on the four numbers**, which the measured JSON round trip makes available. It lost
  because the raster reader compares the same numbers within 1e-9: an exact rule here would refuse a cache
  for a spelling of the origin that the reader accepted the raster for, and catch nothing the tolerance
  misses.
- **Leave the ledger out of scope**, as issue #45 says, and close `FU-11` on the header alone. The smaller
  PR, and the one the issue asked for. It lost to `smallest/cache.rs:118` and `:234`: the ledger spells the
  three numbers itself rather than embedding `Identity`, so it inherits nothing, and it is the file that
  turns a stale key into a published coordinate.
- **Four more fields in each of the two structs, with the two `check` bodies kept separate.** The smallest
  diff, no shared type, no `flatten`. It lost to the history of the two bodies: they are already copies, the
  copy propagated this defect once, and it would put the tolerance rule in two places in a repository whose
  geodesy rule is that a second copy is a defect.
- **`#[serde(default)]` on the new fields**, so a v1 header still parses and no probe is needed. It lost
  outright rather than narrowly: an absent origin defaults to 0.0, which the header then asserts as a
  geometry, so a v1 cache would be compared against a grid nobody declared — and a document's fate would
  depend on which field it happened to lack.
- **Key the cache path on the geometry**, deriving a directory name from the six numbers so a differently
  declared grid misses rather than collides. No format change, no invalidation, and it would have prevented
  the collision before either file existed. It lost because this program does not own where a cache lives:
  `--cache` defaults to `out/table` and `--ledger` is a path a caller names outright, so the convention is
  one a caller can decline while the failure stays silent — and a path cannot report which number differed.
