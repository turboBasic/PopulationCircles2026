---
name: housekeeping
description: Audit repository hygiene - gates, instruction-layer duplication, the structure tree, the dataset registry, the follow-up register and stale permissions - and report the findings without fixing them. Use when the user asks for a repo audit, hygiene check, or housekeeping pass.
---

# Housekeeping sweep

Report-only. Run every check, then hand the user one list; a sweep that fixes as it goes buries what
drifted under the repair. Each finding names what drifted, the file that owns it, and the fix — the
user decides which fixes happen, and each one is separate work afterwards.

Run every check below even when an earlier one fails: a broken hook says nothing about the rest.

## Checks

1. **Gates.** `prek run --all-files`, then `mise run ci`. Each failure is a finding, quoted with the
   file and line the tool printed. A hook that rewrites a file is a finding too — the rewrite is
   drift that was sitting in the tree, even though the hook repaired it.
2. **Duplication.** Every fact has one owner; a mention anywhere else is a citation of that owner or a
   finding. Read the live Markdown — the instruction layer, `.claude/skills/`, the two documents in
   `.github/`, and the human layer — and apply the route-or-fact test from
   `docs/ai-instructions.md` "Layering": delete the sentence mentally and ask whether a route or a
   fact was lost. Reading is the check; a term list would only find drift someone already noticed.

   The enforcement details are where it bites, because prose naming one has copied a fact a config
   file owns and will not be corrected when that file changes: a lint level, a task's command line, a
   hook's name, a nodata value, an LFS setting. Three restatements are sanctioned and not worth
   listing — the invariant list in `docs/ai-instructions.md`, a record in `docs/decisions/` that
   decided a fact, and the human layer, which restates by licence. A human-layer hit is a finding only
   when it contradicts its owner or pins an enforcement detail.
3. **Structure tree, pointers and the always-loaded set.** `mise run lint:docs`
   (`scripts/lint_docs.py`) checks `docs/ai/platform.md` "Structure" against the tree, every pointer
   in the instruction layer, `.claude/skills/`, the two documents in `.github/` and the human layer,
   and that every file in `docs/ai/` has an `@` import in `CLAUDE.md` naming it.
   Run it; any output is a finding.
4. **Dataset registry.** Every file under `data/<kind>/` has a row in `data/README.md`, and every row
   names a file that exists. Run `mise run data:status` and report which objects are pointer-only —
   that is the expected state, and a fetched raster sitting in the tree is worth naming, not fixing.
   A generated artifact anywhere under `data/` is a finding.
5. **Follow-up register.** `docs/follow-ups.md` holds every follow-up this repository has recorded; a
   plan's Follow-ups section is a pointer line into it. Take the `dormant` and `due` entries, answer
   each condition against the repository rather than from memory, and report the ones now true. An
   entry whose condition has not fired is not a finding and is not worth listing; a `dormant` one
   whose condition the repository *cannot* answer is, because it will read as dormant forever. Skip
   `closed` and `retired` entries — a retired entry's condition is unanswerable by construction,
   which is the whole of what retiring it recorded.
6. **Stale local allowlist.** `.claude/settings.local.json`, if it exists, grants permissions by path
   and command name. An entry naming a task, skill or file that no longer exists is a finding. Glob
   patterns covering a directory are not — they age fine.

## Reporting

Group findings by check, most consequential first, and say plainly when a check is clean. Where a fix
is one command, name the command. Where it is a judgment — a duplicated fact needing a home, a
follow-up now due — state what you would do and why, and leave it to the user.

A sweep that finds nothing is a useful result. Report it as such rather than promoting a near-miss to
fill the list.
