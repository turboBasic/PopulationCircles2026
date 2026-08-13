# Platform

Read this when changing tooling, dependencies, input data, quality gates, git conventions or CI, when
authoring a record under `docs/decisions/` or its sibling plan, or when the structure tree moves.

Committed configuration is authoritative for what it declares — read `mise.toml`, `Cargo.toml`,
`pyproject.toml`, `.pre-commit-config.yaml` and `.lfsconfig` rather than assuming. Extend those
files; never regenerate them. What follows is the judgment around them, not a second copy of them.

## Tooling hierarchy

1. **A mise task** — `mise run <task>`. Never bypass one that exists.
2. **prek** — `prek run`. Hooks run through prek, not pre-commit; the header comment in
   `.pre-commit-config.yaml` says why the filename is what it is.
3. **`cargo <cmd>` / `uv run <tool>`** — the language-local tools.
4. **`mise exec -- <tool>`** — system tools mise manages.

`mise.toml` is the roster of both the pinned tools and the tasks: what `lint`, `typecheck`, `test`
and `ci` actually run is declared there, and prose repeating a task's command line is a drift site.
`mise run ci` reproduces CI locally and is the check to run before reporting work done.

Nothing is installed globally. A new runtime or CLI is pinned in `mise.toml`. Never `pip install`,
never activate a venv by hand, never install a Rust toolchain outside mise.

A task deliberately not yet wired into `lint` or `ci` carries a comment saying why. Believe the
comment and satisfy its stated condition before wiring the task in — the two Python tasks are
waiting on sources and tests to exist, not on someone noticing them.

## Dependencies

- Rust: workspace-level settings live in the root `Cargo.toml` (`[workspace.package]`,
  `[workspace.lints]`); a crate inherits with `field.workspace = true`. Commit `Cargo.lock`.
- Python: dev tooling in `[dependency-groups].dev`. No `setup.py`, `setup.cfg` or
  `requirements.txt`. Run `uv lock` after editing dependencies and commit the result in the same
  change.
- Before adding a dependency, check whether one already in the tree covers the need. Prefer the
  standard library for anything small.
- Introducing a new file type or framework updates `.editorconfig`, `.gitattributes` and
  `.gitignore` in the same change.

## Large input data

Input datasets live in `data/`, one directory per kind, contents in Git LFS.
[`data/README.md`](../../data/README.md) is the registry and owns each dataset's grid, CRS, nodata
value, checksum and provenance, plus the mechanics of skipping and fetching the objects
([Fetching](../../data/README.md#fetching)). A dataset gets its row in the same change that adds it.

The judgment around that mechanism:

- **Never claim in docs or a commit message that a clone _cannot_ fetch the rasters.** Skipping is a
  layered default, and git-lfs lets a user's own Git config defeat the committed one. Overstating it
  turns a default into a guarantee nobody is holding.
- A new **input** dataset goes to `data/<kind>/` through LFS, with a registry entry, and only
  deliberately. Generated products are neither committed nor placed there.
- Code that reads a raster fails with a clear message naming `mise run data:pull` when the file is
  an unfetched LFS pointer, rather than parsing the pointer as data.

## Quality gates

### Linting

- prek is the entry point for hooks; `cargo clippy` also runs through its own mise task. Never call
  `ruff` directly.
- clippy warnings are errors. Never silence one with a blanket `#[allow]`; a narrow, commented
  `#[allow]` on a single item is acceptable when the lint is genuinely wrong.
- ruff for Python lint and format. Never add black, isort, flake8 or pylint.
- TOML is formatted with taplo.
- When a hook reformats files, re-stage and re-run. That is expected behaviour, not an error to
  investigate.
- cspell checks every tracked file. A legitimate term it flags goes in `.cspell/project.txt`, in the
  section it belongs to — never an inline ignore.

### Type checking

Never use a blanket ignore or loosen a mode to clear an error. Narrow per-line ignores are
acceptable only at a library boundary, with the reason stated. Python type checking is pyright in
strict mode.

### Testing

- Rust: unit tests in the module under `#[cfg(test)]`, integration tests in `tests/`. Property tests
  are welcome for the geometry and summation invariants.
- Python: pytest. Never `unittest.TestCase` classes.
- What the numeric code's tests must pin, as against merely exercise, is
  [`application.md`](application.md) "Correctness invariants".
- **Never make a test depend on a fetched raster.** The suite runs on a clone with no LFS content, so
  a test that needs raster bytes to pass is a test CI cannot run. Build the fixture in code, or
  decimate one small enough to commit.
- Tests needing network, real credentials or fetched rasters are marked, deselected by default, and
  given their own task. They never run in CI.
- Do not run the suite after every edit — run it when asked or when verifying a fix. A test failing
  after a change is fixed before the work is reported done.

## Architecture decisions

Consulting accepted ADRs before an architecture choice is a project invariant, stated in
`docs/ai-instructions.md`. This section governs authoring them.

- A decision is a numbered record in `docs/decisions/`, named `NNNN-<kebab-slug>.md`.
- An accepted ADR is superseded by a new record, never edited. The two supersession fields and the
  Status section of the superseded record are the only sanctioned edits.
- An ADR records a decision **made**. A ruling that cannot carry a `decided` date is a proposal and
  belongs in a plan or in conversation.
- The record's shape — frontmatter fields and the five sections — is owned by the `write-adr` skill,
  which is where an author meets it.

## Implementation plans

A plan is the sibling `NNNN-<slug>.plan.md` of the ADR whose work it carries, in the same directory;
it may precede the ADR it drives. Two skills read a plan file — one writes it, one executes it — so
its shape is fixed here rather than in either.

**A plan file carries work an ADR decided. The step decomposition of the algorithm roadmap is GitHub
issues**, one per step, tracked from the roadmap issue.

The transition runs one way. An issue that turns out to need an architecture decision produces an ADR,
and the tasks following from that decision move into its plan file; a plan is never reopened to absorb
roadmap work.

Those are the only two homes, and a third takes a record. Work fitting neither — a committed task list
outside `docs/decisions/`, or a roadmap issue whose steps a record decided rather than discovered — is
ruled into `docs/decisions/` naming which side it falls on and why, never settled by judgment in the
moment. The failure to avoid is not a wrong answer but a quiet third convention.

- **Frontmatter** carries `tags: [plan, <domain>, popcircles]` and `created:`, the domain tag
  matching the ADR's.
- **Status line** is the paragraph directly under the title, opening
  `**Status: in progress (YYYY-MM-DD).**` or `**Status: complete (YYYY-MM-DD).**`. A plan marked
  complete is frozen: it stays in place as the record of what was decided then, and is never
  executed or ticked off again.
- **Ground rules** constrain how every task in that plan is done. They add to the executing skill's
  loop and never replace it.
- **Out of scope** records what was weighed and deliberately left out, so a later reader does not
  mistake an omission for an oversight.
- **Phases** group tasks and may carry a `Model:` note naming the model the phase expects.
- **Tasks** are checkboxes numbered `<phase>.<task>`, each ending in a `Verify:` line stating what
  proves it done. The task is the unit of execution and the unit of commit.
- **Follow-ups** close the plan and hold identifiers only: the obligations the plan produced are
  entries in [`../follow-ups.md`](../follow-ups.md), which owns their format, statuses and the bar
  their conditions must meet. The section here names those identifiers and sends the reader there,
  and is frozen with the rest of the plan once written.

## Git

- Conventional Commits, commitizen's default types; the PR title is held to the same format. Both
  are checked — locally by the commit-msg hook, in CI by the shared workflow.
- Commit or push only when asked. Branch first if on the default branch.
- Never commit a secret, a generated artefact, or a raster.

## CI

- Pin actions to a full SHA with the version in a trailing comment, never `@main`.
- Reuse `turboBasic/github-actions` reusable workflows wherever one fits. The Rust job is inline
  because no shared `rust-ci.yml` exists yet; extracting one there is the intended next step, and
  until then this repo's `ci.yml` is the prototype for it. Do not fork Python-specific shared
  workflows to fake Rust support.
- Drive CI through mise tasks so what CI runs and what `mise run ci` runs cannot drift apart.
- Lint, typecheck, test only.
- Secrets via CI environment secrets or OIDC.

## Structure

Hand-maintained. Update it in the change that adds or removes a root. A new file under `docs/ai/` gets
its `@` import line in `CLAUDE.md` in the same change too — without one it never loads, and nothing at
commit time says so.

**Roots only**, so that an omission is not read as drift: where a kind of thing lives, never an
inventory of files. Committed configuration sits at the repository root and documents itself, and the
layering table in `docs/ai-instructions.md` names the files that own an enforced fact. A path here and
absent on disk is drift and a finding; a path on disk and absent here is a finding only when it is a
new root.

```text
CLAUDE.md                        Claude entry point — imports only
.github/copilot-instructions.md  Copilot entry point — routing only
.github/workflows/               CI
docs/ai-instructions.md          the router: invariants and layering
docs/ai/                         the instruction layer, one file per subject
docs/decisions/                  decision records and their sibling plans
docs/follow-ups.md               the register of pending obligations
.claude/skills/                  one directory per task workflow
crates/                          Rust workspace — the search
data/                            input datasets in Git LFS; registry in data/README.md
```
