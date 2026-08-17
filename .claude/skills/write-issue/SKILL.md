---
name: write-issue
description: Author an issue or a roadmap epic as product owner - one deliverable, a goal that shows the evidence, acceptance boxes that can fail, decisions named but not settled, and relationships wired through the API. Use when the user asks to open, draft or rewrite an issue, an epic, or a milestone's body.
---

# Write an issue

**Acting as product owner.** The stance, and what it refuses, is
[`product-owner`](../../agents/product-owner.md). The limit this workflow adds: an issue is written, never
worked. Nothing here implements what it describes, and no record is written ahead of the work — a decision
the issue carries is named in it and ruled when the milestone holding it starts.

What the instruction layer already owns, cited rather than restated: `docs/ai/platform.md` "Milestones,
epics and labels" for what an issue and an epic each are and for the label vocabulary; "Relationships are
structural, never prose" for how a relationship is set; "Prose published to GitHub" for how the body is
wrapped.

## Before writing

**Look for the issue that already covers it.** A second issue on one deliverable is worse than a wrong one,
because both stay open and each reads as the whole of the work. Where one exists, edit that body and comment
what changed.

**Name the deliverable in one sentence about a person.** If that sentence will not come, this is not an
issue yet: it is a fragment of one, or a decision, or a follow-up register entry.

## The body

1. **`## Goal` is the story, and it shows its evidence.** What is true now that should not be, with the
   `file:line`, the measured figure or the `gh` read that proves it. Not a task list — the tasks are the
   plan's, and a numbered list of work in a body is what [ADR
   0001](../../../docs/decisions/0001-a-record-carries-one-ruling.md) was written against.
2. **`## Done when` boxes are acceptance criteria that can fail.** A box states a condition someone can
   check and get "no" from. "Consider whether X" cannot fail; "X exists, and a test pins it" can. Include
   the box that catches the regression, not only the boxes that describe the feature.
3. **A box already satisfied is not a box.** Check each against the tree before writing it; where the answer
   is "already true", say so under a heading that is not `## Done when` so nobody re-implements it.
4. **`## Decide in this issue` names a decision without settling it**, with each route and what it costs.
   Where a route would reverse something recorded or ruled, say which, so the cost is visible before anyone
   starts.
5. **Say what the issue is not**, where an adjacent thing would otherwise be assumed in — and name the issue
   or the register entry that holds it.
6. **Labels: one area, one `type:`, one `size:`**, from platform.md's vocabulary. An epic is the exception
   and carries `roadmap` alone.
7. **Wire the relationships**: the parent through the sub-issue API, blocking through the dependency API. No
   `- Blocked by:` line in a body. `- Relates to:` stays prose, because GitHub models nothing for it — and a
   section holding nothing else goes.
8. **Record what would otherwise be rediscovered.** A figure measured while writing, a defect verified in
   the tree, a route weighed and dropped: it belongs in the body or in a comment now, per
   `groom-milestone` "What earns a comment". A finding left in the session that found it is a finding
   re-derived later without the context that made it visible.

## An epic

A roadmap epic is the milestone's body, and it describes **what the milestone contains now** — an epic still
describing what it used to contain is how a milestone acquires a second theme without anyone deciding to.

- One deliverable sentence about a person, first, and the order between its threads if the order is the
  point.
- One paragraph per thread, naming the issue that owns it. Never a checklist of its children: the sub-issue
  panel is that, and a second copy in prose is the one that rots.
- **What left the milestone and why**, where anything did. A reader who remembers an issue being here needs
  to know it moved rather than vanished.
- **What the milestone must not do**, where a gate or an invariant bounds it.
- Where nothing numeric changes, say so — it is the sentence that tells a reader no answer can move.
