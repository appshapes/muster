# 0009. Email the week's changes to a list, drafted by an agent and gated before it is sent

- **Status:** Accepted
- **Date:** 2026-11-02
- **Invariants affected:** 6
- **Supersedes:** none

## Context

Invariant 6 promises that a stranger can find, from the repository alone, why any
behaviour happens and why any decision was made. The people who want to follow Muster
without reading its commits are not served by the repository alone, and the owner's brief
of 2026-11-02 names the release-notes workflow among the six Cavalry workflows muster
takes. Cavalry has sent that email since 2026-10-16 (Cavalry ADR 0026), and Brigade, the
other sibling, since 2026-09-29: a weekly cron, a writer agent that drafts, a deterministic
lint, a reviewer agent, one fix cycle, a gate and one mail step, with the list of readers a
secret and every reader `bcc`.

Three facts shape the port. First, Muster has no release and no tag: the crate is
`publish = false` (ADR 0003) and `CHANGELOG.md` has one section, Unreleased, so a window of
releases has nothing to cover. What Muster has is `master`: one commit per change, each
message the review record, each user-visible change a line in the changelog in the reader's
words, in the same commit (`CLAUDE.md`). Second, this repository is public and so are its
run logs: a repository variable is printed unmasked, and an address printed once is public.
Third, an email cannot be taken back, and a model names things that do not exist: Brigade's
first release notes named a skill that was written later, which is why its lint exists, and
Brigade's scheduled run of 2026-10-12 passed that lint and was refused by the reviewer for
stating an older release's behaviour as current. Cavalry's lint reads the binary's command
list from a `USAGE` constant in its source; muster's binary builds its command tree with
`clap`, so there is no such constant, but `docs/reference/commands.md` is generated from
`muster schema` and `just lint-commands` fails when the two differ (ADR 0008), so that
page is the binary's command list, readable without a build.

## Decision

One workflow, `.github/workflows/send-release-notes.yml`, runs on Mondays and by hand. It
covers the commits on `master` after the last email and up to the run's own commit, merge
commits left out; the last email is the newest successful run whose step "Send to the
recipients" concluded `success`; with no earlier email the window is seven days; a `from`
input, a commit or a date, overrides both and is inclusive. `scripts/release-notes.py
window` lays out the writer's sources: the lines the window added to `CHANGELOG.md`, every
commit message, the ADRs added or changed with their status, the documentation pages
touched. A writer agent drafts in the shape `.claude/agents/release-notes-email.md` gives:
what a reader can now do, what was decided, what to do before updating, how to try it.
`release-notes.py lint` refuses a draft that names a `just` recipe, a `muster` command, a
`MUSTER_` variable or `LINEAR_API_KEY`, an ADR, a path or a link that does not exist in the
checkout, the commands and the variables read from `docs/reference/commands.md`, or that
carries a commit hash, an issue or card number, an address, a placeholder, the word
Unreleased, more than 600 words, or a construct the layout refuses. A reviewer agent reads
the draft against the sources, the writer gets exactly one fix cycle, and only an approved,
lint-clean draft is sent. `scripts/release-notes-email.py` lays the email out as HTML,
inline styles, no picture, under 100 KB, and in its check mode refuses what the shape
forbids with line numbers, which the lint hands back to the writer. The list is the secret
`RELEASE_NOTES_RECIPIENTS`, read after the last agent step has ended, every address masked
before anything can print it, every reader `bcc`. A run by hand is a preview to one address
unless it says otherwise; a `draft_run` input sends an earlier run's approved draft when
that run covered the very same commits. The mail goes through SMTP from the address in
`SMTP_FROM_ADDRESS`, an inbox someone reads, because the footer tells a reader to reply to
stop. The scripts are Python 3.9 standard library, as the repository's other scripts are
(ADR 0003), with tests that `just test-scripts` runs and `just test` depends on.

## Alternatives considered

- **Cover releases, as Brigade does.** Lost: none exist. Revisited at the first release.
- **One agent writes, the email is sent unread.** Lost: an email cannot be taken back, and
  Brigade's run of 2026-10-12 is a lint-clean draft a reviewer refused for a sentence that
  was wrong about the newest release.
- **The list as a repository variable, or a file in the repository.** Lost: a variable is
  printed unmasked in logs anyone can read, and a file in a public repository is public.
- **No email: readers watch the repository on GitHub.** Lost: a watch is one notification
  per event, not a digest in plain words with the decisions, and the owner asked for the
  workflow. Nor can a list be collected from the watchers: GitHub published the address of
  one of the four accounts that had starred Brigade when it measured on 2026-09-29.
- **Build the binary in the job and read `muster --help`.** Lost: a Rust toolchain and a
  build in a job that otherwise needs Python alone, when the generated reference is the
  same list, held equal to the binary by `just lint-commands` on every commit.
- **Keep Cavalry's counter and configuration-key checks.** Lost: muster has no counters
  file and no configuration registry; the names a draft can get wrong here are commands,
  variables, exit codes and bounds, and all four are on the generated page.
- **A `Reply-To` address.** Lost: the sender's own address is the inbox that is read, as
  Brigade's owner decided on 2026-09-29. One address to keep, not two.
- **Checking fewer names in the lint and trusting the reviewer.** Lost: the lint is the
  deterministic half; every class of name a draft can get wrong that the checkout can
  settle, the lint settles, and the reviewer spends its minute on meaning.

## Evidence

- Brigade's runs, by id: 36626521185, a preview approved in 18 min 36 s; 36636440249, the
  send of the same releases, refused by the gate after 18 min 41 s; 36639635317, the
  approved preview's draft adopted and sent in 36 s; 37694159920, a preview approved in
  27 min 25 s, the longest run, which sets the job's bound; 38265059855, the scheduled send
  of 2026-10-12, 23 min 8 s, lint clean, refused by the reviewer with one finding, nothing
  sent.
- Cavalry's runs, by id: 38973732554, the first preview on 2026-10-16, two writer sessions
  that wrote no file because the writing directory was not a working directory of the
  session, which is why every agent step carries `--add-dir`; 38976056292, the first
  preview sent end to end; 38991604611, the second preview on Fable, stopped at the
  account's usage limit, which is why the four agent steps run Opus.
- Brigade's measurement of 2026-09-29: GitHub published the address of 1 of the 4 accounts
  that had starred it, so the list is kept by hand.
- `scripts/release-notes-test.py` on this laptop on 2026-11-02, Python 3.14: 39 tests
  against a made-up checkout and a fake GitHub API on loopback, the window from a commit, a
  date, the last email and the clock; the adopt refusals; the masks, refusals and
  deduplication of the list; every lint finding, the command and variable checks against a
  made-up `docs/reference/commands.md` included; and the joins between the workflow file,
  the script, the brief and the sample. Two of them, the ones that read the real generated
  reference, skip until `just gen-commands` has written it.
  `scripts/release-notes-email-test.py`: 49 tests, the sample laid out, every forbidden
  construct a finding with its line, every text colour at 4.5:1 or better on its
  background, nothing raw from a draft in the HTML.
- The first Muster run is a preview the owner reads before the first scheduled send; its
  run id and duration go in the commit that follows, not here.

## Consequences

Easier: a reader who wants to follow Muster gets one email a week in plain words, with the
decisions and what to do before updating; the owner reads a preview before anything is
sent; a false claim or a name that does not exist is refused before it leaves; a command
renamed in the binary refuses a draft the same Monday, through the generated page.

Harder: two to four agent sessions, about 25 minutes, every Monday, and a person's attention
when the gate refuses: a preview by hand, read, then sent with `draft_run`. The list is kept
by hand in a secret, and a reader who asks to stop is removed by hand. The sender's address
must be an inbox someone reads. The workflow holds the Claude token in four steps and the
mail credentials in three; the repository's agent token is rotated on any odd run, as
ADR 0004 says. The mail variables and secrets are not set on this repository yet: until
they are, a scheduled run is a green run that says so, and a preview is attached to its
run and mailed to nobody.

Revisit: at the first release, whether the email covers releases, commits or both; when
readers should subscribe and unsubscribe themselves; when the first measured Muster runs
justify tighter turn or time bounds than Brigade's; when `claude-code-action` scrubs
secrets without admitting strangers (ADR 0004).

## References

- `.github/workflows/send-release-notes.yml`; `scripts/release-notes.py`,
  `scripts/release-notes-email.py`, `scripts/release-notes-email-sample.md` and their
  tests; `.claude/agents/release-notes-email.md`;
  `docs/how-to/send-the-release-notes.md`
- ADR 0003 (scripts are Python standard library; `publish = false`); ADR 0004 (agents on
  Blacksmith runners, pinned action, no web tools, token rotation); ADR 0008
  (`docs/reference/commands.md` is generated from the binary)
- Cavalry ADR 0026 and its `.github/workflows/send-release-notes.yml`, `scripts/`,
  `.claude/agents/release-notes-email.md` in <https://github.com/appshapes/cavalry>;
  Brigade: `.github/workflows/send-release-notes.yml`, `scripts/ci/send-release-notes.sh`,
  `scripts/ci/release-notes-lint.sh`, `cmd/brigade-release-email`
