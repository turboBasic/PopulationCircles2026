# Data

Committed **input** datasets, one directory per kind. Git LFS holds the raster contents; a clone gets
pointers and fetches deliberately (see [Fetching](#fetching)).

Generated products — summation tables, rendered maps — never live here. They are gitignored
(`*.bin`, `out/`) and are reproducible from these inputs.

```text
data/
  population/   population rasters
  boundaries/   coastlines and country borders
```

A new kind gets its own directory, a row in [`registry.toml`](registry.toml) and an entry below. Keep
names lowercase and hyphenated, and put the grid resolution in the filename when a dataset comes in
several.

**Every filename here is this project's own description of the contents, and matches no publisher's.**
So what identifies a dataset is its row's Provenance, never its name — and a file downloaded from the
source is renamed to the row's heading before it is usable.

**LFS is for the rasters, not for `data/` as such.** A vector dataset small enough to read on every
render is a Git blob: it costs a hundred kilobytes of pack, and in exchange every clone and every CI
job has it without a fetch step. `.gitattributes` routes only `*.tif`/`*.tiff` to LFS, and the
`geo-data-lfs` hook is the tripwire for the binary formats neither of them names.

## Registry

### `population/population-count-2020-30arcsec.tif`

| Property | Value |
| --- | --- |
| Grid | 43200 × 21600 (30 arc-second, 1/120°) |
| Extent | whole globe, origin (−180°, 90°) |
| CRS | EPSG:4326 (WGS 84) |
| Pixel type | Float32, single band, LZW compressed |
| Nodata | −3.40282306073709653e+38 (Float32, two ulps above −max) |
| Size | 428 465 215 bytes (409 MiB) |
| SHA-256 | `956993aa500774aed548c8e1af1a3a68fc164577be82ca799d4ae8568d445e9d` |
| Land cells | 222 669 928 of 933 120 000 (182 358 616 populated, 40 311 312 zero) |
| World total | 7 757 982 599.32 persons |
| Largest cell | 602 380.375 persons |

Every value above was measured from the file, not copied from a datasheet. The total is a compensated
(Neumaier) sum: over 933 120 000 additions into a running 7.8e9, where one ulp is 1.9e-6, a naive f64
accumulator lands 0.15 low. Reproducing it that way gives 7 757 982 599.17, and the file is not what
changed.

#### Provenance

The file is **[Gridded Population of the World, Version 4.11 (GPWv4.11): Population Count Adjusted
to Match the 2015 Revision of UN WPP Country Totals, Revision 11][gpw-adj]**, year **2020**, 30
arc-second GeoTIFF — CIESIN, Columbia University, distributed by NASA SEDAC, DOI
[10.7927/H4PN93PB](https://doi.org/10.7927/H4PN93PB). SEDAC's own `sedac.ciesin.columbia.edu` host
is gone; the DOI is the route that survived it, and resolves to the catalogue entry above.

It is the UN WPP-adjusted variant, not the unadjusted [Population Count][gpw-raw]: those differ
only in values, and their catalogued maxima are 602 380 and 627 597 respectively — the measured
`Largest cell` above is what identifies this copy as the adjusted one.

**The file itself claims none of this.** Its `GDALMetadata` is generic, the name above is this
project's own, and the copy here reached the repository without a recorded download. What ties it to
the dataset named is measurement: the grid, extent, nodata sentinel and maximum in the registry all
match, and nothing else does. Still open is whether it is byte-identical to a fresh download — the
checksum above is of *our* copy, and [Obtaining it](#obtaining-it) is how to get one that is not.

#### Obtaining it

The raster is not in a normal clone. The [`data-v1` release][data-tag] carries it, and that copy needs
no account — its body holds the provenance, the licence and the checksum.

What follows is the other route: obtaining an **independent** copy from the publisher, which is what
makes the `SHA-256` above something more than a record of one download. The whole of it is four
commands, and the last one is the point.

**It needs a free [NASA Earthdata Login][urs-new].** The archive is behind URS OAuth, so an
anonymous request gets a 401 and a redirect rather than the file. A browser download from the
[dataset's granules in Earthdata Search][gpw-search] is the simplest route — pick the 2020, 30
arc-second GeoTIFF granule. For `curl` or `wget`, NASA documents the [cookie and netrc
setup][urs-curl] the redirect needs.

The granule is a ~405 MB zip. Extract just the raster, and rename it to what this repository's
registry, tests and examples expect:

```sh
unzip -j <granule>.zip '*.tif' -d data/population/
mv data/population/gpw_v4_population_count_*_2020_30_sec.tif \
   data/population/population-count-2020-30arcsec.tif
shasum -a 256 data/population/population-count-2020-30arcsec.tif
```

The zip carries one `.tif` per year, so the glob is what selects 2020 rather than an assumption
about the name inside.

**Check the last line against the `SHA-256` in the registry.** A match means the copy this project
measured every figure above from is the copy the archive serves. A mismatch is a finding, not a
broken download: it means the two differ, and the registry — measured from ours — is what would then
need re-measuring. Say so rather than working around it.

`mise run data:pull` fetches the copy already committed here instead, for anyone with access to the
LFS objects; [Fetching](#fetching) covers that path.

#### Licence and attribution

GPWv4.11 is released under [CC BY 4.0][cc-by]. Reuse, including commercial, requires attribution,
so any published map or figure derived from this raster carries the citation:

> Center for International Earth Science Information Network — CIESIN — Columbia University. 2018.
> *Gridded Population of the World, Version 4 (GPWv4): Population Count Adjusted to Match 2015
> Revision of UN WPP Country Totals, Revision 11.* Palisades, NY: NASA Socioeconomic Data and
> Applications Center (SEDAC). <https://doi.org/10.7927/H4PN93PB>

The dataset is also mirrored in the Google Earth Engine catalog as
`CIESIN/GPWv411/GPW_UNWPP-Adjusted_Population_Count`.

[cc-by]: https://creativecommons.org/licenses/by/4.0/
[data-tag]: https://github.com/turboBasic/PopulationCircles2026/releases/tag/data-v1
[gpw-adj]: https://www.earthdata.nasa.gov/data/catalog/sedac-ciesin-sedac-gpwv4-apct-wpp-2015-r11-4.11
[gpw-raw]: https://doi.org/10.7927/H4JW8BX5
[gpw-search]: https://search.earthdata.nasa.gov/search/granules?p=C3540909447-ESDIS
[urs-curl]: https://urs.earthdata.nasa.gov/documentation/for_users/data_access/curl_and_wget
[urs-new]: https://urs.earthdata.nasa.gov/users/new

### `boundaries/coastline-1to110m.geojson`

| Property | Value |
| --- | --- |
| Geometry | 134 LineStrings, 5128 vertices |
| Extent | whole globe, −180° to 180°, −85.609038° to 83.645130° |
| CRS | `urn:ogc:def:crs:OGC:1.3:CRS84`, the file's own declaration (EPSG:4326, axes as longitude then latitude) |
| Size | 136.6 KiB, one line, **not in LFS** |
| SHA-256 | `851f581ff5ffb844deed8ae1a9ce22e3c4bb3d74fa342cadb5d8e39b41ae7c3c` |
| Properties per feature | `featurecla`, `scalerank`, `min_zoom` — none of them read |

Measured from the file. The declared `bbox` reads `180.00000044181` at its eastern edge, four
ten-millionths of a degree past the antimeridian and 5 cm on the ground; the vertices themselves stop
at 180, so nothing here relies on the declaration.

**It is [Natural Earth][ne]'s 1:110m physical coastline**, from the vector distribution repository at
tag **[v5.1.2][ne-tag]**, path `geojson/ne_110m_coastline.geojson` — stored here under the heading
above, which is the only difference between the two. Committed byte-for-byte as that tag serves it,
which is what makes the checksum above something a reader can check rather than a record of one
download; there is no step to obtain it, because every clone has it already:

```sh
curl -sL https://raw.githubusercontent.com/nvkelso/natural-earth-vector/v5.1.2/geojson/ne_110m_coastline.geojson \
  | shasum -a 256
```

GeoJSON rather than the shapefile the same data is published as: `json` needs no reader beyond the
standard library, where a shapefile would put a driver in the dependency tree to draw a coastline
with.

**Natural Earth is in the public domain.** Its [terms of use][ne-terms] place no restriction on use
and ask for no permission, fee or attribution, so a figure drawn over this basemap carries the
raster's citation alone — the one licence here that does require one. Crediting Natural Earth as well
is welcomed by the project and is not done, for the reason the citation's wording is checked at all:
what a figure says about its sources should be what those sources ask for and nothing else.

[ne]: https://www.naturalearthdata.com/downloads/110m-physical-vectors/
[ne-tag]: https://github.com/nvkelso/natural-earth-vector/releases/tag/v5.1.2
[ne-terms]: https://www.naturalearthdata.com/about/terms-of-use/

## Fetching

`.lfsconfig` asks Git LFS to skip the rasters by default, but **a Git config setting overrides
`.lfsconfig`** — so a machine with a global `lfs.fetchexclude` ignores it. Two layers make the
intent hold:

```sh
GIT_LFS_SKIP_SMUDGE=1 git clone <url>   # nothing downloads: the environment wins over all config
mise run setup                          # pins lfs.fetchexclude=* in .git/config, which beats a global setting
```

Then, when the data is actually wanted:

```sh
mise run data:get       # fetch every registered dataset not already here, and verify it
mise run data:pull      # git lfs pull --include='*.tif' --exclude=''
mise run data:status    # size and whether each object is present or pointer-only
```

`data:get` is the one that needs no access to this repository's LFS objects: it reads
[`registry.toml`](registry.toml), checks each file against the `sha256` recorded there **before**
putting it in place, and downloads nothing that is already present and correct. A download that fails
verification leaves nothing behind. On success it prints the attribution each licence requires, which is the moment
a user acquires the obligation.
