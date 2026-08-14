---
tags: [adr, code, popcircles]
created: 2026-08-13
decided: 2026-08-13
supersedes: null
superseded_by: null
---

# ADR 0002 - The build depends on no system GDAL; raster decoding is the pure-Rust `tiff` crate

## Status

Accepted - 2026-08-13. It supersedes nothing.

It extends [ADR 0001](0001-cli-and-output-layer.md) along the axis that record opened: 0001 ruled
which crate may hold which dependency, and this one rules what the library's first non-`serde`
dependency is. Issue #2 asks for "a pure-Rust decoder so the build needs no system GDAL" and asks
that the fallback be documented "if one becomes necessary"; a fallback condition nobody can evaluate
later is not documented, so naming one in a form a reader can test is the substance of this record.

## Context

Nothing in the tree does I/O. Issue #2 is the change that adds the first reader, so the decoder is
chosen once here for every raster this project will read.

The registry's only raster, `data/population/gpw-v4-11-unwpp-adjusted-count-2020-30arcsec.tif`, was
dumped with `tiffinfo -D` on 2026-08-13 at the checksum its `data/README.md` row records. Its
**TIFF structure** — which the registry does not carry, and which decides what a decoder has to cover
— is a classic little-endian TIFF, single IFD, LZW with `Predictor` 1, `SampleFormat` 3 (IEEEFP) at
32 bits, `SamplesPerPixel` 1, `PlanarConfig` 1, and `RowsPerStrip` 1. Every tag the reader must read,
and how `tiff` 0.11.3 exposes it:

| Needed | `tiff` 0.11.3 |
| --- | --- |
| dimensions, `SamplesPerPixel`, `PlanarConfig` | `Decoder::dimensions`, `Tag::SamplesPerPixel`, `Tag::PlanarConfiguration` |
| `SampleFormat` / `BitsPerSample` | `Decoder::colortype`, and the `DecodingResult::F32` a chunk decodes into |
| strip layout, ragged last strip | `Decoder::chunk_type`, `chunk_data_dimensions`, `read_chunk` |
| `ModelPixelScale` (33550), `ModelTiepoint` (33922) | `Tag::ModelPixelScaleTag`, `Tag::ModelTiepointTag` |
| `GeoKeyDirectory` (34735) | `Tag::GeoKeyDirectoryTag` |
| `GDAL_NODATA` (42113) | `Tag::GdalNodata` |
| `ModelTransformation` (34264), to reject | `Tag::ModelTransformationTag` |

That table is the coverage claim: no tag this reader needs arrives as `Tag::Unknown`, and the
geodetic keys the file also carries — `GeogAngularUnits` 9102, `GeogSemiMajorAxis`,
`GeogInvFlattening` — are read past rather than needed, because the earth model is `geodesy.rs`'s
sphere.

Dependency cost, measured 2026-08-13 with `cargo add` and `cargo tree -e normal` in scratch projects
outside this tree, counting crates excluding the probe itself:

| Addition | Crates | System library |
| --- | --- | --- |
| `tiff` 0.11.3, defaults on | 21 | none |
| `tiff` 0.11.3, `default-features = false`, `features = ["lzw"]` | 11 | none |
| `gdal` 0.19.0 | 13 | libgdal, via `gdal-sys` |

The 21-against-11 gap is `deflate`, `fax` and `jpeg`, which pull `flate2` (with `crc32fast`,
`miniz_oxide`, `adler2`, `simd-adler32`), `fax` and `zune-jpeg` (with `zune-core`). A Float32
population raster can contain none of those encodings. What remains after trimming is `weezl` for
LZW, `quick-error`, and `half` with `zerocopy` for the f16 arm of `DecodingResult` — unconditional,
and the price of the crate's single result type.

`gdal`'s crate count is the smaller number and the misleading one. `cargo build` on that probe failed
here, before compiling a line of Rust:

```text
The system library `gdal` required by crate `gdal-sys` was not found.
The file `gdal.pc` needs to be installed and the PKG_CONFIG_PATH environment variable must contain
its parent directory.
```

That is the whole difference. `mise.toml` pins every tool this project needs and installs nothing
globally; libgdal is not a crate, not a mise-managed tool, and not something `cargo fetch` acquires,
so taking it would put a build prerequisite outside the one file that is meant to hold them all. The
`gdal-src` feature answers that by compiling GDAL and PROJ from source, which trades the missing
prerequisite for a multi-minute build in every CI job and every clean clone.

## Decision

**The build depends on no system GDAL. Raster decoding is `tiff` 0.11 with
`default-features = false, features = ["lzw"]`, and `cargo fetch` plus a mise-pinned toolchain remain
the whole of what a clone needs to compile and test.**

The trimmed feature set is part of the ruling, not tidiness: a default-on `deflate` or `jpeg` is a
decoder path this project never exercises and never tests, reachable from a malformed header.
Widening it takes a registry row for a dataset that needs it.

`tiff`'s default `Limits` stand. The temptation is `Limits::unlimited()`; the default 1 MB IFD value
cap comfortably admits this file's 86 KB `StripOffsets` array and the 256 MB decode cap admits its
173 KB strips, so unlimited would buy nothing and would remove the guard that stops a malformed
header from asking for gigabytes.

**The fallback condition.** System GDAL comes back only when all three of these hold, and each is
meant to be answerable from the tree rather than argued:

1. **A dataset the project needs is one `tiff` cannot read.** Concretely: it is a row in
   `data/README.md`'s registry, or a candidate for one, and `tiff` returns an error or wrong values on
   it. Tiled layout, BigTIFF and an unsupported compression are not this condition on their own —
   `tiff` handles the first two and the third is a feature flag.
2. **Converting it once is not an acceptable answer.** The cheap route is a one-off conversion to a
   striped Float32 GeoTIFF with GDAL as a *command*, the converted file getting its own registry row
   and checksum. That keeps GDAL out of the build entirely, and it fails only when the conversion
   would lose something the search needs or has to be repeated per run.
3. **What is wanted is GDAL's library, not its file format support.** Reprojection, VRT, on-the-fly
   resampling, or reading a format zoo. `application.md` "Out of scope" for #2 puts reprojection and
   resampling outside this reader deliberately, so this clause becomes true only if a later issue
   rules them back in.

A decoder bug alone is not the condition. It is a patch or a narrow workaround in
`raster/geotiff.rs`, and if it cannot be worked around it becomes clause 1.

## Consequences

**Positive**

- A clone compiles and tests with `cargo fetch` and the mise-pinned toolchain, on a CI runner with no
  system packages. `mise.toml` stays the complete answer to "what does this need installed".
- The dependency surface is 11 crates, each traceable to a tag or an encoding this raster uses. No
  unexercised decode path ships enabled.
- `unsafe_code = "forbid"` holds across the reader. A C library behind `gdal-sys` puts the decode of
  428 MB of untrusted-shaped input outside the guarantee the workspace lint is there to make.
- Swapping the decoder later stays cheap, because it is confined to `raster/geotiff.rs` and named in
  no public type: the trait's error carries the semantic rejections plus one opaque `Decode` variant,
  so `tiff::TiffError` never reaches a consumer's `match`.
- The fallback has a condition instead of a sentiment, which is what makes it possible to say "no,
  not yet" to it twice and "yes" the third time for a stated reason.

**Negative / costs**

- No reprojection, no VRT, no format zoo. Every one of those becomes either a preprocessing step
  outside the build or a new decision, and the second time one is wanted this record will look like
  the reason the project had to do the work twice.
- A `tiff` bug is ours to work around. GDAL's decoder is the one every geospatial tool in the world
  exercises daily; `tiff` is a good crate with a much smaller blast radius of use, and a wrong value
  from it on some file we have not tried yet is a failure mode with no upstream to lean on.
- Reading a raster's tags and validating a geotransform is now this project's code. GDAL hands over a
  geotransform and a nodata value already normalised; here that is a module with its own rejection
  variants, its own fixtures, and its own opportunity to be subtly wrong.
- The trimmed feature set is a guess about future inputs. A deflate-compressed GPW variant is a
  plausible download away, and then the two lines in `Cargo.toml` change and the "never exercised"
  argument above weakens.
- `half` and `zerocopy` arrive for an f16 arm this project will never decode. Ten crates is small, but
  it is not the zero the trimming implies.
- Choosing before the reader exists. The coverage table above is a tag dump, not a working decode of
  933 120 000 cells; #2's phase 4 is what turns it into a fact, and until that test runs this record
  rests on a claim about an API surface.

## Alternatives considered

- **The `gdal` crate, 0.19.0.** The industry decoder, and it would answer the geotransform, the
  nodata value and any future format in one dependency. It lost on the build prerequisite measured
  above: `gdal-sys` needs libgdal from `pkg-config`, which is not a crate and not mise-pinned, so a
  clone or a CI runner without it fails at build time — and the alternative to that, `gdal-src`,
  compiles GDAL and PROJ from source in every clean build.
- **`tiff` with default features.** One less line in `Cargo.toml`. It lost on the 21-against-11
  measurement: three encodings this project cannot encounter, each an enabled decode path with no
  test behind it.
- **`Limits::unlimited()`.** The reflex when a 428 MB file is involved. It lost because the file's
  actual demands — 86 KB of strip offsets, 173 KB strips — sit far inside the defaults, so the only
  thing unlimited would change is what a malformed header can ask for.
- **A dedicated GeoTIFF crate: `geotiff` 0.1.0, or `geotiff-reader` 0.8.1.** Nominally the exact shape
  of the problem, and both are pure Rust. Measured the same way as the table above, they cost 31 and
  47 crates — the first brings its own TIFF decoder plus `geo-types` and `num_enum`, the second brings
  `ndarray` and `flate2`. Neither removes the work this issue is actually about: the geotransform still
  has to be checked against a declared `Grid`, the nodata sentinel still has to be compared bit-exactly
  against a declared one, and the row-at-a-time streaming shape with tallies is ours either way. So the
  choice was three to four times the dependency surface for a layer over the tags, and the tags are
  what this reader wants.
- **Shelling out to `gdal_translate` at run time.** Keeps the build pure and gets GDAL's format
  support. It lost because it makes a system binary a run-time prerequisite instead of a build-time
  one, which is worse: the failure moves from a clean compile error to the middle of a long search.
  It survives as clause 2 of the fallback, where it is a deliberate one-off, not a run-time
  dependency.
- **Writing the TIFF decode by hand, LZW included.** No dependency at all, and the file's structure is
  simple enough that it is genuinely feasible — phase 2's fixture writer emits TIFF bytes from the
  specification, so half the work happens either way. It lost on the asymmetry between writing a file
  we control and reading files we do not: an LZW decoder is exactly the kind of code where `unsafe`,
  or a subtle bounds bug, arrives under pressure.
