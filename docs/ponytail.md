# Ponytail here

TLDR hints for the [ponytail](https://github.com/DietrichGebert/ponytail) plugin in this repository.
It governs code, never the instruction layer — `docs/ai-instructions.md` and `docs/ai/` still win.

## Commands

| Command | Use it for |
| --- | --- |
| `/ponytail` | turn the stance on for a session; `/ponytail lite\|full\|ultra` sets intensity, `full` is default |
| `/ponytail-review` | a branch before the PR — the only one worth running habitually |
| `/ponytail-audit` | whole-repo bloat scan; a once-a-milestone thing, not a per-branch one |
| `/ponytail-debt` | harvest the `ponytail:` ceiling comments into a ledger |
| `stop ponytail` | off |

## Step by step, from a fresh branch

The repo's own rules for branching, committing and landing are
[`ai/platform.md`](ai/platform.md) "Git"; only what ponytail adds is below.

1. **Turn the stance on before writing anything.** `/ponytail`. It only shapes replies that come
   after it, so turning it on once code exists gets you a review, not a stance. `full` is the default
   and the right one.
2. **Read the issue and its whole thread first.** Ponytail shortens the solution, never the reading. A
   small diff aimed at the wrong thing is the failure mode it is most likely to produce, and the
   thread is where the requirement was actually cut down.
3. **Climb [the rungs](#the-rungs) against the change you were about to make.** Out loud is fine —
   "does `progress.rs` already do this" is a two-minute grep that deletes a day of work.
4. **Write the minimum that works, and leave one runnable check.** For Rust that is a `#[cfg(test)]`
   test in the module; the rung ladder applies to tests too, so one test that fails if the logic
   breaks, not a suite per function.
5. **Mark a real ceiling where you accepted one.** `// ponytail: <ceiling>, <what triggers a
   revisit>`, at the site, and only for a shortcut with a known limit — a quadratic scan, an in-memory
   read of something that will grow. A ceiling whose trigger is a real condition rather than "when
   this measurably hurts" is a [`follow-ups.md`](follow-ups.md) entry instead, not a comment.
6. **`/ponytail-review` on the diff.** Before the PR, and it is not a substitute for a correctness
   review — it hunts complexity and nothing else.
7. **`mise run ci`**, then commit and open the PR as usual.

Steps 1, 3, 5 and 6 are the whole of it. Everything else is what you were doing anyway.

## The rungs

Ponytail's ladder, in order. Stop at the first rung that holds; where two work, take the earlier one.
It runs *after* you understand the change, never instead.

1. **Does this need to exist at all?** A speculative need is skipped, in one line. This is the same
   question as test 1 of the record bar in [`ai/platform.md`](ai/platform.md) "The bar" — if the thing
   is not owed, neither is its record.
2. **Is it already in this codebase?** The rung that fires most often, and the one worth actually
   grepping for. The seams that already exist and get reinvented: the `Progress` sink, the
   `RasterSource` trait, `bracket` for an expensive step's log pair, `report`'s versioned envelope,
   the CLI's own error-to-exit-code mapping.
3. **Does the standard library do it?** The precedent is in the CLI's manifest: its integration tests
   reach the binary through `CARGO_BIN_EXE_*` rather than a command runner.
4. **Does something that already owns the job do it?** The platform-feature rung — here, PROJ
   transforms the polygon rather than us walking a ring
   ([ADR 0008](decisions/0008-a-circle-is-projected-never-drawn.md)).
5. **Does a dependency already in the tree solve it?** Read the manifest. `clap` and `anyhow` are the
   CLI's only, and the library's manifest may not grow them.
6. **Can it be one line?** Then one line.
7. **Only then**, the minimum code that works.

## Where it must not fire

Ponytail's own "when NOT to be lazy" list, plus what is specific here:

- **The correctness invariants** ([`ai/application.md`](ai/application.md)). f64 in the summation
  table, nodata converted once at ingest, antimeridian and pole wrapping in every traversal,
  deterministic tie-breaks. A shorter diff that drops one of these is a plausible wrong answer, which
  is the expensive kind.
- **Newtypes are not speculative abstraction.** "Model the domain in types, not primitives" is a
  standing rule; collapsing a checked `Grid` or a radius type back to `f64` is the bug that rule
  exists to prevent, however much code it deletes.
- **The non-negotiables** (`docs/ai-instructions.md`) — laziness never reaches for one.
- **The copying rule.** Transliterating upstream C++ is not a shortcut, it is the one thing the
  project cannot undo.
