# Contributing

## Setup

Everything is pinned in `mise.toml`; nothing is installed globally.

```sh
mise trust          # first time in a fresh clone
mise run setup      # toolchains, dependencies, git hooks
```

## The loop

```sh
mise run fmt        # format
mise run lint       # lint
mise run typecheck  # type-check
mise run test       # test
mise run ci         # all of the above — what CI runs
mise run build      # the release binary, at target/release/popcircles
```

Each task's description in `mise.toml` names what it actually runs; `mise run ci` is the check to
run before opening a PR. Hooks run through
[prek](https://github.com/j178/prek), not pre-commit; the config keeps pre-commit's filename and
format because prek reads it.

## Data

Input datasets live in [`data/`](data/README.md) with their contents in Git LFS. Clone without
pulling hundreds of megabytes, then fetch only when you need them:

```sh
GIT_LFS_SKIP_SMUDGE=1 git clone <url>
mise run setup          # also pins the skip in repo-local git config
mise run data:pull      # fetch the rasters
mise run data:status    # present locally, or pointer-only
```

Skipping is a layered default, not a guarantee — [`data/README.md`](data/README.md#fetching) explains
which layer holds and why the environment variable is still on you.

A new input dataset goes in `data/<kind>/` with a registry entry. Never make a test depend on raster
content: CI runs with LFS content absent.

## Sending a change

- Conventional Commits, commitizen's default types. The PR title follows the same format; the
  commit-msg hook enforces it locally and CI checks both the title and the commits.
- Branch off `main`; do not push to `main` directly.
- Rebase, never merge. `main` rejects a merge commit, and PRs land squashed or rebased; when your
  branch falls behind, `git rebase main` rather than merging it in. `mise run setup` also points this
  clone at the same rule, so an accidental `git pull` fails early instead of after a push.
- **Do not port code from the upstream C++ project.** It carries no licence. Implementations
  written from a description of the algorithm are welcome; transliterations are not.
- Documentation moves with the change. Stale framing is a defect, not a follow-up.

## The conventions

The conventions AI tools follow are the conventions humans follow here.
[`docs/ai-instructions.md`](docs/ai-instructions.md) holds the project invariants and maps the rest;
[`docs/ai/`](docs/ai/) holds the per-subject detail, and every file in it applies to every change —
the split is so each subject can be corrected on its own, not so any of them is optional. Read those
before a first change. [`docs/decisions/`](docs/decisions/) says why a constraint exists, and
[`docs/follow-ups.md`](docs/follow-ups.md) what is still owed; open those when you need them.

A decision about how the repository is built is recorded as an ADR rather than argued again; the work
that follows from one is its sibling plan file, while the algorithm roadmap stays in GitHub issues.
