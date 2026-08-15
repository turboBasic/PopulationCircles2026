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

Deselected from `test` and `ci`, because each needs the raster, time, or both — `mise.toml`'s comment on
each says which:

```sh
mise run test:validate  # the real raster, end to end, against the published result
mise run bench          # kernel construction and circle evaluation
mise run bench:table    # the table build — writes 7.5 GB, which is why it is separate
```

A benchmark asserts nothing and no gate compares it against a baseline, so its figures are read rather
than checked; the ones taken on one machine are recorded in
[ADR 0009](docs/decisions/0009-validation-brackets-cheap-and-certifies-dear.md).

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

## Releasing

[ADR 0006](docs/decisions/0006-release-shape-and-format-promises.md) rules what a release is; this is
the sequence for cutting one.

```sh
mise run release:smoke  # both build legs on demand, macOS included, publishing nothing
```

Run it before cutting a tag, and after bumping a pinned toolchain. Nothing else in this repository ever
compiles for `aarch64-apple-darwin` — `ci.yml` is `ubuntu-latest` and, per ADR 0006, stays that way — so
without it the first macOS compile of a tree is the tag you are trying to ship. It builds the branch as
`origin` has it, takes about as long as a release, and leaves a run page and its two artifacts behind:
no tag, no Release. It does not exercise the publish job, which only a tag reaches.

1. Bump `version` in `[workspace.package]`, which is the only place it lives, and land it on `main` like
   any other change. Every report snapshot moves with it, because every document carries `tool_version`.
2. Tag the merged commit `vX.Y.Z` for that same version and push the tag. The workflow's gate compares
   the two and refuses a tag that disagrees, so a mismatch costs a run rather than a wrong binary.
3. Write the notes by hand. The workflow opens the body empty on purpose: the notes have to say that
   `schema_version` is a contract across releases while a cache or a ledger may be invalidated by this
   one and rebuilt, and nothing can generate that from a commit range.

When a run fails, what to do next turns on one fact — **whether the publish job ran.**

- **It did not.** No Release exists, so the tag is still retractable. Re-run the workflow if the cause
  was the runner rather than the commit; otherwise delete the tag, fix the cause, and tag the same
  version again.
- **It did.** A Release exists and that version is spent, so the next attempt is a version bump rather
  than a moved tag — a tag that moves lies to everyone who already fetched it.

## The conventions

The conventions AI tools follow are the conventions humans follow here.
[`docs/ai-instructions.md`](docs/ai-instructions.md) holds the project invariants and maps the rest;
[`docs/ai/`](docs/ai/) holds the per-subject detail, and every file in it applies to every change —
the split is so each subject can be corrected on its own, not so any of them is optional. Read those
before a first change. [`docs/decisions/`](docs/decisions/) says why a constraint exists, and
[`docs/follow-ups.md`](docs/follow-ups.md) what is still owed; open those when you need them.

A decision about how the repository is built is recorded as an ADR rather than argued again; the work
that follows from one is its sibling plan file, while the algorithm roadmap stays in GitHub issues.
