---
tags: [plan, code, popcircles]
created: 2026-08-14
---

# Implementation plan — ADR 0005, the span an ambiguous comparison cannot separate

**Status: in progress (2026-08-14).** Carries [ADR 0005](0005-ambiguous-minimality-is-reported.md) into the
tree, which discharges [`FU-09`](../follow-ups.md#fu-09---the-predicate-slack-is-reported-and-nothing-acts-on-it).
The last task closes that entry, and the PR carrying this plan is what lands it. It has an ordering
constraint the entry itself records: it changes every document publishing `predicate_slack_persons`, so it
goes in **before issue #9**, or the renderer is built against a shape that then moves.

Measured on this tree before drafting: 249 tests passing, `mise run ci` green, `cargo tree -e normal` at 33
crates.

Four facts settled here rather than met mid-task:

- **The ambiguous case needs a fixture with an ocean, and the committed one has none.** Measured, because
  the obvious guess is wrong: the dense 36 × 18 fixture at a share of one answers **20 016 km with
  `covers_whole_grid` true** — every cell is populated, so a circle holding everyone must span the grid and
  the search takes step 4's ceiling path. Its short radius falls 1 person shy against a slack of 2.94e-09,
  so that case is *separated* and produces no ambiguity at all. A fixture whose cells are nodata outside a
  small patch reproduces what the registry raster does: **1572 km, `covers_whole_grid` false, reaching
  margin exactly 0, and 7 visited radii spanning 1572 to 2048 km** — 477 km against an adjacent pair of 1.
  That is the shape the real table shows at 14 962 km, so the new coverage is an ocean fixture and the
  existing one stays the separated control.
- **The reaching margin and the short margin are different worries and both count.** A reaching margin
  inside the slack says the answer may not reach the target at all; a short margin inside it says a smaller
  radius may also reach, which is minimality. The ocean fixture has the first and not the second — margin 0
  above, 306 persons below. The registry raster at a share of one has **both**. So the field is set when
  *either* margin is inside the slack, and a task testing only one of them has tested half of it.
- **No committed snapshot changes.** Every one of them is separated, and the field is absent in that case,
  so `FU-03`'s condition — which fires on a *modified* snapshot and exempts an added one — has nothing to
  fire on. A task that rewrites an existing `.snap` has broken the ordinary case; that is the ground rule
  below rather than a discovery to make later.
- **Only two sites construct a `Smallest`**, `smallest.rs:419` and `smallest.rs:462`, and no test builds one
  by literal. So the field can be added in one task without a sweep over call sites.
- **`RadiusLedger` is `get` and `put`, and `()` implements it with no recall** (`smallest.rs:133`). ADR 0005
  decision 2 rests on this: the span is accumulated over the visit because there is nothing to read back.
- **No fixture pins the slack comparison, so a pure function has to.** Every ambiguous fixture is ambiguous
  by a margin of **exactly zero**, measured at unit magnitude and again at the 2^40 magnitude `scaled()`
  uses (`smallest.rs:1118`): a four-cell patch contributes two nonzero row terms, and two terms cannot be
  reordered into a rounding. `|margin| <= slack` is then satisfied whatever the slack is, so an
  implementation testing `margin == 0.0` would pass every fixture case in this plan. The comparison is
  therefore a function of two numbers, tested on margins straddling the slack, and the fixtures only check
  that it is wired in. The nonzero-residue case is the registry raster's — +1.34e-05 at 14 962 km — and
  ADR 0005's Context is where it is recorded, because `platform.md` "Testing" forbids a test that needs
  those bytes.
- **The ceiling radius is never probed, and that exclusion is load-bearing.** `CEILING_KM` is answered by
  `Table::whole` rather than searched, so it never reaches `probe`. At a share of one the target is `total`
  bit for bit and so is the ceiling's population, so its own margin is exactly zero *every* time the ceiling
  fires — and a scan sweeping in the returned answer rather than the probed radii would report the dense
  fixture at a share of one as unseparated, when its short radius is a measured 1 person below the target
  and the answer is decided. The dense-fixture case in 1.1 is the test that would catch it, but the rule is
  settled here rather than discovered there.

## Ground rules

These add to the normal task loop; they do not replace it.

- **The search's answer does not move, bit for bit.** No task touches the predicate at `smallest.rs:398`
  and `:448`, or `circle::population`. `search`'s determinism tests are what say so, and a task that makes
  one of them fail has changed the answer rather than what is reported about it — which is ADR 0005
  decision 4 and a different record's question.
- **The field is absent when the answer is separated.** `git diff --stat` naming any existing file under
  `crates/popcircles/src/snapshots/` is a failure of the task that produced it, not a snapshot to accept.
- **Every name for the span says it is a floor.** Not `interval`, not `range`, not `bounds` — the ends are
  the widest pair *measured*, and the radii between them mostly were not, because the climb doubles. A name
  implying the ends were found is the same defect this plan closes, one level up.
- **No third method on `RadiusLedger`.** Decision 2's ground, and the seam is ISP-narrow deliberately.
- **The scan is over radii that went through `probe`, and the flag and the span come from the same one.**
  Not over the returned answer, which is how the ceiling's always-zero margin gets in; not over the final
  pair alone, which would narrow the span to 2 km where the measurement is 1425. A second derivation for
  the flag is the defect, even where monotonicity makes it agree.

## Out of scope

- **Probing outward until a radius falls outside the slack.** ADR 0005's third alternative and the honest
  ideal, refused on termination: at a share of one no radius above the answer ever falls outside, so the
  walk runs to the ceiling and the already-slow case gets slower.
- **Shrinking the slack, and a compensated fold in `circle::population`.** Closed in the record's Context —
  the first by `mise run test:fold` measuring the real error as exactly 0 against a bound of 0.021839
  persons, the second as a change to the answer's bits that `search`'s determinism tests pin.
- **Applying the slack as a tolerance inside the comparison.** It would decide what this plan reports, and
  contradict `tolerance_persons: 0.0`, which issue #6 chose so a caller reads a result rather than this
  crate's constants.
- **A summary record at the end of a sweep.** ADR 0005 accepts one `warn` per unseparated share instead;
  a summary is a second surface saying what the records already said.
- **A fixture that reproduces a nonzero residue.** It would take many populated rows of large partials —
  the registry raster has 2160 — and a committed fixture of that size is a test CI cannot run for the reason
  `test:fold` is a task of its own. Measured instead: no small fixture rounds at all, so the boundary is
  pinned on the comparison itself and the residue case stays a figure in ADR 0005's Context.
- **Rendering the span.** Issue #9's, and the reason this plan precedes it rather than follows it.
- **`FU-11`'s cache geometry.** The other `due` entry, and it takes its own record reopening ADR 0003
  decision 3.

## Phase 1 — the domain

The result learns to say it. Declare the comparison, then wire it, then say it out loud — so the one part no
fixture can check is a commit whose whole content is the check.

- [x] **1.1 The slack comparison is a function of two numbers, and nothing calls it yet.**
      `pub fn within_slack(margin: f64, slack: f64) -> bool` in `smallest.rs`, `margin.abs() <= slack`, with
      no caller until 1.2 wires it. It takes the margin rather than a population and a target so that one
      function serves both sides — the reaching margin is `population - target` and the short margin is its
      negation, and an absolute value is what makes them the same question.
      **`pub` rather than private, and measured rather than chosen:** private and uncalled is
      `dead_code`, which `mise run lint:rust` promotes to an error, so a task whose whole point is that
      nothing calls it yet cannot be private without an `#[allow]` silencing a lint that is right. It
      belongs beside [`predicate_slack_persons`](../../crates/popcircles/src/smallest.rs) anyway, which is
      public and is what supplies the second argument.
      **This task exists because no fixture can check it.** Every ambiguous fixture is ambiguous by an
      exactly zero margin, measured, so `margin == 0.0` would pass every case in 1.2; the boundary is only
      reachable as two literals. Which makes this the one place the record's own measurement can be pinned
      as a test.
      *Verify:* one unit test walking the boundary. `within_slack(slack, slack)` is **true** — the slack is
      a bound on the error, so a margin equal to it is inside — and `slack.next_up()` against it is
      **false** while `slack.next_down()` is **true**, which pins the comparison as `<=` at the exact bit
      rather than at a rounding. `0.0` and `-0.0` are both true. The registry raster's own figures from
      ADR 0005's Context appear as literals both ways round: a margin of `1.34e-5` against a slack of
      `0.0120` is true, and the same margin against a slack of `1e-9` is false — the case the fixtures
      cannot reach. The 50% run's `121_814.0` against `0.0120` is false. A `f64::NAN` margin is **false**,
      which is deliberate rather than incidental: `search` documents that a NaN population cannot arise
      from a sanitised raster, so the arm exists to be stated rather than relied on, and a test is where it
      is stated.

- [ ] **1.2 `Ambiguity` exists, and `Smallest` carries it accumulated over the radii the search probed.**
      A `pub struct Ambiguity` in `smallest.rs` beside `Smallest`, `Copy` because `Smallest` is, holding the
      lowest and highest radius in kilometres whose recorded population was `within_slack` of the target, and
      how many probed radii fell inside. That third figure is what tells a reader the ends are far apart
      because the probes were: nine radii across 1425 km is a different statement from 1425 of them.
      `Smallest` gains `pub ambiguity: Option<Ambiguity>`, `None` when no probed radius was inside.
      Accumulated in `smallest` as each `probe` returns — decision 2, and the `()` ledger is why. **Both the
      flag and the ends come from that one scan**, and the scan sees probed radii only: the ceiling is not
      one, per the ground rule, so the ceiling path at `smallest.rs:419` publishes whatever its probes
      accumulated rather than anything derived from the answer it returns. Both construction sites set the
      field, `:419` as well as `:462`.
      *Verify:* four unit tests in `smallest.rs`, over the two fixtures the measured facts above name. An
      ocean fixture — nodata outside a small patch, so a circle holds everyone without spanning the grid —
      at a share of one reports `Some`, with `lowest_km` strictly below `highest_km` and `radii` at least
      three; the measured figures are 1572 to 2048 km over 7 radii, and the test pins the inequality while
      2.1's snapshot pins the numbers. The dense fixture at 0.25 reports `None`, the ordinary case staying
      ordinary. The dense fixture at a share of one reports `None` too, and that is the ceiling case: its
      own margin is exactly zero, its short radius is a measured 1 person below the target, and a scan that
      swept in the returned answer would report `Some` here — this is the test that fails when it does. And
      a warm ledger and a cold one report the **same** `Ambiguity`, which is what decision 2 buys and what
      reading the ledger would have cost.

- [ ] **1.3 An unseparated answer emits one `warn` naming the span.** ADR 0005 decision 6, in `smallest`
      where the field is set, so the record and the field cannot disagree. Target is the module path, like
      every other library record. `warn` gets its first call site in the repository — the plan for ADR 0004
      noted it had none — and this is the level for it because it is the only thing this program says that
      means the answer is weaker than it looks. One record per unseparated answer, so a sweep warns per
      share; that repetition is the record's accepted cost and not a defect to fix here.
      *Verify:* a CLI integration test in `crates/popcircles-cli/tests/commands.rs`, over a **second**
      fixture that fixture builds — the ocean one, since `Fixture::build`'s dense cells answer a share of
      one down the ceiling path and emit nothing. Its cells are `NODATA` outside the patch, so the fixture
      reaches the ambiguous case through the nodata-to-zero conversion a real raster takes rather than
      around it. `smallest-for-share --share 100` at `--log-level warn` prints one record on stderr naming
      both ends of the span; the same command at `--share 25` against the existing dense fixture prints
      nothing. stdout is one JSON document in both cases, which is what keeps decision 6 a second surface
      rather than a second answer. The two suites build their own fixtures rather than sharing one, which is
      what they already do.

## Phase 2 — the wire

- [ ] **2.1 `report` publishes the span, absent when the answer is separated.** An `AmbiguityReport` beside
      `ShortBelowReport` in `report.rs`, and `SmallestReport` gains
      `#[serde(skip_serializing_if = "Option::is_none")] ambiguity` — `short_below`'s convention at
      `report.rs:521`, for its reason. `SCHEMA_VERSION` does not move: the field is additive under ADR 0001
      decision 3, and the measurement that makes that more than a claim is the next line.
      *Verify:* `git diff --stat crates/popcircles/src/snapshots/` names **no existing file** — every
      committed snapshot is separated, so none of them may move, and this is the check the ground rule
      exists for. One snapshot is **added**, over the ocean fixture at a share of one, and
      `rg -n 'ambiguity' crates/popcircles/src/snapshots/` matches in that file and no other. Its
      `lowest_km` and `highest_km` are 1572 and 2048 with `radii` 7, so it pins the measured span rather
      than a degenerate one, and a change to how the span is accumulated moves those three numbers.

## Phase 3 — documentation, register, close-out

- [ ] **3.1 The docs that describe the old claim describe the new one.**
      [`application.md`](../ai/application.md) "Approach" step 4 ends "minimality holds for a target further
      from a plateau than the summation slack the result carries" — a condition it left to a reader, which
      the result now answers for itself. That clause is what moves, and it moves by naming the field rather
      than by restating the arithmetic, which `smallest`'s own documentation owns. `README.md`'s Circles
      section shows a 50% result whose `predicate_slack_persons` is published and which is *separated*, so
      what it owes is one sentence saying when the field appears — not a second worked example, because a
      share of one is a case the section does not otherwise cover and adding it would double the output
      already quoted there.
      *Verify:* `mise run lint:docs` and `mise run lint:markdown` green, and
      `rg -n 'minimality holds for a target further' docs/` returns nothing — the old clause is gone rather
      than sitting beside its replacement.

- [ ] **3.2 `FU-09` is closed, the register says what closed it, and this plan is closed.**
      Status to `closed` with the date, naming ADR 0005 as the record and this plan as the implementation,
      and stating the one way what landed departs from the Fix as written: the Fix names `[short, reaching]`
      as the honest bracket, and that pair is 2 km wide where the run measured 1425 km it cannot separate,
      so the span is accumulated over the visit instead. The entry keeps its condition, which stays true and
      stays checkable — the field is published, and now something acts on it.
      Then this file's status line reads `**Status: complete (YYYY-MM-DD).**` and its Follow-ups section
      holds identifiers only. No issue box moves: `FU-09` is a register entry, not a roadmap step, and
      roadmap #11 has no box for it.
      *Verify:* `rg -n '^### FU-' docs/follow-ups.md` still lists ten entries and the Status lines read:
      02 closed, 03 dormant, 04 closed, 05 closed, 06 dormant, 07 closed, 08 dormant, 09 **closed**,
      10 closed, 11 due. `FU-09`'s own sweep,
      `rg -n 'predicate_slack_persons' crates/popcircles/src/report.rs crates/popcircles-cli/src scripts`,
      still matches — the field is still published, which is why the entry closes on the fix rather than on
      the condition lapsing. `git log --oneline` shows one commit per task above and no merge commit, and
      `mise run ci` green.

## Follow-ups

None yet. The last task replaces this section with the identifiers it produced, or with this sentence if it
produced none.
