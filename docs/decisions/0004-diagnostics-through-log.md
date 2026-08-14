---
tags: [adr, code, popcircles]
created: 2026-08-14
decided: 2026-08-14
supersedes: null
superseded_by: null
---

# ADR 0004 - The library emits diagnostics through `log`, and the CLI is its own subscriber

## Status

Accepted - 2026-08-14.

It **extends [ADR 0001](0001-cli-and-output-layer.md) decision 4 into diagnostics**, which that
decision did not reach: decision 4 routes *progress* through a sink the caller supplies, and a sink
reports how far a run has got while a log reports what happened. Nothing in decision 4 moves, and
`StderrProgress` stays exactly as it is.

It supersedes nothing. ADR 0001 weighed `tracing` in its Alternatives section — "for diagnostics and
progress", rejected for a batch CLI, with "it can arrive later without touching the wire format"
written in — and this record is that later, upholding the rejection on fresh measurements rather than
reversing it. `FU-04` is the entry that gated any facade behind a record extending that clause.

## Context

Issue #8 has four boxes left, and three of them describe a facade this repository does not have:

| Box | What it asks |
| --- | --- |
| 5 | a `--log-level` flag, `error` / `warn` / `info` / `debug`, default `info` |
| 6 | `info` narrates phase boundaries and resolved inputs: the raster and cache in use, the search parameters, the final answer |
| 7 | `debug` brackets each expensive step — table build or load, kernel placement, each search level or radius trial — with a start and an end line carrying the same operation name, "so a duration is a subtraction over two log lines rather than a stopwatch run by hand" |

The fourth is box 8, which says the facade "is not this issue's to pick" and that a record lands before
that box is checked. This is that record. The CLI landed in #38 with boxes 1 to 4 met and 5 to 8 open.

**The library must not print, and today it does not.** `rg -n 'print!|println!|eprintln!'
crates/popcircles/src` is empty on 2026-08-14 — that emptiness is half of `FU-04`'s condition, and
`application.md` "Architecture" is what makes it a rule rather than a habit: "the domain computes and
returns; it does not read, write, print, or format". So the question is not how to tidy ad-hoc printing.
It is which seam diagnostics cross, before there is any.

### The costs, measured on this tree

`FU-04` carries a cost table measured 2026-08-13 in a scratch project. Re-measured 2026-08-14 the same
way, against this workspace's own 32-crate tree and counting only crates that are **new** to it, the
figures are materially different and the ordering has inverted:

| Shape | New crates | Where they land |
| --- | --- | --- |
| `log` in the library, a hand-written `log::Log` in the CLI | **1** | library |
| `tracing` + `tracing-subscriber`, `json` and `env-filter`, defaults off | 11 | 5 library, 6 binary |
| `log` + `env_logger` | 15 | 1 library, 14 binary |
| `tracing` + `tracing-subscriber`, `json` and `env-filter`, defaults on | 17 | 5 library, 12 binary |

Three corrections to that entry's table, which is why the numbers are restated here rather than cited:

- **Its headline 41 for `tracing-subscriber` is 11.** Part is version drift; part is that it counted the
  default feature set; and part is that four of `tracing`'s crates are proc-macro dependencies this tree
  already carries through `serde`'s derive and `thiserror`. Its argument that forty-one is "nearly triple
  the trimmed clap tree ADR 0001 accepted" does not survive the correction — eleven is below it.
- **`env_logger` is the most expensive of the three, not the middle one.** It pulls `jiff` and
  `jiff-core` for timestamps, `anstream` and three `anstyle` crates for colour, and `regex` with
  `aho-corasick` for filtering. `FU-04` had it at 25 against `tracing-subscriber`'s 41; today it is 15
  against 11.
- **A hand-written subscriber was never priced.** It costs nothing beyond `log`, and it is not a novel
  shape here: `StderrProgress` in `crates/popcircles-cli/src/main.rs` is already a hand-written sink
  chosen over a progress-bar dependency, on decision 4's reasoning.

### The nesting, measured on a real run

`FU-04` says the cheaper shape "has to be ruled out rather than skipped", and names the test: `log`
"costs one crate and buys no spans. If the nesting turns out shallow, that is the better answer and the
record should say so." What spans buy over lines is context propagated to a child event without the
child restating it, and timing recorded rather than subtracted.

The 50% run recorded in `README.md` — the 5 arcmin table, `--spacing 32` — settled 24 radii over 144
search levels, examining 549 940 candidate blocks and building 15 891 kernels. Against box 7's stated
granularities that gives:

- **Two levels of nesting**, radius trial inside the run and search level inside the radius trial, and
  about 170 bracketed events in total.
- **No third level.** Blocks and candidates are the next one down and there are half a million of them,
  so they are not logged at any level and no span would carry them.
- **No concurrency.** `crates/popcircles/src/circle.rs` states that nothing in the crate is parallel,
  which is a determinism requirement rather than a present limitation, so there is no interleaving for a
  span to disambiguate.

Two levels, sequential, ~170 events. That is the shallow case the entry describes. Box 7's own wording
asks for the `log` shape outright: two lines carrying one operation name, with the duration obtained by
subtraction.

## Decision

**1. The library emits diagnostics through the `log` facade, and takes no other logging dependency.**
`log` is one crate with no dependencies of its own. `tracing` is not added to
`crates/popcircles/`; ADR 0001 held that crate's surface at `serde` deliberately, and five crates for
spans nothing in the measured nesting needs is not a trade this record makes. The library never prints:
`println!` and `eprintln!` stay absent from it, which is what `FU-04`'s first sweep watches.

**2. The CLI implements `log::Log` itself, and is the only place a diagnostic reaches a stream.** A
level filter, one line per record to stderr, no colour. This is decision 4's shape one step out — the
library emits, the binary chooses the stream — and it is why stdout stays exactly one JSON document. No
`env_logger`, no `tracing-subscriber`.

Each line carries **milliseconds elapsed since the process started**, from `std::time::Instant`, and no
wall-clock time. That is what makes box 7's subtraction possible at all — two lines with no time on them
cannot yield a duration — and it is deliberately the weaker of the two things a timestamp could be: a
monotonic elapsed figure is in `std`, while a formatted wall-clock time needs a datetime library, which
is a third of what makes `env_logger` cost what it does. A run that needs to be correlated with
something outside itself is a case this does not serve, and would be served by the JSON on stdout rather
than by a log line.

**3. `--log-level` sets the filter, and progress stays separate from it.** The flag takes `error`,
`warn`, `info`, `debug`, default `info`, per box 5. It does not govern the progress meter:
`StderrProgress` reports how far a run has got and is silent when stderr is not a terminal, which is a
different question from what happened and is decision 4's, not this record's. A run may be quiet and
still draw a meter, or verbose with no meter, and neither is a contradiction.

**4. `RUST_LOG` is not supported, and structured output is not published.** Box 5 asks for a flag; an
environment variable is a second way to say the same thing and a second thing to document. Diagnostics
are for a person reading a terminal — the machine-readable surface is the JSON on stdout, which
`report` owns and this record does not touch.

## Consequences

**Positive**

- One new crate in the whole workspace, on the library side, with no transitive dependencies. The
  binary side costs nothing.
- `application.md`'s "Architecture" arrow is preserved without a new abstraction: the library emits
  through a facade and knows nothing of streams, formats or levels.
- Boxes 5 to 7 become implementable, so issue #8 can close, and roadmap #11's sixth step with it.
- Consistent with what the CLI already does. A reader who understands `StderrProgress` understands the
  logger beside it, and neither is a dependency someone has to learn.
- The facade is the `log` crate's, so swapping the consuming side later — for `env_logger`, or for
  `tracing-subscriber` through `tracing-log` — is a change in one file of the binary and touches no
  emitting call site.

**Negative / costs**

- **No span timing.** Box 7's durations are a subtraction the reader performs, which is what the box
  asks for and less than what a subscriber would give. Issue #10 is validation *and benchmarks*, and if
  it wants per-operation durations reported rather than derived, this is the decision it reopens. The
  answer then is likely `criterion` or timing at the CLI edge rather than spans, but that is not settled
  here.
- **No structured diagnostics.** Nothing can aggregate a run's log lines as fields. If a per-country
  sweep (#18, ninety-plus countries) ever wants machine-readable per-record diagnostics, a hand-written
  formatter is the wrong place to grow them and the JSON on stdout is the right one — but that will be an
  argument, not a lookup.
- **A formatter this repository maintains.** Roughly thirty lines, and every line of it is a line nobody
  else tests. The mitigation is that it does very little: filter, format, write, flush.
- **No `RUST_LOG`.** A contributor whose reflex is to set that variable will find it does nothing, and
  the flag is the only way in. Worth a line in `README.md` rather than a shrug.
- **Two mechanisms that look like one.** A progress sink and a logger both write to stderr and neither
  governs the other. The distinction is real and documented, and it will still be asked about.

## Alternatives considered

- **`tracing` in the library with `tracing-subscriber` in the CLI.** The right answer where nesting is
  deep, context has to reach child events, or concurrent work interleaves. Eleven new crates, five of
  them on the library ADR 0001 held at `serde`. It lost on the measurement `FU-04` named as decisive:
  two levels of nesting, ~170 events, no concurrency. Its own strongest argument — per-span timing — is
  wanted by #10 and not by #8, so buying it now is buying it for a caller that has not arrived, which
  `application.md` "Architecture" refuses.
- **`log` in the library with `env_logger` in the CLI.** The obvious pairing, and it was the cheaper of
  `FU-04`'s two binary-side options when that entry was written. It lost on re-measurement: 15 new crates
  against a hand-written subscriber's nothing, including a datetime library and a regex engine to
  implement a five-way level filter that `--log-level` already parses. It would also give `RUST_LOG`
  precedence questions this record does not want to answer.
- **`log` in the library, bridged into a `tracing-subscriber` by `tracing-log`.** `FU-04`'s own
  suggestion for the cheap shape. It lost because it pays for the expensive consuming side while
  emitting through the cheap facade — the four crates of the bridge plus the subscriber's — and buys
  spans that no `log` call site can open. It is, however, the migration path if the costs above bite.
- **No facade: keep the library silent and narrate from the CLI only.** The status quo, and genuinely
  sufficient for boxes 5 and 6 — the resolved inputs and the final answer are both known at the binary
  edge. It lost on box 7: the expensive steps to bracket are inside the search, per radius and per
  level, and the CLI cannot see them without the library telling it. Reporting them through the existing
  progress sink was considered and rejected as worse than either option — it would overload a
  two-integer meter into an event channel and collapse the distinction decision 3 above preserves.
