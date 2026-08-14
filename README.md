# PopulationCircles2026

[![CI][ci-badge]][ci] [![Rust 1.97+][rust-badge]][rust] [![License: MIT][license-badge]][license]

Finds the smallest circle on Earth containing a given share of the world's population, from a
~1 km resolution population raster — smallest measured on the globe, not on a projected map, which
is what makes the answer differ from the familiar viral versions. Also renders the results as maps.

**Status: early.** Grid geometry, spherical geodesy and raster ingest are implemented and tested; the
summation table, the kernels and the search itself are not.

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

`mise run cli -- --help` has the full command and flag reference.

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
[rust]: https://www.rust-lang.org/
[rust-badge]: https://img.shields.io/badge/rust-1.97%2B-orange.svg
[valeriepieris]: https://en.wikipedia.org/wiki/Valeriepieris_circle
