---
id: 0003
status: accepted
date: 2026-08-13
scope: build
tags: [adr, popcircles]
---

# ADR 0003 — The search builds from a mise-pinned toolchain and `cargo fetch` alone

## Decision

The Rust half takes no build prerequisite that is not a crate or a mise-pinned tool. No system
library, no `pkg-config` lookup, no C dependency compiled from source. Raster decoding is therefore
pure Rust, and any future capability wanting a system library is a new decision rather than an
installation instruction.

## Context

The project reads GeoTIFF, and the obvious answer is GDAL. The `gdal` crate is the smaller dependency
tree of the candidates — 13 crates against a trimmed pure-Rust decoder's 11 — and it would answer the
geotransform, the nodata value and every future format in one line. It also fails before compiling
any Rust: `gdal-sys` needs `gdal.pc` on `PKG_CONFIG_PATH`. The alternative it offers, `gdal-src`,
compiles GDAL and PROJ from source in every clean build.

`mise.toml` is meant to be the complete answer to "what does this need installed", and nothing here
is installed globally. A prerequisite outside that file is one no CI job and no clean clone can be
told about. The registry's only raster needs a narrow slice of TIFF — little-endian, single IFD, LZW,
Float32, one sample, one row per strip — which a pure-Rust decoder covers today.

This rules the Rust half only. Python's PROJ arrives through pyproj wheels and is not on the search's
build path.

## Options

### Option 1 (SELECTED): no non-Rust build prerequisite

- Adopted because: a clone compiles and tests on a bare CI runner with `cargo fetch` and the pinned
  toolchain, so `mise.toml` stays true.
- Adopted because: every crate in the decode path is traceable to a tag or an encoding this raster
  actually uses, and no unexercised decode path ships enabled.
- Adopted because: the memory-safety posture of ADR 0006 holds across the decode of 428 MB of
  externally-shaped input; a C library behind `gdal-sys` would sit outside it.
- Adopted despite: no reprojection, no VRT and no format zoo — each becomes a preprocessing step or
  a new decision.
- Adopted despite: a decoder bug is ours to work around, with no upstream that every geospatial tool
  in the world exercises daily.

### Option 2: system GDAL

- Rejected because: it puts a build prerequisite outside `mise.toml`, so a clean clone fails at build
  time with a `pkg-config` error.
- Rejected despite: it is the industry decoder and would settle every format question permanently.

### Option 3: GDAL from source

- Rejected because: it trades a missing prerequisite for a multi-minute build in every CI job and
  every clean clone.

## Consequences

- The decoder is an implementation detail behind an internal seam, so swapping it needs a PR, not a
  record. This record binds the constraint, not the crate.
- Reading tags and validating a geotransform is now this project's code, with its own rejection
  variants and its own opportunity to be subtly wrong.
- **Reopened only when all three hold:** a dataset the project needs cannot be read by any pure-Rust
  decoder; converting it once, offline, into a supported form is not acceptable; and what is wanted
  is GDAL's *library* — reprojection, VRT, resampling — rather than its format support. A decoder bug
  alone is a patch.

## Links

- Issue #2 — the decoder survey and the dependency measurements.
