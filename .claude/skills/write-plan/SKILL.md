---
name: write-plan
description: Author an implementation plan that run-plan can execute - tasks sized to one green commit, each ending in a Verify line that can fail. Use when the user asks to write, draft or decompose an implementation plan.
---

# Write a plan

`docs/ai/platform.md` "Implementation plans" fixes a plan's two homes, its frontmatter and its
seven sections — there rather than here, because two skills read a plan file and neither may own its
shape. What follows is what that shape cannot say: how work becomes tasks, and what makes a
`Verify:` line worth trusting. Executing the result is the `run-plan` skill.

Every rule below exists because that skill reads the file literally — first unchecked task, top to
bottom, executed as written, gated, committed. So one question sits behind each step: can the
executor act on this without asking the author what was meant?

## Steps

1. **Settle the home before drafting.** Work an ADR decided goes in that record's sibling plan; the
   step decomposition of the algorithm roadmap goes in GitHub issues. Both are platform.md's, and
   the trap is the third home it forbids. Two consequences that section leaves to the author:
   `run-plan` resolves plans in `docs/decisions/` only, so a working plan for a roadmap issue is
   scratch — under the gitignored `tmp/`, executed by hand, never committed — and work wanting a
   committed plan in neither home is a record to write, not a call to make while drafting. A scratch
   plan takes the same shape as a committed one, status line included: the two sanctioned values are
   all there are, and `draft` is not among them.
2. **Measure the tree before drafting too.** Read the files the work will touch, run the gates, get
   the counts and the values. Cite what you found: line numbers, measured figures, what a command
   actually printed. A task naming what it will change is executable; a task naming a goal is a
   wish, and the difference only surfaces when someone tries to run it.
3. **Decompose into tasks first, write the sections after.** The task list is the plan and
   everything else is packaging. Order the tasks so each is executable when it is reached — a task
   needing a later one is a stall the executor cannot resolve, because it may not skip ahead.
4. **Size every task to one commit that leaves the tree green.** This is the constraint that
   actually shapes a decomposition, and the one most often broken: `run-plan` runs the gates after
   each task and commits the result, so a task that only compiles once its successor lands can never
   pass. It is why the shape that works is usually declare, then wire, then fill, rather than one
   task per finished feature.
5. **Write each `Verify:` so that it can fail.** Name a command and what its output must show, or a
   state a reader can check and disagree with. "Tests pass" verifies nothing — the gates run anyway,
   on every task. Prefer the assertion that would have caught the mistake you are worried about:
   `rg -n 'scaffolding only'` returns nothing; the totals match the four figures the registry
   records; the same 1 test runs before and after.
6. **Phase only where the grouping carries information** — a shared prerequisite, a change of
   subject, a `Model:` the phase expects. Phases exist so a reader can see the shape of the work;
   numbering that only counts is noise. A boundary also costs something now that `run-plan` stops at
   one: it is where the author hands the plan back for a look, so put it where a look is worth taking.
7. **Make every Ground rule and every Out of scope entry rule something out.** A ground rule earns
   its place by naming a mistake available in *this* work, not by restating a convention the
   instruction layer already binds. An Out of scope entry carries the reason it lost, so a later
   reader cannot mistake the omission for an oversight.
8. **Make the last task close the plan** — the status line to complete with the date, and Follow-ups
   replaced by the identifiers of the register entries the plan produced. `run-plan` expects this
   and refuses the plan afterwards; a plan left open at the end goes on describing work that is
   finished.

## Judgment

- **A `Verify:` line is the only part of a plan that cannot be fudged.** The task text says what to
  do and a determined reader can satisfy it badly; the verification either holds or does not. Where
  a task resists a checkable verification, that is evidence the task is not yet understood — split
  it until one is available.
- **Phrase a task as an end state, not an action.** `run-plan` verifies an already-done task and
  checks it off rather than redoing it, and "add the module" is ambiguous the moment the module
  exists while "the module exists, declared in `lib.rs`, with these two functions" is not. Plans get
  partially overtaken by other work; the phrasing is what survives that.
- **A report-only task must land something durable.** Its whole diff is the checked box, so unless
  it leaves a register entry, a term in a list or a corrected document behind, the work evaporates
  and the tick is the only trace it happened.
- **A task that would break a non-negotiable does not belong in a plan.** A plan is not
  authorisation; writing one in is how the router's protocol gets bypassed by a document nobody
  re-reads. Same for a task whose verification needs fetched raster content — that gets marked,
  deselected and given its own task, or it is a test CI cannot run.
- **If the tasks keep coming out as "decide X", the decision is not made.** Those belong in an ADR
  or in conversation; a plan carries work a ruling settled, and a plan full of open questions is a
  proposal wearing checkboxes.
- **The sections are platform.md's, not yours to vary.** Adding one, renaming one, or copying a
  filled-in template into this file are all the same defect: a second owner for a fact that already
  has one.
- **A close-out task ticks the issue's checkboxes; it does not close the issue.** Tick `- [ ]` items
  in the issue body and the roadmap box; leave the close itself to the PR's `Closes #N`, per
  platform.md "Git" — closing it here is exactly the ambiguity that let #2 get closed by hand hours
  before the PR carrying its work existed.
