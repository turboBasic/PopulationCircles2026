---
tags: [adr, code, popcircles]
created: 2026-08-15
decided: 2026-08-15
supersedes: null
superseded_by: null
---

# ADR 0006 - A release ships two binaries from one workspace version, and promises only the wire format

## Status

Accepted - 2026-08-15.

It supersedes nothing. It **extends** ADR 0001 decision 3 and ADR 0003 into a case neither reached: both
version an on-disk shape, and neither had a distributed binary to version it against. ADR 0001 gave the
wire format its own version because "the domain types change when the search changes"; ADR 0003 gave the
cache a `FORMAT_VERSION` for the same reason one layer down. Until something ships, both numbers are
internal hygiene. Decision 5 below says which of them becomes a promise and which stays hygiene, and it
changes neither record's ruling about how the numbers are used.

## Context

Issue #28 has existed since 2026-08-14 with its first box unticked: "a concrete consumer names why a
standalone binary or release process is needed". On 2026-08-15 that consumer was named — the v0.1
milestone itself. Its nine closed issues are the search; #9 and #10 are the result and its validation;
what is missing is a way to hand any of it to someone. The process pays for itself twice, because v0.2 is
then a re-run rather than the same question asked again.

**Nothing in the tree builds a distributable binary.** `mise.toml` has no `build` task. `[tasks.cli]` runs
`cargo run --release -p popcircles-cli --`, and `README.md`'s Usage documents that and `cargo run` —
enough for every use to date, all of which happen in a clone.

**The version is already published, and it is `0.0.0`.** `crates/popcircles/src/report.rs` sets
`tool_version: env!("CARGO_PKG_VERSION")` in both envelope constructors, so every document the CLI writes
carries it, and all ten snapshots under `crates/popcircles/src/snapshots/` hold `"tool_version": "0.0.0"`
un-redacted — the only redaction in that module is `.result.great_circle_km` to six places. Both crates
declare `version = "0.0.0"` of their own; `[workspace.package]` carries `edition`, `rust-version`,
`license`, `repository` and `authors`, and no `version`.

**The documents and the binary already disagree about the tool's name.** Every envelope reports
`"tool": "popcircles"`. Cargo produces `popcircles-cli`, because `crates/popcircles-cli/Cargo.toml` has no
`[[bin]]` section and the binary takes the package name. #28's own goal line calls it "the `popcircles`
binary". Nothing has had to resolve this, because no artifact has ever been named.

**What the binary prints today was written for a maintainer.** `main.rs:30` already sets
`#[command(name = "popcircles", version)]`, and `popcircles-cli -- --version` answers `popcircles 0.0.0`,
so no clap feature is missing. But two things a clone never notices: the usage line reads
`Usage: popcircles-cli [OPTIONS] <COMMAND>`, because clap takes it from the binary's own name and not from
`name`; and the command has no description of its own, so clap falls back to the doc comment of the
`LogArgs` struct that `Cli` flattens. `popcircles --help` therefore opens with "ADR 0004 decision 3. There
is no boolean pair beside it: two flags standing in for a threshold is the shape `FU-04` names" — a record
citation and a register identifier, as the first thing a user reads, with no line anywhere saying what the
program does.

**crates.io is reachable today.** Neither manifest sets `publish`, so `cargo publish` in either crate
directory would attempt one.

**There is no shared release workflow to reuse.** `turboBasic/github-actions` carried, when read on
2026-08-15, `ci.yml`, `python-ci.yml`, `conventional-commits.yml`, `precommit-advisory.yml` and
`semantic-pull-request.yml`. `docs/ai/platform.md` "CI" already records this repo's inline Rust job as the
prototype for a shared `rust-ci.yml`; a release job here is the second such prototype rather than a fork
of anything.

**CI runs on `ubuntu-latest` and nothing else**, with `GIT_LFS_SKIP_SMUDGE: 1` at workflow level. Two
consequences for a release job: it needs that same variable or a tag build fetches the rasters, and no
gate in this repository has ever compiled this code on macOS — including the `unsafe` mmap site ADR 0003
decision 5 reviewed.

**All three format constants are at 1**: `report.rs`'s `SCHEMA_VERSION`, and the `FORMAT_VERSION` of
`table/cache.rs` and of `smallest/cache.rs`. Issue #45 will change the table header inside v0.1, which is
the immediate reason the promise has to be settled now rather than at the tag: a table at full resolution
is 933 120 000 cells and 7.5 GB, so what a version bump costs a user is a raster pass, not a download.

## Decision

**1. A release is a Git tag and a GitHub Release with binaries attached, and no registry.**
`publish = false` in both manifests, so the ruled-out channel is enforced rather than merely intended. The
release job runs `mise run ci` on the tagged commit before it builds — a tag can name any commit, `main`'s
required checks say nothing about the one it points at, and a red release cannot be unpublished the way a
late one can be waited for. Each artifact ships with a SHA-256 sum beside it. Signing and macOS
notarization are out of scope, which is a cost stated below and in the notes rather than a gap.

**2. Two artifacts: macOS arm64 and Linux x86_64, one GitHub-hosted runner each.** Named by target triple,
built with the existing `[profile.release]` — `codegen-units = 1`, `lto = "thin"` — so the artifact is the
binary `mise run cli` already produces. No cross-compilation toolchain enters `mise.toml`.

**3. One version for the workspace, and the manifest is its source of truth.** `version` moves into
`[workspace.package]` and both crates inherit it with `version.workspace = true`. The release job refuses a
tag whose name does not match that version, because the two agreeing is what makes `tool_version` in a
published document mean the release it came from.

**4. The artifact is named `popcircles`, and what it prints is written for whoever runs it.**
`[[bin]] name = "popcircles"` in the CLI crate: the documents already report that name, the README and #28
already use it, and it is also what fixes the usage line, which clap takes from the binary rather than from
`name`. The same ruling covers the help text, because a release is the moment both stop being internal —
the command carries a description of what the program does, and a maintainer's reasoning reaches a `//`
comment rather than the `///` clap publishes. `docs/ai/code.md` already restricts `///` to a non-obvious
WHY; this says where that rule has a user-visible edge.

**5. Only the wire format is promised. The two cache formats are internal.** `SCHEMA_VERSION` is the
compatibility contract a renderer or a downstream consumer may rely on across releases, and ADR 0001
decision 3 already gives it the versioning discipline that requires. A `FORMAT_VERSION` may change in any
release: a cache is refused and rebuilt, never migrated. The release notes say so in those terms, because a
tool that silently declines to reuse a 7.5 GB file it wrote last month reads as a bug otherwise.

## Consequences

**Positive**

- #28's remaining boxes become mechanical: a `mise run build` task, a workflow, and two documentation
  lines. What was blocking it was never the shell command.
- v0.2 costs nothing here. The process is a tag away, which is the argument that put this in v0.1.
- `publish = false` and the tag-versus-manifest check are both gates rather than conventions, so the two
  ways this could go wrong quietly — an accidental publish, a tag naming a version the binary does not
  report — are closed by machinery.
- The wire format's promise is now stated, which is what lets #9's renderer and anything downstream read
  `schema_version` and know what the guarantee behind it is.
- Declaring the cache internal keeps #45 cheap. It changes a header and invalidates every cache, and it can
  land inside v0.1 as a bug fix rather than as a migration.

**Negative / costs**

- **The macOS artifact is built by a runner no gate exercises.** CI is `ubuntu-latest` only, so the first
  time this code compiles for Apple silicon is on a tag — including the mmap site behind ADR 0003 decision
  5's `deny(unsafe_code)` exception. A macOS-only break surfaces at release time, when the tag is already
  pushed. Adding macOS to the CI matrix is the fix and is deliberately not taken here; the cost of taking
  it is a second runner on every pull request rather than on every tag.
- **An unsigned macOS binary is quarantined on download.** A user has to clear the attribute by hand, and
  the notes have to tell them so. Signing means a paid identity and a secret in CI, which is a decision
  with a consumer of its own and no consumer yet.
- **Moving to a real version rewrites all ten snapshots.** `tool_version` is in every one of them, so the
  first release's version bump lands as a ten-file snapshot diff. The `version-bumps` hook stays quiet by
  design — no key is dropped — so nothing gates that review beyond a reader.
- **Renaming the binary touches a passing test.** `crates/popcircles-cli/tests/commands.rs` reaches it
  through `CARGO_BIN_EXE_*`, whose suffix follows the binary name, and `mise run cli` names the package.
  Mechanical, but it is a change to the one test that runs the real binary.
- **`publish = false` forecloses crates.io until a record reopens it.** Anyone wanting `popcircles` as a
  dependency has to depend on the Git repository, which is the trade this takes deliberately and not a gap
  to be worked around in a manifest.
- **A user who upgrades pays a full rebuild whenever a cache format moves**, and at full resolution that is
  the raster read again. The promise is honest, but it is not free, and it is the half of decision 5 the
  notes must not bury.
- **Two artifacts are the floor, not the ceiling.** The first request for a Windows or an x86_64 macOS
  build reopens decision 2, and adding a target later means a release where some platforms have history and
  one does not.

## Alternatives considered

- **A tag and notes with no artifacts**, users building from a clone. This was the shape #28 assumed while
  it waited for a consumer, and it lost to the consumer it got: "properly released" has to mean someone
  without a Rust toolchain can run the thing, or the release is a changelog.
- **crates.io as well, or instead.** Lost on obligations with no beneficiary: the library's surface is
  eleven modules under active change, publishing takes on semver for all of it, and no consumer outside
  this repository exists. The CLI cannot be published without the library going first.
- **macOS arm64 only** — the machine this is developed on. Lost because the Linux artifact is the one
  someone else's server or CI job would use, and it is the cheaper of the two runners.
- **Adding macOS x86_64 and Windows.** Lost on the same test the others passed: no user named. Windows
  additionally has the memory-mapped 7.5 GB payload path untested, so shipping it would be a claim this
  repository has not earned.
- **Independent per-crate versions**, as if the library were published for others. Lost because
  `tool_version` comes from the library: a CLI-only release would leave every document reporting an
  unchanged version, which is the one thing that field exists to prevent.
- **Deriving the version from the tag** — `git describe` at build time, or a release tool that writes the
  manifest. Lost because `env!("CARGO_PKG_VERSION")` is read at compile time from the manifest, so the
  manifest has to carry the number anyway; a tag-derived version would produce a binary that differs from
  what a clone of the same commit builds.
- **Promising the cache formats too**, with a migration path per bump. Lost because a cache is derived data
  the tool can regenerate from an input the user already has, and #45 changes a header inside this
  milestone. Migrating a file the program can rebuild is cost with no consumer.
