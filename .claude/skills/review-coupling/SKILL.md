---
name: review-coupling
description: Convene the architect and the product owner in dialog over the dependency structure of planned work - the debt no ticket carries, which blocking edges are real, and what re-cut shortens the flow - each position relayed verbatim, ruled by the owner, then applied to the board. Use when the user asks why a range of milestones is slow, whether its blockers are real, or for a re-cut of how planned work is decomposed.
model: opus
effort: high
---

# Review coupling

**Convening two personas and acting as neither.** [`architect`](../../agents/architect.md) owns whether a
coupling is real and where a boundary belongs; [`product-owner`](../../agents/product-owner.md) owns what ships
and in what order. Neither owns the other's half, and **the findings worth having are where they collide** —
"this coupling is real" met with "and it is not worth a ticket". This session owns the graph, the briefs, the
relay, its own verdict, the writes and the verification, and argues in neither persona's place.

**Every write to the board is the owner's**, executed here after a sign-off. Both personas hold `Bash`, so
withholding `Edit` does not make them read-only against `gh`: each brief says `gh` reads only.

## Before the first brief

1. **Read the graph yourself and put it in every brief** — `gh api repos/:owner/:repo/issues/<n>/dependencies/blocked_by`
   per issue, the sub-issue panels, each issue's band and milestone, each milestone's due date. **Drop every
   edge naming a closed issue**; a discharged edge is dead, and classifying one spends budget on nothing.
2. **Compute what the review is measured against**: the live edge count, the longest chain in nodes, and each
   milestone's cost as a sum of bands. A brief carrying those buys a persona's context for judgment instead
   of discovery.
3. **List what the owner has already settled** — an order, a split, a rejected tool, anything a record or a
   thread closed — as not to be reopened. Otherwise a round goes on re-deriving a decision.

## The rounds

Each round is one subagent context. **Relay every position verbatim and in full before spawning the next**,
quoted rather than summarised. The transcripts are invisible to the owner, so the relay is the deliverable,
and a paraphrase is the one thing that makes the dialog not worth running.

0. **Architect sweeps for debt that no ticket and no register entry carries.** `docs/follow-ups.md` says of
   itself that an entry arrives when work stumbles over one, so a review drawing on tickets alone classifies
   only the debt someone happened to trip over. Each item takes a `file:line`, one sentence, and **a flag for
   whether it bears on a blocking edge** — that flag is what makes this a round of the dialog rather than an
   audit. File nothing: a ticket asserts the work is worth tracking, which is round 2's question.
1. **Architect classifies every live edge** as a genuine technical prerequisite, an artefact of how the
   tickets were written, or a sequencing preference — then proposes the smallest changes removing the most of
   the middle kind, and names any place a high-level goal carries complexity that belongs below it. Round 0's
   edge-bearing items are an input here: one may *be* that smallest change. **A missing edge is as much a
   finding as a spurious one.**
2. **Product owner rebuts every candidate**, round 0's edge-bearing items included, since a debt item
   promoted to an enabler faces the same test as an extraction. Three tests: has it a deliverable of its own;
   what does extracting it cost in milestone shape and count; does it shorten the flow, or move the same work
   earlier and add an issue to track. Rejecting freely is the point — an extraction that only satisfies a
   diagram is not one.
3. **Your verdict, the owner's sign-off, then apply.**
4. **Both personas re-read the board as changed**, told exactly what was applied. Mandatory rather than
   conditional on the change looking large: this is where a misapplied ruling is caught, and after round 0 it
   yields the most.
5. **Final sign-off, apply, verify, report.**

## Your verdict

Before the ask, state your own position: where the two readings collide and which is the better one and why,
what both missed, and what in your own execution is most likely to be wrong. A relay with no verdict leaves
the owner arbitrating two texts alone, and a verdict agreeing with whichever spoke last is not one. It is a
position, not a decision.

## Briefs

- **Name the subject and rule out the neighbouring skills by name.** Each persona is told to invoke a skill
  rather than restate one, so an unbounded brief comes back in the nearest skill's shape.
- **Two overrides every brief carries**, because each cuts against its persona's own file. The architect stops
  at a verdict, so say that proposing the re-cut is part of the subject or round 1 arrives trimmed to a
  classification. The product owner asks the owner directly where an answer changes what ships, so say to
  frame the choice and ask nothing — the sign-off is this session's to put.
- Evidence as `file:line` or a `gh` read, never recollection, and a thread read whole before it is ruled on.
- Cap a position at about 600 words, 400 for round 4, and say so in the brief. It is written for the owner.

## Sign-off

Batch the rulings once per round, three or four questions. **Each option says what changes on the board and
what it costs**, and spells out any term the review coined: the owner chooses from the option text, not from a
transcript they cannot see. Recommend one, and record the loser beside the winner on the thread it belongs to
so nothing is re-proposed blind.

**A rejected question is an unanswered question.** Selections displayed alongside a rejection are not
approval — ask again, reworded.

## Applying

- One surgical edit per body, never a wholesale rewrite: the diff is what shows the change was the ruled one.
- A comment only where scope moved or a ruling reversed, saying what moved, why, and what lost.
- **Re-band an issue that gained boxes**, or say why the band still holds.
- Relationships through the API, per `docs/ai/platform.md` "Relationships are structural, never prose".
- **Then verify.** Re-read every panel and recompute the edge count, the longest chain and each milestone's
  cost. Your own writes are the likeliest defect in the review.

## Budget

Five subagent contexts is the floor — rounds 0, 1, 2 and both halves of round 4 — and six the cap, the spare
spent on a re-ask rather than a second opinion. The frontmatter pins what this session runs at, for the reason
the budget is small: it carries a graph across five contexts, writes the option text a sign-off rests on, and
has to catch its own bad writes.

## What this is not

`housekeeping` audits gates and instruction-layer hygiene and so reaches none of the debt inside the modules.
`review-backlog` reads one milestone's composition through one persona and changes nothing. `groom-milestone`
executes the moves this dialog exists to argue for first. And it is not a workflow script: the rulings
interleave with the rounds, and a script cannot stop for one.
