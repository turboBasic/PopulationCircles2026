---
tags: [plan, code, popcircles]
created: 2026-08-15
---

# Implementation plan — ADR 0008, map rendering from the published document

**Status: in progress (2026-08-15).** Carries issue #9, the ninth step of roadmap #11 and step 5 of
`application.md` "Approach": the results rendered as maps in Python, from the published document and
nothing else. It is the sibling of [ADR 0008](0008-rendering-reads-the-published-document.md), whose
four rulings every task below implements. Task 1.1 wrote that record and moved this file here from
`tmp/`, which is why it was executed by hand — `run-plan` resolves plans in `docs/decisions/` only,
and before that commit there was no record for a plan to be the sibling of and no number for the file
to carry.

Everything below rests on measurements taken 2026-08-15 in a scratch uv project outside this tree,
the way ADR 0002 measured its decoder:

| Measured | Result |
| --- | --- |
| `uv add cartopy matplotlib pyproj shapely` on Python 3.14 | 16 packages; cartopy 0.25.0, matplotlib 3.11.1, pyproj 3.7.2, shapely 2.1.2, numpy 2.5.2 |
| `uv pip install --only-binary :all: --python-version 3.14 cartopy` | fails: "all versions of cartopy have no usable wheels" — every version builds from source, 35 s here |
| system libraries needed | none: shapely and pyproj ship GEOS and PROJ in their wheels, and cartopy 0.25 links neither directly |
| `py.typed` | matplotlib yes, pyproj yes, numpy yes; **cartopy no, shapely no** |
| pyright strict over a 35-line renderer | 4 errors: 1 `reportMissingTypeStubs` for `cartopy.crs`, 3 `reportUnknownMemberType` for `pyplot.figure`, `Figure.text`, `Figure.savefig` |
| the same, with a 20-line stub under `typings/cartopy/` | 3 errors, all matplotlib's `**kwargs: Unknown` |
| `ax.fill(lons, lats, transform=ccrs.Geodetic())` on a ring crossing the antimeridian | **fills the complement** — the circle is white and the rest of the globe is coloured |
| the same over a ring enclosing the north pole | **fills the southern side** instead of the cap |
| `PlateCarree.project_geometry(Point(0,0).buffer(r), AzimuthalEquidistant(centre))` | correct in all four hard cases, and offline |

The last row is the design. A cap is a circle in an azimuthal equidistant projection centred on it,
and PROJ's polygon transform is what splits it at the seam and closes it over a pole; the ring of
latitudes and longitudes a naive renderer draws is the thing that cannot express either. Measured
shapes, which are the assertions Phase 2 pins:

| Case | Projected geometry |
| --- | --- |
| 10 N 178 E, 3000 km | `MultiPolygon`, **2 parts**, lon bounds exactly [-180, 180] |
| 78 N 20 E, 3000 km | 1 part, lat bounds [51.020, **90.000**], lon [-180, 180] |
| 80 S 170 W, 4000 km | 1 part, lat bounds [**-90.000**, -44.027] |
| 30 N 100 E, 3300 km | 1 part, lon bounds [65.130, 134.870] — no seam, no pole |

**The drawn ring is not the boundary, and that decides how 2.4 is tested.** Closing a cap into flat
coordinates means synthesising vertices that are on no boundary: measured against the source ring's 721
points, the polar case's projected ring carries 725 — two of them at latitude exactly 90 and the nearest
1334.341 km from the centre, which is the centre-to-pole distance — and the seam case carries 728, the
nearest 219.010 km from the centre, because the cut runs down the ±180 meridian. Only the case that
crosses nothing round-trips: 721 points, every one 3300.000000 km out. So a "every vertex is the radius
from the centre" assertion belongs to the ring the buffer produced, transformed point by point, and the
drawn geometry is asserted on topologically instead. Two objects, two assertions; one assertion over
both fails on precisely the two cases it exists to defend.

## Ground rules

- **No rendered map enters the repository.** That is the first project invariant, and it rules out
  the obvious way to test a renderer: matplotlib's image comparison needs a committed baseline PNG.
  Tests assert on geometry and on figure state; a figure a test writes goes to pytest's `tmp_path`.
- **`ax.coastlines()` and `add_feature` download Natural Earth on first use** — measured, from
  `naturalearth.s3.amazonaws.com` into `~/.local/share/cartopy/`. No test that runs by default may
  reach them: they belong behind the `network` marker, which is `platform.md` "Testing"'s treatment.
- **The sphere's radius is never written in a Python source file.** `geodesy.rs` owns the earth
  model, and a literal `6371.0088` in `scripts/` is a second copy of it — the defect the "Ground
  distance" invariant names. Phase 1 publishes the model so Phase 2 can read it.
- **`plot` and `fill` with `transform=ccrs.Geodetic()` are not the drawing path.** Measured wrong at
  the seam and at both poles, in the direction that looks plausible: a filled complement is still a
  map. Drawing goes through `add_geometries` with the azimuthal source CRS.
- **A pyright complaint is answered at the boundary or with a stub, never by a setting.** A narrow
  per-line ignore naming its rule and its reason, or a declaration under `typings/`. Lowering
  `typeCheckingMode`, or adding a rule set to `"none"` in `[tool.pyright]`, is the non-negotiable
  `platform.md` "Type checking" states.
- **The renderer opens one file: the JSON document.** No raster, no LFS object, no summation table.
  Fixtures are dictionaries in the test module.

## Out of scope

- **The sweep's share-against-radius curve.** #9 is map rendering; that plot is a chart, and
  `SweepDocument`'s ascending `records` will still be there when someone wants it.
- **Country outlines and masks.** #12, #13 and #17 own the boundary dataset and its rendering, and
  drawing an outline here would need the dataset before its licence check.
- **A population-density basemap.** The renderer reading the raster is the arrow `application.md`
  "Language split" forbids; a density background has to arrive as a published product, not as a
  second raster stack.
- **Committing Natural Earth coastlines to `data/`.** Offline figures would be worth having, but the
  registry row, the LFS object and the licence check are #12's work and would be done twice.
- **Ellipsoidal geodesics.** Rejected deliberately: a WGS 84 circle is a few tenths of a percent off
  the spherical cap the search summed, so the outline would stop being the answer's own boundary.
  The direct geodesic problem is still what verifies the vertices — on the sphere, which is the
  flattening-zero case of it.
- **An importable Python package.** `pyproject.toml` is a virtual project by decision; nothing here
  imports across modules that `pythonpath = ["scripts"]` does not already resolve.

## Phase 1 - What the document has to publish

The renderer needs two facts the format does not carry: which sphere the answer is on, and which
document it is holding. Both are additive, so neither bumps `SCHEMA_VERSION` — and
`scripts/lint_version_bumps.py` agrees by construction, because `check_snapshots` fires on a key
that disappears and these tasks only add keys.

**Model: Opus 5.** The record rules four things at once, and the two fields are the wire format that
issues #16 and #17 will read as well as this renderer — a field placed wrongly here is a promise
withdrawn later.

- [x] **1.1** `docs/decisions/NNNN-rendering-reads-the-published-document.md` is an accepted record,
  and this file is committed beside it as `NNNN-rendering-reads-the-published-document.plan.md`, its
  title and status line taking that number. NNNN is the next free one when the record is written —
  0008 against a tree holding 0001 to 0007, unless another record landed first, which is why nothing
  before this task writes a number down. It rules four things, each with the measurements above behind
  it: rendering is a downstream client of the published document; the circle is drawn on the sphere
  `geodesy.rs` owns, which the document must therefore publish; the stack is cartopy over matplotlib,
  bought at a from-source build on Python 3.14 and a render path that cannot run in CI because Natural
  Earth arrives over the network; and the cap is an azimuthal-equidistant buffer rather than a ring of
  coordinates, because the ring is measurably wrong at the seam and the poles. Written with the
  `write-adr` skill.
  Verify: `mise run lint:docs` passes with the new record in the tree, and `rg -l 'ADR <NNNN>' docs/`
  names exactly the record and this plan — each referring to the other by the number the record took.

- [x] **1.2** `Envelope` publishes the earth model: an `EarthModel { model, radius_km }` emitted
  between `tool_version` and `provenance`, whose `model` is `"sphere"` and whose `radius_km` is
  `geodesy::EARTH_RADIUS_KM` — read from the constant, so this is a publication of the owner and not
  a second copy. `report.rs`'s module documentation says what the field attests: which earth model
  every distance in the document was measured on, so a consumer drawing on another one is drawing a
  different figure.
  Verify: all 10 files under `crates/popcircles/src/snapshots/` carry `"earth_model"` with
  `"radius_km": 6371.0088`; `rg -n '6371' crates/ --glob '!*.snap'` still names `geodesy.rs` only;
  `prek run --all-files version-bumps` passes with `SCHEMA_VERSION` unchanged at 1.

- [x] **1.3** `Envelope` publishes which document it is, as a `document` key emitted directly after
  `schema_version` — a consumer branches on the kind before reading anything under `result`. A
  `Document` trait with `const KIND: &'static str`, implemented for the nine payload types, so no call
  site passes a string and a kind is a property of its type rather than of whoever wrapped it. The trait
  does not make the kinds distinct — an associated const cannot — so a test collects all nine into a
  `BTreeSet` and asserts nine members, which turns a typo that duplicates one into a failure. A tenth
  payload type owes that test a line, the same obligation the doc table below already carries. The
  kinds:
  `distance`, `grid`, `table-build`, `table-query`, `circle`, `most-populous`, `smallest-circle`
  (a bare `SmallestReport`, which is what a snapshot pins), `smallest` (`SmallestDocument`, which is
  what the CLI emits) and `sweep`. `()` gets a `cfg(test)` implementation for the three envelope
  tests that carry no payload. The table in `report.rs`'s module documentation gains the kind beside
  each payload, because that table is where a consumer meets them.
  Verify: `rg -c '"document":' crates/popcircles/src/snapshots/*.snap` reports 1 for each of the 10
  files; the distinctness test fails when one `KIND` is edited to match another, checked by making that
  edit and running `cargo test -p popcircles kinds` before reverting it; `mise run test` passes; `prek
  run --all-files version-bumps` passes with `SCHEMA_VERSION` at 1.

## Phase 2 - The stack, typed and offline

**Model: Sonnet 5 for 2.1 to 2.3, Opus 5 for 2.4.** The first three are a manifest, a stub file and a
parser with three refusals. 2.4 is the one task that fails by drawing a plausible wrong circle, and its
test is what stands between a seam artefact and a figure nobody checks.

- [x] **2.1** The five Python dependencies are in `[dependency-groups].dev` with `uv.lock`
  updated in the same change — the four rendering libraries, plus `pydantic` for the boundary 2.3
  parses at — and the cost the measurements found is paid where it shows: `ci.yml`
  caches `~/.cache/uv` keyed on `uv.lock`, because without it every CI run rebuilds cartopy from
  source. The comment beside that step names the missing cp314 wheel as the reason, not "speed".
  pydantic is not that cost and does not add a second one: it resolves under
  `uv pip install --only-binary :all: --python-version 3.14`, so it arrives as a wheel.
  `.gitattributes` gains `*.pyi text` — a new file type arrives in 2.2 — and `.cspell/project.txt`
  gains the library and geospatial terms in the sections that hold them (`cartopy`, `pyproj`,
  `shapely`, `matplotlib`, `geoaxes`, `PlateCarree`, `Orthographic`, `naturalearth`, `azimuthal`,
  `equidistant`, `quad`, `savefig`).
  Verify: `uv sync --locked` is clean and `uv sync --locked --reinstall-package cartopy` prints
  `Building cartopy`; `mise run lint:workflows` and `mise run lint:cspell` pass; `mise run lint` is
  green with no `noqa` added anywhere.

- [x] **2.2** `typings/cartopy/` declares the six symbols this renderer uses — `crs.PlateCarree`,
  `crs.Orthographic`, `crs.AzimuthalEquidistant`, `crs.Globe`, `crs.Projection.project_geometry`
  and `mpl.geoaxes.GeoAxes` with the four methods called on it — and `[tool.pyright]` gains
  `stubPath = "typings"`. That is what turns the measured `reportMissingTypeStubs` into a checked
  type rather than an ignored line, and it is why `GeoAxes` is a type at all: matplotlib annotates
  `add_subplot(projection=...)` as returning `Axes3D`, so without the stub `ax.set_global()` is an
  attribute error and `cast(Any, ...)` is the only way past it. `platform.md` "Structure" gains the
  `typings/` root in the same commit, which `lint_docs.py` requires.
  Verify: `mise run typecheck` passes; `mise run lint:docs` passes with `typings/` tracked;
  `rg -n 'Any' typings/` returns nothing.

- [x] **2.3** `scripts/circle_document.py` turns a document into frozen pydantic models, refusing what
  it cannot honestly draw: a `schema_version` above the one it knows, an unrecognised `document` kind,
  and an `earth_model.model` that is not `"sphere"`. Frozen models rather than frozen dataclasses
  because every one of those three refusals is a validation a dataclass performs nowhere, so the
  hand-written alternative is this library's own job done worse — `model_config` carries
  `frozen=True` and `extra="ignore"`, the second being the format's own instruction to consumers
  (`report.rs` "Growth") rather than a convenience. The kind is a `Literal` over the nine
  `Document::KIND` strings and the model is `"sphere"` the same way, so a refusal is a schema fact and
  not a branch someone remembered to write. It exposes the three circle-bearing kinds — `circle`,
  `most-populous`, `smallest` — as one `Circle` with centre, radius, population and share, plus the
  document's radius in kilometres. `tests/test_circle_document.py` builds each of the three from a
  dictionary and asserts each refusal names the field it refused on, read off
  `ValidationError.errors()`'s `loc` rather than matched against a message string.
  Verify: `uv run pytest tests/test_circle_document.py` passes with at least 6 tests, and no fixture
  in it opens a file; `mise run typecheck` passes.

- [x] **2.4** `scripts/circle_geometry.py` builds the cap and knows nothing about figures. Three
  functions, because the paragraph above says the ring and the drawing are different objects: a
  `cap(centre, radius_km, earth_radius_km)` returning the shapely buffer with its source CRS, a
  `boundary(cap)` transforming that buffer's own vertices point by point into latitudes and longitudes,
  and a `drawn(cap, target)` returning what PROJ makes of the polygon. `QUAD_SEGS = 180` is a named
  constant, and the comment beside it states what shapely does with it and what that costs, both
  measured: 180 is per quadrant, so the polygon has 720 sides, and the chord then cuts 31.41 m inside
  the arc at 3300 km — a thirtieth of the registry raster's 926.6 m cell at the equator.
  `tests/test_circle_geometry.py` closes the issue's first checkbox on `boundary`, where every vertex
  is a real one: each is `radius_km` from the centre by a great-circle formula written in the test, and
  the four cardinal vertices match `pyproj.Geod(a=..., f=0).fwd` — the direct geodesic problem at
  flattening zero, from a library that is not the one that drew it. On `drawn` it asserts the topology
  instead, plus the one distance claim that survives synthesised vertices: none lies outside the cap.
  Verify: `uv run pytest tests/test_circle_geometry.py` passes, with the polar case asserting
  `boundary` is 3000.0 km at every one of its 721 vertices while `drawn` has 725 including two at
  latitude exactly 90.0, and the antimeridian case asserting `len(drawn.geoms) == 2`; the whole file
  runs with no network — it passes with `~/.local/share/cartopy` moved aside.

## Phase 3 - The figure, the task and the documents

**Model: Sonnet 5, Opus 5 for 3.3.** A figure, a marker and a close-out are mechanical against the two
modules Phase 2 leaves. 3.3 edits the instruction layer and writes two register entries, where the bar
is a condition the repository can answer rather than a sentence that reads like one.

- [ ] **3.1** `scripts/render_map.py` renders a map from a document path and writes it where it is
  told: `--input` and `--output` both required, `--projection` taking `plate-carree` (the default,
  and the projection the viral maps used) or `orthographic` centred on the circle. It draws the cap
  through `add_geometries`, marks the centre, titles the figure from the document's own figures, and
  puts the CC BY citation in the footer. Every remaining pyright complaint is a per-line ignore
  naming its rule and stating that matplotlib types the keyword arguments as `Unknown`; the
  measurement says there are three of them, at `pyplot.figure`, `Figure.text` and `Figure.savefig`.
  `mise.toml` gains a `render` task passing its arguments through, and its comment says why the task
  is in neither `lint` nor `ci`.
  Verify: `mise run render -- --input <a document written by mise run cli> --output tmp/map.png`
  writes a PNG; `rg -n 'pyright: ignore' scripts/render_map.py` shows 3 lines, each with a reason;
  `mise run typecheck` and `mise run lint:python` pass.

- [ ] **3.2** The attribution is checked rather than trusted: `tests/test_render_map.py` asserts the
  citation constant appears in `data/README.md` with whitespace normalised, so the registry stays the
  owner of the text and a drift between the two fails a test instead of shipping a figure that
  credits nobody. The same module asserts the footer artist carries it, over a figure built without
  coastlines. The one test that renders a full figure with coastlines is marked `network`, and
  `pyproject.toml` declares the marker and adds `-m "not network"` to `addopts`; `mise.toml` gains
  `test:render` for the marked set, with the comment `test:raster` carries.
  Verify: `uv run pytest` collects the suite with the marked test deselected and
  `uv run pytest -m network` collects exactly 1; `uv run pytest -m network` passes on a machine with
  network; `rg -n 'CIESIN' scripts/ tests/` names the constant and the assertion only.

- [ ] **3.3** The documentation the change invalidated moves with it. `application.md` step 5 says
  what rendering is now — the three modules, the azimuthal cap, and that the renderer reads the
  document and the earth model it publishes — and its "Approach" paragraph names `report`'s two new
  fields. `README.md` gains the render task under "Usage" and the `typings/` row under "Layout".
  `docs/follow-ups.md` gains the two entries this work produced, each with a condition the
  repository can answer: **FU-15**, cartopy is built from source because no cp314 wheel exists,
  fired by `uv pip install --only-binary :all: --python-version 3.14 cartopy` resolving; **FU-16**,
  the citation is a Python constant rather than the registry's own text, fired by `data/README.md`
  carrying a second dataset row, at which point the constant has to become a mapping.
  Verify: `mise run lint` passes, `lint:docs` included; `rg -n 'render' docs/ai/application.md
  README.md` shows both updated; the two entries are the last two in `docs/follow-ups.md` and each
  names a command or a file.

- [ ] **3.4** The plan is closed: the status line reads `**Status: complete (YYYY-MM-DD).**`,
  Follow-ups holds `FU-15` and `FU-16` and nothing else, and the four checkboxes in issue #9's body
  are ticked along with #9's box in the roadmap issue #11. The issue itself is left open — the PR's
  `Closes #9` is what closes it, per `platform.md` "Git".
  Verify: `gh issue view 9` shows four ticked boxes and state OPEN; `gh issue view 11` shows #9
  ticked; `mise run ci` is green.

## Follow-ups

`FU-15`, `FU-16`.
