---
name: write-adr
description: Author an architecture decision record in docs/decisions/ - the bar applied first, so the answer is allowed to be "this belongs in the PR", then one ruling on one page. Use when the user asks to record, write, or supersede an architecture decision.
---

# Write an ADR

`docs/ai/platform.md` "Architecture decisions" holds the bar a change must clear and the five scoping
rules, the last of which is the line ceiling. This skill applies them and owns what they leave open: the
frontmatter fields, the sections, and the judgment writing one takes.

**Step 1 can end the skill.** Most work that reaches here warrants no record, and saying so is this
skill succeeding rather than refusing.

## Steps

1. **Apply the bar before drafting anything.** Take platform.md's three tests against the change in
   hand and answer each out loud, on this change rather than on the topic it belongs to. If any fails,
   stop, name where the change belongs instead, and say what would have to become true for a record to
   be warranted. Do not write a short record as the compromise — a record failing the bar costs every
   later reader the time to discover it did.
2. **Reduce it to one ruling, in one sentence, present tense.** Write that sentence before anything
   else; it is the title and the slug. If it needs an "and" joining two independent claims, or the
   draft starts wanting a numbered list, there are two questions here: record the one asked about and
   name the other for its own record.
3. **Take the next free number** — the lowest unused four-digit prefix in `docs/decisions/`. The
   filename is that number plus a kebab-case slug of the ruling.
4. **Write the frontmatter.**

   ```yaml
   ---
   id: NNNN
   status: accepted
   date: YYYY-MM-DD
   scope: <one value>
   tags: [adr, popcircles]
   ---
   ```

   `scope:` takes exactly one value, from `architecture`, `build`, `contract`, `caching`, `safety`,
   `output`, `rendering`, `process`. It is what turns "what have we decided about caching" into a query
   rather than a read, so a record wanting two values is two records. A ruling no value fits adds one,
   to this list and in the same change — but check first that the want is not step 2's "and" in
   disguise. `superseded_by: NNNN` appears only when `status: superseded`, and there is no
   `supersedes:`: one relation stored in two places can contradict itself, and the forward pointer is
   the direction a reader who landed on a dead record actually needs.
5. **Fill four sections, in this order,** under an `# ADR NNNN — <ruling>` heading.
   - **Decision.** Two to four sentences, present tense, the rule as it now stands. A reader who stops
     here can apply it. Nothing about how it is implemented.
   - **Context.** One or two paragraphs: what forced the question, and what constrains the answer.
     Context that could have been written before the problem appeared is not context. Cite inline the
     one figure that decided it; the issue holds the rest of the evidence.
   - **Options.** One `###` subsection per option actually weighed, the winner first and marked
     `(SELECTED)`, carrying `Adopted because:` and `Adopted despite:` bullets where the rest carry
     `Rejected because:` and `Rejected despite:`. **Every bullet is one line.** That is the whole
     discipline of the section: the one-line form makes a reason be a reason, where the paragraph form is
     how a rejection becomes an essay with a measurement table inside it. The `despite` bullets are not
     decoration — the winner's are the costs taken deliberately, and a loser's is its genuine merit.
   - **Consequences.** What the ruling now forbids and where the gate is, what it obliges of every
     future change in this scope, and the condition under which it would be reopened if one is knowable.
6. **Close with a Links section** naming the issue the evidence lives in, and the one or two records
   this one sits beside. Nothing else goes there.
7. **Count the lines.** `wc -l` on the finished file. Over the ceiling, what comes out is a whole thing
   — the implementation, the evidence, or a second record — not prose tightened until it fits.
8. **On supersession, edit the old record minimally** — `status: superseded`, `superseded_by: NNNN`, and
   one blockquote line under its title:

   ```markdown
   > Superseded by [ADR NNNN](NNNN-<slug>.md).
   ```

   Its four sections stay exactly as written, even where later events proved them wrong. What the new
   record replaces is a sentence in the new record's Context, which is where a human wants it anyway.
9. **Correct the instruction layer in the same change.** A record that changes a rule leaves `docs/ai/`
   stating the old one. The rule goes there in present tense with a one-line link here for the why;
   this record does not restate the rule at length, and that file does not restate the reasoning.

## Judgment

- **"This belongs in the PR" is the most common correct answer, and it is worth saying well.** Name the
  target rather than declining: the PR description, a comment beside the configuration, a sentence in
  `docs/ai/`, a test that pins the tolerance, a register entry in `docs/follow-ups.md`, or nothing at
  all. A choice with a demotion target named is settled; a choice merely refused a record comes back.
- **The options are the ones actually weighed.** Padding the section with plausible rejects is worse
  than a short list — it tells a later reader an option was examined when nothing was learned about it.
  The option the user first proposed belongs there by name whenever it lost.
- **A draft that opens by positioning itself against another record is reporting a scoping error.**
  "Extends 0003 along the axis that record opened" is what a reader writes when two records overlap,
  and the fix is upstream in step 2, not a better paragraph. Genuine supersession is step 8 and is rare.
- **Re-deriving an accepted record's reasoning means superseding it.** Reopening a settled question
  either replaces the old ruling or does not happen. Doing neither leaves two live rulings on one
  question and no way to tell which governs.
- **A decision this repository did not make is not a record.** Upstream conventions, a language's
  defaults and a tool's behaviour are cited, not ruled on.
- **Records 0001 to 0010 were not written carelessly.** Every ruling in them was real and correctly
  reasoned; what failed was that the trigger for writing was an issue rather than a question. Assume the
  same pressure is on this draft.
