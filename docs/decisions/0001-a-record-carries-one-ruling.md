---
id: 0001
status: accepted
date: 2026-08-15
scope: process
tags: [adr, popcircles]
---

# ADR 0001 — A record carries one ruling and fits one page

## Decision

A change is recorded only when reversing it costs more than a PR, a competent person would have chosen
differently, and nobody could answer "why is it like this?" from the code. A record clearing that bar
carries **one ruling, one `scope:` and at most 80 lines**, rules the constraint rather than its
implementation, and enumerates no schema, field list or layout. What will not fit belongs to the PR, the
issue that measured it, or a present-tense sentence in `docs/ai/`.

Work decomposition leaves this directory with it: the steps live in an issue, and a plan file is scratch
under `tmp/`.

## Context

The trigger for writing a record had become "I am working an issue" rather than "I have hit an
architectural question". But an issue is a work package, and working one forces choices at every level of
impact: which module a struct lives in, which hash function, whether to relax a workspace-wide lint.
Attached to the issue rather than to the question, the whole list of them lands in one file.

**A review on the day this was decided counted 44 rulings across the corpus it replaced, the longest of
them 363 lines**, and almost every record opening by positioning itself against an earlier one — a
symptom of breadth, not carelessness, since that is the correct thing to write when two records overlap.
One existed only because an earlier one had enumerated a field list that proved incomplete, promoting a
version-bumped bug fix to a record. What settles it is the asymmetry: that set recorded the file layout
of `.github/workflows/` and never recorded the choice of two languages.

## Options

### Option 1 (SELECTED): a bar that can answer no, a line ceiling, one ruling each

- Adopted because: the ceiling is the only rule here that intention cannot satisfy — it forces the other
  four or it fails out loud.
- Adopted because: a bar allowed to answer "no" makes a record a decision rather than the default action
  of working an issue.
- Adopted despite: the rulings that fail the bar keep their reasoning only in the PR that made them, a
  comment beside the configuration, or a sentence in `docs/ai/`.
- Adopted despite: a compatibility surface is now read from the code rather than from a record listing
  it.

### Option 2: keep the format and apply it more carefully

- Rejected because: nothing in it fails when ignored, and it was ignored every time while every
  individual ruling in it stayed correctly reasoned.
- Rejected despite: its split of Consequences into positives and costs beats what replaces it at making
  an author argue against their own decision.

### Option 3: freeze the existing corpus beside the new shape

- Rejected because: two shapes in one directory make the rules advisory — the next record over the
  ceiling joins the exempt era rather than failing.
- Rejected despite: an accepted record is frozen, and replacing a corpus wholesale is the edit that
  freezing exists to prevent. It is done once, to open the convention, and not again.

## Consequences

- A ruling wanting a numbered list, a field table or a measurement is refused at the ceiling, and that
  refusal lands somewhere named — a PR, an issue, a `docs/ai/` sentence, a test — or the choice comes
  back.
- `scope:` is closed, so a question no value fits is two questions or a value a record adds deliberately.
- The ceiling, the single ruling and the closed list apply to every record here, are machine-checkable,
  and are gated by nothing (`FU-19`).
- A record is superseded from here on, never replaced: this ruling spent that move opening the
  convention, and spending it again would make the freeze a preference.
- Reopened if a decision arrives that cannot be stated in 80 lines without losing what makes it a
  decision.

## Links

- No issue: this came out of a review of the accepted records, held in conversation.
- `docs/ai/platform.md` "Architecture decisions" holds the bar and the scoping rules; the `write-adr`
  skill holds this shape.
