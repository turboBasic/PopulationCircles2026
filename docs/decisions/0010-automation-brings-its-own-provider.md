---
id: 0010
status: accepted
date: 2026-08-17
scope: ci
tags: [adr, popcircles]
---

# ADR 0010 — An automated contributor runs in this repository's own CI, on a model provider this repository holds the key for

## Decision

Work this repository has done for it by an automated contributor runs **in its own CI, driven by a model
provider it holds a key for**. The provider and the model are configuration this repository owns and can
change without anyone's permission.

It is not delegated to a hosted agent whose inference belongs to the platform. What the platform supplies
is the runner, the trigger and the identity; what it does not supply is the model.

## Context

Every agent the platform runs inside a repository is priced as a per-seat subscription and answers from
the platform's own inference, so the provider and the model are the vendor's choice rather than this
repository's. Bring-your-own-key exists but is documented only for surfaces on a developer's own machine —
editors, a local CLI, an SDK — and not for anything the platform runs on a repository's behalf. There is
no setting that changes that, so paying the seat and choosing the model are not separable.

A key for a metered provider already exists here, billed by the token with no seat attached.

## Options

### Option 1 (SELECTED): the agent runs in this repository's CI on its own provider key

- Adopted because: the model becomes a decision this repository makes and revisits per run, at the cost of
  an input, rather than one a subscription makes for it.
- Adopted because: cost is metered by use, so an automated contributor nobody invokes costs nothing.
- Adopted despite: a credential that spends real money is now present in CI, which makes the gate on who
  may trigger a run a security boundary rather than a convenience.
- Adopted despite: nothing about the quality of what it produces is anyone's guarantee, where a vendor's
  hosted agent is at least a vendor's problem.

### Option 2: a hosted agent on a per-seat subscription

- Rejected because: it prices a capability by the month that this repository would use in bursts, and
  fixes the provider and the model outside its control.
- Rejected despite: no credential of ours in CI, no identity of ours to manage, and a supported product
  with someone to complain to.

### Option 3: an agent only at a developer's terminal, never on the platform

- Rejected because: it cannot act on work that arrives while nobody is at a terminal, which is the whole
  of what this adds — an issue is where the work appears and a pull request is where it lands.
- Rejected despite: this is the status quo, it costs nothing new, and it keeps every credential local.
  It remains how most work here is done; this record adds a second route rather than replacing it.

## Consequences

- **An identity that is not a person holds write access to this repository**, and it authors what it
  produces. Its permission set is therefore part of the design and is asserted rather than assumed.
- **That identity has to be distinct from CI's own token.** The platform starts no workflow run for a pull
  request opened by the token it issues to a workflow, so required checks would never report and the
  result could be merged by nobody. This is forced, not chosen, which is why it is a consequence here and
  not a second ruling.
- **The trigger is a security boundary.** In a public repository anyone can ask; only whoever may spend the
  key should be answered, and loosening that is a decision to argue for rather than a default to drift
  into.
- **Cost follows what an agent reads, not what it was asked.** Nothing in this repository bounds it — a
  ceiling, if one is wanted, belongs on the provider's side where it cannot be edited away by the thing
  it limits.
- The provider and the model may be replaced without a record, being configuration. Replacing *this* —
  going back to a seat — supersedes it.

## Links

- Issue #108 — where the seat-versus-key comparison and the required-check behaviour were established.
