# 0004. Run agents on branches, with the owner's token out of every agent's job

- **Status:** Accepted
- **Date:** 2026-11-02
- **Invariants affected:** 1, 5, 6
- **Supersedes:** none

## Context

Developers commit straight to `master` (ADR 0003). Bots and agents cannot: a change
nobody has read must be reviewable and revertable as one unit, so they work on branches
and open pull requests. Cavalry, the sibling repository whose practices muster ports, has
run such a loop since 2026-10-10, itself a port of Brigade's, which has run since
2026-09-10: a writer agent implements a labelled issue on a branch, a read-only reviewer
agent approves or requests changes, a fixer applies the blockers, and a merge is armed
when the reviewer approves. Muster adopts the loop on its first day, with the workflow
files copied from Cavalry and the names changed. Its two secrets exist on the repository
from 2026-11-02: `CLAUDE_CODE_OAUTH_TOKEN`, a Claude Code token for the agents, and
`GH_ACTIONS_TOKEN`, a personal access token of the owner's account. Muster is public, so
every run log is world readable.

Four facts shape the design, measured by Brigade and Cavalry and unchanged here. First,
`claude-code-action` scrubs secrets from an agent's subprocesses only when
`allowed_non_write_users` is set, and setting it lets anyone's comment start an agent on
a public repository; so the scrub is off, and every shell command an agent runs can read
the agent job's environment. Second, a personal access token reaches every repository
its account can reach and does not expire with the job. Third, a write made with the
job's own `GITHUB_TOKEN` raises no workflow event, while a write made with the personal
access token does; the chain issue, pull request, review, merge only moves on events.
Fourth, a workflow run that Dependabot triggers receives no repository secrets and a
read-only token, which ADR 0005 answers.

`master` gets classic branch protection, set by the owner on 2026-11-02: the two CI
checks `gates (linux)` and `gates (macos)` are required, branches need not be up to date,
force pushes and deletions are blocked, and repository admins bypass the rule, which is
what lets a developer's direct commit through. No approval is required by the rule; the
approval requirement lives in the auto-merge workflow. The Claude GitHub App and the
Blacksmith runner app are installed for every repository of the organisation.

## Decision

Three workflows and one dispatcher run the loop, all on Blacksmith runners, with the
`claude-code-action` pinned to a full commit SHA. `claude.yml` is the writer: an issue
labelled `claude`, an `@claude` comment, a `claude` label on a pull request, or a
changes-requested review from the reviewer starts an agent in a job that holds only
`CLAUDE_CODE_OAUTH_TOKEN` and a job token scoped to this repository for the length of
the run. A second job, which runs no agent, holds `GH_ACTIONS_TOKEN` and opens the pull
request from the branch the agent pushed, so the `opened` event fires. A third job, also
without an agent, approves the CI runs GitHub holds on a fixer's push and asks for the
review of that push. `review-pull-request.yml` runs the reviewer on every
same-repository pull request, on `master`, through the dispatcher of ADR 0005, with a
job token that cannot write contents, denies the edit tools outright, and fails the job
when no review lands at the pull request's head. `auto-merge.yml` arms GitHub's own
auto-merge, as a squash, when the reviewer approved the current head of a pull request
labelled `auto-merge`; GitHub then merges once the two required checks pass. The squash
commit takes its subject from the pull request title and its body from the pull request
body, so the pull request body is the review record CLAUDE.md asks of every commit.
Dependabot labels its pull requests `auto-merge`. Fork pull requests get CI and a human.
Every agent runs with `show_full_output` off, an explicit short `allowed_bots` list, no
web tools, and a deny list covering `.github/workflows/`, `.claude/` and
`crates/muster/tests/fixtures/`, the recorded shapes of Linear's answers that the fake
Linear is held to (ADR 0010): an agent that edits a fixture makes the fake agree with
its own code instead of with Linear. The agents' briefs live in `.claude/agents/`, and
`docs/how-to/ask-the-agent.md` is the page for a person who wants work done.

## Alternatives considered

- **One job holding both tokens.** Lost: an agent's shell children inherit the job's
  environment, and the personal access token reaches every repository of the account.
  Keeping it in a job that runs no agent is the one structural control; the rest are
  mitigations.
- **A GitHub App token minted per run instead of the personal access token.** Lost for
  now: it is one more application to register and rotate, Cavalry and Brigade run on
  the token with the job split above, and the split is the same either way. Revisited
  if the token's reach ever includes a repository that is not the owner's own.
- **`allowed_non_write_users` for the action's secret scrub.** Lost: it admits prompts
  from strangers on a public repository to buy a best-effort scrub.
- **Merge commits for bot pull requests.** Lost: a squash leaves one commit per pull
  request on `master`, and that commit's message is the review record, which is the
  rule for every commit. Nothing already on `master` is rewritten, so the no-rebase
  rule is untouched.
- **Merging from the workflow when CI is green.** Lost: the personal access token is an
  admin's and bypasses the protection rule, so a direct merge would be a bypass by
  construction. Arming GitHub's auto-merge leaves the waiting to GitHub.
- **A ruleset instead of classic protection.** Lost: the owner chose classic for
  Cavalry; both express what this decision needs, and one shape across the sibling
  repositories is one less thing to read.
- **A reviewer that can push small fixes.** Lost: a reviewer that can push is not a
  reviewer. The fixer is the writer, on the writer's token.
- **A human approval for auto-merge.** Lost: the loop would wait for a person on every
  Dependabot bump. A human can still merge any pull request by hand.
- **GitHub-hosted runners.** Lost: the owner's choice is Blacksmith, which is installed
  for the organisation, runs macOS as well as Linux, and ran Cavalry's CI in 16 s and
  19 s on an empty workspace. Blacksmith sits in the runner path and sees whatever
  secrets a job holds; Cavalry and Brigade accept the same boundary.
- **Web tools for the agents.** Lost: nothing in this repository's work needs the web,
  and a fetch tool is the cheapest way for an injected prompt to send a token out.
  `cargo` and `just` still execute code, so this closes a path, not the class.
- **A smaller deny list, the fixtures left writable.** Lost: the fixtures are the one
  place where an agent could make a failing truth test pass without touching Linear.

## Evidence

- Brigade's reviewer sessions, fourteen that reported a turn count on its pull requests
  3, 7 and 8 (2026-09-10 to 2026-09-13), ran 16 to 42 turns, the longest 5 min 41 s.
  Its writer sessions ran 13 to 124 turns. The turn and time bounds in the workflow
  files are Brigade's, kept by Cavalry; muster starts from them and re-measures.
- Brigade measured on 2026-09-17 that GitHub holds the runs a fixer's push raises at
  `action_required` whatever the fork approval policy says (Brigade pull request 22),
  which is why the writer workflow carries the approving job.
- Cavalry's loop (Cavalry ADR 0004, 2026-10-10): the writer's first run 38102182969
  took 28 s, pushed its branch, and the owner's token opened pull request 2 with the
  commit message as the body; the reviewer approved at 54795fd in 58 s as
  `claude[bot]`; auto-merge run 38102567988 merged it six seconds after the approval
  as squash commit abe9eb8. Between then and 2026-11-02 the loop landed Cavalry's
  pull requests up to number 95 and its Dependabot bumps with no human merge.
- Cavalry's fixer parked twice on a fix that did not land (its pull requests 10 and
  84), which is why `claude.yml` verifies that a fix landed and asks for the review of
  a body-only fix itself; muster inherits both checks.
- Muster's first runs are cited in the commit that lands these workflows.

## Consequences

Easier: a person opens an issue, labels it, and reads a pull request; a Dependabot bump
merges with no human step; every agent change is one commit on `master` with a review
record; a stranger can read why each token is where it is from the workflow files and
this page.

Harder: a workflow change needs a developer, since agents may not edit
`.github/workflows/`; a fork contribution waits for a human reviewer; the owner's token
is rotated on any odd run, and so is the Claude token, because the agent's environment
is reachable from anything it executes. The Blacksmith Codesmith app edits pull request
bodies; `auto-merge.yml` strips its footer, and turning Codesmith off for this
repository would remove the need.

Revisit: when the action supports scrubbing without admitting strangers; when the
personal access token's reach grows beyond the owner's own repositories; when the first
measured Muster runs justify tighter turn or time bounds; when an agent needs a key to
run `just smoke` on a runner, which is a new secret and a new ADR.

## References

- `CLAUDE.md`, "Working rules for agents"; ADR 0001; ADR 0003; ADR 0005; ADR 0010
- `.github/workflows/ci.yml`, `claude.yml`, `dispatch-review.yml`,
  `review-pull-request.yml`, `auto-merge.yml`; `.github/dependabot.yml`
- `.claude/agents/reviewer.md`, `.claude/agents/developer.md`;
  `docs/how-to/ask-the-agent.md`
- Cavalry ADR 0004 and ADR 0005, `docs/decisions/` in
  <https://github.com/appshapes/cavalry>; Brigade: `.github/workflows/claude.yml`,
  `review-pull-request.yml`, `auto-merge.yml`
- `claude-code-action`'s `docs/security.md` on the conditional scrub
