---
paths:
  - "docs/**"
  - "**/README.md"
  - "CHANGELOG.md"
---

# Writing documentation for Muster

The reader is a competent engineer who has never used Linear, works in another time
zone, and has nobody to ask. Many readers are coding agents reading a page as context,
and many of the people are not native English speakers. Everything below follows from
that.

## Pick the kind of page first

Each page in `docs/` is exactly one of these. If you cannot name the kind, you are
not ready to write the page.

| Kind | Directory | Answers | Shape |
| --- | --- | --- | --- |
| Tutorial | `docs/tutorials/` | "Teach me, by doing" | Numbered steps, a stated time budget, a visible result after every step, no choices offered |
| How-to | `docs/how-to/` | "I need to do X" | Prerequisites, steps, how to verify it worked, what to do when it does not |
| Reference | `docs/reference/` | "What exactly does Y do" | Generated from `muster schema` and rustdoc. Do not edit by hand; edit the source and run `just gen-commands` |
| Explanation | `docs/explanation/` | "Why is it like this" | The question in the title, the answer first, then the reasoning, then links to the ADRs |
| Decision | `docs/decisions/` | "Why did we choose this" | The ADR template in `docs/decisions/0000-template.md` |

A page that answers two of these questions is split into two pages that link to each
other.

## Style

- Short sentences. One idea per sentence. Active voice.
- Define a term on first use, or link to `docs/glossary.md`. Never both.
- No "simply", "just", "obviously", "of course", or "easy". If it were, the page
  would not exist.
- No idioms, no humour that depends on English, no cultural references.
- Name the exact command, flag, field or exit code. "Close the issue" is not an
  instruction; `muster issue close APP-12` is.
- Say what the reader will see after a step, so they know whether it worked: the JSON
  line, the exit code.
- When a thing has a Trello equivalent, say so once, in a sentence beginning "In
  Trello, this is …", and link the glossary entry. Then never mention Trello again on
  that page.

## Code in docs

- Every Rust code block is run by `just docs`. A block that cannot run is marked
  `ignore` with a comment saying why, and that is rare.
- Shell blocks that show `muster` commands are the ones `just quickstart` runs against
  the fake Linear, or they are marked with the words "against your workspace" and show
  real output with the identifiers of a real issue.
- Examples are complete: the command, the output, the exit code. The reader copies the
  block and it works.
- The crate README's example is the crate's doctest. Changing one changes the other.

## Glossary

`docs/glossary.md` is one table: Muster term, one-sentence definition, Linear API
name, Trello word (or "none"), link to the reference page. Every term that appears in a
command, a flag, an output field or an error message has a row.

## Decisions (ADRs)

- One decision per file, numbered, `NNNN-short-title.md`, copied from
  `0000-template.md`.
- Status is one of Proposed, Accepted, Superseded by NNNN, Rejected. Accepted ADRs
  are not edited except to change Status.
- Required for any change to an output field, an exit code, a key source, a flag's
  meaning, a dependency, or an invariant in `CLAUDE.md`. Encouraged for anything a new
  team member would ask "why" about.
- Record the alternatives that lost and the evidence (measurements, recorded responses,
  links). A decision without alternatives is a preference.
- A conversation in chat, a call or a commit message is not a decision. Write the
  ADR and link the conversation from it.

## Changelog

`CHANGELOG.md` follows Keep a Changelog. Every user-visible change adds a line under
`Unreleased` in the same commit, in the reader's words ("`muster issue close` now takes
`--cancel`"), linking the ADR or docs page where one exists.

## Before committing

1. `just docs` passes with no warnings.
2. `just lint-docs` passes.
3. You read the page once as the reader in the first paragraph. If they would have to
   ask a question, answer it on the page.
