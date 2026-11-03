# 0005. Review pull requests in the base repository's context

- **Status:** Accepted
- **Date:** 2026-11-02
- **Invariants affected:** 6; the reviewer's enforcement of 1 to 5 is unchanged
- **Supersedes:** none

## Context

The reviewer of ADR 0004 could run on the `pull_request` event, as Cavalry's first did.
The `claude-code-action` mints its GitHub App token through an exchange that validates
the workflow file: the copy at the run's commit must be identical to the copy on
`master`, or the exchange refuses and the action skips itself. For `pull_request` the
run's commit is the merge commit GitHub computes for the pull request, which GitHub
recomputes lazily, and a pull request that changes the file itself is refused by
design. Cavalry met that on its pull request 2 (run 38102414890, refused with the
workflow file one commit behind `master`).

That exception is not rare. The action released eleven versions between 2026-09-30 and
2026-10-09 (v1.0.238 to v1.0.248), so Dependabot's weekly grouped pull request moves
the reviewer's own pin most weeks, and every one of those would need a human merge. A
second gap of the same trigger: a run that Dependabot's own events raise gets no
repository secrets, so the reviewer cannot run directly on a Dependabot pull request.

Running the reviewer itself on `pull_request_target` does not work: that trigger takes
the workflow file from the base, but the exchange refuses the run's OIDC token
outright. Cavalry's pull request 3 (run 38114955728) failed three times with "Invalid
OIDC token", and the action's tracker has the same refusal open since 2025-12-02 (issue
713, confirmed as issue 1886 on 2026-10-07): the exchange allowlists the token's
`event_name` claim and `pull_request_target` is not on it. `workflow_dispatch` is on it:
the action lists it among the automation events it is built for, and a dispatched run on
`master` carries a plain branch token. GitHub documents that a write made with a job's
own token raises no workflow event, with `workflow_dispatch` and `repository_dispatch`
as the two exceptions, so a workflow of this repository can start another one without
the owner's token.

## Decision

Two workflows carry the review, as in Cavalry since 2026-10-11. `dispatch-review.yml`
runs on `pull_request_target` (opened, synchronize, reopened, labeled), only for a pull
request whose head is in this repository, holds no secret, checks out nothing, executes
nothing from the pull request, and does two things with the job token: takes the
`review` label off if it is on, and asks GitHub to run the reviewer on `master` with the
pull request's number and head commit as inputs. `review-pull-request.yml` runs on
`workflow_dispatch` with those two inputs: it checks their shape, refuses a pull request
from a fork, stops when the head has moved on, checks out `master` at the root so the
working directory, `CLAUDE.md` and `.claude/` are `master`'s, checks the head commit out
under `pr-head/` with no persisted credentials and hands it to the agent with
`--add-dir`, and reviews as ADR 0004 says: no `just`, no `cargo`, no write tool, a job
token that cannot write contents, and a review that is posted with the App token as
`claude[bot]`, so the `pull_request_review` event that arms auto-merge and the fixer is
unchanged. A session that ends without submitting a review is dispatched once more by
a `retry` job that runs no agent; the second miss fails the run. Dependabot's pull
requests reach the reviewer through the dispatcher like any other.

## Alternatives considered

- **Run the reviewer on `pull_request` and have a person merge each bump that touches
  the file.** Lost: at the action's release cadence that is most weeks, and the loop
  exists so that a Dependabot bump needs no human step.
- **`pull_request` and tell Dependabot to ignore the action.** Lost: the one pin that
  runs a model with repository access is the one that most needs moving.
- **`pull_request` and merge `master` into every branch before review.** Lost: it does
  nothing for a pull request that changes the file, which is the case that matters,
  and adds a push by a token to every branch.
- **The reviewer itself on `pull_request_target`.** Lost to the exchange's refusal
  above, which Cavalry proved. The checkout pattern it needed, base at the root and
  head in a subdirectory, is kept, because a dispatched run reads an untrusted head the
  same way.
- **`workflow_run` after CI.** Lost: whether the exchange accepts its token is as
  unknown as it was for `pull_request_target`, it ties the review to CI finishing, and
  the `review` label path would need a dispatch anyway.
- **The job token as the action's `github_token`, skipping the exchange.** Lost: the
  review would be posted by `github-actions[bot]`, a job-token write raises no
  `pull_request_review` event so auto-merge and the fixer never arm, and a repository's
  Actions settings can forbid Actions from approving pull requests.
- **Open the dispatcher to fork pull requests.** Lost: a stranger's pull request would
  reach an agent with the Claude token in its environment. The no-execute checkout is a
  mitigation, not a boundary. Forks keep CI and a human.
- **An owner-token hand-off for Dependabot only, as Cavalry's ADR 0004 first had.**
  Lost: it is the dispatcher, with the owner's token where the job token suffices.

## Evidence

- Cavalry run 38102414890 on its pull request 2 was refused by the exchange's workflow
  validation; its checkout was merge commit 1c9b251 while `master` stood at a55d36b.
- Cavalry run 38114955728 on its pull request 3, the reviewer on `pull_request_target`:
  the checkouts worked, then the exchange answered 401 "Invalid OIDC token" on each of
  three attempts.
- `claude-code-action` issues 713 (opened 2025-12-02) and 1886 (2026-10-07): the same
  refusal, with the `event_name` allowlist as the reporter's diagnosis and no maintainer
  workaround.
- The proof in Cavalry, its pull request 4, which edited `review-pull-request.yml`
  itself: dispatch run 38115409465 started reviewer run 38115417542 on `master` at
  05:38:06Z on 2026-10-11; the exchange issued the App token at 05:38:22Z; the session
  approved head 5962f82 at 05:38:59Z as `claude[bot]`; auto-merge run 38115466161
  merged it at 05:39:15Z. Cavalry's Dependabot bumps have merged through the dispatcher
  since.
- Three of Cavalry's first hundred reviewer runs ended as a success with no review
  (38983393674, 40322905586, 40357199403), and the `review` label produced a normal
  review each time, which is why the `retry` job exists.
- Muster's first runs are cited in the commit that lands these workflows.

## Consequences

Easier: a Dependabot bump of any pin, the reviewer's own included, merges with no human
step; no job in the loop's review path holds the owner's token; a branch cut before a
change to the reviewer file needs nothing merged in.

Harder: the reviewer's run is not listed among the pull request's checks, because it
runs on `master`; a person looks for it on the Actions tab under `review pr`, and the
how-to says so. The dispatcher's same-repository condition must be read as a boundary,
and a reader who knows the trigger's reputation has to be told why it is safe here,
which the file's header does. The reviewer reads the pull request's tree under
`pr-head/` rather than at the root, so its brief says where to look.

Revisit: when the exchange accepts `pull_request_target` (issue 713 closed), which
would let the dispatcher go; when a fork contribution should be agent-reviewed (new
ADR, with the secrets question answered first).

## References

- ADR 0004; `CLAUDE.md`, "Working rules for agents"
- `.github/workflows/dispatch-review.yml`, `.github/workflows/review-pull-request.yml`,
  `.github/dependabot.yml`, `.claude/agents/reviewer.md`,
  `docs/how-to/ask-the-agent.md`
- Cavalry ADR 0005, `docs/decisions/0005-review-in-the-base-context.md` in
  <https://github.com/appshapes/cavalry>
- `claude-code-action`: `docs/security.md`, `src/github/context.ts`; issues 713 and
  1886
- GitHub Docs, "Automatic token authentication", on events a `GITHUB_TOKEN` write does
  not raise; GitHub Security Lab, "Preventing pwn requests"
