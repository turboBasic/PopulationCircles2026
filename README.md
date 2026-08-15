# PopulationCircles2026

[![CI][ci-badge]][ci] [![Rust 1.97+][rust-badge]][rust] [![License: MIT][license-badge]][license]

Finds the smallest circle on Earth containing a given share of the world's population, from a
~1 km resolution population raster — smallest measured on the globe, not on a projected map, which
is what makes the answer differ from the familiar viral versions. Also renders the results as maps.

**Status: early.** Grid geometry, spherical geodesy, raster ingest, the summation table with its
on-disk cache, the circular kernels a circle is measured through, the population inside one such
circle, the most populous circle of a fixed radius, and the smallest circle holding a given share of a
population — resumable across runs — are implemented and tested, with a command line over that search
and maps rendered from what it publishes. The answer for half the world has been run against the real
raster at full resolution and checked against the published prior art ([Validation](#validation)). What
is left is the same question restricted to a single country.

## Getting started

```sh
mise run setup      # toolchains, dependencies, git hooks
mise run ci         # lint, typecheck, test — the same checks CI runs
```

Input datasets live in [`data/`](data/README.md) and their contents are Git LFS objects, kept out
of a normal clone so cloning stays fast and cheap:

```sh
GIT_LFS_SKIP_SMUDGE=1 git clone …   # clone without downloading the rasters
mise run data:pull                  # fetch them when you actually need them
mise run data:status                # what is present locally versus pointer-only
```

Skipping is a layered default rather than a guarantee, and the layers are worth knowing before a
clone surprises you: [`data/README.md`](data/README.md#fetching).

## Usage

Great-circle distance, Wiesbaden to Rome:

```sh
$ mise run cli -- distance 50.0782 8.2398 41.9028 12.4964
{"schema_version":1,"tool":"popcircles","result":{"great_circle_km":966.3013398709427, …}}
```

Grid geometry for the GPWv4.11 raster (see [`data/README.md`](data/README.md)), without reading
the file itself:

```sh
$ mise run cli -- grid describe --width 43200 --height 21600 --origin-lat 90 --origin-lon -180 \
    --lon-step 0.0083333333333333 --lat-step -0.0083333333333333
{"schema_version":1,"tool":"popcircles","result":{"middle_row_cell_area_km2":0.8586351267048046, …}}
```

A summation table over that raster, decimated to 5 arcmin so the cache stays small, and then
population queries answered from it by mmap — the payload is never resident. The `query` function is
this file's doing rather than the CLI's, because the same grid, table and digest go to every query and
only the window changes:

```sh
$ mise run cli -- table build --raster data/population/gpw-v4-11-unwpp-adjusted-count-2020-30arcsec.tif \
    --width 43200 --height 21600 --origin-lat 90 --origin-lon -180 \
    --lon-step 0.0083333333333333 --lat-step -0.0083333333333333 \
    --nodata -3.40282306073709653e38 --epsg 4326 --decimate 10 --cache out/gpw-5arcmin
{"schema_version":1, …,"result":{"digest":"0xf17aa802a6890f0c","total_population":7757982599.323671, …}}

$ query() { mise run cli -- table query --width 43200 --height 21600 --origin-lat 90 \
    --origin-lon -180 --lon-step 0.0083333333333333 --lat-step -0.0083333333333333 \
    --decimate 10 --cache out/gpw-5arcmin --digest 0xf17aa802a6890f0c "$@"; }

$ query                                                            # every cell there is
{…,"rows":{"north":0,"south":2159},"columns":{"west":0,"east":4319,"full_turn":true},"population":7757982599.323671}}

$ query --north 55.06 --south 47.27 --west 5.87 --east 15.04       # the box around Germany
{…,"rows":{"north":419,"south":512},"columns":{"west":2230,"east":2340,"full_turn":false},"population":103837251.96947795}}

$ query --north 52.38 --south 44.39 --west 22.14 --east 40.23      # the box around Ukraine
{…,"rows":{"north":451,"south":547},"columns":{"west":2425,"east":2642,"full_turn":false},"population":75109401.82679437}}

$ query --north -12 --south -21 --west 176 --east -178             # Fiji, across the antimeridian
{…,"rows":{"north":1224,"south":1332},"columns":{"west":4272,"east":24,"full_turn":false},"population":919250.7823575613}}
```

The digest names the cells a table was built from, so it is what a query passes back to say which
table it wants — and a cache of any other table is refused rather than reused. The Fiji window wraps
the antimeridian, which needs nothing said about it because `west` is above `east`; the whole extent,
though, is what a query with no window covers, because −180 and 180 reduce to the same column and so
no pair of coordinates can mean the globe.

**A window is a box, not a country.** The middle two are the bounding boxes of Germany and Ukraine,
and each holds a good deal more than the country it is named for: the German box takes in the
Netherlands, Belgium, Czechia, Austria and Switzerland whole, and parts of Poland, France and Denmark.
Counting a country needs a mask over the grid, which is a later step — and treating a box, or a
circle, as a country is the spec error
[the application doc](docs/ai/application.md#what-this-program-does) names.

Both cache files land under `out/`, which is gitignored because a generated table is never committed.
Building needs the raster, so `mise run data:pull` first.

### Circles

The four search commands read the same table, so they take the same flags — collected into a shell
function again, and the outputs below are from one run against the 5 arcmin table above:

```sh
$ search() { local command="$1"; shift; mise run cli -- "$command" --width 43200 --height 21600 \
    --origin-lat 90 --origin-lon -180 --lon-step 0.0083333333333333 --lat-step -0.0083333333333333 \
    --decimate 10 --cache out/gpw-5arcmin --digest 0xf17aa802a6890f0c "$@"; }
```

The population inside a circle you name. A thousand kilometres around Dhaka is a tenth of everyone:

```sh
$ search population-at --lat 23.8103 --lon 90.4125 --radius-km 1000
{…,"result":{"requested":{"lat":23.8103,…},"centre":{"lat":23.791666666666927,"lon":90.37499999999898},
 "radius_km":1000.0,"population":769799773.1688497,"share_of_total":0.0992267981157833}}
```

The centre is not the coordinate asked for: it is the centre of the cell containing it, and both are
published because they are different questions.

The most populous circle of that radius, found by branch and bound over every cell centre — a good deal
further west, and holding 16% of the world rather than 10%:

```sh
$ search most-populous --radius-km 1000 --spacing 32
{…,"result":{"centre":{"lat":25.125000000000256,"lon":79.70833333333235},"radius_km":1000.0,
 "population":1254363867.9300776,"share_of_total":0.1616868627727305,"tolerance_persons":0.0,
 "stats":{"levels":6,"blocks_examined":13908,"blocks_pruned":12725,…}}}
```

`--spacing` is required and has no default: it changes how long the search takes and not what it answers,
and the useful value is a property of the raster and the radius that nothing here has measured.
`blocks_pruned` against `blocks_examined` is how you tell the bound is biting — 12 725 of 13 908 here.

And the question the program is named for. The smallest circle holding half the world's population:

```sh
$ search smallest-for-share --share 50 --spacing 32 --ledger out/radii.json
{…,"result":{"ledger":{"path":"out/radii.json","radii":24},"circle":{"radius_km":3360,
 "centre":{"lat":28.791666666666906,"lon":100.625},"population":3879165388.019252,
 "target":{"share":0.5,"persons":3878991299.6618357,"total":7757982599.323671},
 "short_below":{"radius_km":3359,"population":3878869485.4163485},"covers_whole_grid":false,
 "predicate_slack_persons":0.01196060136531932,…}}}
```

3360 km, centred in western China. `short_below` is the other end of the bracket the search proved: that
radius was measured too and falls short of the target, so minimality is readable off the document rather
than taken on trust. The share is given in **whole percent**, because a fraction stepped in f64 publishes
`0.30000000000000004` as a third share and a renderer then labels a chart with it.

This answer is separated by 174 088 people against a `predicate_slack_persons` of 0.012, so it carries no
`ambiguity` block. A share whose target sits on a flat stretch — anything near everyone, or a plateau of
ocean — grows one, naming the probed radii the arithmetic cannot tell apart, and the run says so once on
stderr as well.

Every radius tried goes in the ledger, so a sweep of several shares pays for each radius once. Here the
50% record costs no search at all, because the run above already settled its radii:

```sh
$ search sweep --from 10 --to 50 --step 20 --spacing 32 --ledger out/radii.json
{…,"result":{"ledger":{"path":"out/radii.json","radii":43},"shares":{"from_percent":10,"to_percent":50,
 "step_percent":20},"records":[
  {"radius_km":702,…,"target":{"share":0.1,…},"stats":{"radii_evaluated":9,"radii_reused":11,…}},
  {"radius_km":2129,…,"target":{"share":0.3,…},"stats":{"radii_evaluated":10,"radii_reused":14,…}},
  {"radius_km":3360,…,"target":{"share":0.5,…},"stats":{"radii_evaluated":0,"radii_reused":24,…}}]}}
```

`records` ascend by requested share, which is part of the format rather than an accident of iteration.
A ledger describing another table is refused rather than resumed from, which is why there is no way to
turn it off.

**These figures are a decimated table's.** The 5 arcmin grid is a tenth of the raster's resolution in
each direction, so a radius here is good to about the width of one of its cells — which for half the
world turns out to be no error at all. [Validation](#validation) is where that was checked at full
resolution.

### Maps

Rendering opens one file — a document one of the commands above wrote — and nothing else. Not the
raster, not the table, not the ledger:

```sh
search most-populous --radius-km 1000 --spacing 32 > out/most-populous.json
mise run render -- --input out/most-populous.json --output out/most-populous.png
mise run render -- --input out/most-populous.json --output out/globe.png --projection orthographic
```

`--projection` takes `plate-carree`, which is what the viral maps used, or `orthographic` centred on the
circle. Every figure carries the CC BY citation the raster's licence requires, and a test fails if that
wording drifts from [`data/README.md`](data/README.md#licence-and-attribution).

**The circle is a spherical cap projected by PROJ, not a ring of coordinates.** So one crossing the
antimeridian comes out in two pieces at either edge of the map, and one covering a pole closes across the
top of it — where drawing the ring directly fills the complement in the first case and the wrong
hemisphere in the second. Both of those look like maps, which is why they are tested rather than eyeballed.

`--no-coastlines` is the only form that needs no network: coastlines are fetched from Natural Earth on
first use and cached under `~/.local/share/cartopy/`. `mise run test:render` is the one test that draws a
complete figure, kept out of CI for that reason.

### Watching a run

Every command takes `--log-level`, the only control over what a run says about itself: `error`, `warn`,
`info` or `debug`, and `info` unless you say otherwise. At `info` a run names the table it resolved and the
answer it reached; at `debug` each expensive step is bracketed by a begin and an end record carrying one
operation name, so its duration is the difference between the two elapsed figures on the left. `RUST_LOG`
does nothing here — the flag is the only way in.

The progress meter is a **second mechanism, and the flag does not govern it.** A log says what happened;
the meter says how far a run has got. So `--log-level error` still draws a meter, and a `debug` run piped
to a file draws none — the meter is silent when stderr is not a terminal. Both write to stderr, and
stdout stays exactly one JSON document at every level.

`mise run cli -- --help` has the full command and flag reference.

## Validation

Half the world's population — half of this raster's own 7 757 982 599 persons — is held by a circle of
**3 360 km** centred at 28.84 N, 100.66 E, in western Yunnan. Measured 2026-08-15 on the full 30
arc-second grid, and it is a bracket rather than an estimate: 3 360 km reaches half by 655 480 persons and
3 359 km falls 75 397 short. Both were computed, and the summation slack between them is 0.2 of a person.

The published prior art is Danny Quah's ~3 300 km, and the [Valeriepieris circle][valeriepieris] before
it. **The 60 km is explained rather than tuned away**, by four things and no defect:

- the raster is 2020 and the published figures are earlier, over a world 4% smaller;
- half of *this* dataset is not half of the world, and the target is always the dataset's own total;
- distances here are great-circle arcs on a sphere of 6 371.0088 km, published in every document's
  `earth_model`;
- the answer is the best cell centre, not the best point — and at 30 arc-seconds that mesh is 926 m.

[ADR 0009](docs/decisions/0009-validation-brackets-cheap-and-certifies-dear.md) holds the measurements,
including what the search actually spends its time on: at full resolution it is **6.5% CPU** and the rest
is page faults against a 7.5 GB table. `mise run test:validate` is the end-to-end run against the real
raster, and it skips with a message when the raster has not been fetched.

## Releases

A [release][releases] attaches two binaries, each named by its target triple with a `.sha256` beside it:

| Asset | For |
| --- | --- |
| `popcircles-aarch64-apple-darwin` | macOS on Apple silicon |
| `popcircles-x86_64-unknown-linux-gnu` | Linux on x86-64 |

Neither carries a Developer ID signature and neither is notarized, which costs you something only on
macOS and only depending on how you fetched it. **A browser download** is tagged
`com.apple.quarantine`, and Gatekeeper then kills the binary with "Apple could not verify … is free of
malware" — offering to move it to the Bin, which you do not want. Clearing the attribute is the whole of
the fix:

```sh
xattr -d com.apple.quarantine popcircles-aarch64-apple-darwin
```

**A download with `curl`, `wget` or `gh release download` is not tagged** and runs as it is. Nothing here
depends on Gatekeeper being disabled: the macOS binary is ad-hoc signed, which is what the linker does by
default and what lets it run on Apple silicon at all.

**What a release promises is the wire format, and only that.** The `schema_version` the JSON documents
carry is a contract across releases, so a renderer or any other consumer may rely on it. The summation
table cache and the ledger are internal by contrast, and **any release may invalidate either**: one it
did not write is refused and rebuilt rather than migrated, which at full resolution costs a pass over
the raster rather than a download.

## Data

The population raster is [GPWv4.11 UN WPP-adjusted population count for 2020][gpw] at 30
arc-second resolution — CIESIN / Columbia University, distributed by NASA SEDAC, DOI
[10.7927/H4PN93PB][gpw-doi], [CC BY 4.0][cc-by]. Attribution is required of anything published from
it; [`data/README.md`](data/README.md#provenance) holds the citation, the grid details, and what
about this copy is still unverified.

## Layout

| Path | What it is |
| --- | --- |
| `crates/popcircles/` | Rust library — the search |
| `crates/popcircles-cli/` | The `popcircles` binary — a client of the library |
| `data/` | Input datasets in Git LFS, with a registry in [`data/README.md`](data/README.md) |
| `pyproject.toml` | Python tooling for data prep and map rendering (no package yet) |
| `typings/` | Type stubs for libraries shipping no `py.typed`, so pyright stays strict |
| `docs/ai-instructions.md` | The instruction router: project invariants, and what to read for a task |
| `docs/ai/` | Per-task conventions: [platform](docs/ai/platform.md), [code](docs/ai/code.md), [application](docs/ai/application.md) |
| `docs/decisions/` | Architecture decision records and their implementation plans |
| `docs/follow-ups.md` | The register of pending obligations |

Those documents are the conventions for humans as much as for AI tools. They are split across files so
each subject can be corrected and reviewed on its own, not so that any of them is optional — all of
them apply to every change. [`CONTRIBUTING.md`](CONTRIBUTING.md) covers setup and the task loop.

## Inspiration

The problem, and the demonstration that it is tractable at full raster resolution, come from
[alexmijo/PopulationCircles] — a C++ project that produced the published maps of these circles.
This is an independent implementation: the approach is reused, the code is not. That repository
carries no licence, so none of its source is copied or ported here;
see [the application doc](docs/ai/application.md#provenance-and-the-copying-rule).

Prior art on the 50% circle specifically: the [Valeriepieris circle][valeriepieris].

## License

[MIT](LICENSE).

<!-- Link references: badges at the top of this file. -->

[alexmijo/PopulationCircles]: https://github.com/alexmijo/PopulationCircles
[cc-by]: https://creativecommons.org/licenses/by/4.0/
[ci]: https://github.com/turboBasic/PopulationCircles2026/actions/workflows/ci.yml?query=branch%3Amain
[ci-badge]: https://github.com/turboBasic/PopulationCircles2026/actions/workflows/ci.yml/badge.svg?branch=main
[gpw]: https://sedac.ciesin.columbia.edu/data/set/gpw-v4-population-count-adjusted-to-2015-unwpp-country-totals-rev11
[gpw-doi]: https://doi.org/10.7927/H4PN93PB
[license]: LICENSE
[license-badge]: https://img.shields.io/github/license/turboBasic/PopulationCircles2026
[releases]: https://github.com/turboBasic/PopulationCircles2026/releases
[rust]: https://www.rust-lang.org/
[rust-badge]: https://img.shields.io/badge/rust-1.97%2B-orange.svg
[valeriepieris]: https://en.wikipedia.org/wiki/Valeriepieris_circle
