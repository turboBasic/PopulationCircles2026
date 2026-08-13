# AI Instructions — application

Scope: this application. Platform conventions live in [`ai-instructions.md`](ai-instructions.md)
and win wherever the two would conflict. Read both.

## What this program does

Given a share of world population, find the smallest circle **on the globe** that contains it —
smallest by great-circle radius, not by area on a projected map. The input is a ~1 km resolution
equirectangular population raster (30 arc-second grid, 21600 × 43200). Secondary outputs: the most
populous circle of a fixed radius, and rendered maps of the results.

"On the globe" is the whole point. A circle drawn on an equirectangular image is not a circle on
the Earth, and the well-known viral maps of the 50% circle got this wrong. Any change that treats
pixel distance as ground distance is a correctness bug, not an approximation.

## Provenance and the copying rule

This project is inspired by [alexmijo/PopulationCircles](https://github.com/alexmijo/PopulationCircles),
which solved the same problem in C++ and produced the published maps.

**That repository has no licence.** Default copyright applies, so its source is not ours to reuse:

- **Never copy, paste, transliterate, or line-by-line port** source from that project — or from any
  local checkout of it — into this repo. Mechanical translation of its C++ into Rust is copying.
- **Algorithms and mathematics are free to use.** Summed-area tables, per-latitude circular
  kernels, and binary search over radius are well-known published techniques, and the problem
  itself has prior art (the Valeriepieris circle; Danny Quah's 3300 km result). Implementing a
  documented technique from its description is independent work.
- Practical line: read a *description* of the approach, then write the implementation from the
  description. Do not open the upstream source to "check how it did this".
- Third-party files vendored upstream carry their own permissive licences and are irrelevant here —
  pull such a dependency from its own origin through Cargo or uv, never from that repo.

Treat a request to lift upstream code as a non-negotiable conflict per the platform doc: say so,
then offer the from-description alternative.

## Approach

The intended shape, stated as targets rather than as existing code — nothing is implemented yet.

1. **Summation table.** Convert the raster into a 2D prefix-sum table so the population of any
   axis-aligned pixel rectangle is four lookups. Built once, cached to disk, never committed.
2. **Circular kernels.** A circle of a given ground radius covers a different pixel span at each
   latitude, so decompose it into per-row rectangles — a kernel — reusable for every longitude at
   that latitude. Building kernels is the only step that computes geodesic distance.
3. **Most populous circle of a given radius.** Scan the globe at a coarse step, then refine around
   the best candidates. This is the step that can be wrong on adversarial input; its tolerance is a
   deliberate, documented choice, not an accident.
4. **Smallest circle for a given population.** Binary search over integer radius in km, driving
   step 3. Cache every radius tried so a rerun resumes instead of repeating work.
5. **Rendering.** Python, from the search results, kept out of the Rust search path entirely.

Nothing above is a commitment to a module layout. When the first code lands, this section gets
updated to describe what exists.

## Correctness constraints

- **Antimeridian and poles.** A circle may wrap longitude or cover a pole. Every raster traversal
  handles wrapping; a kernel row that spans the full width is a normal case, not an edge case.
- **Ground distance.** Distances are geodesic on the sphere (or ellipsoid, once chosen and
  documented). Never Euclidean in pixel or degree space.
- **Determinism.** Same raster plus same parameters gives the same answer, including tie-breaks.
  A result that moves between runs is a bug.
- **Nodata.** Rasters carry negative or sentinel nodata cells; they are zero population, and the
  conversion happens once at ingest, not scattered through the search.
- **Cache invalidation.** A cached summation table or result file records what raster and what
  parameters produced it, and is rejected rather than silently reused when those differ.
- **Precision.** Populations sum to ~8 × 10⁹ over ~9 × 10⁸ cells; f64 throughout the summation
  table. Do not narrow it for memory without a documented error analysis.

## Data

- Input rasters are Git LFS objects, not fetched by default: `mise run data:pull`. See the platform
  doc's **Large input data**.
- The raster used, its source, its licence, and its grid dimensions get recorded in `README.md`
  when one is actually wired up. Do not assume the upstream project's 2015 GHSL dataset — it was
  never in that repo either.
- Population totals are properties of a dataset, never constants in code: derive the world total
  from the raster, do not hardcode a figure taken from elsewhere.
