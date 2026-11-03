#!/usr/bin/env bash
# The README's ten-minute path, timed, against the fake Linear (CLAUDE.md, gate 5; ADR
# 0010). The README's first two commands install the binary and write the key file; this
# script builds the binary from the checkout and gives it the fake's key through the
# environment instead, then runs the five commands that reach Linear exactly as the
# README shows them, with the second slice's project steps where the tutorial's step 4
# names them (ADR 0013), and checks what each one printed. What CI runs on Linux and
# macOS.
set -euo pipefail
cd "$(dirname "$0")/.."

command -v jq >/dev/null || { echo "quickstart: jq is not installed (brew install jq, or apt-get install jq)" >&2; exit 1; }
start=$(date +%s)

cargo build --release -p muster-linear --bin muster --example fake-linear
bin=target/release/muster
fake=target/release/examples/fake-linear

tmp=$(mktemp -d)
trap '{ kill "${fake_pid:-0}" && wait "${fake_pid:-0}"; } 2>/dev/null || true; rm -rf "$tmp"' EXIT
"$fake" > "$tmp/endpoint" &
fake_pid=$!
for _ in $(seq 1 100); do [ -s "$tmp/endpoint" ] && break; sleep 0.05; done
[ -s "$tmp/endpoint" ] || { echo "quickstart: the fake Linear did not start" >&2; exit 1; }

# The environment the README's reader has, with the fake in Linear's place: no key file
# (HOME is empty), the key in the environment, no team named.
export HOME="$tmp" MUSTER_ENDPOINT="$(head -1 "$tmp/endpoint")" LINEAR_API_KEY=lin_api_fake
unset MUSTER_TEAM MUSTER_KEY_FILE XDG_CONFIG_HOME

step() { echo; echo "quickstart: $*"; }

step "muster whoami"
"$bin" whoami | tee "$tmp/whoami.json"
jq -e '.viewer.id and .muster.team.key == "APP" and .muster.key.source == "LINEAR_API_KEY"' "$tmp/whoami.json" >/dev/null

step "muster workspace get"
"$bin" workspace get | tee "$tmp/workspace.json"
jq -e '.urlKey == "fake" and (.userCount | type) == "number" and .subscription == null' "$tmp/workspace.json" >/dev/null

step "muster team get"
"$bin" team get | tee "$tmp/team.json"
jq -e '.key == "APP" and .cyclesEnabled == true and .defaultIssueState.name == "Backlog"' "$tmp/team.json" >/dev/null

step "muster user get me"
"$bin" user get me | tee "$tmp/user.json"
jq -e '.isMe == true and .admin == true and .active == true' "$tmp/user.json" >/dev/null

step "muster member list"
"$bin" member list | tee "$tmp/members.jsonl"
jq -s -e 'length == 2 and .[0].owner == true and .[0].user.email == "fake@example.com"' "$tmp/members.jsonl" >/dev/null

step "muster issue create"
"$bin" issue create --title "Try muster" --description "Created from the README." | tee "$tmp/create.json"
id=$(jq -r .identifier "$tmp/create.json")
[[ "$id" =~ ^APP-[0-9]+$ ]] || { echo "quickstart: no identifier in the create answer" >&2; exit 1; }

step "muster issue comment $id"
"$bin" issue comment "$id" --body "Read it back with issue get." | tee "$tmp/comment.json"
jq -e '.body == "Read it back with issue get." and .issue.identifier == "'"$id"'"' "$tmp/comment.json" >/dev/null

step "muster issue get $id"
"$bin" issue get "$id" | jq '{title, state: .state.name, comments: [.comments.nodes[].body]}' | tee "$tmp/get.json"
jq -e '.title == "Try muster" and .state == "Backlog" and .comments == ["Read it back with issue get."]' "$tmp/get.json" >/dev/null

step "muster state create"
"$bin" state create --name "Trying muster" --type started --color "#4ea7fc" | tee "$tmp/state.json"
jq -e '.name == "Trying muster" and .type == "started" and .team.key == "APP"' "$tmp/state.json" >/dev/null

step "muster label create"
"$bin" label create --name "Trying muster" --color "#26b5ce" | tee "$tmp/label.json"
jq -e '.name == "Trying muster" and .team.key == "APP" and .retiredAt == null' "$tmp/label.json" >/dev/null

step "muster project create"
"$bin" project create --name "Trying muster" | tee "$tmp/project.json"
jq -e '.name == "Trying muster" and .teams.nodes[0].key == "APP"' "$tmp/project.json" >/dev/null

step "muster project-status list"
"$bin" project-status list | tee "$tmp/statuses.jsonl"
jq -s -e 'length == 5 and .[0].name == "Backlog" and .[2].type == "started"' "$tmp/statuses.jsonl" >/dev/null

step "muster project get and update"
"$bin" project update "Trying muster" --status "In Progress" --lead me | jq -e '.status.name == "In Progress" and .lead.id' >/dev/null
"$bin" project get "Trying muster" | tee "$tmp/project-get.json"
jq -e '.name == "Trying muster" and .status.name == "In Progress" and (.members.nodes | type) == "array"' "$tmp/project-get.json" >/dev/null

step "muster cycle list --active and the issue's --cycle"
"$bin" cycle list --active | tee "$tmp/cycle.json"
jq -e '.isActive == true and .number == 2 and .team.key == "APP"' "$tmp/cycle.json" >/dev/null
"$bin" issue update "$id" --cycle active | jq -e '.cycle.number == 2' >/dev/null
"$bin" cycle get 2 | jq -e --arg id "$id" '[.issues.nodes[].identifier] | index($id) != null' >/dev/null

step "muster issue list --project"
"$bin" issue update "$id" --project "Trying muster" | jq -e '.project.name == "Trying muster"' >/dev/null
"$bin" issue list --project "Trying muster" | tee "$tmp/in-project.jsonl"
jq -s -e --arg id "$id" 'map(.identifier) == [$id]' "$tmp/in-project.jsonl" >/dev/null

# After the issue is in the project: `--milestone` resolves in the issue's own project.
step "muster milestone create and the issue's --milestone"
"$bin" milestone create --project "Trying muster" --name "Alpha" --target-date 2026-12-01 | tee "$tmp/milestone.json"
jq -e '.name == "Alpha" and .project.name == "Trying muster" and .targetDate == "2026-12-01"' "$tmp/milestone.json" >/dev/null
"$bin" issue update "$id" --milestone Alpha | jq -e '.projectMilestone.name == "Alpha"' >/dev/null
"$bin" issue list --milestone Alpha | jq -s -e --arg id "$id" 'map(.identifier) == [$id]' >/dev/null

step "muster project-update create and list"
"$bin" project-update create --project "Trying muster" --body "Shipping on Friday." --health onTrack | tee "$tmp/project-update.json"
jq -e '.health == "onTrack" and .body == "Shipping on Friday." and .project.name == "Trying muster"' "$tmp/project-update.json" >/dev/null
"$bin" project-update list --project "Trying muster" | jq -s -e 'length == 1' >/dev/null

step "muster initiative create, add-project and get"
# The fake seeds an initiative named "Launch", so the README's is "Growth".
"$bin" initiative create --name "Growth" --owner me --status Active | tee "$tmp/initiative.json"
jq -e '.name == "Growth" and .status == "Active" and .leadTeam == null' "$tmp/initiative.json" >/dev/null
"$bin" initiative update Growth --lead-team APP | jq -e '.leadTeam.key == "APP"' >/dev/null
"$bin" initiative add-project Growth --project "Trying muster" | jq -e '.project.name == "Trying muster"' >/dev/null
"$bin" initiative get Growth | jq -e '.projects.nodes[0].name == "Trying muster"' >/dev/null

step "muster issue close $id"
"$bin" issue close "$id" --comment "Done from the README." | tee "$tmp/close.jsonl"
jq -s -e 'length == 2 and .[0].body == "Done from the README." and .[1].state.type == "completed" and .[1].identifier == "'"$id"'"' "$tmp/close.jsonl" >/dev/null

step "the exit codes an agent branches on"
set +e
"$bin" issue get APP-999 2>"$tmp/err.json"; code=$?
set -e
[ "$code" = 4 ] || { echo "quickstart: issue get APP-999 exited $code, not 4" >&2; exit 1; }
jq -e '.error.code == "not_found" and .error.exit == 4' "$tmp/err.json" >/dev/null
echo "exit 4, $(jq -r .error.message "$tmp/err.json")"

echo
echo "quickstart: ok in $(( $(date +%s) - start )) s"
