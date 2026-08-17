---
name: groom-milestone
description: Execute a reading of the backlog as product owner - move, split, cut, resize and renumber the issues a review concluded were the wrong issues, and leave the record of what moved. Use when the user asks to reshape or re-plan a milestone, its scope, or the issues inside it, or to act on a scope review already in hand.
---

# Groom a milestone

**Acting as product owner.** The stance, and what it refuses, is
[`product-owner`](../../agents/product-owner.md): it owns a milestone's composition and an issue's scope, and
it never touches `crates/` or `python/` — a milestone is reshaped by changing issues, never by changing the
code they describe.

**This skill executes a reading; it does not produce one.** The reading is `review-backlog`'s: a deliverable
stated as one sentence about a person, and a keep, cut, split or resize answer per issue. Invoke that skill
first where one is not already in hand, and where the owner has ruled on it, execute the ruling rather than
re-deriving it.

`docs/ai/platform.md` "Issues" owns how an issue is worked and "Milestones, epics and labels" what a
milestone is. This skill owns the opposite direction: changing them once they are the wrong issues.

## Executing

1. **Land the content, not a description of it.** A draft file, a measured number, a schema: paste it into
   the thread. A scratch file under `tmp/` is lost by the next session, and whoever picks the issue up
   re-invents what it held.
2. **A cut issue closes with where its parts went**, each named. That comment is the whole reason no scope
   is lost, so it is written before the issue closes rather than after.
3. **A body rewritten says what is true now.** `write-issue` holds the shape both an issue body and an
   epic's take, including that an epic describes what its milestone contains now rather than what it used
   to.
4. **Relationships move through the API, never through a body** — `docs/ai/platform.md` "Relationships are
   structural, never prose". Changing an issue's milestone does not move its parent link, and a prose
   `Blocked by:` line left behind is the copy that rots.
5. **Report what the board now costs**: which milestone holds what, and what the longest remaining
   dependency chain is. A groom that does not say this leaves the owner to re-read the board to find out
   what changed.

## What earns a comment

platform.md says the comments are where scope was cut, a figure settled or a step reordered. The converse is
this skill's: **a comment narrating backlog mechanics is noise, and future agents read it as context.**

- **Delete:** milestone moves, "the link was dead", "renamed to", any before-and-after account of how an
  issue changed. `git log` and the body's current state own that, per
  `docs/ai-instructions.md` "Working style".
- **Keep:** a scope cut, a measured figure, a dependency order, a draft artefact, a ruling and what lost
  to it.
- **Trim rather than delete** where a real kernel sits wrapped in that narration.
- A comment answering a scope proposal above it earns its place: without one, nobody can tell whether the
  proposal was adopted or is still open.

## Prose

`docs/ai/platform.md` "Prose published to GitHub" owns the rule for every body and comment this skill
writes. What this workflow adds: fixing a milestone's worth of ragged wrapping is a mechanical
transformation, so write it when it is needed rather than carrying a script around for it.

## Executing a renumber

- **Retitle milestones from the highest number downwards**, so no two ever collide on a title.
- **An epic follows its theme, not its number.** A milestone whose content is unchanged keeps its epic and
  takes a title edit; only a theme that moves needs a body rewritten or a new epic opened.
- **Closed issues stay where they are.** They ship in that release whichever milestone labels them, so
  moving them would claim otherwise. Say so in the epic body rather than leaving the mismatch unexplained.
- **Re-parent sub-issues explicitly**, detaching from the old epic and attaching to the new. An issue may
  hold only one parent, so the detach comes first or the attach is refused.
- **Sweep for pointers to anything deleted.** A comment whose whole content was a link to a deleted
  comment is now noise itself.
- **Repoint the dependency graph out of a closed issue** before finishing. An issue closed as superseded
  never closes as done, so a dependency on it blocks its successors for good — and the graph is where that
  hides, since `docs/ai/platform.md` "Relationships are structural, never prose" keeps it out of the
  bodies.
