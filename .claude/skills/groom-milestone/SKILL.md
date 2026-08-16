---
name: groom-milestone
description: Reshape a milestone and the issues in it as product owner - derive its deliverable from the person it serves rather than inheriting the issue bodies, cut what has no deliverable of its own, and execute the moves. Use when the user asks to review, reshape or re-plan a milestone, its scope, or the issues inside it, or asks you to act as product owner over the backlog.
---

# Groom a milestone

`docs/ai/platform.md` "Issues" owns how an issue is worked, and `docs/ai/platform.md`
"Milestones, epics and labels" what a milestone is. This skill owns the opposite direction: concluding
that the issues in hand are the wrong issues, and changing them.

**An issue body is an opening position, not a specification.** Bodies here were drafted quickly and
approved in bulk, and one may describe work nobody needs. Reading them as settled requirements is how a
milestone acquires a phantom deliverable that then gets built.

## Work as product owner

The role is what licenses overruling an issue body, and it also bounds what may be overruled.

- **It owns what ships and in what order:** a milestone's composition, an issue's scope, closing one,
  splitting or merging two, and the labels that balance them against each other. An issue is evidence of
  intent, not a contract.
- **It does not own the non-negotiables** in `docs/ai-instructions.md`, an architecture ruling, or an
  implementation choice inside an issue. A discovery about any of those is a proposal to raise, not a
  decision to take.
- **Where an answer changes what ships, ask rather than assume** — hosting, a name a user will type, a
  publishing channel, the order two issues land in. Frame the choice with a recommendation and with what
  the rejected option costs, then record the loser beside the winner so nobody reopens it blind.

## Steps

1. **State the milestone's deliverable as one sentence about a person**: who can do what afterwards that
   they cannot do now. Where the title describes only some of its issues, that mismatch is the finding —
   a milestone carrying two themes has two customers and no legible deliverable.
2. **Derive the work from what that person's walk consumes.** Write the walk as the commands they run,
   then ask of each step what it actually reads. A field nothing in the walk consumes belongs to a later
   milestone even when an issue in this one lists it.
3. **Sort every acceptance box into one of four kinds**, checked against the tree rather than recalled: a
   one-time data or document edit; code someone runs; machinery whose only job is holding two copies in
   agreement; or already satisfied by something committed. The last two are where a milestone shrinks,
   and machinery to hold copies in step is nearly always avoidable by deleting one of the copies.
4. **Test each issue for a deliverable of its own.** One whose consumers all live in a later milestone,
   or whose only visible surface exists to demonstrate an internal component, is not an issue. Close it
   and re-home its parts — no scope is lost if the closing comment names where each part went.
5. **Verify before believing a box.** Measure the figure, read the config, run the gate. A box asserting
   that some check exists is wrong often enough to be worth checking every time.
6. **Land decisions in the threads, and land the content rather than a description of it.** A draft file,
   a measured number, a schema: paste it in. A scratch file under `tmp/` is lost by the next session, and
   whoever picks the issue up re-invents what it held.

Weigh a proposal against the smallest thing that serves the person in step 1, not against the issue it
replaces. An issue that shrinks to a paragraph, or to nothing, is this skill working.

## What earns a comment

platform.md says the comments are where scope was cut, a figure settled or a step reordered. The converse
is this skill's: **a comment narrating backlog mechanics is noise, and future agents read it as context.**

- **Delete:** milestone moves, "the link was dead", "renamed to", any before-and-after account of how an
  issue changed. `git log` and the body's current state own that, per
  `docs/ai-instructions.md` "Working style".
- **Keep:** a scope cut, a measured figure, a dependency order, a draft artefact, a ruling and what lost
  to it.
- **Trim rather than delete** where a real kernel sits wrapped in that narration.
- A comment answering a scope proposal above it earns its place: without one, nobody can tell whether the
  proposal was adopted or is still open.

## Prose in issues and comments

**GitHub renders a single newline inside a paragraph as a line break**, so the repository's hard wrapping
at about 110 columns reads as ragged breaks mid-sentence. In issue bodies and comments:

- One paragraph, one line. One list item, one line, however long it runs.
- A blank line between blocks. A heading, a table row, a horizontal rule and a fenced block keep their own
  lines, and fenced content is never reflowed.
- Consecutive blockquote lines join too — a hard break reads no better inside a quote.

Fixing a milestone's worth of these is a mechanical transformation. Write it when it is needed rather than
carrying a script around for it.

## Executing a renumber

- **Retitle milestones from the highest number downwards**, so no two ever collide on a title.
- **An epic follows its theme, not its number.** A milestone whose content is unchanged keeps its epic and
  takes a title edit; only a theme that moves needs a body rewritten or a new epic opened.
- **Closed issues stay where they are.** They ship in that release whichever milestone labels them, so
  moving them would claim otherwise. Say so in the epic body rather than leaving the mismatch unexplained.
- **Re-parent sub-issues explicitly**, detaching from the old epic and attaching to the new. Changing an
  issue's milestone does not move its parent link.
- **Sweep for pointers to anything deleted.** A comment whose whole content was a link to a deleted
  comment is now noise itself.
- **Repoint the dependency graph out of a closed issue** before finishing. An issue closed as superseded
  never closes as done, so a dependency on it blocks its successors for good — and the graph is where that
  hides, since `docs/ai/platform.md` "Relationships are structural, never prose" keeps it out of the
  bodies.
