# Data

Committed **input** datasets, one directory per kind. Git LFS holds the contents; a clone gets
pointers and fetches deliberately (see [Fetching](#fetching)).

Generated products — summation tables, rendered maps — never live here. They are gitignored
(`*.bin`, `out/`) and are reproducible from these inputs.

```text
data/
  population/   population rasters
  boundaries/   country borders and coastlines (none yet)
```

A new kind gets its own directory and a row in the registry below. Keep names lowercase and
hyphenated, and put the grid resolution in the filename when a dataset comes in several.

## Registry

### `population/gpw-v4-11-unwpp-adjusted-count-2020-30arcsec.tif`

| Property | Value |
| --- | --- |
| Grid | 43200 × 21600 (30 arc-second, 0.008333°) |
| Extent | whole globe, origin (−180°, 90°) |
| CRS | EPSG:4326 (WGS 84) |
| Pixel type | Float32, LZW compressed |
| Nodata | −3.40282306073709653e+38 (Float32, two ulps above −max) |
| Size | 409 MiB |
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
[10.7927/H4PN93PB](https://doi.org/10.7927/H4PN93PB).

It is the UN WPP-adjusted variant, not the unadjusted [Population Count][gpw-raw]: those differ
only in values, and SEDAC catalogues their maxima as 602 380 and 627 597 respectively.

The file itself claims none of this — its `GDALMetadata` is generic and the name it arrived with,
`NASA2020POPDATA.tif`, came from the upstream project. Still open: which SEDAC release this copy
came from, and whether it was modified after download. The checksum above is of our copy; compare it
against a fresh download from the landing page before publishing a result.

#### Licence and attribution

GPWv4.11 is released under [CC BY 4.0][cc-by]. Reuse, including commercial, requires attribution,
so any published map or figure derived from this raster carries the citation:

> Center for International Earth Science Information Network — CIESIN — Columbia University. 2018.
> *Gridded Population of the World, Version 4 (GPWv4): Population Count Adjusted to Match 2015
> Revision of UN WPP Country Totals, Revision 11.* Palisades, NY: NASA Socioeconomic Data and
> Applications Center (SEDAC). <https://doi.org/10.7927/H4PN93PB>

The original download requires a free NASA Earthdata login; the dataset is also mirrored in the
Google Earth Engine catalog as `CIESIN/GPWv411/GPW_UNWPP-Adjusted_Population_Count`.

[cc-by]: https://creativecommons.org/licenses/by/4.0/
[gpw-adj]: https://sedac.ciesin.columbia.edu/data/set/gpw-v4-population-count-adjusted-to-2015-unwpp-country-totals-rev11
[gpw-raw]: https://sedac.ciesin.columbia.edu/data/set/gpw-v4-population-count-rev11

## Fetching

`.lfsconfig` asks Git LFS to skip these files by default, but **a Git config setting overrides
`.lfsconfig`** — so a machine with a global `lfs.fetchexclude` ignores it. Two layers make the
intent hold:

```sh
GIT_LFS_SKIP_SMUDGE=1 git clone <url>   # nothing downloads: the environment wins over all config
mise run setup                          # pins lfs.fetchexclude=* in .git/config, which beats a global setting
```

Then, when the data is actually wanted:

```sh
mise run data:pull      # git lfs pull --include='*.tif' --exclude=''
mise run data:status    # size and whether each object is present or pointer-only
```
