# AI Instructions — platform

Source of truth for all AI coding tools (Claude Code, GitHub Copilot) working in this repo.
`CLAUDE.md` and `.github/copilot-instructions.md` both point here.

Scope: the Rust and Python platform. Application conventions live in
[`ai-instructions-application.md`](ai-instructions-application.md), which references this one and
defers to it wherever the two would conflict. Read both.

Committed configuration is authoritative for settings it already declares — read `mise.toml`,
`Cargo.toml`, `pyproject.toml`, `.pre-commit-config.yaml`, and `.lfsconfig` rather than assuming.
Extend those files; never regenerate them.

## Working style

- Read the file, run the tool, check the config rather than guessing at structure or conventions.
- Ask when genuinely ambiguous; take the sensible default otherwise and say so.
- Match existing patterns over personal preference.
- Scope to the request. No refactoring adjacent code or improving what was not asked about.

### Changes to these rules

A rule is **non-negotiable** when breaking it is irreversible, weakens security, or erases a
module boundary — committing a credential, committing a raster or a generated summation table,
`unsafe` code, a blanket `#[allow]` or `# type: ignore`, loosening a lint level or a tool mode to
clear an error, regenerating committed config, an unpinned CI action, copying source from the
upstream project (see the application doc).

Treat a change to a non-negotiable as a design change, not a task. Before implementing one, in a
short paragraph: name the rule, state concretely what breaks without it, and offer the smallest
alternative that still meets the underlying need. Then stop and wait.

- **Report the conflict even when it is incidental.** A change that erodes one of these as a side
  effect gets the same treatment as a request to drop it outright. Drift is how they are actually
  lost.
- **Once the objection is heard and the request restated, implement it fully.** Do not relitigate,
  hedge the implementation, or leave the old path in place as a safety net.
- **Never weaken one silently** to make a task easier.
- Do not object over conventions: line length, naming, file placement, or how a test is organised.

## Environment

### Tooling hierarchy

1. **Project task** — a `mise.toml` task (`lint`, `typecheck`, `test`, `fmt`). Never bypass it.
2. **prek** — `prek run`. This repo uses prek, not pre-commit; the config file keeps pre-commit's
   name and format because prek reads it.
3. **`cargo <cmd>`** / **`uv run <tool>`** — the language-local tools.
4. **`mise exec -- <tool>`** — system tools mise manages.

Nothing is installed globally: a new runtime or CLI is pinned in `mise.toml`. Never `pip install`,
never activate a venv by hand, never install a Rust toolchain outside mise.

`mise run ci` reproduces CI locally and is the check to run before reporting work done.

### Dependencies

- Rust: workspace-level settings in the root `Cargo.toml` (`[workspace.package]`,
  `[workspace.lints]`); a crate inherits with `field.workspace = true`. Commit `Cargo.lock`.
- Python: dev tooling in `[dependency-groups].dev`. No `setup.py`, `setup.cfg`, or
  `requirements.txt`. Run `uv lock` after editing dependencies and commit the result in the same
  change.
- Before adding a dependency, check whether one already in the tree covers the need. Prefer the
  standard library for anything small.
- Introducing a new file type or framework updates `.editorconfig`, `.gitattributes`, and
  `.gitignore` in the same change.

### Large input data

Input datasets live in `data/`, one directory per kind, contents in Git LFS. Fetch them with
`mise run data:pull`, inspect what is present with `mise run data:status`, and register any new
dataset in [`data/README.md`](../data/README.md) with its grid, CRS, nodata value and checksum.

Skipping the download by default is layered, because `.lfsconfig` alone does not hold — git-lfs
lets any Git config file override it, so a global `lfs.fetchexclude` beats the committed one:

- `GIT_LFS_SKIP_SMUDGE=1 git clone` — the environment outranks every config file.
- `mise run data:skip` (part of `setup`) — repo-local config, which outranks a global setting.
- `.lfsconfig` — the committed default, for a machine with no override.

Never claim in docs or a commit message that a clone *cannot* fetch the rasters. It is a default
that a user's own config can defeat.

- Never commit a generated summation table or a rendered map: `.gitignore` covers them and they are
  reproducible from the inputs. A new *input* dataset goes to `data/<kind>/` through LFS, with a
  registry entry, and only deliberately.
- Never make a test depend on a raster being present. Tests run on a clone with no LFS content.
- Code that reads a raster fails with a clear message naming `mise run data:pull` when the file is
  an unfetched LFS pointer, rather than parsing the pointer as data.

## Code

### Rust

Edition 2024, toolchain pinned in `mise.toml`. Lints are configured once at the workspace root and
inherited; `unsafe_code` is forbidden.

- No `unsafe`. If a problem seems to need it, raise it as a design question first.
- `unwrap()` and `expect()` are warn-level: acceptable in tests and in a `main` that is documenting
  an invariant, not in library paths. Return `Result` and propagate with `?`.
- Errors: a concrete error enum per crate boundary (`thiserror` when it earns its place),
  `anyhow`-style context only at the binary edge. Never `panic!` for an expected failure.
- Numeric casts in geospatial code are the sharpest edge here: `cast_possible_truncation` and
  `cast_precision_loss` are warn-level deliberately. Make each conversion explicit and state why it
  is safe in a comment when it is not obvious.
- Prefer iterators and slices over index arithmetic; where index arithmetic is the clearer
  expression of a raster traversal, keep it local and named.
- No `mod.rs`: a module is `foo.rs` plus `foo/`.
- Public items get doc comments only where the WHY is non-obvious — see **Comments and docs**.

### Python

Python 3.14, used for data preparation and map rendering, not for the search itself. No
compatibility shims or version guards for earlier releases.

- `X | None`, not `typing.Optional`. Built-in `dict`/`list`/`tuple`, not `typing.Dict`.
- No `from __future__ import annotations`.
- No `if TYPE_CHECKING:` guard except to break an import cycle.
- Full type hints on every signature, tests included.
- There is no importable package yet: `pyproject.toml` is a virtual project (`[tool.uv] package =
  false`). Adding the first module means adding the package layout and wiring
  `typecheck:python` into `lint`, in the same change.

### Comments and docs

- No docstrings in Python. In Rust, `///` only where the WHY is non-obvious.
- Comments only where the reasoning is non-obvious, never restating what the code does.
- No multi-line comment blocks.
- Match surrounding comment density, naming, and idiom.
- Update the single source of truth and link to it rather than creating parallel docs.
- `README.md` and `CONTRIBUTING.md` are the human layer — what the repo is, how to set it up, how
  to send a change. They link into this document instead of repeating it.
- Every change ends by checking the documentation it affects — this document, the human layer, and
  any doc naming a file, task, or convention that moved — and correcting it in the same change.
  Stale framing is a defect, not a follow-up.

## Quality gates

### Linting

- prek is the linting entry point for hooks. Never call `ruff` directly; `cargo clippy` runs
  through `mise run lint:rust` or its hook.
- clippy warnings are errors (`-D warnings`). Never silence one with a blanket `#[allow]`; a
  narrow, commented `#[allow]` on a single item is acceptable when the lint is genuinely wrong.
- ruff for Python lint and format. Never add black, isort, flake8, or pylint.
- TOML is formatted with taplo.
- When a hook reformats files, re-stage and re-run.
- cspell checks every tracked file. A legitimate term it flags goes in `.cspell/project.txt`, in
  the section it belongs to — never an inline ignore.

### Type checking

- Rust: `mise run typecheck` (`cargo check --all-targets --all-features`).
- Python: pyright strict, via `typecheck:python`. Not yet wired into `lint`/`ci` because pyright
  fails when it resolves no source files — wire it in with the first Python module.
- Never use a blanket ignore or loosen a mode to clear an error. Narrow per-line ignores are
  acceptable only at a library boundary, with the reason stated.

### Testing

- Rust: `cargo test`, unit tests in the module under `#[cfg(test)]`, integration tests in
  `tests/`. Property tests are welcome for the geometry and summation invariants.
- Python: pytest. Never `unittest.TestCase` classes. `test:python` is not yet in `ci` because
  pytest exits 5 on an empty suite — wire it in with the first test.
- Numerical work needs its invariants tested, not just its outputs: a summation table agrees with
  a naive sum on small inputs; a circle's contained population is monotonic in radius.
- Tests needing network, real credentials, or fetched rasters are marked, deselected by default,
  and given their own task. They never run in CI.
- Do not run the suite after every edit — run it when asked or when verifying a fix.
- A test failing after a change is fixed before the work is reported done.

## Shipping

### Git

- Conventional Commits, commitizen's default types. The PR title is held to the same format.
- Commit or push only when asked. Branch first if on the default branch.
- Never commit a secret, a generated artefact, or a raster.

### CI

- Pin actions to a full SHA with the version in a trailing comment, never `@main`.
- Reuse `turboBasic/github-actions` reusable workflows wherever one fits — today that is
  `conventional-commits.yml@v2`. The Rust job is inline because no shared `rust-ci.yml` exists
  yet; extracting one there is the intended next step, and until then this repo's `ci.yml` is the
  prototype for it. Do not fork Python-specific shared workflows to fake Rust support.
- Use mise in CI so tool versions match local development, and drive CI through mise tasks so what
  CI runs and what `mise run ci` runs cannot drift apart.
- Lint, typecheck, test only.
- Secrets via CI environment secrets or OIDC.
