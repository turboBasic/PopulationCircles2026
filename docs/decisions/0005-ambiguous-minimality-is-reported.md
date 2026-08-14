---
tags: [adr, code, popcircles]
created: 2026-08-14
decided: 2026-08-14
supersedes: null
superseded_by: null
---

# ADR 0005 - A result that cannot separate two radii reports the span it cannot separate

## Status

Accepted - 2026-08-14.

It supersedes nothing and reopens nothing. The predicate slack is not a record's ruling: it arrived
through issues #6 and #7 and is described in `docs/ai/application.md` "Approach" step 4, whose last
clause — "minimality holds for a target further from a plateau than the summation slack the result
carries" — states the condition this record makes a result answer for itself rather than leaving to a
reader.

The field it adds is **additive under [ADR 0001](0001-cli-and-output-layer.md) decision 3** rather than a
change to that decision's wire format: `report::SCHEMA_VERSION` does not move, and no committed snapshot
changes, so `FU-03`'s tripwire has nothing to fire on.

`FU-09` is the entry this discharges. It **corrects that entry's Fix on a measurement**, in the way
[ADR 0004](0004-diagnostics-through-log.md) corrected `FU-04`'s cost table: the Fix names
`[short, reaching]` as the honest bracket, and the measurement in Context below shows that pair is 2 km
wide where the run's own probes span 1425 km it cannot separate.

## Context

`FU-09` fired when #8 landed: `SmallestDocument` and `SweepDocument` both publish
`predicate_slack_persons`, and nothing acts on it. `crates/popcircles/src/report.rs:505` already promises
what is missing — `SmallestReport`'s own doc comment says the document holds "what the arithmetic beneath
it can and cannot separate" — while the type asserts one `radius_km` whatever that arithmetic can support.

### The ambiguous case is reachable today, with no fixture

Measured 2026-08-14 against the 5 arcmin table in `out/`, whose digest is `0xf17aa802a6890f0c`:

```sh
popcircles-cli smallest-for-share … --share 100 --spacing 32
```

| Figure | Value |
| --- | --- |
| answer | 14 962 km, `covers_whole_grid` **false** |
| `target.persons` | 7 757 982 599.323671 |
| population at 14 962 km | 7 757 982 599.3236**85**, margin **+1.34e-05** |
| population at 14 961 km | 7 757 982 599.3236**68**, margin **−3.81e-06** |
| `predicate_slack_persons` | **0.0120** |

Both margins are inside the slack, by about three orders of magnitude. The two populations differ in
their last few bits and nothing else: they are the same people summed over a different set of rows, so
"14 962 rather than 14 961" is not a distinction the arithmetic makes at all. The document nonetheless
publishes 14 962 as the answer and 0.0120 as the slack, side by side, and says nothing about the two
being incompatible.

This is not the ceiling special case. `covers_whole_grid` is false, so the circle reached the whole total
by covering every populated cell without spanning every row, and step 4's whole-extent shortcut never
fires. Nothing in the result flags it.

**A target of everyone makes this structural rather than a coincidence.** `Target::of` computes
`share × total`, and at a share of one that is `total` exactly. Every radius wide enough to hold everyone
therefore has a margin of pure summation residue, so the comparison that picks between them is deciding on
noise. Issue #18 sweeps ninety-plus countries where a small country's own share of itself is exactly this
case; issue #10 validates against the published 3300 km result, whose interesting shares sit on plateaus
of ocean.

### The pair `FU-09` names is narrower than the ambiguity

The same run's ledger holds 28 radii. Nine of them have populations within the slack of the target:

| Radius | Margin |
| --- | --- |
| 14 960 km | −1.83e-04 |
| 14 961 km | −3.81e-06 |
| 14 962 km | +1.34e-05 |
| 14 964 km | +1.91e-05 |
| 14 968 km | +1.72e-05 |
| 14 976 km | +2.67e-05 |
| 15 104 km | +1.14e-05 |
| 15 360 km | +2.10e-05 |
| 16 384 km | +1.72e-05 |

So the span this run measured and cannot separate is **at least 14 960 to 16 384 km, 1425 km wide**, where
`[short_below, radius_km]` is **2 km**. Publishing the pair as the honest bracket would disclose 0.1% of
the ambiguity it exists to disclose, which is why this record does not implement `FU-09`'s Fix as worded.

The span is a floor and not the interval. The probes above the answer are the climb's, which doubles, so
nothing between 15 360 and 16 384 km was measured and the true interval runs past both ends — at a share of
one it runs to the ceiling, because no radius above the answer can catch anyone new.

### The ordinary case is unaffected

The 50% run in `README.md`, same table and spacing:

| Figure | Value |
| --- | --- |
| answer | 3360 km |
| margin at 3360 km | +174 088 persons |
| margin at 3359 km | −121 814 persons |
| slack | 0.0120 |

Seven orders of magnitude outside the slack on both sides. Whatever this record adds must be absent here,
or it turns the headline result into a hedge.

The committed fixture is separated by the same margin. `the_smallest_document_holds_its_shape`'s snapshot
records 52 635 against a target of 52 569 and a short radius holding 51 510, against a slack of 2.94e-09 —
so **no existing snapshot changes**, and `FU-03`'s condition, which fires on a *modified* snapshot and
exempts an added one, does not fire. That is a property of the fixture rather than a design goal, and it is
why the ambiguous case needs a fixture of its own.

### Two cheaper outs are already closed

- **Shrinking the slack.** `mise run test:fold` measures the fold's real error at magnitude as **exactly
  0** against a derived bound of **0.021839 persons** at a total of 1.117e10, on this tree on 2026-08-14.
  The bound is conservative by orders of magnitude, and tightening it would be a claim about cancellation
  rather than about the arithmetic.
- **A compensated fold in `circle::population`.** It changes the answer's bits, which `search`'s
  determinism tests pin. That is a different record's question and not this one's.

### The ledger cannot be read back

`RadiusLedger` is `get` and `put` (`crates/popcircles/src/smallest.rs:133`), and `()` implements it as a
legal ledger whose `get` always returns `None`. There is no recall over what a ledger holds, and a caller
that wants no resumption has nothing to recall.

## Decision

**1. `smallest` reports whether the arithmetic separated its answer, and never asserts a minimality it
cannot support.** Every radius the search probed has its margin against the target compared with
`predicate_slack_persons`, and the result is unseparated when any of them is inside it. The two margins that
motivate the check are the answer's own population above the target and the short radius's below it, and
they are different worries: a reaching margin inside the slack says the answer may not reach the target at
all, a short margin inside it says a smaller radius may also reach, which is minimality. Both count.

**The flag and the span are one scan, not two.** Restricting the flag to the final pair would give the same
answer — computed population is monotone in radius to within this slack, so a separated final pair leaves no
room for an interior radius to sit nearer the target — but it would give a *narrower* span, because the
1425 km measured above exists only through radii that are neither the answer nor the radius below it. One
scan is what keeps the flag and the ends it publishes from being derived differently.

**2. The span is accumulated over the radii this search visited, not read back from the ledger.** Every
radius passes through `probe`, which is where the comparison against the target happens, so the span costs
one comparison per probe and no new trait method. Reading a ledger instead would fail twice over: `()` is a
legal ledger with no recall, so the field would be empty for the caller that wants none; and a full ledger
holds radii other shares put there, so the same question would get different answers according to what had
been run against the file before it. Accumulating over the visit keeps the result a function of the table,
the share and the spacing — identical on a warm ledger and a cold one, because a warm run visits the same
radii.

**The ceiling radius is not a probed value and contributes nothing to the scan.** `CEILING_KM` is answered by
`Table::whole`'s single query rather than searched, so it never passes through `probe`. That exclusion is
load-bearing rather than incidental: at a share of one the target is `total` bit for bit and the ceiling's
population is `total` bit for bit, so the ceiling's own margin is exactly zero *whenever the ceiling fires*,
and a scan that swept in the returned answer rather than the probed radii would report every
whole-population ceiling result as unseparated — including the ones whose short radius is measurably below
the target and which are therefore decided.

**3. The span is published as a floor on the ambiguity and named as one.** The radii visited are sparse —
the climb doubles — so the ends are the widest pair *measured*, not the interval's. The field says how many
visited radii fell inside, which is what tells a reader the ends are far apart because the probes were, and
its documentation states that the true interval can be wider. A field that named an interval it had not
measured would be the same defect this record is closing, one level up.

**4. The search itself does not change, and neither does the answer's radius.** The predicate stays
`population >= target.persons`, the bits stay what `search`'s determinism tests pin, and `radius_km` stays
the bisection's result — a caller needs one number. What is new is the statement of how much to trust it as
the *minimum*. `tolerance_persons` stays `0.0`: the slack is reported and now bracketed, never applied
inside a comparison.

**5. The field is absent when the answer is separated, and `SCHEMA_VERSION` does not move.** Absent rather
than null, which is `short_below`'s and `TableQueryReport::window`'s convention, so the ordinary result is
byte-identical to today's and a consumer branches on presence. Additive under ADR 0001 decision 3, and
measured additive: every committed snapshot is separated, so none of them changes and the coverage of the
new field is a snapshot added rather than one rewritten.

**6. An unseparated answer is a `warn`, not only a field.** A document is read by a program and a log by a
person, and the person running `--share 100` at the default level should not have to parse JSON to learn the
radius was picked on noise. It is `warn` rather than `info` because it is the one thing in this program that
says an answer is weaker than it looks, and ADR 0004 gave the facade no `warn` call site — this is its
first. The record names the span, and the field stays the machine-readable surface: ADR 0004 decision 4's
split, not a second answer.

## Consequences

**Positive**

- The document stops contradicting itself. `SmallestReport`'s doc comment already claims to publish what
  the arithmetic can and cannot separate, and after this it does.
- #10 and #18 get an answer they can act on. A validation run against a share on an ocean plateau, and a
  per-country sweep at a share of one, both currently receive a single radius with no signal that it was
  picked on noise.
- #9 renders documents whose shape is settled. `FU-09`'s ordering constraint was that its fix changes every
  document carrying `predicate_slack_persons`, so landing it before the renderer is what stops the renderer
  being built twice.
- No cost in the ordinary case: one comparison per probe, of the order of 24 per run, and no field on the
  wire.
- The `RadiusLedger` seam stays two methods wide.
- `warn` acquires the call site ADR 0004 left it without, and it is the right one: the only thing this
  program says that means "the answer is weaker than it looks".

**Negative / costs**

- **A floor is a weaker statement than an interval, and reads like a stronger one.** "At least 14 960 to
  16 384 km" invites being quoted as the ambiguity rather than as its lower bound. The mitigation is naming
  and documentation, which is the weakest kind there is. A caller who wants the interval has to probe for
  it, and nothing here does that for them.
- **The span depends on the spacing and the share through the visit.** Two runs asking different shares of
  one table report different spans over the same underlying flatness, because they probed different radii.
  That is honest — each reports what it measured — but it is not a property of the table, and a reader may
  expect it to be.
- **A share of one now always carries the field.** Every `--share 100` result, and every per-country result
  at a country's own total in #18, will report an ambiguity of hundreds of kilometres. That is true and it
  is the point, but it makes the whole-population answer look far weaker than the 50% answer, and someone
  will read that as a regression rather than as disclosure.
- **No fixture small enough to commit reproduces the case this record is about.** The margins that motivate
  it are summation residue — +1.34e-05 at 14 962 km, from 2160 rows of large partials added in two orders.
  A synthetic table that is ambiguous at all is ambiguous by an **exactly zero** margin, measured on this
  tree 2026-08-14 at both unit magnitude and the 2^40 magnitude `scaled()` uses: a four-cell patch has two
  nonzero row terms and no reordering to round. So a fixture satisfies `|margin| <= slack` trivially, and an
  implementation comparing against zero rather than against the slack would pass every fixture test there
  is. The comparison therefore has to be pinned as a function of two numbers, away from any search, and the
  residue case rests on the registry-raster measurement in Context rather than on a committed test. That is a
  weaker position than a fixture would be and it is the honest one available.
- **The ambiguous fixture is contrived.** Its flatness is planted rather than found, and a fixture written to
  produce a property is weaker evidence for that property than one that happens to have it.
- **A sweep over high shares warns per share.** Decision 6 emits one record per unseparated answer, so
  #18's ninety-country sweep at a share of one warns ninety times and the useful signal is that all of them
  did. A summary at the end would read better and would be a second surface saying what the records already
  say, so this record accepts the repetition rather than adding one.
- **One more field on a payload that is already twelve wide.** `SmallestReport` is near the size at which a
  reader stops reading, and this record adds to it rather than reorganising it.

## Alternatives considered

- **A flag beside the existing `[short_below, radius_km]` pair — `FU-09`'s Fix as written.** The cheapest
  option, one boolean and no accumulation, and the one the register proposed. It lost on the measurement in
  Context: at a share of one that pair is 2 km wide against 1425 km measured, so it would publish a bracket
  narrower than the ambiguity by three orders of magnitude and be *more* misleading than the single radius
  it replaced, because it would carry the authority of a disclosure.
- **Deriving the span from the ledger file.** The obvious source, since the ledger already holds every
  radius and its population. Lost on decision 2's two grounds: `()` is a legal ledger with no recall, and a
  shared ledger would make one share's answer depend on which other shares had been run first — a
  determinism failure in a program whose invariant list opens with determinism.
- **Probing outward until a radius falls outside the slack.** The only option that yields measured ends
  rather than a floor, and the honest ideal. Lost on termination and cost: at a share of one no radius above
  the answer *ever* falls outside the slack, so the walk runs to the ceiling and the ambiguous case — the
  slow one already — becomes slower still, for a bound the caller can reconstruct from the ledger.
- **Applying the slack as a tolerance inside the comparison**, treating within-slack as reaching. Lost
  because it decides what this record exists to report, and it contradicts `tolerance_persons: 0.0`, which
  issue #6 chose deliberately so that a caller reads a result rather than this crate's constants.
- **Shrinking the slack, or a compensated fold.** Both closed in Context, the first by measurement and the
  second as another record's question.
- **Leaving `FU-09` open and letting #10 discover it.** Genuinely arguable: the ambiguity bites only at a
  share of one or on a plateau, and neither #9 nor the README's headline run touches either. It lost on the
  ordering `FU-09` itself records — the fix changes every document carrying the slack, so discovering it
  after #9 means rebuilding the renderer against a changed shape.
