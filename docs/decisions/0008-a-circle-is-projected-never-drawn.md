---
id: 0008
status: accepted
date: 2026-08-15
scope: rendering
tags: [adr, popcircles]
---

# ADR 0008 — A circle is built in a projection and transformed, never drawn as a ring of coordinates

## Decision

A rendered circle is a buffer constructed in an azimuthal-equidistant projection centred on the
circle and handed to PROJ's polygon transform. It is never a ring of latitudes and longitudes filled
by the plotting library's geodetic transform. The buffer's own vertices and the polygon PROJ returns
are two objects carrying two different assertions, and both are tested.

## Context

This whole program exists because the well-known viral maps of the 50% population circle drew a
circle on an equirectangular image, which is not a circle on the Earth. Getting the rendering wrong in
a different way would be a poor outcome.

The naive approach — sample the circle's boundary as lat/lon points and let the plotting library fill
them — fails in two specific, measurable ways: it fills the *complement* when the circle crosses the
antimeridian, and it fills the *wrong hemisphere* when the circle covers a pole. Both are exactly the
cases this project's search is built to handle correctly, and both produce a plausible map rather than
an error.

## Options

### Option 1 (SELECTED): an AEQD buffer transformed by PROJ

- Adopted because: in the projection centred on the circle, the shape *is* a circle of the stated
  radius, so the geometry is correct by construction rather than by sampling.
- Adopted because: PROJ's polygon transform handles the antimeridian split and the polar case, which
  is what the naive path gets wrong.
- Adopted because: the buffer is an object a test can hold, so the distance claim ("every vertex is
  the radius from the centre") is verified on real vertices before any drawing happens.
- Adopted despite: the drawn polygon carries synthesised vertices, so it can only be asserted
  topologically plus "nothing lies outside the cap" — one assertion cannot cover both objects.
- Adopted despite: it commits the renderer to a projection library rather than to plotting alone.

### Option 2: a ring of lat/lon vertices

- Rejected because: it fills the complement across the antimeridian and the wrong hemisphere over a
  pole — a wrong map that looks like a map.
- Rejected despite: it is what most examples show, needs no projection step, and is correct for a
  small circle far from both seams, which is most of them.

## Consequences

- The failure this record prevents is silent, so it is caught by a test or it is not caught. The two
  seam cases are permanent test fixtures.
- The radius a cap is sized on comes from the document's published earth model (ADR 0004), so no
  Python file names a sphere radius of its own.
- The renderer must refuse an earth model it cannot draw on rather than assuming the number it was
  handed is a radius it understands.

## Links

- Issue #9 — map rendering from results, where both failure modes were reproduced.
