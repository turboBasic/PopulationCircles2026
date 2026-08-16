---
id: 0009
status: accepted
date: 2026-08-16
scope: data
tags: [adr, popcircles]
---

# ADR 0009 — Input data too large to carry is published and verified, never versioned

## Decision

An input dataset too large for every clone to carry reaches a working copy by **fetch and
verification from a publicly readable location**, and the repository carries its description rather
than its bytes. The description is machine-readable and is what a fetch reads: where the file comes
from, what it must hash to, and what its licence obliges. Verification happens before the file is put
in place, so a working copy holds the dataset or nothing.

An anonymous request has to be enough. A dataset whose only route needs an account is not published
for this purpose, whatever else it is.

## Context

The population raster is 428 MB and most work here never reads it. It arrived in Git LFS, which made
a clone's default a choice between paying bandwidth and a two-layer opt-out that a user's own Git
config can defeat. The documented alternative was worse: an account with the publisher, a browser
download, an unzip and a rename, with no command able to do any of it.

Both routes share the defect this rules out — the repository held the bytes, so the cost of having
them scaled with the number of clones rather than the number of people who wanted the data.

A licence is what makes publishing available at all. This raster is CC BY 4.0, so redistribution is
permitted with attribution; a dataset whose licence forbade it would fall outside this ruling and
need its own.

## Options

### Option 1 (SELECTED): published for anonymous fetch, verified before use

- Adopted because: the bytes leave the repository, so a clone costs what the code costs and fetching
  is a decision made once, by whoever needs the data.
- Adopted because: a checksum in the description turns "the download worked" into a claim that can
  fail, which is what a truncated or substituted file needs.
- Adopted because: one command can then do the whole of it, which neither prior route offered.
- Adopted despite: the published copy becomes something this project maintains, and a dataset removed
  from where it is published breaks every fresh working copy.
- Adopted despite: each dataset now has two descriptions in the tree, one for a machine and one for a
  person, and they can drift.

### Option 2: Git LFS

- Rejected because: bandwidth is charged against the repository, so the cost tracks clones and CI
  runs rather than people who wanted the data.
- Rejected because: skipping is a layered default and not a guarantee — a user's Git config overrides
  the committed one, so the cost cannot be reasoned about from what is committed.
- Rejected despite: it needs no publishing step and already addresses content by digest.

### Option 3: an object store this project owns

- Rejected because: it costs an account, a bucket whose permissions are a standing liability, and
  credentials someone has to hold, for a file that changes approximately never.
- Rejected despite: it would decouple the data's lifetime from the repository's host entirely.

## Consequences

- **A published dataset cannot be unpublished.** Whoever fetched it has it, and a moved or deleted
  asset breaks fresh working copies and not merely future ones, so publishing is deliberate.
- The attribution a licence requires travels with the fetch, because acquiring the bytes is when the
  obligation is acquired.
- A dataset small enough that fetching costs more than carrying stays a committed blob. Which side of
  that line one falls is stated per dataset rather than derived from where it sits.
- ADR 0005 rules what this project derives and caches; this rules what it reads. Both refuse an
  artefact rather than trusting it, and neither migrates one.

## Links

- Issue #57 — publishing the raster, and the command that fetches and verifies it.
- Issue #85 — removing the LFS route this replaces.
