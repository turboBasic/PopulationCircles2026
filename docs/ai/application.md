# Application

Read this when working on the search itself: what the program answers, how it is meant to get there,
how the code doing it is arranged, what must hold for an answer to be right, and whose code may not
be reused doing it.
[`platform.md`](platform.md) wins wherever it and this file would conflict.

## What this program does

Given a share of world population, find the smallest circle **on the globe** that contains it —
smallest by great-circle radius, not by area on a projected map. The input is a ~1 km resolution
equirectangular population raster (30 arc-second grid, 21600 × 43200). Secondary outputs: the most
populous circle of a fixed radius, and rendered maps of the results.

The same question restricted to **one country** is a second supported goal: the smallest circle
containing a share of that country's population, with 100% meaning everyone in it. The circle's
centre may lie outside the country. This is not the smallest circle enclosing a country's landmass —
that is a geometric problem this program does not solve, and conflating the two is a spec error.
Restricting to a country adds boundary data and a mask over the grid; the search itself is the same.

"On the globe" is the whole point. A circle drawn on an equirectangular image is not a circle on the
Earth, and the well-known viral maps of the 50% circle got this wrong.

## Provenance and the copying rule

This project is inspired by [alexmijo/PopulationCircles](https://github.com/alexmijo/PopulationCircles),
which solved the same problem in C++ and produced the published maps.

**That repository has no licence.** Default copyright applies, so its source is not ours to reuse:

- **Never copy, paste, transliterate, or line-by-line port** source from that project — or from any
  local checkout of it — into this repo. Mechanical translation of its C++ into Rust is copying.
- **Algorithms and mathematics are free to use.** Summed-area tables, per-latitude circular kernels,
  and binary search over radius are well-known published techniques, and the problem itself has prior
  art (the Valeriepieris circle; Danny Quah's 3300 km result). Implementing a documented technique
  from its description is independent work.
- Practical line: consulting that repo is allowed, and useful — it is the record of what the published
  maps were actually computed with, and answers questions no description does. What may not cross over
  is expression: take a fact, a constant or a convention from it, and write the code itself from the
  reference or from a description of the technique.
- Third-party files vendored upstream carry their own permissive licences and are irrelevant here —
  pull such a dependency from its own origin through Cargo or uv, never from that repo.

A port cannot be un-published, so this is non-negotiable by the test in `docs/ai-instructions.md`
"Non-negotiables": a request to lift upstream code gets the protocol that section sets out, not a
quiet workaround.

## Approach

Steps 1 to 5 are targets rather than existing code. What exists is the ground they stand on: one
library crate, `crates/popcircles/`, with `geodesy` holding the earth model, longitude wrapping and
great-circle distance, and `grid` holding the raster's geometry — the checked `Grid`, pixel centres
and their inverse, cell edges, and cell area. Both are pure computation with no I/O, and steps 1 and
2 are the first callers.

1. **Summation table.** Convert the raster into a 2D prefix-sum table so the population of any
   axis-aligned pixel rectangle is four lookups. Built once, cached to disk, never committed. At full
   resolution the table is ~7.5 GB (933M cells × 8 bytes), so it is read by mmap rather than held
   resident, and a decimated table (coarser grid, same code path) exists for fast iteration and for
   tests.
2. **Circular kernels.** A circle of a given ground radius covers a different pixel span at each
   latitude, so decompose it into per-row rectangles — a kernel — reusable for every longitude at
   that latitude. Building kernels is the only step that computes geodesic distance.
3. **Most populous circle of a given radius.** Scan the globe at a coarse step, then refine around
   the best candidates. This is the step that can be wrong on adversarial input; its tolerance is a
   deliberate, documented choice, not an accident.
4. **Smallest circle for a given population.** Binary search over integer radius in km, driving
   step 3. Cache every radius tried so a rerun resumes instead of repeating work.
5. **Rendering.** Python, from the search results, kept out of the Rust search path entirely.

A module per subject, and two crates: the library `crates/popcircles/` and the binary
`crates/popcircles-cli/`. A dependency forced that boundary and is what a further split takes too —
`clap` and `anyhow` live in the CLI's manifest, and the library's may not grow them.
[`platform.md`](platform.md) "Structure" needs nothing for this: `crates/` is already a root
there, and that section carries roots rather than an inventory.

## Architecture

The search is a library; the CLI and the renderer are clients of it. Dependencies point one way —
view depends on results, CLI depends on domain, domain depends on neither — and a change that reverses
an arrow is an architecture change, not a refactor.

- **Model the domain in types, not primitives.** A latitude, a longitude, a pixel index, a radius in
  km and a population count are five different things, and a bare `f64` or `usize` standing in for
  one is where a wrong-units bug lives undetected. `Grid` is the existing example: the constructor
  rejects the invalid shape, so no later stage revalidates and no caller can build the impossible
  value. Prefer a type whose invalid states do not construct over a check repeated at every use.
- **The domain computes and returns; it does not read, write, print, or format.** geodesy, grid,
  ingest, table, kernels and search take domain types and give back domain types. Paths, file formats,
  stdout, progress reporting and CLI flags are not domain concerns — a module that grows one has taken
  a second responsibility (SRP), and the test that used to be a pure function call now needs a
  filesystem. Where a long-running step must report progress or a raster must be sourced, take the
  sink or the source as a parameter (DIP) so a fixture, a decimated raster and an mmap-backed table
  are the same seam. Keep those abstractions narrow (ISP): the search needs "population of this
  rectangle", not a general raster API.
- **The CLI is a shell.** Parse arguments, resolve paths and datasets, build a config value, call the
  library, serialise the result, map errors to exit codes. Any arithmetic on coordinates or
  populations, any candidate ordering, any tolerance, belongs below it — if the CLI acquires a branch
  the library should have owned, move the branch down rather than duplicating the logic.
- **The view is downstream of results, never of the domain.** Rendering reads the serialised result
  (see [Language split](#language-split)) and knows nothing of the raster, the table or the domain
  types. That serialised shape is the contract between the two halves: extend it additively, and
  treat a renderer that needs a new field as a reason to publish the field, not to reach past the
  boundary.
- **Substitutability is behavioural.** An implementation swapped in behind one of those abstractions —
  a coarser grid, a cached table, a masked raster — must preserve the invariants below, not merely
  typecheck (LSP). OCP is the weakest of the five here: a new dataset, mask or output format should
  arrive as a new implementation of an existing seam, but do not build extension points ahead of a
  second caller.

## Correctness invariants

Properties any numeric change must preserve — geodesy, raster ingest, the summation table, kernels,
the search, and anything that caches their results. Each is a claim a test can pin, and pinning the
invariant matters more than pinning the output: a summation table agrees with a naive sum on small
inputs; a circle's contained population is monotonic in radius. [`platform.md`](platform.md) "Testing"
holds how those tests are organised.

- **Ground distance.** Distances are great-circle arcs on a sphere. `crates/popcircles/src/geodesy.rs`
  states the radius and the formula and is the only place either appears; a second copy anywhere is a
  defect to fix there. **Never treat pixel or degree distance as ground distance, and never measure
  Euclidean in pixel or degree space.** That is a correctness bug, not an approximation: it produces a
  plausible wrong answer rather than a failure, which is why it needs a test and not just a review.
- **Antimeridian and poles.** A circle may wrap longitude or cover a pole. Every raster traversal
  handles wrapping, and a kernel row spanning the full width is a normal case rather than an edge
  case. A traversal correct only away from the seam is not correct.
- **Determinism.** Same raster and same parameters give the same answer, tie-breaks included. Order
  the candidates rather than relying on iteration order, and break ties on a stated rule.
- **Nodata.** Negative or sentinel cells are zero population, converted once at ingest so that no
  later stage has to know a sentinel from a count. `data/README.md` records each dataset's value.
- **Precision.** Populations sum to ~8 × 10⁹ over ~9 × 10⁸ cells; f64 throughout the summation table.
  Narrowing the element type is not the way out of the table's size — that size is a stated cost of
  the approach — and needs a documented error analysis first.
- **Cache invalidation.** A cached table or result file records what raster and what parameters
  produced it, and is rejected rather than silently reused when those differ.
- **Population totals are dataset properties.** Derive the world total from the raster; never hardcode
  a figure taken from elsewhere. `data/README.md` records the measured total as a sanity check, not as
  a value to embed, and anything published from the raster carries the attribution its registry entry
  states.

## Language split

Rust owns everything numeric: geodesy, raster ingest, the table, kernels, the search, the CLI —
including one-off utilities that reuse the reader, so there is no second raster stack to keep
correct. Python owns rendering and figures, and reads the CLI's JSON output rather than the raster. A
Python helper that touches raster pixels is a sign the work belongs in Rust.
