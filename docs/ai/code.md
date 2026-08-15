# Code

Read this when writing or changing code in either language, including its comments and the
documentation that travels with it. The lint levels that back these rules
are declared once in the root `Cargo.toml` and in `pyproject.toml`; this file holds the judgment
those levels cannot express, and loosening one to clear an error is a non-negotiable.

## Rust

- **If a problem seems to need `unsafe`, raise it as a design question** rather than reaching for the
  escape hatch.
- **`unwrap()` and `expect()`** are acceptable in tests, and in a `main` that is documenting an
  invariant. Not in library paths: return `Result` and propagate with `?`.
- **A lint exemption names only the lints the code under it actually trips.** Not the set a
  neighbouring module needed: a test module reaching for `expect_err` alone takes
  `clippy::expect_used` and stops there. An exemption listing a lint nothing beneath it triggers
  reads as a policy about the module rather than a fact about its code, and it stops being
  reviewable — nobody can tell which entries are load-bearing. The count of exemptions is not the
  measure and never was; a module gaining tests of its own gains one legitimately.
- **Errors:** a concrete error enum per crate boundary (`thiserror` when it earns its place),
  `anyhow`-style context only at the binary edge. Never `panic!` for an expected failure.
- **Numeric casts are the sharpest edge in this codebase.** Make every conversion explicit and state
  in a comment why it is safe when that is not obvious; [`application.md`](application.md)
  "Correctness invariants" owns what "safe" means for the summation table and the geodesy.
- Prefer iterators and slices over index arithmetic. Where index arithmetic is the clearer
  expression of a raster traversal, keep it local and named.
- No `mod.rs`: a module is `foo.rs` plus `foo/`.

## Python

Python 3.14. No compatibility shims or version guards for earlier releases.

- `X | None`, not `typing.Optional`. Built-in `dict`/`list`/`tuple`, not `typing.Dict`.
- No `from __future__ import annotations`.
- No `if TYPE_CHECKING:` guard except to break an import cycle.
- Full type hints on every signature, tests included.

## Comments and docs

- No docstrings in Python. In Rust, `///` only where the WHY is non-obvious.
- Comments only where the reasoning is non-obvious, never restating what the code does.
- No multi-line comment blocks.
- Match surrounding comment density, naming and idiom.
- A comment explaining why a task, hook or lint is configured the way it is belongs beside that
  configuration, which is what makes the configuration the owner of that fact.
- Every change ends by checking the documentation it affects — the instruction layer, the human
  layer, and any doc naming a file, task or convention that moved — and correcting it in the same
  change.
