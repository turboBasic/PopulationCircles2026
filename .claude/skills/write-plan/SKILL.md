---
name: write-plan
description: Author a scratch implementation plan under tmp/ that run-plan can execute - tasks sized to one green commit, each ending in a Verify line that can fail. Use when the user asks to write, draft or decompose an implementation plan.
---

# Write a plan

`docs/ai/platform.md` "Implementation plans" fixes where a plan file lives, what it is for, and its six
sections — there rather than here, because two skills read a plan file and neither may own its shape.
What follows is what that shape cannot say: how work becomes tasks, and what makes a `Verify:` line
worth trusting. Executing the result is the `run-plan` skill.

Every rule below exists because that skill reads the file literally — first unchecked task, top to
bottom, executed as written, gated, committed. So one question sits behind each step: can the executor
act on this without asking the author what was meant?

## Steps

1. **Start from the issue, not from a blank plan.** The issue and its thread are the decomposition; this
   file is that decomposition made executable, and the two are not allowed to disagree about what the
   work is. Read the whole thread and the roadmap issue above it first —
   `docs/ai/platform.md` "Issues" says why the body alone is not the requirement. Where the plan has to
   depart from the issue, the departure is a comment on the issue before it is a task here.
2. **Measure the tree before drafting.** Read the files the work will touch, run the gates, get the
   counts and the values. Cite what you found: line numbers, measured figures, what a command actually
   printed. A task naming what it will change is executable; a task naming a goal is a wish, and the
   difference only surfaces when someone tries to run it.
3. **Decompose into tasks first, write the sections after.** The task list is the plan and everything
   else is packaging. Order the tasks so each is executable when it is reached — a task needing a later
   one is a stall the executor cannot resolve, because it may not skip ahead.
4. **Size every task to one commit that leaves the tree green.** This is the constraint that actually
   shapes a decomposition, and the one most often broken: `run-plan` runs the gates after each task and
   commits the result, so a task that only compiles once its successor lands can never pass. It is why
   the shape that works is usually declare, then wire, then fill, rather than one task per finished
   feature.
5. **Write each `Verify:` so that it can fail.** Name a command and what its output must show, or a
   state a reader can check and disagree with. "Tests pass" verifies nothing — the gates run anyway, on
   every task. Prefer the assertion that would have caught the mistake you are worried about:
   `rg -n 'scaffolding only'` returns nothing; the totals match the four figures the registry records;
   the same 1 test runs before and after.
6. **Phase only where the grouping carries information** — a shared prerequisite, a change of subject, a
   `Model:` the phase expects. Phases exist so a reader can see the shape of the work; numbering that
   only counts is noise. A boundary also costs something, now that `run-plan` stops at one: it is where
   the plan is handed back for a look, so put it where a look is worth taking.
7. **Make every Ground rule and every Out of scope entry rule something out.** A ground rule earns its
   place by naming a mistake available in *this* work, not by restating a convention the instruction
   layer already binds. An Out of scope entry carries the reason it lost, so a later reader cannot
   mistake the omission for an oversight.
8. **Make the last task land what outlives the file.** The plan itself is scratch and is thrown away, so
   the closing task is what survives it: the issue's checkboxes and the roadmap box ticked, any
   obligation the work produced written into `docs/follow-ups.md`, the Follow-ups section here reduced to
   those identifiers, and the status line set to complete with the date. `run-plan` refuses a plan marked
   complete, which is the only thing keeping a finished plan from being run twice.

## Judgment

- **A `Verify:` line is the only part of a plan that cannot be fudged.** The task text says what to do
  and a determined reader can satisfy it badly; the verification either holds or does not. Where a task
  resists a checkable verification, that is evidence the task is not yet understood — split it until one
  is available.
- **Phrase a task as an end state, not an action.** `run-plan` verifies an already-done task and checks
  it off rather than redoing it, and "add the module" is ambiguous the moment the module exists while
  "the module exists, declared in `lib.rs`, with these two functions" is not. Plans get partially
  overtaken by other work; the phrasing is what survives that.
- **A report-only task must land something durable.** Nothing here is committed, so its whole product is
  what it writes elsewhere — a register entry, a term in a list, a corrected document. A report-only task
  that leaves none of those behind evaporates when the file does.
- **A task that would break a non-negotiable does not belong in a plan.** A plan is not authorisation;
  writing one in is how the router's protocol gets bypassed by a document nobody re-reads. Same for a
  task whose verification needs fetched raster content — that gets marked, deselected and given its own
  task, or it is a test CI cannot run.
- **If the tasks keep coming out as "decide X", the decision is not made.** Those belong in the issue
  thread or in conversation, and the ones clearing platform.md's bar belong in a record. A plan carries
  work a ruling settled, and a plan full of open questions is a proposal wearing checkboxes.
- **A scratch file is not a licence to be vague.** The reader is a session with no memory of drafting
  it, which is exactly the reader a committed plan had. Uncommitted means unreviewed, not unspecified.
- **The sections are platform.md's, not yours to vary.** Adding one, renaming one, or copying a filled-in
  template into this file are all the same defect: a second owner for a fact that already has one.
- **A close-out task ticks the issue's checkboxes; it does not close the issue.** Leave the close itself
  to the PR's `Closes #N`, per `docs/ai/platform.md` "Git" — closing it here is the ambiguity that let #2
  get closed by hand hours before the PR carrying its work existed.
