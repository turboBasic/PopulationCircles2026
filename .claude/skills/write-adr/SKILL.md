---
name: write-adr
description: Author an architecture decision record in docs/decisions/, with its sibling implementation plan when the work needs one. Use when the user asks to record, write, or supersede an architecture decision.
---

# Write an ADR

`docs/ai/platform.md` "Architecture decisions" states the policy this skill works under, and its
"Implementation plans" section decides which work gets a plan at all. What follows is the shape every
record takes and the judgment writing one needs.

## Steps

1. **Take the next free number.** The lowest unused four-digit prefix in `docs/decisions/`; the
   filename is that number plus a kebab-case slug of the ruling.
2. **Write the frontmatter.**

   ```yaml
   ---
   tags: [adr, docs|code|data, popcircles]
   created: YYYY-MM-DD
   decided: YYYY-MM-DD
   supersedes: null
   superseded_by: null
   ---
   ```

   The middle tag names the domain the decision touches. The two supersession fields hold a bare
   number, or a number with a parenthetical scope when only one clause moves:
   `0002 (roadmap clause only)`.
3. **Fill the sections, in this order.**
   - **Title.** `# ADR NNNN - <ruling>` — the decision as a statement, not a topic.
   - **Status.** `Accepted - YYYY-MM-DD.`, then what this ADR does to its neighbours: which one it
     extends, supplements or supersedes, and in what part.
   - **Context.** What forced the decision — measured facts, named files, counts, line numbers.
     Context that could have been written before the problem appeared is not context. Where the
     evidence comes from another repository or from prior art, say so; borrowed measurements are
     usable, but never presented as this tree's.
   - **Decision.** One bolded ruling, phrased as a rule the repository now follows, then the
     paragraphs that qualify it. A decision with several parts bolds and numbers each part.
   - **Consequences.** A `**Positive**` list and a `**Negative / costs**` list. The costs are the
     half read later, when the decision is under question; write them as if arguing against it.
   - **Alternatives considered.** One bullet per option actually weighed, each ending in the reason
     it lost.
4. **On supersession, edit both files.** The new ADR sets `supersedes`; the superseded one receives
   `superseded_by` and a Status section rewritten to `Superseded by ADR NNNN - YYYY-MM-DD`, naming
   what survived and sending the reader to the replacement. Those two are the only sanctioned edits
   to an accepted ADR — its Context, Decision and Consequences stay as written, even where later
   events proved them wrong.
5. **Write the sibling plan when the work needs one.** Implementation spanning more than one sitting
   or more than one task warrants one, provided this ADR decided it — roadmap work stays in issues.
   Its sections are fixed by `docs/ai/platform.md` "Implementation plans". Drafting the
   plan before the ADR is often the better order: the ruling and its costs are clearer once the work
   is laid out. Running it afterwards is the `run-plan` skill.

## Judgment

- **An ADR records a decision made, not one under consideration.** If the ruling cannot carry a
  `decided` date, it is a proposal and stays in a plan or in conversation until it is settled.
- **The alternatives are the ones actually weighed.** Padding the section with plausible rejects is
  worse than a short list: it tells a later reader an option was examined when nothing was learned
  about it. The option the user first proposed belongs here by name whenever it lost.
- **Re-deriving an accepted ADR's reasoning means superseding it.** A new ADR that reopens a settled
  question either replaces the old ruling or extends it into cases it did not reach. Say which, in
  both files. Doing neither leaves two live rulings on one question and no way to tell which governs.
- **A decision this repository did not make is not an ADR.** Upstream conventions, a language's
  defaults and a tool's behaviour are cited, not ruled on.
- New vocabulary the record introduces goes in `.cspell/project.txt` in the same change, or the
  hooks fail on the commit that adds it.
