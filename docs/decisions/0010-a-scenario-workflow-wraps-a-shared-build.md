---
tags: [adr, code, popcircles]
created: 2026-08-15
decided: 2026-08-15
supersedes: null
superseded_by: null
---

# ADR 0010 - A scenario is its own workflow, and shared work is a workflow it calls

## Status

Accepted - 2026-08-15.

It supersedes nothing. It **extends** ADR 0006 decisions 1 and 2 into a case they did not reach: both
describe what a tag builds, and neither had a second scenario asking for the same build. What a release
does is unchanged.

## Context

Issue #50 needed the release build on demand, with no tag and nothing published. The first shape put a
`workflow_dispatch` trigger on `release.yml` and told the jobs apart by event: `if: github.event_name ==
'push'` on the gate and on publish, and this on the build —

```yaml
if: ${{ !cancelled() && needs.gate.result != 'failure' }}
```

which is there only because a skipped `needs` skips its dependents, and an `if` naming no status function
has `success()` folded into it. Three conditions, at three points, encoding one fact: which scenario is
running. Nothing checked they agreed, so the test suite grepped the workflow for its own guards.

`turboBasic/github-actions` still carries no release workflow, so the extraction is local.

## Decision

**1. A scenario gets a workflow of its own, and work two scenarios share moves to a `workflow_call`
workflow they both call.** `build-binaries.yml` holds the matrix, the build and the upload;
`release.yml` is a tag's gate, that call, and publish; `release-smoke.yml` is that call alone.

**2. No job is conditional on the event that started it.** A wrapper says what its scenario does by
which jobs it declares. A smoke cannot publish because its workflow has no publish job, which is a
property of the file rather than a guard to verify at run time.

**3. `ci.yml` stays inline.** A called job reports as `caller / job`, and `main`'s ruleset matches its
three required checks by name, so the same split there renames a required context and blocks every pull
request until the ruleset moves with it. That is a change to make deliberately, not as a side effect.

## Consequences

**Positive**

- Each entry point is read rather than evaluated: `release-smoke.yml` is a trigger and one call.
- The build exists once, so a matrix or an artifact-naming change cannot land in one scenario only.
- The guarantee that a smoke publishes nothing is structural, and the test that grepped for guards is
  replaced by one asserting the wrapper declares no write permission.

**Negative / costs**

- Three files where there was one, for a build of about thirty lines.
- Job names gain a prefix, which is invisible until something matches on them — decision 3 is that cost
  arriving somewhere it would hurt.
- A workflow-level `env` does not reach a called workflow, so `GIT_LFS_SKIP_SMUDGE` is declared in
  `build-binaries.yml` as well as in the wrappers that need it.
- `release.yml` no longer shows the build steps a tag runs; a reader follows one hop to see them.

## Alternatives considered

- **Keep the one workflow and its three conditions.** Lost on the line quoted above: a guard that has to
  be explained rather than read, in a file whose failure mode is a published release.
- **A composite action for the build steps.** Lost because what the two scenarios share is a job — runner
  selection and the matrix — which a composite action cannot hold, leaving the matrix duplicated.
- **Duplicate the build job in a second workflow.** Lost because the two would drift, and the drift shows
  up as a release differing from the smoke that certified it.
- **Extract into `turboBasic/github-actions` now.** Lost on the reason `ci.yml` is still inline: nothing
  there to extend yet, and a second repository is a worse place to learn the shape.
