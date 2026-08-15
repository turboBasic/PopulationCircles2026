---
id: 0007
status: accepted
date: 2026-08-14
scope: output
tags: [adr, popcircles]
---

# ADR 0007 — A result states the uncertainty of its own minimality rather than asserting it

## Decision

When the summation arithmetic cannot separate a probed radius from the target population, the result
says so and publishes the span of radii it could not separate, named as a floor on the ambiguity
rather than as the interval itself. The answer's radius does not change and the search does not
change; what changes is that the document no longer claims a minimality it has not proved.

## Context

The program's headline output is "the smallest circle containing N% of the world's population". A
circle's population is a sum over one rectangle query per row, so it carries a slack, and near a
plateau — which is exactly where the interesting answers sit — that slack can exceed the difference
between adjacent radii. The search still returns a number; the question is whether the document is
entitled to call it the minimum.

A measured run makes the size of the problem concrete: the honest bracket suggested by the earlier
follow-up entry was the final pair of radii, 2 km apart, while the run's own probes spanned **1425 km**
that the slack could not separate. Reporting the final pair would have been a narrower and more
confident-looking claim than the evidence supports, which is the same defect one level up.

## Options

### Option 1 (SELECTED): publish the unseparated span

- Adopted because: an answer weaker than it looks is the one thing a consumer cannot detect for
  itself, and the program is the only party that knows.
- Adopted because: naming it a floor is honest about sparse probing — the search doubles before it
  bisects, so radii between two probes were never measured and the true interval can be wider.
- Adopted because: the field is absent when the answer is separated, so the ordinary result is
  unchanged and the schema version does not move.
- Adopted despite: a headline result now sometimes comes with a caveat wide enough to embarrass it,
  which is a product cost taken deliberately.
- Adopted despite: it publishes a property of the arithmetic, which is a detail a contract would
  usually hide.

### Option 2: report the answer alone

- Rejected because: the document would assert minimality on a comparison that did not establish it,
  and nothing downstream could tell the difference.
- Rejected despite: it is simpler, and every result committed so far happens to be separated.

### Option 3: apply the slack inside the comparison

- Rejected because: it changes the answer to buy a claim, and the answer's determinism is pinned by
  tests for good reasons.
- Rejected because: a tolerance folded into a predicate is invisible; a reported span is not.

## Consequences

- The reported tolerance stays exactly zero. Slack is reported, never applied.
- An unseparated answer is also worth saying out loud to whoever ran the command — a person at a
  terminal should not have to parse JSON to learn the radius was picked on noise.
- Any future refinement of the search's arithmetic narrows the span rather than removing the field.
  The field is not a temporary state of the implementation.

## Links

- `FU-09` in `docs/follow-ups.md` — where the slack was published and nothing yet acted on it.
