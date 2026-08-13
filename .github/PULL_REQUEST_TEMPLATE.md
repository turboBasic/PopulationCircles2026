<!--
The title is a Conventional Commit — a squash merge takes its subject from there.
Everything below renders as prose; these hints disappear.
-->

## Why?

<!--
Required. The problem this solves, in your own words — not a restatement of the diff.
Link the issue if there is one (`Closes #12`).
-->

## Verification

<!--
`mise run ci` covers lint, typecheck, and tests. Say what it cannot see: a command you
ran by hand, a result you checked against a known figure, a case you left untested.
Numerical changes: say what invariant you verified, not just that it ran.
-->

## Docs

<!--
Name the documentation you touched, or `none — no doc describes this`. What the change
obliges you to touch is the invariant list in docs/ai-instructions.md.
-->

---

<!--
Trading away a rule marked non-negotiable in docs/ai-instructions.md is a design change:
name the rule and what breaks without it, here, before the review starts.

If any of this change came near the upstream C++ project, say how — the copying rule is in
docs/ai/application.md "Provenance and the copying rule".
-->
