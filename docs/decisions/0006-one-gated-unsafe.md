---
id: 0006
status: accepted
date: 2026-08-14
scope: safety
tags: [adr, popcircles]
---

# ADR 0006 — The workspace allows exactly one `unsafe` block, gated by a hook

## Decision

`unsafe_code` is `deny` rather than `forbid` across the workspace, so that the summation table can be
memory-mapped. Exactly one `#[allow(unsafe_code)]` may exist in the tree, at the mapping site, and a
hook fails the build if the count or the location changes. A second `unsafe` is a new decision, not a
second exception.

## Context

The table is ~7.5 GB against 16 GiB of RAM, so it cannot be resident, and the search performs
four-corner rectangle queries billions of times. Measured on a 2 GiB payload with a warm page cache:
a mapped query costs **18.6 ns**, a `pread` per corner costs **2 324 ns**. Two orders of magnitude on
the program's hottest operation.

`Mmap::map` is an `unsafe fn` and cannot be otherwise — another process truncating the file
invalidates bytes already borrowed, which no signature prevents. The workspace lint was `forbid`,
which by definition cannot be overridden by an `#[allow]`, so any mapping at all is a change to that
line rather than a local exception to it.

What makes the mapping sound is not that this crate wrote the file — that says nothing about what any
other process does next. It is ADR 0005's rule that artefacts are **published by rename and never
modified in place**. A mapping is of an inode, not a path: `rename` repoints the directory entry at a
different inode while the mapped one stays alive, so a rebuild publishing a fresh table over the same
path leaves a live mapping undisturbed. The two ways a writer could break it — modifying in place, or
truncating — are the two things no writer here does.

The residual is a third party truncating the cache by hand, against which mmap has no defence — a
fault on access rather than a wrong number accepted as right, over a rebuildable artefact.

## Options

### Option 1 (SELECTED): demote to `deny`, gate the count

- Adopted because: it buys the 124× on the operation the whole program is made of.
- Adopted because: the property `forbid` was actually providing — "there is no `unsafe` here" — is
  recoverable as a gate, in the one form that admits the single exception.
- Adopted because: the byte-to-f64 view above the mapping is a checked cast, so no second `unsafe`
  follows the first.
- Adopted despite: **`forbid` will not come back.** Every future crate inherits `deny`, and the hook
  can be deleted in one line by anyone who finds it inconvenient. `forbid` could not be.
- Adopted despite: the failure mode is memory unsafety under a condition no test in this repository
  will ever produce.

### Option 2: keep `forbid`, use `pread`

- Rejected because: 2 324 ns against 18.6 ns, to preserve a lint level rather than a property, when
  the property is recoverable by a hook.
- Rejected despite: it is the only option needing no `unsafe` at all, and was the one to beat.

### Option 3: quarantine in another crate

- Rejected because: Cargo does not merge a crate's `[lints]` table with the workspace's, so a new
  crate would restate the whole lint block — the drift site the workspace table exists to prevent.
- Rejected because: routing around a lint by moving the `unsafe` into a less-reviewed dependency is
  worse than writing one block and gating its count.
- Rejected despite: it is genuinely the tightest containment of the three.

## Consequences

- The single site carries a `// SAFETY:` comment naming the invariant it rests on, including the
  residual it cannot defend against.
- The soundness argument depends on ADR 0005's atomic publication. A change that writes an artefact
  in place breaks this record, not just that one.
- A second `unsafe` — for a different mapping, a FFI call, or a performance trick — reopens this
  record. The hook makes that a build failure rather than a review question.

## Links

- Issue #3 — the mmap-against-`pread` measurement, taken while building the table cache.
