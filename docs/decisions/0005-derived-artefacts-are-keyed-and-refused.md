---
id: 0005
status: accepted
date: 2026-08-14
scope: caching
tags: [adr, popcircles]
---

# ADR 0005 — A derived artefact is keyed on the data it was derived from, and refused rather than migrated

## Decision

Anything this program caches on disk records enough about its inputs that a reader can prove it was
derived from the same data and the same parameters, and refuses it otherwise. The key is a digest of
the **decoded values**, not of the source file. A cache is never migrated and never repaired: it is
rejected and rebuilt.

**Publication is by rename**: written to a temporary name and renamed into place, with a file already
in place never modified. So a reader never sees a partial artefact, and a rebuild never disturbs one
already open.

## Context

The summation table is ~7.5 GB derived from 933 million cells, and every later step reads it billions
of times. Silently reusing one built from different data does not fail — it returns a plausible wrong
answer, which is the failure mode this project cares most about.

Two things make refusal the cheap option rather than the strict one. A full-resolution table rebuilds
from the raster in about 15 seconds, so nothing a migration could save is worth the code that would
save it. And the prior art's cache format — two host-endian integers and then the payload, no version,
no checksum, no declared byte order — is a demonstration of the alternative.

Keying on decoded values rather than the source file matters for a reason easy to miss: the search
will later read decimated and masked sources, which have no file of their own. A file checksum cannot
answer "was this built from these cells" for any of them, and that is the only question a cache asks.

## Options

### Option 1 (SELECTED): key on decoded values; refuse and rebuild

- Adopted because: it is the only key a decimated or masked source can also answer.
- Adopted because: refusal has no failure mode — the worst case is 15 seconds — while a migration's
  worst case is a wrong answer nobody notices.
- Adopted because: atomic publication is what makes the artefact safe to memory-map (ADR 0006); the
  two rulings are one property seen from two sides.
- Adopted despite: a parameter that is not in the key is a silent-reuse bug waiting to happen, and
  the only defence is that the key is reviewed whenever a parameter is added.
- Adopted despite: publication by rename needs a temporary file, so a build wants twice the
  artefact's size free.

### Option 2: key on the source file's checksum

- Rejected because: it answers "is this the file from the provider" when the question is "was this
  built from these cells", and the two diverge the moment a source is derived rather than downloaded.
- Rejected because: it requires reading the file a second time, beside a reader that is already
  streaming it.
- Rejected despite: the registry already records that checksum, so it is free to compare.

### Option 3: version and migrate

- Rejected because: a migration is code that runs rarely, is tested least, and whose bugs surface as
  wrong numbers rather than errors.
- Rejected despite: it is the right answer for an artefact expensive to rebuild, which this is not.

## Consequences

- A parameter that changes what an artefact contains must join its key in the same change. A key
  missing a field is a **bug** — refused artefacts are the design, wrongly-accepted ones are the
  defect — and it is fixed by a PR with a version bump, not by a record.
- Artefacts are local and derived. Nothing here promises they travel between machines, and the
  document (ADR 0004) is the only thing that does.
- A refusal must name which ground fired. "Not the document this format is" for every possible
  mismatch would make the whole mechanism decoration.
- Reopened if an artefact appears whose rebuild cost is minutes rather than seconds, or if a cache
  ever has to be published rather than built.

## Links

- Issue #3 — the table cache and its measurements; issue #45 — the geometry its key was missing.
