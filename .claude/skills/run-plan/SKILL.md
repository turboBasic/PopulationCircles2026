---
name: run-plan
description: Execute one task of an implementation plan in docs/decisions/ - first unchecked task, verified, checked off, committed together with the plan. Use when the user asks to run, execute, or continue a plan in docs/decisions/.
---

# Run a plan

One invocation advances a plan by exactly one task. The loop below is the plan-execution contract;
`docs/ai/platform.md` "Implementation plans" owns what a plan file contains, and a plan's own ground
rules add to this loop without replacing it. Roadmap work is not here — it is GitHub issues.

## Steps

1. **Locate the plan.** Resolve the number, slug or bare description the user gave against the
   `*.plan.md` files in `docs/decisions/`; if more than one matches, ask rather than guessing.
2. **Read the status line first.** A plan marked complete is frozen and is never executed — say so
   and stop.
3. **Read the plan's ground rules.** They constrain how the task is done. Read the phase heading
   above the task too: a mismatch between the model it expects and the running session is worth
   naming before starting, not after.
4. **Take the first unchecked task, top to bottom.** One task per invocation — never two, even when
   the second looks trivial.
5. **Execute it as written.** The task text is the contract. If executing it reveals the task is
   wrong, misordered or already superseded, report that and ask; do not silently rescope it.
6. **Verify** exactly what the task's `Verify:` line asks for, and nothing less. A `Verify:` naming a
   command means running it, not reasoning about what it would print.
7. **Run the gates** — `prek run --all-files`, then `mise run ci` — and fix what fails. Stage new
   files first: `prek` reads tracked files only, so a run that passes over an untracked file has
   checked nothing and the commit fails a moment later. Re-stage what a hook reformats and re-run;
   that is expected, not a defect to investigate. A task's `Verify:` line may ask for less than both;
   it never licenses less than both.
8. **Land it, then stop.** Read `docs/ai/platform.md` "Git" before a plan's first commit rather than
   after — it constrains which branch may receive one, and a commit on the wrong branch is not
   something the next task can undo. The task's change and the updated plan go in one
   Conventional Commit. Then stop — do not roll into the next task.

## Judgment

- **An already-done task is verified and checked off, not redone.** Plans get partially executed by
  other work; confirm the end state the task describes actually holds, then check the box.
- **A report-only task changes no file but the plan.** Its product is the report, delivered in the
  session and durable only where it lands something — a register entry, a term added to a sweep's
  list. The checked box is the whole diff, so commit the plan on its own rather than holding the task
  open for a file change that is not coming.
- **A failed verification leaves the box unchecked.** Report what failed and stop. A box checked
  optimistically makes the plan lie about the repository, and the plan freezes in that state.
- **A task that names a non-negotiable is a stop, not a licence.** A plan is not authorisation to
  break one; the router's protocol applies inside a plan exactly as outside it.
- **The last task usually closes the plan** — status line to complete with the date, and the
  Follow-ups section replaced by the pointer line naming the register entries the plan produced
  (`docs/follow-ups.md`, which owns their format and the bar their conditions must meet). After that
  commit the plan is history, and the next run of this skill will refuse it at step 2.
