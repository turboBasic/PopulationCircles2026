# Contributing

## Setup

Everything is pinned in `mise.toml`; nothing is installed globally.

```sh
mise trust          # first time in a fresh clone
mise run setup      # toolchains, dependencies, git hooks
```

## The loop

```sh
mise run fmt        # cargo fmt, ruff format, taplo fmt
mise run lint       # clippy (warnings are errors), ruff, actionlint
mise run typecheck  # cargo check
mise run test       # cargo test
mise run ci         # all of the above — what CI runs
```

`mise run ci` is the check to run before opening a PR. Hooks run through
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

The committed `.lfsconfig` is a default, not a guarantee: git-lfs lets a global
`lfs.fetchexclude` override it, which is why the environment variable and `data:skip` exist. See
[`data/README.md`](data/README.md#fetching).

Never commit a generated summation table or a rendered map. A new input dataset goes in
`data/<kind>/` with a registry entry. Never make a test depend on raster content — CI runs with LFS
content absent.

## Sending a change

- Conventional Commits, commitizen's default types. The PR title follows the same format; the
  commit-msg hook enforces it locally and CI checks both the title and the commits.
- Branch off `main`; do not push to `main` directly.
- The conventions AI tools follow are the conventions humans follow here:
  [`docs/ai-instructions.md`](docs/ai-instructions.md) is the source of truth, with the
  application-specific rules in
  [`docs/ai-instructions-application.md`](docs/ai-instructions-application.md). Read both before a
  first change.
- **Do not port code from the upstream C++ project.** It carries no licence. Implementations
  written from a description of the algorithm are welcome; transliterations are not.
- Documentation moves with the change. Stale framing is a defect, not a follow-up.
