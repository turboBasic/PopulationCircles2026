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

Eight facts settled here rather than met mid-task:

- **The ambiguous fixture is already committed, and three existing tests start reporting `Some` the moment
  1.2 lands.** `planted` in `smallest.rs`'s test module builds exactly the shape needed — a patch of equal cells with
  zero elsewhere, so a circle holds everyone without spanning the grid. Measured on this tree:

  | Fixture, at a share of one | Answer | Radii inside the slack |
  | --- | --- | --- |
  | `planted([((8,9),(15,16))], 100.0)` — issue #6's cluster | 1572 km | **7**: 1572, 1576, 1584, 1600, 1664, 1792, 2048 |
  | `planted([((4,4),(7,7))], 50.0)` — one cell | 0 km | **2**: 0, 1 |
  | `distinct` at a share of a half | 5770 km | **0** |

  So no new fixture is needed and none may be added: the cluster's span is 1572 to 2048 km, 477 km against
  an adjacent pair of 1, which is the shape the registry raster shows at 14 962 km. The tests that will
  start carrying a `Some` that nothing asserts are `the_answer_reaches_the_target_and_the_kilometre_below_it_does_not`
  , `no_radius_under_the_answer_reaches_the_target` and
  `a_share_one_cell_holds_is_answered_at_zero_kilometres` — none of them names `.ambiguity` today,
  so adding the field breaks no compile and changes what three tests assert about silently. 1.2 asserts it in
  all three rather than leaving it to be noticed.
- **The dense fixture at a share of one is the ceiling case, and it is separated.** It answers **20 016 km
  with `covers_whole_grid` true** — every cell is populated, so a circle holding everyone must span the grid
  — and its short radius falls 1 person shy against a slack of 2.94e-09. That is the control for the ceiling
  rule below, and `a_target_only_the_whole_grid_reaches_is_answered_at_the_ceiling` is where it
  already lives.
- **The reaching margin and the short margin are different worries and both count.** A reaching margin
  inside the slack says the answer may not reach the target at all; a short margin inside it says a smaller
  radius may also reach, which is minimality. The cluster fixture has the first and not the second — margin 0
  above, 306 persons below. The registry raster at a share of one has **both**. So the field is set when
  *either* margin is inside the slack, and a task testing only one of them has tested half of it.
- **No committed snapshot changes.** Every one of them is separated, and the field is absent in that case,
  so `FU-03`'s condition — which fires on a *modified* snapshot and exempts an added one — has nothing to
  fire on. A task that rewrites an existing `.snap` has broken the ordinary case; that is the ground rule
  below rather than a discovery to make later.
- **Only two sites construct a `Smallest`**, `smallest.rs:435` and `:478`, and no test builds one
  by literal. So the field can be added in one task without a sweep over call sites.
- **`RadiusLedger` is `get` and `put`, and `()` implements it with no recall.** ADR 0005
  decision 2 rests on this: the span is accumulated over the visit because there is nothing to read back.
- **No fixture pins the slack comparison, so a pure function has to.** Every ambiguous fixture is ambiguous
  by a margin of **exactly zero**, measured at unit magnitude and again at the 2^40 magnitude `scaled()`
  uses: a four-cell patch contributes two nonzero row terms, and two terms cannot be
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
  and the answer is decided. The dense-fixture case in 1.2 is the test that would catch it, but the rule is
  settled here rather than discovered there.

## Ground rules

These add to the normal task loop; they do not replace it.

- **The search's answer does not move, bit for bit.** No task touches either `found.population >= target.persons`
  — the climb's and the bisection's — or `circle::population`. `search`'s determinism tests are what say so, and a task that makes
  one of them fail has changed the answer rather than what is reported about it — which is ADR 0005
  decision 4 and a different record's question.
- **The field is absent when the answer is separated.** `git diff --stat` naming any existing file under
  `crates/popcircles/src/snapshots/` is a failure of the task that produced it, not a snapshot to accept.
- **Every name for the span says it is a floor.** Not `interval`, not `range`, not `bounds` — the ends are
  the widest pair *measured*, and the radii between them mostly were not, because the climb doubles. A name
  implying the ends were found is the same defect this plan closes, one level up.
- **No third method on `RadiusLedger`.** Decision 2's ground, and the seam is ISP-narrow deliberately.
- **No new fixture in `smallest.rs`.** `planted` already builds the ambiguous shape and three committed tests
  already run it; a fixture added beside them would be a second table making the same point, with the first
  still carrying the field with nothing asserting it. Where a suite genuinely has no such fixture — `report.rs` and
  `commands.rs` each build their own — the task says so.
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

- [x] **1.2 `Ambiguity` exists, and `Smallest` carries it accumulated over the radii the search probed.**
      A `pub struct Ambiguity` in `smallest.rs` beside `Smallest`, deriving
      `Debug, Clone, Copy, PartialEq, Eq` — `Copy` and `PartialEq` because `Smallest` derives them and this
      becomes one of its fields, `Eq` because all three fields are integers. Three `pub` fields:
      `lowest_km: u32` and `highest_km: u32`, the extreme radii whose recorded population was `within_slack`
      of the target, and `radii: u32`, how many probed radii fell inside. That third figure is what tells a
      reader the ends are far apart because the probes were: 7 radii across 477 km is a different statement
      from 477 of them. `Smallest` gains `pub ambiguity: Option<Ambiguity>`, `None` when no probed radius was
      inside.
      Accumulated in `smallest` as each `probe` returns — decision 2, and the `()` ledger is why. **Both the
      flag and the ends come from that one scan**, and the scan sees probed radii only: the ceiling is not
      one, per the ground rule, so the ceiling path at `smallest.rs:435` publishes whatever its probes
      accumulated rather than anything derived from the answer it returns. Both construction sites set the
      field, `:435` as well as `:478`.
      *Verify:* the three committed tests the preamble names gain an `.ambiguity` assertion, because that is
      where the field's behaviour already shows and a `Some` no test names is the outcome this task exists to
      prevent. In `the_answer_reaches_the_target_and_the_kilometre_below_it_does_not` and in
      `no_radius_under_the_answer_reaches_the_target`, the cluster at a share of one asserts
      `Some(Ambiguity { lowest_km: 1572, highest_km: 2048, radii: 7 })` — every figure a literal, which is
      that test's own standard; the same test's `distinct` half-share case asserts `None`, so both arms sit
      side by side. In `a_share_one_cell_holds_is_answered_at_zero_kilometres`, the single cell asserts
      `Some(Ambiguity { lowest_km: 0, highest_km: 1, radii: 2 })` — the degenerate span, and the case that
      says an answer of 0 km is still scanned.
      Then two tests of its own. `a_target_only_the_whole_grid_reaches_is_answered_at_the_ceiling` asserts
      `None`: the ceiling's own margin is exactly zero, its short radius is a measured 1 person below the
      target, and a scan that swept in the returned answer rather than the probed radii would report `Some`
      here — this is the assertion that fails when it does. And a warm ledger and a cold one report the
      **same** `Ambiguity`, which is what decision 2 buys and what reading the ledger would have cost.

- [x] **1.3 An unseparated answer emits one `warn` naming the span.** ADR 0005 decision 6, in `smallest`
      where the field is set, so the record and the field cannot disagree. Target is the module path, like
      every other library record. `warn` gets its first call site in the repository — the plan for ADR 0004
      noted it had none — and this is the level for it because it is the only thing this program says that
      means the answer is weaker than it looks. One record per unseparated answer, so a sweep warns per
      share; that repetition is the record's accepted cost and not a defect to fix here.
      *Verify:* a CLI integration test in `crates/popcircles-cli/tests/commands.rs`, over a **second** cache
      that file builds — a clustered one, since `Fixture::build`'s dense cells answer a share of one down the
      ceiling path and emit nothing. `planted` is private to the library's own test module, so this is that
      shape rebuilt rather than shared: `NODATA` outside a four-cell patch, which also carries the fixture to
      the ambiguous case through the nodata-to-zero conversion a real raster takes rather than around it.
      `smallest-for-share --share 100` at `--log-level warn` prints one record on stderr naming both ends of
      the span; the same command at `--share 25` against the existing dense cache prints nothing. stdout is
      one JSON document in both cases, which is what keeps decision 6 a second surface rather than a second
      answer.

## Phase 2 — the wire

- [x] **2.1 `report` publishes the span, absent when the answer is separated.** An `AmbiguityReport` beside
      `ShortBelowReport` in `report.rs`, and `SmallestReport` gains
      `#[serde(skip_serializing_if = "Option::is_none")] ambiguity` — `short_below`'s convention in that file, for its reason. `SCHEMA_VERSION` does not move: the field is additive under ADR 0001
      decision 3, and the measurement that makes that more than a claim is the next line.
      *Verify:* `git diff --stat crates/popcircles/src/snapshots/` names **no existing file** — every
      committed snapshot is separated, so none of them may move, and this is the check the ground rule
      exists for. One snapshot is **added**, over the cluster at a share of one — `report.rs` has its own
      `payload_over` and no `planted`, so the patch closure is one line there, which is
      what the two suites already do with fixtures. `rg -n 'ambiguity' crates/popcircles/src/snapshots/`
      matches in that added file and no other. Its `lowest_km` and `highest_km` are 1572 and 2048 with
      `radii` 7, so it pins the measured span rather than a degenerate one, and a change to how the span is
      accumulated moves those three numbers.

## Phase 3 — documentation, register, close-out

- [x] **3.1 The docs that describe the old claim describe the new one.**
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
