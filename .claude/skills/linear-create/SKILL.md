---
name: linear-create
description: Create a Linear issue in the team with the muster CLI, from a title or a markdown file, with optional labels, state, priority, project and assignee
argument-hint: "<title> [--team <key>] [--label <name>]... [--state <name>] [--priority 0-4] [--project <name>] [--assignee me|email|name] [--parent <APP-12>] [--from <markdown file>]"
---
Create one Linear issue with `muster issue create`. `muster` is this repository's own
binary (`cargo install --path crates/muster`); it prints the created issue as one JSON
line and exits with a documented code (`docs/reference/commands.md#exit-codes`).

The arguments are: $ARGUMENTS

## Defaults and conventions

- The team is inferred when the workspace has one; otherwise `--team <key>` or
  `MUSTER_TEAM` names it (`muster team list` shows the keys). The appshapes workspace
  has two teams, `APP` for the house's work and `SMK`, the smoke test's own (ADR 0014),
  so its sessions set `MUSTER_TEAM=APP` in `.claude/settings.local.json`; without it a
  create exits 2 and names the teams.
- One team with a project per product (APP-6, 2026-11-04): muster's issues go in the
  `Muster` project, so pass `--project Muster`; the smoke test files its own issues in
  `Smoke tests`.
- Titles are plain, without an identifier. Linear assigns the number; the identifier it
  prints (`APP-12`) is what a commit subject cites.
- Labels are by name, `--label` repeatable; `muster label list` shows them. A state is by
  name (`muster state list`); the team's default state is used when none is given.
- If a markdown file is given (`--from`), its first `# Heading` is the title and the rest
  is the description; pass the rest through `--description-file`. Linear descriptions
  render markdown.
- A title an open issue of the team already has is refused with exit 6 and the existing
  identifier in the message: use that issue, or pass `--allow-duplicate` when a second one
  is wanted on purpose.
- `--parent <APP-12>` makes the issue a sub-issue of that one: a part of a feature split
  into several pull requests, or a follow-up under the issue that found it. The sub-issue
  is created in the parent's team unless `--team` or `MUSTER_TEAM` names one, and
  `muster issue get APP-12` lists it under `children`.
- Never put a token, an API key, a password or any other credential on an issue.

## Steps

1. If `muster whoami` exits 3, the key is not in place: it lives in `CLAUDE.user.md` under
   `# Linear` and belongs in `~/.config/muster/key`, written with a quoted heredoc
   (`(umask 077; cat > ~/.config/muster/key <<'EOF'` … `EOF)`), never on a command line
   and never echoed.
2. Create the issue. Pass a long description through a file:
   ```
   sed '1{/^# /d;}' issue.md > /tmp/description.md
   muster issue create --title "<title>" --label Feature --description-file /tmp/description.md
   ```
   Optional flags: `--state <name>`, `--priority <0-4>` (1 urgent, 4 low), `--project
   <name>`, `--assignee me`, `--parent <APP-12>`.
3. Read the JSON line and report the `identifier` and `url`. Check that `labels.nodes`
   names the labels asked for.
4. To add a comment afterwards: `muster issue comment <identifier> --body-file <file>`.

## Report

One line per issue: identifier, title, state, URL. Do not paste the description back.
