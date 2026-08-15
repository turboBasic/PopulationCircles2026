---
id: 0004
status: accepted
date: 2026-08-13
scope: contract
tags: [adr, popcircles]
---

# ADR 0004 — The published document is the only contract, and the only thing promised across releases

## Decision

The library owns a versioned result document, distinct from the domain types, and it is the sole
interface any consumer may depend on. It is extended additively; a change that is not additive bumps
its schema version. Everything else this program writes — caches, ledgers, log lines, exit-code
detail — is internal and may change in any release without notice.

## Context

The serialised result has four parties: two command surfaces produce it and two renderers consume it,
one pair in each language. `application.md` already calls it "the contract between the two halves". A
contract with four parties and no owner is what this record prevents.

Two candidate owners existed. The domain types are the wrong one: `Grid`, `LatLon` and `Row` change
when the search changes, and a format that moves with them is not a contract. The CLI is also the
wrong one — the renderers are downstream of *results*, not of an argument parser, so defining the
format in the binary would make a Python figure depend on clap's crate for its schema.

The second half of the ruling only became answerable once binaries shipped. Two on-disk shapes carry
version numbers, and until something was distributed both were internal hygiene. Someone has to say
which of them a downstream consumer is entitled to rely on.

## Options

### Option 1 (SELECTED): a library-owned versioned document

- Adopted because: one owner for four parties, and a renderer that needs a field has one file to
  change.
- Adopted because: snapshot tests over the format are library tests, so they do not need the argument
  parser to run.
- Adopted because: promising exactly one shape means a cache can be refused and rebuilt rather than
  migrated, which is the freedom ADR 0005 spends.
- Adopted despite: every published field is written twice — once in the domain type, once in the
  document type and its conversion — and the compiler cannot tell you the two have drifted in
  meaning.
- Adopted despite: `serde` in the library, which is a dependency the pure-geometry crate would
  otherwise not carry.

### Option 2: the CLI owns the output shape

- Rejected because: it makes the renderers depend on the binary crate for their schema, reversing the
  dependency arrow ADR 0002 draws.
- Rejected because: a second command surface would invent a second output shape with nothing to stop
  it.
- Rejected despite: it keeps the format next to the thing that writes it, and keeps `serde` out of the
  library.

### Option 3: serialise the domain types

- Rejected because: the wire format would then change every time the search's internals do, which is
  the definition of not being a contract.
- Rejected despite: zero duplication, and no conversion layer to keep in step.

## Consequences

- A renderer that wants something must have it published. Reaching past the document is an
  architecture violation, not an optimisation.
- Adding a field is a PR. Changing or removing one is a version bump and a note in the release.
- Cache and ledger formats may change silently — so a tool that declines to reuse a 7.5 GB file it
  wrote last month must say why, or it reads as a bug.
- This record deliberately states no field, no key and no layout. Those belong to the code and its
  version constant.

## Links

- Issue #8 — the versioned output surface the CLI publishes.
- Issue #28 — what a release promises, which is where the second half was settled.
