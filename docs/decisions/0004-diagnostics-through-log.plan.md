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

**`rg -n 'verbose|quiet' crates/popcircles-cli/src` matches once today, and it is a false positive.**
The match is the word "quiet" in a prose comment at `main.rs:1266`, not a flag. It matters because that
sweep is half of `FU-04`'s condition, and this plan closes `FU-04` — an entry closed while its own sweep
returns a match a reader has to dismiss by hand is an entry the next sweep cannot evaluate. 1.2 rewords
the comment.

## Ground rules

These add to the normal task loop; they do not replace it.

- **No `verbose` and no `quiet` flag, ever.** `--log-level` is the only control over what is emitted.
  `FU-04`'s condition watches for exactly the other shape — a CLI hand-rolling level filtering out of two
  boolean flags — and this plan closes that entry, so shipping the shape it would have fired on is how a
  closed entry becomes a lie.
- **The library never prints.** Every diagnostic in `crates/popcircles/` goes through `log`;
  `println!` and `eprintln!` stay absent from it. That is `application.md` "Architecture" before it is
  `FU-04`'s first sweep.
- **A `debug` bracket is a pair, added in one commit, closed on every exit path.** A task that adds a
  "begin" line adds its "end" line beside it, including on the error return. A bracket that only closes
  when the step succeeds reports a duration that is missing precisely when someone is reading the log to
  find out why something failed.
- **No dependency beyond `log`.** No `env_logger`, no `tracing`, no colour crate and no datetime crate.
  Elapsed time comes from `std::time::Instant`, which is why none of those is needed.
- **Progress and diagnostics do not touch.** `--log-level` does not govern `StderrProgress`, no progress
  figure is routed through `log`, and no log line goes through the sink. ADR 0004 decision 3.

## Out of scope

- **`RUST_LOG`.** ADR 0004 decision 4: box 5 asks for a flag, and an environment variable is a second way
  to say the same thing, with a precedence question attached.
- **Structured or JSON diagnostics.** The machine-readable surface is the document on stdout, which
  `report` owns. A second one would be two answers to what a run produced.
- **Per-operation durations reported rather than derived, and benchmarks.** Issue #10's, and ADR 0004
  names it as the condition under which that record is reopened. Box 7 asks for a subtraction and gets one.
- **The `trace` level.** Box 5 names four levels. A fifth with no call sites is an extension point ahead
  of its caller, which `application.md` "Architecture" refuses.
- **A line per candidate block or per kernel.** The measured 50% run examined 549 940 blocks and built
  15 891 kernels; box 7's granularities are the search level and the radius trial, which are 144 and 24.
  Logging the level below would make `debug` unreadable and slower than the search.

## Phase 1 — the seam

The plumbing, and nothing narrates yet. Both tasks leave observable output unchanged, which is what makes
them separately verifiable: after 1.2 the flag exists, is honoured, and has nothing to say.

- [ ] **1.1 `log` is a workspace dependency and both crates take it.** `log = "0.4"` in the root
      `Cargo.toml`'s `[workspace.dependencies]`, with `log.workspace = true` in
      `crates/popcircles/Cargo.toml` and in `crates/popcircles-cli/Cargo.toml` — the library because it
      emits, the CLI because it both emits its own records and implements the sink. `Cargo.lock` is
      updated in the same commit. Nothing calls it yet.
      *Verify:* `rg -n 'log.workspace = true' crates/popcircles/Cargo.toml crates/popcircles-cli/Cargo.toml`
      matches twice, and `cargo tree -e normal | rg -o '[a-z0-9_-]+ v[0-9]' | sed 's/ v[0-9]//' | sort -u | wc -l`
      prints 33 where it printed 32 — one new crate and no transitive ones, which is the figure ADR 0004
      decided on. 243 tests still pass.

- [ ] **1.2 `--log-level` is a global flag, and the CLI installs a `log::Log` that honours it.** A
      `LogArgs` on `Cli` itself with `global = true`, so every command takes it rather than each
      subcommand declaring it, defaulting to `info` and parsed by a `value_parser` mapping the four names
      box 5 gives — `error`, `warn`, `info`, `debug` — onto `log::LevelFilter`. `trace` is **not** a
      name it accepts. Then `StderrLog` beside `StderrProgress`: it holds the process's start `Instant`,
      writes `<elapsed ms> <LEVEL> <target>: <message>` to stderr, and is installed in `main` before
      `run` is called, with `log::set_max_level` set from the flag. Installing it is infallible in
      practice — it happens once — but the `Result` is handled rather than unwrapped, because a failed
      install must not take the run down: a diagnostic that cannot be printed is not a reason to lose a
      result, which is `StderrProgress::advance`'s existing reasoning.
      In the same commit, reword the "quiet" prose comment at `main.rs:1266` so `FU-04`'s sweep
      distinguishes a flag from a sentence.
      *Verify:* `cargo run -p popcircles-cli -- distance 0 0 0 90 --log-level debug` prints one JSON
      document on stdout and **nothing** on stderr, because no call site emits yet — that is what makes
      this task's change invisible and the next task's visible. `--log-level trace` and
      `--log-level nonsense` are both usage errors at exit 2. `rg -n 'verbose|quiet' crates/popcircles-cli/src`
      returns nothing. Three CLI tests: the parser maps the four names and rejects `trace`, the flag is
      accepted after any subcommand as well as before it, and a `StderrLog` set to `warn` reports
      `enabled()` false for an `info` record and true for an `error` one.

## Phase 2 — what it says

The call sites. Read the actual output at the end of this phase rather than trusting the tests: what these
tasks are for is a human watching a terminal, and no assertion checks that a line is worth reading.

- [ ] **2.1 `info` narrates the run's resolved inputs and its answer, from the CLI.** Box 6's two ends,
      both of which are known at the binary edge and neither of which the library should be asked for: one
      record after the table is resolved naming the cache path, the digest, the decimation and the grid's
      shape, and one at the end naming the answer — the radius for the two smallest-circle commands, the
      centre and population for the other two. `CachedTable::open` is where the first belongs, since it is
      already the one place a cache is opened.
      *Verify:* `cargo run -p popcircles-cli -- population-at …` against the 5 arcmin table with
      `--log-level info` prints the two records on stderr and one document on stdout, and with
      `--log-level warn` prints nothing on stderr — the same invocation, so the flag is doing the work
      rather than the code path. stdout is byte-identical between the two runs, which is the property box 6
      exists to protect.

- [ ] **2.2 `info` marks the phase boundaries inside the library.** The boundaries a run has: the table
      build (`table::build`), the search over radius entering and leaving (`smallest::smallest`), and each
      radius the search settles (`smallest::probe`, one record naming the radius and whether the ledger
      answered it). Targets are the module paths, so a reader can tell a library record from the CLI's.
      Not the fixed-radius search's levels — those are `debug`'s, in 2.3.
      *Verify:* a smallest-for-share run at `--log-level info` prints one record per settled radius, and
      the count of those lines equals `stats.radii_evaluated + stats.radii_reused` in the document on
      stdout — the document and the log agree about how much work happened, which is the check that would
      catch a record emitted on the wrong side of the ledger's early return. `rg -n 'print!|println!|eprintln!' crates/popcircles/src`
      still returns nothing.

- [ ] **2.3 `debug` brackets every expensive step with a matched pair.** Box 7's list: the table build or
      load, the kernel placement, each search level, each radius trial. One "begin" and one "end" record
      per step carrying the same operation name, so the elapsed figures 1.2 prefixes subtract to a
      duration. Every pair closes on the error path too, per the ground rule.
      *Verify:* a `most-populous` run at `--log-level debug` emits `begin` and `end` records in equal
      numbers — `rg -c begin` and `rg -c end` over the captured stderr print the same figure — and
      the level records number `stats.levels` from the document. Subtracting the elapsed figures of one
      matched pair gives a duration between zero and the run's own wall time, which is the claim box 7
      makes and the reason the elapsed prefix exists. A run whose cache is absent still emits the closing
      record of the pair it opened, checked by pointing `--cache` at a path that is not there.

## Phase 3 — documentation, register, close-out

- [ ] **3.1 README says what the flag does and why there are two mechanisms.** A short block in the Usage
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
      *Verify:* `rg -n '^### FU-' docs/follow-ups.md` still lists ten entries and the Status lines read:
      02 closed, 03 dormant, 04 closed, 05 closed, 06 dormant, 07 closed, 08 dormant, 09 due, 10 closed,
      11 due. Both of `FU-04`'s own sweeps come back clean on this tree:
      `rg -n 'print!|println!|eprintln!' crates/popcircles/src` and
      `rg -n 'verbose|quiet' crates/popcircles-cli/src` are both empty.

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
