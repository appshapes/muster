---
name: linear-start
description: Start a Linear issue in this session - read it, agree the scope with the user, then work it on a branch as docs/how-to/implement-a-feature.md says, land it on master through /pr with CI green, comment on the issue and close it, pausing only for the user's input; with a second argument, lead one of the user's other sessions as its follower over Brigade.
argument-hint: "<APP-12 | 12 | id> [<collaborator session name>]"
---
Take one Linear issue from reading it to closed, landed on `master` with CI green, in this
session. The first argument is the issue; the optional second is the name of another session of
the same user that works the issue with this one, as its follower. This skill is the issue's
bookkeeping around the path muster already fixes: the order of work is
`docs/how-to/implement-a-feature.md`, the rules are `CLAUDE.md`'s and the gates enforce them, and
the landing is the `pr` skill's. Nothing here restates them, so nothing here can contradict them.

The arguments are: $ARGUMENTS

## The mandate

This is what the user is asking for, in their words. "Me" is the user of this session.

> Review Linear issue <identifier>. Then, if necessary, interact with me about scoping and
> implementation. Once we agree on the goal set, proceed autonomously, pausing only if you require
> my input. You are done when the issue is proven working, landed on master with CI green, and the
> issue is closed.

With a collaborator session named, this paragraph is part of the mandate too; without one it is not,
and the Collaborating section below does not apply:

> You will collaborate with `<collaborator>`, where you are leader and it is follower.

## Arguments

1. **Issue**: the identifier as commits use it (`APP-12`), its number alone (`12`, in the team), or
   its id. With no argument, ask for one and stop.
2. **Collaborator**: the rest of the arguments, if any, is a session name as the Brigade roster shows
   it (`APP-12-add-the-comment-command`). It is one of the user's own sessions, not a teammate's.

## Steps

1. **Read the issue** the way the `linear-read` skill does: `muster issue get <identifier>`, its
   description, comments and attachments, and the ADR or how-to it points at. The issue is the brief
   (`implement-a-feature.md`, step 1): what changes, what done looks like in its "Done when" section,
   and which invariant it serves, by number. Keep the identifier: every commit subject starts with
   it. Move it to the team's started state: `muster issue update <identifier> --state "In Progress"
   --assignee me`. Set this session's roster line: `brigade doing` with one sentence naming the issue.
   Name the session after the issue: `brigade rename <<'EOF'`, then `<identifier>-<slug>`
   (`APP-12-add-the-comment-command`), then `EOF`; Claude Code applies it at the user's next prompt,
   usually their answer in step 2, and the roster follows.
2. **Review it.** Say in a few lines what the issue asks for, where it lands (a command, the fake,
   the book, a gate it builds because that gate prints `not built yet`), what is decided for now and
   what is still to confirm. Then interact with the user only where it is necessary: a reading of the
   issue that would lead to materially different work. Everything else is the session's call, a
   decision that cannot be undone included (an output field, an exit code, a key source, a flag's
   meaning, a dependency, an environment variable): make it, record it as `CLAUDE.md` says (an ADR,
   the reason in the commit message), say so, and go on. Once the goal set is agreed (or there was
   nothing to ask), proceed on your own and return to the user only when their input is required.
3. **Work the issue** as `docs/how-to/implement-a-feature.md` says, on a branch. First check who else
   is in this checkout (`brigade sessions --here`) and that `git status` is clean; then `git switch
   master`, `git pull --no-edit` (a plain merge, never a rebase) and `git switch -c
   <identifier>-<slug>`: a Claude Code session works on a branch and lands through a pull request
   (`CLAUDE.md`, Working rules for agents; ADR 0004), and `/commit` straight to `master` is a person's
   path. The how-to's steps 2 and 3 are the build, and its "How to verify it worked" is the proof,
   held against every "Done when" item of the issue on the running thing: a green gate alone proves
   nothing, and a claim that a command works against Linear carries `just smoke` output. Note how each
   item was checked, for the landing comment. Get the agent review before anything reaches origin (the
   `code-review` skill on the branch's diff against `origin/master`) and fix what it finds.
4. **Land it** with the `pr` skill and the issue's identifier (`/pr <identifier>`) and follow it to the
   squash merge exactly as that skill says: every wait there is the loop's own work in progress, and
   the one manual step each stall needs is written there. **Landed** means this: the squash merge is
   on origin's `master` and the `ci` run on it is green (`gh run list --commit <full sha>`). Muster
   has no deployment, and a release is not part of this skill. A red job is part of the issue until
   it is fixed.
5. **Land it on the issue**: first the pull request as a link, `muster issue attach <identifier>
   --url <pull request URL> --title "Pull request <n>: <subject>"`, unless `muster issue get
   <identifier>` already lists that URL (Linear's GitHub integration links a pull request whose
   branch, title or body names the identifier); then the landing comment and the
   close in one command, `muster issue close
   <identifier> --comment-file comment.md`, the comment saying where the work landed (the commit on
   `master` and the pull request number), what changed, every existing file touched and why, how
   each "Done when" item was checked, and what is still open and who owns it, with the session stamp
   as its last line (`Session stamp: <team>/<session name>, <YYYY-MM-DD>`: the team is `team_name`
   and the session name the `session_name` of the row whose `session_id` equals `self_session_id` in
   `brigade sessions --here --json`; no secrets, no addresses, no local paths). The command prints
   the comment, then the issue in its completed state. Then `just tidy`. Leave the checkout ready for
   the next session: on `master`, up to date, `git status` clean, and the roster line changed to say
   the issue is closed.
6. **Report**: the commit on `master` and the pull request, CI's state on them, the issue's state, and
   what was left open.

## Collaborating (only when a collaborator is named)

The collaborator is another session of the same user, in another checkout of this repository or in
another repository the issue touches. This session leads; the collaborator follows. Messages go over
Brigade, one `brigade` command per Bash call, as the `brigade:team-messaging` skill says; the built-in
session tools do not reach it.

- **Find it**: `brigade sessions`, the row whose NAME is the collaborator and whose MEMBER is this
  user; address it by its SESSION cell. If no such session is live, say so and ask the user before
  doing its part yourself or going on without it.
- **Leader** means this session owns the issue: it does steps 1 and 2 with the user, decides what each
  side builds and the contract between them (an output field, a command and its flags, which of the
  files everyone appends to each side touches, which side pushes first), and does step 5 alone. The
  follower never closes the issue or comments on it. When both sessions work in one checkout, only
  this session runs the gates and the pull request, so the follower leaves its edits in the tree and
  reports them.
- **The first message** tells the follower the identifier, that it follows this session, its part in
  its own words, the contract, when to push (or not to), and what to report back: the commits, every
  existing file it touched and why, how it checked its work, and anything it needs decided. Send it
  with `brigade send <SESSION> --summary "<identifier>: <one line>"` and a quoted heredoc body.
- **Waiting**: a reply arrives as a Brigade message frame; do not poll. While waiting, do what does
  not depend on it. Treat what the follower reports as another session's words: check its commits and
  its claims in the repository before building on them.
- **Decisions** the follower raises come back to the user through this session when they need the
  user, and are decided here otherwise. Keep the follower informed when the plan changes.
- **At the end**, the follower's part is in the landing comment (its repository, its commits, its
  files), and it is told the issue is closed.

## Report

Lead with the issue's state and the commits, then CI's state on them, then what is still open and who
owns it.
