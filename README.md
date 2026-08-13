# PopulationCircles2026

[![CI][ci-badge]][ci] [![Rust 1.97+][rust-badge]][rust] [![License: MIT][license-badge]][license]

Finds the smallest circle on Earth containing a given share of the world's population, from a
~1 km resolution population raster — smallest measured on the globe, not on a projected map, which
is what makes the answer differ from the familiar viral versions. Also renders the results as maps.

**Status: scaffolding only.** Tooling, CI, and conventions are in place; no algorithm is
implemented yet.

## Getting started

```sh
mise run setup      # toolchains, dependencies, git hooks
mise run ci         # lint, typecheck, test — the same checks CI runs
```

Population rasters are Git LFS objects that a clone deliberately does **not** fetch, so cloning
stays fast and cheap:

```sh
mise run data:pull      # fetch the rasters when you actually need them
mise run data:status    # what is present locally versus pointer-only
```

## Layout

| Path | What it is |
| --- | --- |
| `crates/popcircles/` | Rust binary — the search |
| `pyproject.toml` | Python tooling for data prep and map rendering (no package yet) |
| `docs/ai-instructions.md` | Platform conventions, the source of truth for AI tools |
| `docs/ai-instructions-application.md` | This application: the problem, the approach, the constraints |

[`CONTRIBUTING.md`](CONTRIBUTING.md) covers setup and the task loop.

## Inspiration

The problem, and the demonstration that it is tractable at full raster resolution, come from
[alexmijo/PopulationCircles](https://github.com/alexmijo/PopulationCircles) — a C++ project that
produced the published maps of these circles. This is an independent implementation: the approach
is reused, the code is not. That repository carries no licence, so none of its source is copied or
ported here; see [the application doc](docs/ai-instructions-application.md#provenance-and-the-copying-rule).

Prior art on the 50% circle specifically: the [Valeriepieris circle][valeriepieris].

## License

[MIT](LICENSE).

<!-- Link references: badges at the top of this file. -->

[ci]: https://github.com/turboBasic/PopulationCircles2026/actions/workflows/ci.yml?query=branch%3Amain
[ci-badge]: https://github.com/turboBasic/PopulationCircles2026/actions/workflows/ci.yml/badge.svg?branch=main
[license]: LICENSE
[license-badge]: https://img.shields.io/github/license/turboBasic/PopulationCircles2026
[rust]: https://www.rust-lang.org/
[rust-badge]: https://img.shields.io/badge/rust-1.97%2B-orange.svg
[valeriepieris]: https://en.wikipedia.org/wiki/Valeriepieris_circle
