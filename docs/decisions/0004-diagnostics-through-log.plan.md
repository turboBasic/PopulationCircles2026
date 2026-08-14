---
tags: [plan, code, popcircles]
created: 2026-08-14
---

# Implementation plan — ADR 0004, diagnostics through `log`

**Status: in progress (2026-08-14).** Carries [ADR 0004](0004-diagnostics-through-log.md) into the tree,
which is issue #8's last four boxes: `--log-level`, the `info` narration, the `debug` bracketing, and the
box saying a record picks the facade. Those four are the whole of what keeps #8 open after #38, so the
last task ticks them and roadmap #11's sixth step, and the PR carrying this plan closes the issue.

Measured on this tree before drafting: 243 tests passing, `mise run ci` green, 32 crates in
`cargo tree -e normal`. `rg -n 'print!|println!|eprintln!' crates/popcircles/src` is empty, so the
library prints nothing today and this plan is adding a seam rather than replacing ad-hoc output.

The ruling is ADR 0004's and no task below reopens it: `log` in the library, a hand-written `log::Log` in
the CLI, `--log-level` as the only control, elapsed milliseconds from `std::time::Instant` and no
wall-clock, and progress left exactly where ADR 0001 decision 4 put it.

Two measured facts Phase 2 walks into, settled here rather than met mid-task:

- **The CLI suite asserts stderr is empty on every success.** `one_document_naming_the_fixture` in
  `crates/popcircles-cli/tests/commands.rs` is that assertion for all four search commands, and the file's
  header states it as the contract. `info` is the default level, so the first record 2.1 emits turns every
  one of those cases red and the 243-test baseline above stops holding. 2.1 is the task that moves the
  contract, and it narrows it rather than dropping it.
- **`rg -n 'verbose|quiet' crates/popcircles-cli/src` matches once, and the match is prose** — the word
  "quiet" in a comment at `main.rs:1266`, not a flag. That sweep is half of `FU-04`'s condition and this
  plan closes the entry, so one of the two has to move. 3.2 moves the condition to the flag shape it was
  always describing rather than 1.2 rewording English out of a comment: the library itself uses "quietly"
  twice (`grid.rs:909`, `geodesy.rs:486`), so a condition that fires on the word bans a word rather than a
  flag, and would keep firing on the next comment that needs it.

## Ground rules

These add to the normal task loop; they do not replace it.

- **No `verbose` and no `quiet` flag, ever.** `--log-level` is the only control over what is emitted.
  `FU-04`'s condition watches for exactly the other shape — a CLI hand-rolling level filtering out of two
  boolean flags — and this plan closes that entry, so shipping the shape it would have fired on is how a
  closed entry becomes a lie.
- **The library never prints.** Every diagnostic in `crates/popcircles/` goes through `log`;
  `println!` and `eprintln!` stay absent from it. That is `application.md` "Architecture" before it is
  `FU-04`'s first sweep.
- **A `debug` bracket is a pair, added in one commit, closed on every exit path.** A bracket that only
  closes when the step succeeds reports a duration that is missing precisely when someone is reading the
  log to find out why something failed. Every region box 7 names is threaded with `?`, so the closing
  record is written by a guard's `Drop` rather than by hand at each exit — 2.4 builds the guard and states
  the two properties it needs.
- **No dependency beyond `log`.** No `env_logger`, no `tracing`, no colour crate and no datetime crate.
  Elapsed time comes from `std::time::Instant`, which is why none of those is needed.
- **Progress and diagnostics do not touch, and they share one stream.** `--log-level` does not govern
  `StderrProgress`, no progress figure is routed through `log`, and no log line goes through the sink:
  that is ADR 0004 decision 3, and it is about routing. The file descriptor is a separate fact and needs
  an owner. `StderrProgress::advance` leaves the cursor mid-line by design, so `StderrLog` clears that
  line before writing when stderr is a terminal, and is the only thing that touches the column. The
  knowledge runs one way and the direction is the point: a logger that clears a line something may have
  drawn learns nothing about the meter, whereas a meter redrawn after a log line would have to hold the
  logger. Two consequences follow and are **owned rather than fixed**, because ADR 0004 froze
  `StderrProgress`: `advance` redraws only when the whole percent changes, so after a cleared line the
  meter stays dark until the next tick — at `debug`, with a record per level, mostly dark — and
  `finish` emits its newline whenever a percent was ever drawn, so a `most-populous` run at `debug` ends
  with a blank line, `search::most_populous` having no trailing `advance` to redraw before it.

## Out of scope

- **`RUST_LOG`.** ADR 0004 decision 4: box 5 asks for a flag, and an environment variable is a second way
  to say the same thing, with a precedence question attached.
- **Structured or JSON diagnostics.** The machine-readable surface is the document on stdout, which
  `report` owns. A second one would be two answers to what a run produced.
- **Per-operation durations reported rather than derived, and benchmarks.** Issue #10's, and ADR 0004
  names it as the condition under which that record is reopened. Box 7 asks for a subtraction and gets one.
- **The `trace` level.** Box 5 names four levels, and the reason a fifth is refused is not that it would
  have no call sites — under this plan neither `error` nor `warn` has one either. It is that a level is
  accepted as a *threshold a reader asks for*, and `trace` adds no threshold `debug` does not already
  give: the only granularity below `debug`'s is the block and the kernel, which the next bullet rules out
  at every level.
- **A line per candidate block or per kernel.** The measured 50% run examined 549 940 blocks and built
  15 891 kernels; box 7's granularities are the search level and the radius trial, which are 144 and 24.
  Logging the level below would make `debug` unreadable and slower than the search.
- **Routing the failure sentence through `log`.** `main.rs:292` prints the message and returns the exit
  code, and it stays an `eprintln!`. A run that failed says why at every level, because no `--log-level`
  should be able to suppress the reason a non-zero exit happened.

## Phase 1 — the seam

The plumbing, and nothing narrates yet. Both tasks leave observable output unchanged, which is what makes
them separately verifiable: after 1.2 the flag exists, is honoured, and has nothing to say.

- [x] **1.1 `log` is a workspace dependency and both crates take it.**
      `log = { version = "0.4", features = ["std"] }` in the root
      `Cargo.toml`'s `[workspace.dependencies]` — the feature because the shape 1.2 installs needs it:
      `set_boxed_logger` and `impl Error for SetLoggerError` are both behind `std` and `log` enables no
      features by default, so a boxed logger with a handled `Result` does not compile without it. (A
      `set_logger` over a `OnceLock` would; the feature is what the chosen shape costs, not what the crate
      requires.) `std = []` in `log`'s own manifest, so it pulls nothing and the figure below is the same
      with it as without. Then `log.workspace = true` in
      `crates/popcircles/Cargo.toml` and in `crates/popcircles-cli/Cargo.toml` — the library because it
      emits, the CLI because it both emits its own records and implements the sink. `Cargo.lock` is
      updated in the same commit. Nothing calls it yet.
      *Verify:* `rg -n 'log.workspace = true' crates/popcircles/Cargo.toml crates/popcircles-cli/Cargo.toml`
      matches twice, and `cargo tree -e normal | rg -o '[a-z0-9_-]+ v[0-9]' | sed 's/ v[0-9]//' | sort -u | wc -l`
      prints 33 where it printed 32 — one new crate and no transitive ones, which is the figure ADR 0004
      decided on. 243 tests still pass.

- [x] **1.2 `--log-level` is a global flag, and the CLI installs a `log::Log` that honours it.** A
      `LogArgs` flattened onto `Cli` itself, with `global = true` on the `#[arg]` **inside** it rather
      than on the `#[command(flatten)]` — the attribute is the argument's — so every command takes it
      rather than each subcommand declaring it, defaulting to `info` and parsed by a `value_parser`
      mapping the four names box 5 gives — `error`, `warn`, `info`, `debug` — onto `log::LevelFilter`.
      `trace` is **not** a name it accepts. Then `StderrLog` beside `StderrProgress`, in two pieces so the
      format is testable without a process: a `fn line(elapsed: Duration, record: &Record) -> String`
      rendering `<elapsed ms> <LEVEL> <target>: <message>`, and a `log::Log` that holds the start
      `Instant`, calls it, and writes the result to stderr — clearing the meter's line first when stderr
      is a terminal, per the ground rule. The format is what box 7's subtraction rests on, and a pure
      function is the only way a test pins it. The `Instant` is taken as the **first statement of `main`**,
      before `Cli::parse()`: ADR 0004 decision 2 says elapsed since the process started, and a clock
      started after argument parsing and the install is not that. The logger is installed before `run` is
      called, with `log::set_max_level` set from the flag. Installing it is infallible in practice — it
      happens once — but the `Result` is handled rather than unwrapped, because a failed install must not
      take the run down: a diagnostic that cannot be printed is not a reason to lose a result, which is
      `StderrProgress::advance`'s existing reasoning. **`enabled()` reads the filter the `StderrLog`
      holds, not `log::max_level()`.** The latter is process-global and `cargo test` runs a binary's unit
      tests as parallel threads in one process, so an `enabled()` built on it makes the test below depend
      on which other test called `set_max_level` last.
      *Verify:* `mise run cli -- distance 0 0 0 90 --log-level debug` prints one JSON document on stdout
      and **nothing** on stderr, because no call site emits yet — that is what makes this task's change
      invisible and the next task's visible. `--log-level trace` and `--log-level nonsense` are both usage
      errors at exit 2. Four CLI tests: the parser maps the four names and rejects `trace`, the flag is
      accepted after any subcommand as well as before it, `line` renders a known elapsed and `Record` to
      the exact expected string, and a `StderrLog` set to `warn` reports `enabled()` false for an `info`
      record and true for an `error` one.

## Phase 2 — what it says

The call sites. Read the actual output at the end of this phase rather than trusting the tests: what these
tasks are for is a human watching a terminal, and no assertion checks that a line is worth reading.

- [x] **2.1 `info` narrates the resolved table and the answer, from the CLI.** Box 6's two ends for the
      four search commands, both known at the binary edge and neither of which the library should be asked
      for: one record after the table is resolved naming the cache path, the digest, the decimation and the
      grid's shape, and one at the end naming the answer — the radius for `smallest-for-share`, one record
      per settled share for `sweep`, the centre and population for the other two. `CachedTable::open` is
      where the first belongs, since it is already the one place a cache is opened.
      In the same commit the CLI suite absorbs the narration rather than breaking on it: `Fixture::flags`
      passes `--log-level error`, so the four success cases stay silent and
      `one_document_naming_the_fixture` **keeps** its `stderr.is_empty()` — with the flag that assertion is
      strictly stronger than dropping it and it is the contract the file's header states, so what moves is
      the reason it holds, not the claim. Its doc comment says so. One new test carries what this task adds:
      the same invocation narrates on stderr at `info` and is silent at `warn`, with byte-identical stdout
      either way. `a_digest_naming_another_table_is_missing_data_and_prints_nothing` keeps asserting stderr
      is non-empty — that sentence is `main`'s `eprintln!` and no level governs it — and the other failure
      case asserts nothing about the stream, which needs no change. `Fixture::flags`'s "nine flags" comment
      counts a tenth.
      *Verify:* the new test passes and the suite is green with one test more than 1.2 left it.
      `mise run cli -- population-at …` against a cache at `--log-level info` prints the two records on
      stderr and one document on stdout, and `--log-level warn` prints nothing on stderr — the same
      invocation, so the flag is doing the work rather than the code path. stdout is byte-identical between
      the two runs, which is the property box 6 exists to protect. The cache may be the 5 arcmin table or
      the synthetic one `Fixture::build` writes, which reaches the same code path with no raster.

- [x] **2.2 `info` narrates the raster and cache `table build` reads and writes.** The other half of box
      6's "the raster and cache in use", and its own task because `table build` opens no cache — 2.1's
      record is in `CachedTable::open` and this command never calls it. One record naming the raster path
      and the decimation before the pass, one naming the header and payload it published after.
      **This is the one task in the plan whose claim is not machine-checkable**, and it is separated so
      that is visible rather than buried in a verify that passes without it: the only raster in the tree is
      the 30 arc-second LFS object, so exercising these two records needs fetched content, and
      `platform.md` "Testing" forbids a test that depends on it.
      *Verify:* by hand, on a machine that has run `mise run data:pull`:
      `mise run cli -- table build --log-level info …` against that raster prints the two records naming
      the file it read and the two paths it wrote, and prints them **before and after** the meter rather
      than interleaved with it. Nothing in `mise run ci` covers this task, which is why the record above
      says the raster path is the figure a reader checks by eye.

- [x] **2.3 `info` marks the phase boundaries inside the library.** The boundaries a run has: the table
      build (`table::build`), the search over radius entering and leaving (`smallest::smallest`), and each
      radius the search settles (`smallest::probe`, one record naming the radius and whether the ledger
      answered it). Targets are the module paths, so a reader can tell a library record from the CLI's.
      Not the fixed-radius search's levels — those are `debug`'s, in 2.5.
      This is the commit that makes the library emit, so it is the commit that corrects
      [`application.md`](../ai/application.md): its Approach paragraph lists ten modules as "pure
      computation with no I/O" and its Architecture section says the domain "does not read, write, print,
      or format". A reader arriving after this task needs both sentences to route to ADR 0004 instead of
      reading a contradiction. **The correction is one clause about the facade, not a list of which modules
      emit** — no record reaches a stream until the CLI's subscriber writes it, which is true of every
      module in that sentence and stays true as 2.4 to 2.6 add `search` to the emitting set. A per-module
      correction would need re-editing three more times and would be wrong in between.
      *Verify:* a smallest-for-share run at `--log-level info` prints one record per settled radius, and
      the count of those lines equals `stats.radii_evaluated + stats.radii_reused` in the document on
      stdout — the document and the log agree about how much work happened, which is the check that would
      catch a record emitted on the wrong side of the ledger's early return. `rg -n 'print!|println!|eprintln!' crates/popcircles/src`
      still returns nothing, and `mise run lint:docs` and `mise run lint:cspell` are both green.

Box 7's three remaining granularities are 2.4 to 2.6, one task each. They are separate commits because they
are separate call sites in two crates, and because the warm-ledger verify belongs to the radius trial alone.

- [x] **2.4 A `Bracket` guard, and the table build or load wears the first pair.** Box 7's first
      granularity, and the task that settles how every pair closes. **The end record is written by `Drop`,
      not by hand.** Every region box 7 asks to bracket is threaded with `?` — `search.rs:382`, `405` and
      `416`, `smallest.rs:317`, `320`, `426` and `447`, and `build`'s row callback — so a hand-written end
      line is a line at every one of those exits and a line a later `?` silently skips, which is the ground
      rule's failure mode rather than its satisfaction. Two properties the guard needs beyond `Drop`:
      **its target is the caller's**, passed in at construction as `module_path!()` from the call site,
      because that macro expands where it is written and a guard calling it would stamp every end record
      with its own module and make 2.3's "targets are the module paths" false for half the pairs; and it
      carries a figure the caller may set before the scope ends, which is what 2.5 puts the kernel count
      on. It is `pub` in a module of its own in the library, since the CLI's own pair needs the same shape
      and `progress` may not hold it — the ground rule keeps those two apart. `application.md`'s module
      sentence gains it here.
      The first pair is box 7's "table build or load": `table::build` in the library, and `CachedTable::open`
      in the CLI, whose three `?`s are exactly why the guard exists.
      *Verify:* a `population-at` run at `--log-level debug` emits the load pair, and its two elapsed
      figures subtract to a duration between zero and the run's own wall time — the claim box 7 makes and
      the reason the elapsed prefix exists. A run whose cache is absent still emits the closing record of
      the pair it opened, checked by pointing `--cache` at a path that is not there: that is the `Drop`
      doing the work, and hand-written end lines are what it rules out. A unit test on the guard covers the
      target: a bracket constructed in one module reports that module on both records.

- [x] **2.5 Each search level is bracketed, and its end record carries the kernels built.** Box 7's second
      granularity. **Kernel placement is not a bracket of its own**, because there is no discrete placement
      step to open one around — kernels are built lazily inside the per-block loop, through
      `HeldKernel::get`, 15 891 times in the measured run, which is precisely the "line per kernel" the Out
      of scope section refuses. What box 7 wanted from that entry rides on the level's end record instead,
      as the delta over that level: the same fact at a granularity that stays readable.
      **The delta is `exact.built + widest.built`, not `stats.kernels_built`.** That field is assigned once
      after the level loop exits, so a delta read from it is zero on every level. Read the two counters
      directly, and note they stand at 2 before the first level opens: `HeldKernel::new` counts the seed
      kernel it builds, twice, which is a figure the first level's delta must not attribute to itself.
      *Verify:* a `most-populous` run at `--log-level debug` emits level pairs numbering `stats.levels` from
      the document on stdout, and the level deltas sum to `stats.kernels_built` less the two seeds. Begin
      and end records are equal in number **per operation name** — `rg -cw`, counted per name rather than
      globally, since equal global totals survive one step's pair being mismatched against another's.

- [x] **2.6 Each radius trial is bracketed, warm ledger included.** Box 7's third granularity, in
      `smallest::probe`. A radius the ledger answers is bracketed like any other — it opens, it closes, and
      its near-zero duration is what says a rerun did no work, where emitting nothing would leave a reader
      unable to tell that from a radius never tried.
      *Verify:* a smallest-for-share run at `--log-level debug` over a **warm** ledger, which is the path
      the ground rule was written for and the one a first run never takes: every radius returns from
      `probe`'s early return before any search begins, every pair still closes, and the count of
      radius-trial pairs equals `stats.radii_reused` in the document. Then the same run over a fresh
      ledger, where the count equals `stats.radii_evaluated` and each pair encloses that radius's level
      pairs from 2.5.

## Phase 3 — documentation, register, close-out

- [x] **3.1 README says what the flag does and why there are two mechanisms.** A short block in the Usage
      section: `--log-level`, the four names, the default, and the distinction a reader will otherwise ask
      about — a log says what happened and the progress meter says how far a run has got, so a quiet run
      may still draw a meter and a verbose one may draw none. Say that `RUST_LOG` does nothing, because a
      contributor's reflex is to reach for it and silence is a worse answer than a sentence.
      *Verify:* `mise run lint:markdown` and `mise run lint:cspell` green, and the block names no level the
      parser rejects — `trace` does not appear in it.

- [ ] **3.2 `FU-04` is closed, and the register says what closed it.** Status to `closed` with the date,
      naming ADR 0004 as the record its Fix demanded and this plan as the implementation, and stating the
      two ways what landed departs from the Fix as written: `log` rather than `tracing` on the emitting
      side, and a hand-written subscriber rather than `tracing-subscriber` on the consuming side, both on
      the measurements in ADR 0004's Context. The cost table in that entry is left as written — it is the
      figure that was true when it was measured, and ADR 0004 carries the correction.
      The entry's second condition moves in the same edit, to the flag shape it was always describing: a
      `--verbose` or `--quiet` long name, a field of either name in an args struct, or `short = 'v'` or
      `short = 'q'` aliasing one. A closed entry keeps its condition so a later reader can tell "never
      fired" from "cannot fire", and one that fires on an English word in a comment cannot be evaluated
      without a judgment call the register's own bar forbids. The two letters are spelled out rather than
      matching `short =` at large: a condition firing on any short alias the CLI ever grows would ban a
      mechanism instead of a flag, which is the same defect as banning a word.
      *Verify:* `rg -n '^### FU-' docs/follow-ups.md` still lists ten entries and the Status lines read:
      02 closed, 03 dormant, 04 closed, 05 closed, 06 dormant, 07 closed, 08 dormant, 09 due, 10 closed,
      11 due. Both of `FU-04`'s sweeps as the entry now words them come back clean on this tree:
      `rg -n 'print!|println!|eprintln!' crates/popcircles/src` and
      `rg -n "^\s*(verbose|quiet)\s*:|--verbose|--quiet|short = '[vq]'" crates/popcircles-cli/src` are both
      empty — the second was run on this tree while the plan was drafted and matches nothing, where the
      word-based sweep it replaces matches once.

- [ ] **3.3 The issue's last four boxes are ticked, the roadmap's step is ticked, and the plan is
      closed.** Tick boxes 5 to 8 of issue #8 — the three logging boxes and the box saying a record picks
      the facade, which ADR 0004 is — leaving all eight ticked. Tick roadmap #11's `#8` box, which this
      plan is what makes true. **The issue is not closed by hand**: the PR carrying this plan is what
      closes it, with `Closes #8` in its body, per `platform.md` "Git". Then this file's status line reads
      `**Status: complete (YYYY-MM-DD).**` and its Follow-ups section holds identifiers only.
      *Verify:* `gh issue view 8` shows all eight boxes ticked and the issue still `OPEN`;
      `gh issue view 11` shows `#8` ticked; `git log --oneline` shows one commit per task above and no
      merge commit; `mise run ci` green.

## Follow-ups

- [FU-04](../follow-ups.md#fu-04---diagnostics-have-no-facade) — `closed` by 3.2; ADR 0004 is the record
  its Fix required.
