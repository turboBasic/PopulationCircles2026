---
tags: [adr, code, popcircles]
created: 2026-08-13
decided: 2026-08-13
supersedes: null
superseded_by: null
---

# ADR 0001 - The CLI is its own crate, and the library owns the JSON contract

## Status

Accepted - 2026-08-13. First record in `docs/decisions/`; it supersedes nothing.

It settles in structure what [`ai/application.md`](../ai/application.md) "Architecture" states as a
principle and nothing enforces, and it fixes the frame that issues #8 and #16 fill in rather than
choose.

## Context

`docs/ai/application.md` "Architecture" rules that "dependencies point one way — view depends on
results, CLI depends on domain, domain depends on neither — and a change that reverses an arrow is an
architecture change, not a refactor". Today nothing holds that arrow. `crates/popcircles/` is one
crate whose `[dependencies]` table is empty and whose `src/main.rs` is three lines printing
`popcircles: not implemented yet`; the binary and the library are the same compilation unit, so the
arrow is a sentence in a document. The architecture review that landed `ed26ca7` recorded this and
deferred it on the stated condition that argument parsing had not arrived yet. It has.

Four issues depend on the answer, and two of them are not CLI issues:

| Issue | Reads or writes the contract |
| --- | --- |
| #8 CLI with versioned JSON output | writes it; its four "Done when" items fix stdout/stderr split, version stamping, snapshot stability and exit-code classes |
| #16 CLI: per-country circle commands | writes it, second command surface |
| #9 Map rendering from results | reads it, in Python |
| #17 Rendering: country outline with its circle | reads it, in Python |

So the serialised shape has two producers and two consumers, and `application.md` already calls it
"the contract between the two halves". A contract with four parties and no owner is the failure this
record exists to prevent.

Dependency costs, measured 2026-08-13 with `cargo search` and `cargo tree -e normal` in scratch
projects outside this tree:

| Addition | Crates in the tree |
| --- | --- |
| `clap` 4.6.6, `features = ["derive"]`, defaults on | 23 |
| `clap` 4.6.6, `default-features = false`, `features = ["std", "derive", "help", "usage", "error-context"]` | 14 |
| `serde` 1.0.229 + `serde_json` 1.0.151 in a library | 13 |

Those numbers are what make the layout question real rather than stylistic: in one crate, every
consumer of the geometry compiles the argument parser. The 23-against-14 gap is the cost of clap's
`suggestions` and `color` defaults, which a machine-readable CLI does not need to pay for.

One cost is not the CLI's to carry. `application.md` "Correctness invariants" already obliges the
library: "a cached table or result file records what raster and what parameters produced it, and is
rejected rather than silently reused when those differ". Something in the library serialises that
header whatever the CLI does, so `serde` arrives in the library on #3's schedule, not on this one.

## Decision

**1. The binary moves to its own crate.** `crates/popcircles/` stays the library and
`crates/popcircles-cli/` holds the binary. The CLI's dependencies are the CLI's: a library consumer
compiles no argument parser, and the domain cannot import one, because it is not among its
dependencies. Cargo becomes the gate for the arrow `application.md` draws, replacing the convention
that has been holding it.

**2. `clap` 4 with `derive`, `default-features = false`.** Features are `std`, `derive`, `help`,
`usage` and `error-context`. The derive is the point: a subcommand set is an enum and an argument's
type is its validation, which is the property that keeps #8's four commands and #16's country
commands from drifting into hand-rolled parsing.

**3. The library owns the wire format, in a `report` module of version-stamped types.** Those types
are distinct from the domain types and are what `Serialize` is derived on. A domain type never carries
a serde derive: `Grid`, `LatLon` and `Row` change when the search changes, and a wire format that
moves with them is not a contract. The CLI serialises what the library hands it and adds nothing of
its own, so the two renderers and the two command surfaces read one owner.

**4. The library reports progress through a sink it is given, never to a stream it chose.** A narrow
trait parameter, per `application.md`'s DIP bullet; the CLI implements it against stderr so stdout
stays machine-readable, which is #8's third box. Exit codes are `std::process::ExitCode` mapped in the
binary from the library's error enums — #8's fourth box needs three classes, not a crate.

**5. `anyhow` appears in the binary crate only.** `ai/code.md` already sets this ("`anyhow`-style
context only at the binary edge"); the split is what makes it checkable rather than a matter of
attention, since the library does not depend on it.

## Consequences

**Positive**

- The one-way dependency arrow is enforced by the build. Reversing it now requires editing a
  `Cargo.toml`, which is visible in review in a way an `use` line in a 700-line module is not.
- The library's dependency surface stays small and stays *justified*: `serde` for the contract and
  the cache header, and nothing that exists to serve a human at a terminal.
- #8 and #16 inherit a frame instead of choosing one twice. The second command surface cannot quietly
  invent a second output shape.
- The wire format gains a place to be versioned. A renderer that needs a field has one file to change
  and one owner to change it in, which is what "extend it additively" needs to mean in practice.
- Snapshot tests over the JSON become tests of the library's `report` types, so they do not depend on
  the argument parser to run.

**Negative / costs**

- Two crates is more ceremony than one: a second `Cargo.toml`, a second lint block, and a path
  dependency to keep in step. For a project with one binary this is overhead that buys nothing on the
  day it lands, and only pays once a second consumer of the library exists.
- The `report` types are a second representation of results the domain already has in hand. Every
  field published is written twice — once in the domain type, once in the DTO and its conversion —
  and the compiler cannot tell you the two have drifted apart in meaning, only in type.
- `serde` in the library is 13 crates for a library that currently has none. The cache-header argument
  says they were coming anyway, but this record makes them arrive earlier than #3 would have.
- Choosing the parser before there is anything to parse risks fitting the frame to the wrong shape:
  #8's commands are named in an issue, not implemented, and clap's derive will have been chosen
  against four command signatures nobody has run.
- Trimming clap's default features means the CLI does not suggest corrections for a mistyped flag.
  That is a real ergonomic loss, taken deliberately, and it is reversible by adding one feature.
- A pointless indirection if the search never grows a second front end. Then the split cost the
  project two crates and bought a boundary nothing tested.

## Alternatives considered

- **Keep one crate and let `main.rs` grow into the CLI.** Cheapest today, and the layout the tree
  already has. It lost on the measurement: `clap` and `anyhow` become dependencies of the geometry,
  and the arrow stays a sentence — the exact finding the review deferred rather than dismissed.
- **`bpaf` 0.9.27 instead of clap.** Combinator-based with a leaner tree, and genuinely better at
  argument interdependency: mutually exclusive groups and implied flags are checked in the parser's
  type rather than at runtime, which #16's per-country flags may well want. It lost on ecosystem —
  no equivalent of `clap_complete` or `clap_mangen`, thinner documentation, and a much smaller
  community for a surface other people's tooling is meant to consume.
- **Put the DTOs in the CLI crate.** Removes `serde` from the library and keeps the wire format next
  to the thing that writes it. It lost because #9 and #17 read that format and neither is downstream
  of the CLI in any sense but the accidental one; defining the contract in the binary makes the
  renderers depend on an argument parser's crate for their schema.
- **`serde` behind an optional cargo feature on the library.** Keeps the dependency-free library for
  a consumer that never serialises. It lost as an extension point ahead of its second caller: the
  only consumer is the CLI, `lint`, `typecheck` and `test` all pass `--all-features` so the feature
  would be permanently on in every gate, and the cache header needs it unconditionally anyway.
- **`tracing` for diagnostics and progress.** The right answer for a long-lived service. It lost for
  a batch CLI with a single progress sink, where it is a dependency tree in place of the sink trait
  the architecture already calls for. It can arrive later without touching the wire format.
- **Defer all of it to #8, in roadmap order.** #8 sits behind #7, which sits behind #3 through #6, so
  this would land after the whole search. It lost because the framework half of #8 is the expensive-
  to-change half and the search half is not: four commands can be written against a settled frame
  cheaply, while re-crating a library with five modules of callers is the change nobody schedules.
