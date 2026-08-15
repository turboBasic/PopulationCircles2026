---
tags: [adr, code, popcircles]
created: 2026-08-15
decided: 2026-08-15
supersedes: null
superseded_by: null
---

# ADR 0008 - Rendering reads the published document, and draws a circle as an azimuthal-equidistant buffer

## Status

Accepted - 2026-08-15. It supersedes nothing.

It extends [ADR 0001](0001-cli-and-output-layer.md) along the axis that record opened. 0001 put the
wire format in the library's `report` module and named a renderer as one of its two consumers; this
one is the first renderer arriving, and what it needs from that format is two fields the format does
not carry. It borrows [ADR 0002](0002-no-system-gdal.md)'s method rather than its ruling: a dependency
is measured in a scratch project outside this tree before it is taken, and the numbers go in the
record.

## Context

Issue #9 asks for the results rendered as maps in Python, reading only the JSON contract. The contract
exists — ten snapshots under `crates/popcircles/src/snapshots/` pin its shape, key order included —
and `application.md` "Language split" already rules that the renderer reads it rather than the raster.
What was not settled is the stack that draws it and the geometry it draws, and neither turned out to
be a free choice.

Measured 2026-08-15 in a scratch uv project outside this tree, Python 3.14:

| Measured | Result |
| --- | --- |
| `uv add cartopy matplotlib pyproj shapely` | 16 packages; cartopy 0.25.0, matplotlib 3.11.1, pyproj 3.7.2, shapely 2.1.2, numpy 2.5.2 |
| `uv pip install --only-binary :all: --python-version 3.14 cartopy` | fails: "all versions of cartopy have no usable wheels" — every version builds from source, 35 s here |
| system libraries needed | none: shapely and pyproj ship GEOS and PROJ in their own wheels, and cartopy 0.25 links neither directly |
| `py.typed` | matplotlib yes, pyproj yes, numpy yes; **cartopy no, shapely no** |
| pyright strict over a 35-line renderer | 4 errors: 1 `reportMissingTypeStubs` for `cartopy.crs`, 3 `reportUnknownMemberType` for `pyplot.figure`, `Figure.text`, `Figure.savefig` |
| the same, with a 20-line stub under `typings/cartopy/` | 3 errors, all matplotlib's `**kwargs: Unknown` |
| `ax.coastlines()` and `add_feature` | download Natural Earth from `naturalearth.s3.amazonaws.com` into `~/.local/share/cartopy/` on first use |

So the stack costs a from-source build on this Python and a network reach on the drawing path, and
buys a projection library that is offline for everything else.

**The drawing primitive was the decision that could have gone wrong quietly.** The obvious way to draw
a circle on the globe is to walk bearings around the centre, collect latitudes and longitudes, and fill
them with `transform=ccrs.Geodetic()`. Measured on the two cases this project is named for:

| Case | `ax.fill(lons, lats, transform=ccrs.Geodetic())` |
| --- | --- |
| a ring crossing the antimeridian | **fills the complement** — the circle is white and the rest of the globe is coloured |
| a ring enclosing the north pole | **fills the southern side** instead of the cap |

Both failures produce a map. Neither raises. The alternative measured correct in all four hard cases,
and offline: `PlateCarree.project_geometry(Point(0, 0).buffer(r), AzimuthalEquidistant(centre))` — a cap
is a circle in an azimuthal equidistant projection centred on it, and PROJ's polygon transform is what
splits it at the seam and closes it over a pole.

| Case | Projected geometry |
| --- | --- |
| 10 N 178 E, 3000 km | `MultiPolygon`, **2 parts**, lon bounds exactly [-180, 180] |
| 78 N 20 E, 3000 km | 1 part, lat bounds [51.020, **90.000**], lon [-180, 180] |
| 80 S 170 W, 4000 km | 1 part, lat bounds [**-90.000**, -44.027] |
| 30 N 100 E, 3300 km | 1 part, lon bounds [65.130, 134.870] — no seam, no pole |

Closing a cap into flat coordinates means synthesising vertices that lie on no boundary. Against the
source ring's 721 points, the polar case's projected ring carries 725 — two at latitude exactly 90, and
the nearest of them 1334.341 km from the centre, which is the centre-to-pole distance. The seam case
carries 728, the nearest 219.010 km out, because the cut runs down the ±180 meridian. Only the case
crossing nothing round-trips: 721 points, every one 3300.000000 km from the centre.

Last, the sphere. `geodesy.rs` line 4 is `pub const EARTH_RADIUS_KM: f64 = 6371.0088;`, and
`rg -n '6371' crates/` names that file and its own tests and nothing else. A buffer radius in metres is
an angle divided by a radius, so a renderer needs that number — and a literal `6371.0088` in `scripts/`
would be the second copy of the earth model the "Ground distance" invariant exists to forbid.

## Decision

**1. Rendering is a downstream client of the published document, and opens no other file.** The
renderer's input is one JSON path. Not the raster, not an LFS object, not a summation table, not a
ledger, and not a Rust type — `application.md` "Architecture" already points the arrow that way, and
this record is what makes the arrow testable: a fixture for a rendering test is a dictionary in the
test module, so a test that needs raster bytes cannot be written by accident.

A consumer therefore has to know what it is holding before it reads under `result`, so **the envelope
publishes a `document` kind** directly after `schema_version`. The kind is a property of the payload
type rather than of whoever wrapped it, which means a trait with an associated constant and not a
string at the call site.

**2. The document publishes the earth model, and a renderer refuses one it cannot draw on.** An
`earth_model` block carrying `model` and `radius_km`, read from `geodesy::EARTH_RADIUS_KM` so it is a
publication of the owner and not a second copy. What the field attests is which sphere every distance
in the document was measured on; a consumer drawing on another one is drawing a different figure, and
saying so is worth a field because the failure is silent. The renderer refuses a `model` that is not
`"sphere"` rather than assuming the number it was handed is a radius it knows what to do with.

Both fields are additive, so neither bumps `SCHEMA_VERSION`.

**3. The stack is cartopy over matplotlib, with pyproj and shapely, and the cost is paid where it
shows.** Sixteen packages, no system library, and a from-source cartopy build on Python 3.14 — so
`ci.yml` caches `~/.cache/uv` keyed on `uv.lock`, and the comment beside that step names the missing
cp314 wheel rather than "speed". cartopy and shapely ship no `py.typed`, and pyright stays strict: what
answers a complaint is a declaration under `typings/` or a per-line ignore naming its rule and its
reason. Lowering `typeCheckingMode` or setting a rule to `"none"` is not available here.

Natural Earth arriving over the network puts the coastline path behind the `network` marker, deselected
by default and given its own task. **The render path is therefore not a path CI exercises**, and the
tests that do run assert on geometry and on figure state instead — which they can, because decision 4
makes the geometry an object a test can hold.

**4. A circle is an azimuthal-equidistant buffer projected by PROJ, never a ring of coordinates filled
with `transform=ccrs.Geodetic()`.** The measurements above are the reason, and the direction of the
failure is why it is a ruling rather than a preference: a filled complement and a filled southern
hemisphere are both plausible maps, so this is caught by a test or it is not caught.

The buffer and the drawn polygon are **two objects with two different assertions**. The buffer's own
vertices are real ones, transformed point by point, and every one is the radius from the centre — that
is where the distance claim belongs, and where the direct geodesic problem at flattening zero verifies
it. The drawn polygon carries synthesised vertices and is asserted topologically, plus the one distance
claim that survives synthesis: nothing lies outside the cap. One assertion over both objects fails on
precisely the two cases it exists to defend.

## Consequences

**Positive**

- The seam and the poles are handled by PROJ, in the four cases measured above, rather than by
  special-casing a coordinate ring. The antimeridian and poles invariant is met by the primitive
  instead of by a traversal that has to remember to.
- The earth model stops being implicit. A document says which sphere it was measured on, and #16 and
  #17 read the same field rather than each deciding what to assume.
- `document` makes a consumer's first branch a lookup instead of a guess at which keys are present.
  A tenth payload type owes it a kind, which is a compile error rather than a convention.
- The renderer's input is one JSON path, so a rendering test is a dictionary and runs in CI on a clone
  with no LFS content.
- pyright stays strict over the new code, and the one library with no annotations at all is answered by
  20 lines of stub rather than by a setting.

**Negative / costs**

- **cartopy builds from source on every cold environment.** 35 s measured here, and the uv cache in CI
  is what stands between that and every job paying it. A cache miss — a `uv.lock` touched for any
  reason — pays it again.
- **The render path is not exercised by any gate.** Coastlines need the network, so the one test that
  draws a complete figure is deselected by default and never runs in CI. A cartopy upgrade that breaks
  `add_geometries` is caught by a person running `mise run test:render`, or not at all.
- Two fields added to the envelope are two promises. `earth_model` is easy to keep only while the model
  stays a sphere; the moment an ellipsoid is wanted, `radius_km` is a field with no meaning and the
  block has to grow rather than change, because it went out under `SCHEMA_VERSION` 1.
- Nine payload types each gain a kind string, and the trait cannot make them distinct. A test collecting
  them into a set is what does that, which is a check standing in for a language feature.
- `typings/cartopy/` is a hand-written declaration of a third-party API. It is correct on the day it is
  written and silently wrong the day cartopy changes a signature — a stub asserts nothing about the
  library it describes.
- The buffer is a 720-sided polygon, not a circle. `QUAD_SEGS = 180` cuts 31.41 m inside the arc at
  3300 km, which is a thirtieth of the registry raster's equatorial cell and therefore invisible on a
  map — but the outline is an approximation of the answer's boundary, not the boundary.
- Four Python dependencies and 16 packages arrive for a half of the project that renders figures. The
  numeric path gains nothing from any of them, and `uv sync` is slower for everyone.

## Alternatives considered

- **A ring of latitudes and longitudes filled with `transform=ccrs.Geodetic()`.** The obvious
  implementation, and what a naive renderer does. It lost on measurement: it fills the complement for a
  circle crossing the antimeridian and the wrong hemisphere for one over a pole. Both were the cases the
  program exists to get right.
- **Splitting the ring at the antimeridian by hand and closing it over a pole by hand.** Keeps the
  dependency surface at matplotlib alone. It lost because it is the same work PROJ's polygon transform
  already does, in the one place where getting it wrong produces a plausible figure — and because a
  hand-rolled seam split has to be tested on exactly the four cases `project_geometry` was measured
  correct on.
- **GeoPandas, or folium, or plotly.** Each would draw a map. They lost on what they add rather than on
  capability: GeoPandas brings pandas and pyogrio for a single polygon and no table; folium and plotly
  produce interactive HTML, which is a different artifact from the static figure #9 asks for and would
  put a JavaScript bundle in the render path.
- **Ellipsoidal geodesics on WGS 84.** More accurate as geodesy, and pyproj is already here to do it. It
  lost because the answer being drawn is a spherical cap the search summed: a WGS 84 circle is a few
  tenths of a percent off it, so the outline would stop being the boundary of the thing it is drawn to
  show. The direct geodesic problem still verifies the vertices — at flattening zero, which is the same
  library answering about the same sphere.
- **A population-density basemap under the circle.** The figure everyone actually wants, and the raster
  is right there. It lost to `application.md` "Language split": the renderer reading the raster is the
  arrow that record forbids, and a density background has to arrive as a published product rather than
  as a second raster stack in Python.
- **Committing Natural Earth coastlines to `data/` so the render path runs offline and in CI.** It would
  close this record's largest cost. It lost on ownership rather than on merit: a boundary dataset needs a
  registry row, an LFS object and a licence check, which is #12's work, and doing it here would do it
  twice.
- **Rendering in Rust, with `plotters` or `tiny-skia`.** No Python dependency at all, and no second
  language. It lost to `application.md` "Language split", which is a standing ruling this record is not
  reopening: rendering is Python's half, and the projection work is where the ecosystem gap is widest.
