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
mise run build      # the release binary, at build/target/release/popcircles
```

Deselected from `test` and `ci`, because each needs the raster, time, or both — `mise.toml`'s comment on
each says which:

```sh
mise run test:validate      # the real raster, end to end, against the published result
mise run test:python-raster # the registry's fetched rows against the files on disk
mise run bench              # kernel construction and circle evaluation
mise run bench:table        # the table build — writes 7.5 GB, which is why it is separate
```

A benchmark asserts nothing and no gate compares it against a baseline, so its figures are read rather
than checked, and a figure worth keeping belongs to the issue or the PR that measured it.

Each task's description in `mise.toml` names what it actually runs; `mise run ci` is the check to
run before opening a PR. Hooks run through
[prek](https://github.com/j178/prek), not pre-commit; the config keeps pre-commit's filename and
format because prek reads it.

## Data

Input datasets live in [`data/`](data/README.md), described for a machine in
[`data/registry.toml`](data/registry.toml) and for a person in [`data/README.md`](data/README.md). A
large one is published rather than carried, so a clone is cheap and fetching is a separate step:

```sh
git clone <url>
mise run setup
mise run data:get   # fetch every registered dataset not already here, and verify it
```

`data:get` needs no account anywhere: it reads the registry, verifies each file against the recorded
checksum before putting it in place, and prints the attribution each licence requires
([`data/README.md`](data/README.md#getting-it)).

**The raster used to be tracked, so a working copy that had fetched it loses it on the next pull** — the
428 MB object is no longer in the tree, and `mise run data:get` is what puts it back.

A new input dataset goes in `data/<kind>/` with a registry entry. Never make a test depend on raster
content: a CI checkout has none.

### Publishing a dataset

A dataset too large to carry in the repository is published as an asset on a **data-only tag**. Cutting
one is rare enough that its body is written from this list rather than from whatever the person cutting
it remembered.

The tag and its assets:

1. Name it `data-vN`. It must **not** match `v*`, which is what `.github/workflows/release.yml` triggers
   on — a data tag matching that pattern publishes a binary release nobody asked for.
2. Name each asset for the dataset's key plus its extension, so the asset, the file on disk and the
   registry row are one string. Attach the `.sha256` beside it.

The body is read by someone who has never seen this repository, and the asset's name is this project's
own description rather than the publisher's, so it lets them assume nothing. Each item names where its
text already exists, and items 1 and 3 to 6 all come from the dataset's own heading in
[`data/README.md`](data/README.md), which is named for its key:

1. **What the file is** — its nodata sentinel, from the dataset's row in
   [`data/registry.toml`](data/registry.toml); its extent and pixel type from that heading, which is
   where those two are recorded.
2. **Its grid** — dimensions and cell size, from the registry row.
3. **Which variant**, where a dataset is published in several that differ in values — the sentence naming
   the variant and the figure that distinguishes it. The asset's name will not carry it and the numbers
   change, so the body is the only place a fetcher can learn it.
4. **Provenance** — the format, the published name, the publisher and the DOI or tag identifying it.
5. **The licence** and its URL.
6. **The citation** verbatim, where the licence requires one. A fetcher acquires the obligation with the
   bytes, so it travels with them.
7. **The `sha256` and `bytes`**, from the row, so the download can be verified by hand and not only by
   `mise run data:get`.

A figure the body needs and no row holds is a finding, not something to measure into the release notes:
measure it into [`data/registry.toml`](data/registry.toml) where a machine reads it, or into
[`data/README.md`](data/README.md) where a person does, then quote it.

### Verifying a published dataset

`mise run data:get` verifies what it fetches against the registry, so this is not that. It is the other
route: obtaining an **independent** copy from the publisher, which is what makes the recorded `sha256`
something more than a record of one download. Nobody needs it to work on this repository; someone
checking that a republished asset is what it claims does.

For the population raster it needs a free [NASA Earthdata Login][urs-new], because the archive is
behind URS OAuth and answers an anonymous request with a 401 and a redirect rather than the file. A
browser download from the [dataset's granules in Earthdata Search][gpw-search] is simplest — pick the
2020, 30 arc-second GeoTIFF granule. For `curl` or `wget`, NASA documents the [cookie and netrc
setup][urs-curl] the redirect needs.

The granule is a ~405 MB zip. Extract just the raster and rename it to what the registry expects:

```sh
unzip -j <granule>.zip '*.tif' -d data/population/
mv data/population/gpw_v4_population_count_*_2020_30_sec.tif \
   data/population/population-count-2020-30arcsec.tif
shasum -a 256 data/population/population-count-2020-30arcsec.tif
```

The zip carries one `.tif` per year, so the glob is what selects 2020 rather than an assumption about
the name inside.

**Check the last line against the `sha256` in [`data/registry.toml`](data/registry.toml)**, which holds
it and the `bytes` beside it. A match means the copy every figure was measured from is the copy the
archive serves. A mismatch is a finding rather than a broken download: it means the two differ, and the
registry — measured from ours — is what would then need re-measuring. Say so rather than working around
it. [`data/README.md`](data/README.md) is where the provenance that identifies the dataset lives.

[gpw-search]: https://search.earthdata.nasa.gov/search/granules?p=C3540909447-ESDIS
[urs-curl]: https://urs.earthdata.nasa.gov/documentation/for_users/data_access/curl_and_wget
[urs-new]: https://urs.earthdata.nasa.gov/users/new

## Sending a change

- Conventional Commits, commitizen's default types. The PR title follows the same format; the
  commit-msg hook enforces it locally and CI checks both the title and the commits.
- Branch off `main`; do not push to `main` directly. The ruleset refuses it, and the one exception is the
  repository owner, whose `Repository admin` role bypasses the required checks — used for changes that
  touch no code, with the `guard-direct-push` hook running `mise run ci` in their place.
- Rebase, never merge. `main` rejects a merge commit, and PRs land squashed or rebased; when your
  branch falls behind, `git rebase main` rather than merging it in. `mise run setup` also points this
  clone at the same rule, so an accidental `git pull` fails early instead of after a push.
- **Do not port code from the upstream C++ project.** It carries no licence. Implementations
  written from a description of the algorithm are welcome; transliterations are not.
- Documentation moves with the change. Stale framing is a defect, not a follow-up.

## Releasing

A release is a tag and the two binaries it attaches, from one workspace version and no registry; this is
the sequence for cutting one.

```sh
mise run release:smoke  # both build legs on demand, macOS included, publishing nothing
```

Run it before cutting a tag, and after bumping a pinned toolchain. Nothing else in this repository ever
compiles for `aarch64-apple-darwin` — `ci.yml` is `ubuntu-latest` and deliberately stays that way — so
without it the first macOS compile of a tree is the tag you are trying to ship. It builds the branch as
`origin` has it, takes about as long as a release, and leaves a run page and its two artifacts behind:
no tag, no Release. It runs the same build a tag runs — `.github/workflows/release-smoke.yml` and
`release.yml` call one `build-binaries.yml` — and neither the gate nor the publish job, which only a tag
declares.

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

A decision about how the repository is built is recorded as an ADR rather than argued again — but only
where reversing it would cost more than a PR, and then on one page. Most choices are explained by the PR
that makes them, and the work itself is decomposed in GitHub issues.
