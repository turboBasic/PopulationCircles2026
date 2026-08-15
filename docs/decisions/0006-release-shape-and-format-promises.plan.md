---
tags: [plan, code, popcircles]
created: 2026-08-15
---

# Implementation plan — ADR 0006, the release process and what it promises

**Status: in progress (2026-08-15).** Carries [ADR 0006](0006-release-shape-and-format-promises.md) into
the tree, which is the remaining work of issue #28. It builds the process and does **not** publish a
release: the tag waits on #9, #10 and #45, so the last thing this plan leaves behind is a workflow nobody
has fired.

Measured on this tree before drafting: 253 Rust tests and 34 Python tests passing, `mise run ci` green.

Six facts settled here rather than met mid-task:

- **`--version` already works.** `main.rs:30` is `#[command(name = "popcircles", version)]` and
  `cargo run -q -p popcircles-cli -- --version` prints `popcircles 0.0.0`. So no clap feature is added by
  this plan, ADR 0001 decision 2's feature list is untouched, and 1.1's verification is that same command
  answering `popcircles 0.1.0`.
- **The version reaches ten snapshots and nothing else.** `rg -l '"tool_version": "0.0.0"'
  crates/popcircles/src/snapshots/` names all ten, `report.rs:789` redacts only
  `.result.great_circle_km`, and no other file in the tree carries the literal. So the bump and the
  snapshot accept are one task, and the `version-bumps` hook stays quiet through it — a value changes, no
  key is dropped.
- **The binary is `popcircles-cli` and the usage line says so.** There is no `[[bin]]` section in
  `crates/popcircles-cli/Cargo.toml`, so the file takes the package name, and `--help` prints
  `Usage: popcircles-cli [OPTIONS] <COMMAND>` even though `name = "popcircles"` is set — clap takes the
  usage line from the binary. One rename fixes the artifact and the usage line together.
- **The help text's opening paragraph is `LogArgs`'s doc comment.** `Cli` flattens it and declares no
  description of its own, so clap falls back to the flattened struct's `///`, and `--help` opens with "ADR
  0004 decision 3 … the shape `FU-04` names". The fix is an `about` on `Cli` and `//` for the reasoning,
  not a rewrite of what that reasoning says.
- **`tests/commands.rs:166` is the only site the rename breaks.** It reaches the binary through
  `env!("CARGO_BIN_EXE_popcircles-cli")`, whose suffix follows the binary name, and it is the only
  `CARGO_BIN_EXE` in the tree.
- **The release workflow must carry `GIT_LFS_SKIP_SMUDGE: 1`.** `ci.yml` sets it at workflow level for the
  reason its comment gives, and a tag build that omits it clones the rasters — which is minutes and
  hundreds of megabytes per artifact, on a job whose failure mode is a published release.

## Ground rules

- **Every action is pinned to a full SHA with the version in a trailing comment.** `platform.md` "CI"
  allows a version tag for a reusable workflow call into `turboBasic/github-actions`, and that exception
  cannot apply here: no shared release workflow exists, so every `uses:` in this plan takes the SHA form,
  `actions/*` included.
- **No cross-compilation toolchain enters `mise.toml`.** ADR 0006 decision 2 is one runner per target; a
  task reaching for `cross`, `zig` or an added Rust target has left the decision behind.
- **The version lives in `[workspace.package]` only.** After 1.1, a crate manifest carrying a `version` of
  its own is a defect rather than a duplicate, because `tool_version` is read from one of them.
- **No task's verification needs a fetched raster or a network call.** The release job builds and uploads;
  nothing in this plan reads `data/`.
- **The release is not published by this plan.** A task that pushes a tag has done the one thing the status
  line above says this work leaves for later.

## Out of scope

- **Signing and macOS notarization.** ADR 0006 decision 1 puts them outside; they need a paid identity and
  a CI secret, and the consequence — a quarantined download — is documented instead, in 3.1.
- **macOS in the CI matrix.** The ADR states the gap as a cost it accepts: the release job is the first
  thing to compile this for Apple silicon. It becomes a register entry in 3.3 rather than a task, because
  closing it is a second runner on every pull request and that is its own trade.
- **Release-notes automation, a changelog file, or `cargo-release`.** The first tag's notes are written by
  hand. Automating a document that has never been written once would be tooling ahead of its subject.
- **crates.io.** Decision 1 forecloses it, and 1.1's `publish = false` is what makes that a gate. Reopening
  it takes a record.
- **A second consumer's convenience — Homebrew, a container image, `cargo binstall` metadata.** Each is a
  channel decision of its own, and decision 1 named one channel.

## Phase 1 — the manifests and what the binary presents

- [ ] **1.1 One workspace version at `0.1.0`, inherited by both crates, and neither is publishable.**
  `version = "0.1.0"` in `[workspace.package]`, `version.workspace = true` in both crate manifests,
  `publish = false` in both. Accept the ten snapshots the bump rewrites in the same commit — the diff is
  `tool_version` and nothing else, and a snapshot whose diff touches another line is a finding to report
  rather than accept.
  Verify: `cargo run -q -p popcircles-cli -- --version` prints `popcircles 0.1.0`;
  `rg -n '^version' crates/*/Cargo.toml` returns nothing; `rg -c '"tool_version": "0.1.0"'
  crates/popcircles/src/snapshots/*.snap` names ten files and `rg '0\.0\.0' crates/ docs/` returns nothing;
  `cargo publish --dry-run -p popcircles` fails naming `publish = false` rather than a registry error.

- [ ] **1.2 The binary on disk is `popcircles`.** `[[bin]] name = "popcircles"` in the CLI crate, with
  `tests/commands.rs:166` following it to `CARGO_BIN_EXE_popcircles`. `mise run cli` names the package and
  needs no change; check it anyway, because a `-p` that still resolves is not the same as a binary that
  still builds.
  Verify: `cargo build --release -p popcircles-cli` leaves `target/release/popcircles` present and no
  `target/release/popcircles-cli`; `cargo run -q -p popcircles-cli -- --help` opens its usage line with
  `Usage: popcircles [OPTIONS]`; `cargo test -p popcircles-cli --test commands` runs its 8 tests green.

- [ ] **1.3 `--help` describes the program, and the maintainer's reasoning is no longer published.** An
  `about` on `Cli` saying what the tool does in one line, in the terms `docs/ai/application.md` "What this
  program does" already uses. `LogArgs`'s struct-level `///` becomes `//` — the same words, above the
  `#[derive]` — so the ADR citation and the `global`-placement note stay where a maintainer meets them and
  reach no user. The `///` on the `log_level` field itself is genuine help text and stays.
  Verify: `cargo run -q -p popcircles-cli -- --help | head -1` names the program rather than ADR 0004;
  `cargo run -q -p popcircles-cli -- --help | rg -c 'ADR|FU-0'` returns 0; `rg -n 'ADR 0004 decision 3'
  crates/popcircles-cli/src/main.rs` still matches, on a `//` line.

## Phase 2 — the build task and the release job

- [ ] **2.1 `mise run build` produces the release binary from the committed lock, and CONTRIBUTING's loop
  names it.** The task wraps `cargo build --release --locked -p popcircles-cli` and states where the
  artifact lands; the existing `[profile.release]` is what it inherits, so nothing about the profile changes
  here. `--locked` because 2.2 publishes what this task builds: without it cargo may update `Cargo.lock`
  during the release build, and the artifact is then not provably the commit's. The cost is that a manifest
  edit fails this task until the lock is updated, which `platform.md` "Dependencies" asks for anyway.
  `CONTRIBUTING.md`'s "The loop" gains the line, since that block is where a contributor reads the commands
  off.
  Verify: `mise run build` exits 0 and `target/release/popcircles --version` prints `popcircles 0.1.0`; with
  a dependency's version edited in `crates/popcircles-cli/Cargo.toml` the task exits non-zero naming the
  lock file, and the edit reverts clean; `rg -n 'mise run build' CONTRIBUTING.md` matches inside "The loop";
  `mise run lint:docs` stays clean.

- [ ] **2.2 A release workflow builds both targets on a `v*` tag, gated on the tag agreeing with the
  manifest, and publishes from one job.** `on: push: tags: ['v*']`, `GIT_LFS_SKIP_SMUDGE: 1` at workflow
  level, a `timeout-minutes` on every job, and `permissions: contents: write` on the publishing job and
  nothing wider. Three jobs, ordered by `needs` rather than by position in the file, because a gate a
  builder does not declare is not a gate. **gate:** `mise run ci` on the tagged commit, and a step that
  fails when the tag name does not equal `v` plus the `[workspace.package]` version, naming both values it
  compared; `timeout-minutes: 20`, the bound `ci.yml` already puts on the same work. **build:**
  `needs: gate`, a matrix of `macos-latest` and `ubuntu-latest` calling `mise run build` rather than
  repeating the cargo line, renaming the binary to `popcircles-aarch64-apple-darwin` and
  `popcircles-x86_64-unknown-linux-gnu` with a `.sha256` beside it, and handing both to
  `actions/upload-artifact`; `timeout-minutes: 30`, longer than the gate because no gate has ever compiled
  this on macOS and the release profile's cost there is unmeasured. **publish:** `needs: build`, the only
  job holding `contents: write` — it downloads both artifacts and attaches them with `gh`, creating the
  Release if it does not already exist and uploading with `--clobber`. That shape answers the re-run
  deliberately: a leg that fails leaves no Release at all rather than a half-populated one, and re-running
  after a fixed leg replaces assets instead of colliding with them.
  Verify: `mise run lint:workflows` clean; `rg -n 'uses:' .github/workflows/release.yml | rg -v '@[0-9a-f]{40} #'`
  returns nothing; `rg -n 'GIT_LFS_SKIP_SMUDGE|timeout-minutes|contents: write' .github/workflows/release.yml`
  matches all three with `contents: write` once and a `timeout-minutes` per job; `rg -n 'needs:'
  .github/workflows/release.yml` shows build on gate and publish on build; `rg -n 'cargo build'
  .github/workflows/release.yml` returns nothing; the version-gate step's shell run by hand with a
  deliberately wrong tag exits non-zero and names both values it compared.

## Phase 3 — documentation, register, close-out

- [ ] **3.1 The two promises a user needs are written where a user reads them.** `README.md` gains a
  Releases section: the two artifacts and their triples, the `.sha256` beside each, the macOS quarantine
  attribute and the one-line command that clears it, and — in ADR 0006 decision 5's terms — that the JSON
  documents' `schema_version` is a contract across releases while a cached table or ledger may be
  invalidated by any release and rebuilt. `docs/ai/platform.md` "Dependencies" gains nothing: no dependency
  moved. The structure tree gains nothing either — `.github/workflows/` is already a root there.
  Verify: `rg -n 'quarantine|sha256' README.md` matches; `rg -n 'schema_version' README.md` matches in a
  sentence naming what may be invalidated; `mise run lint:docs` and `mise run lint:markdown` clean.

- [ ] **3.2 A maintainer can cut a release, and knows what to do when one fails.** `CONTRIBUTING.md` gains a
  Releasing section: bump `[workspace.package]`'s version, land it, tag `vX.Y.Z` on the merged commit, push
  the tag. Then the two failure cases, which differ on one fact — whether the publish job ran. A run that
  failed before it published left no Release, so the tag is retractable: re-run the workflow if the cause
  was the runner, and otherwise delete the tag, fix, and tag the same version again. Once a Release exists,
  the version is spent and the next attempt is a bump, because a moved tag lies to anyone who already
  fetched it. This is maintainer-facing, so it goes here and not in 3.1's README section.
  Verify: `rg -n '^## Releasing' CONTRIBUTING.md` matches; the section names both cases and the fact that
  separates them; `mise run lint:markdown` and `mise run lint:cspell` clean.

- [ ] **3.3 The register carries what this plan deliberately left, #28's boxes are ticked, and this plan is
  closed.** Two new entries in [`../follow-ups.md`](../follow-ups.md), each with a condition a sweep can
  answer: `FU-12`, no gate compiles this for Apple silicon while a release job ships a macOS artifact; and
  `FU-13`, a release exists while no artifact is signed, so a macOS user is told to clear an attribute by
  hand. Tick the boxes of #28 that this plan discharged and leave the rest, without closing the issue — the
  PR's `Closes #28` does that, per `platform.md` "Git". Then the status line above reads
  `**Status: complete (YYYY-MM-DD).**` and the Follow-ups section below holds the two identifiers.
  Verify: `rg -n '^### FU-1[23]' docs/follow-ups.md` names both; `gh issue view 28` shows the build-task and
  ADR boxes ticked and the issue still open; this file's status line reads complete and its Follow-ups
  section names `FU-12` and `FU-13`.

## Follow-ups

Written by 3.3, in [`../follow-ups.md`](../follow-ups.md): `FU-12`, `FU-13`.
