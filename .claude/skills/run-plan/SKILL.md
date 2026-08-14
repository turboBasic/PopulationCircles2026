---
name: run-plan
description: Execute a phase of an implementation plan in docs/decisions/ - every task in it verified, checked off and committed on its own, stopping at the phase boundary. Use when the user asks to run, execute, or continue a plan in docs/decisions/.
---

# Run a plan

One invocation advances a plan by one phase. The task stays the unit of verification and the unit of
commit — the phase is only how far the run goes without a human in the loop, and neither a task nor a
phase boundary is crossed with work uncommitted. The loop below is the plan-execution contract;
`docs/ai/platform.md` "Implementation plans" owns what a plan file contains, and a plan's own ground
rules add to this loop without replacing it. Roadmap work is not here — it is GitHub issues.

## Steps

1. **Locate the plan.** Resolve the number, slug or bare description the user gave against the
   `*.plan.md` files in `docs/decisions/`; if more than one matches, ask rather than guessing.
2. **Read the status line first.** A plan marked complete is frozen and is never executed — say so
   and stop.
3. **Settle the scope before the first task, never during.** The default is the phase holding the
   first unchecked task, run to its end. Ask up front where the request reads narrower or wider than
   that — one named task, a phase that is not the current one, the rest of the plan — because a scope
   question raised after the second commit is a question about work already landed.
4. **Read the plan's ground rules and the phase heading.** The rules constrain how every task in the
   run is done. A `Model:` note the running session is weaker than stops the run before the first
   task; a stronger session is worth naming and no reason to stop.
5. **Execute the first unchecked task as written.** The task text is the contract. If executing it
   reveals the task is wrong, misordered or already superseded, that ends the run: report it and ask,
   do not silently rescope it and do not step over it to the next task.
6. **Verify** exactly what the task's `Verify:` line asks for, and nothing less. A `Verify:` naming a
   command means running it, not reasoning about what it would print.
7. **Run the gates** — `prek run --all-files`, then `mise run ci` — and fix what fails. Stage new
   files first: `prek` reads tracked files only, so a run that passes over an untracked file has
   checked nothing and the commit fails a moment later. Re-stage what a hook reformats and re-run;
   that is expected, not a defect to investigate. A task's `Verify:` line may ask for less than both;
   it never licenses less than both.
8. **Land it.** Read `docs/ai/platform.md` "Git" before the run's first commit rather than after — it
   constrains which branch may receive one, and a commit on the wrong branch is not something the
   next task can undo. The task's change and its checked box go in one Conventional Commit.
9. **Take the next unchecked task in the same phase and repeat from step 5.** At the phase boundary,
   stop: report the tasks that landed and name what the next phase holds. Do not roll into it.

## Judgment

- **A failure ends the run only once it resists fixing.** A failed verification or a red gate is the
  work rather than an exit — diagnose it, fix it, verify again. What ends the run is a failure this
  session cannot resolve: a missing credential, a task built on a decision nobody made, a gate
  demanding something outside the plan. Then the box stays unchecked and the report names the task and
  what failed, because a box checked optimistically makes the plan lie about the repository and the
  plan freezes in that state.
- **An already-done task is verified and checked off, not redone.** Plans get partially executed by
  other work; confirm the end state the task describes actually holds, then check the box.
- **A report-only task changes no file but the plan.** Its product is the report, delivered in the
  session and durable only where it lands something — a register entry, a term added to a sweep's
  list. The checked box is the whole diff, so commit the plan on its own rather than holding the task
  open for a file change that is not coming.
- **A task that names a non-negotiable is a stop, not a licence.** A plan is not authorisation to
  break one; the router's protocol applies inside a plan exactly as outside it.
- **The last task usually closes the plan** — status line to complete with the date, and the
  Follow-ups section replaced by the pointer line naming the register entries the plan produced
  (`docs/follow-ups.md`, which owns their format and the bar their conditions must meet). After that
  commit the plan is history, and the next run of this skill will refuse it at step 2.
