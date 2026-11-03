#!/usr/bin/env bash
# The README's ten-minute path against a REAL workspace (CLAUDE.md, gate 1; ADR 0010),
# then the second slice's commands (ADR 0013) and the third's, group by group (ADR
# 0014): the key from its usual sources, the team from MUSTER_TEAM, else the first team
# that is not the smoke team `SMK` (which this script creates, so the workspace has
# several teams and none is inferred), and one issue created, commented, read, given a
# sub-issue, and then canceled with it (not completed, so that a smoke test never counts
# as work done). Not in CI; run it by hand before a release and put its output in the
# commit message. Prints every JSON line muster printed.
set -euo pipefail
cd "$(dirname "$0")/.."
unset MUSTER_ENDPOINT

cargo build --release -p muster-linear --bin muster
bin=target/release/muster
stamp=$(date -u +%Y-%m-%dT%H:%M:%SZ)
tmp=$(mktemp -d)
trap 'rm -rf "$tmp"' EXIT

echo "smoke: muster whoami"
"$bin" whoami | tee "$tmp/whoami.json"
echo
# The smoke team (ADR 0014) makes this a workspace with several teams, so the issue
# steps name the one they always used: MUSTER_TEAM when set, else the first team that
# is not SMK.
export MUSTER_TEAM="${MUSTER_TEAM:-$("$bin" team list | jq -r 'select(.key != "SMK") | .key' | head -1)}"
echo "smoke: the issue steps use team $MUSTER_TEAM (MUSTER_TEAM)"
echo
echo "smoke: muster workspace get"
"$bin" workspace get | jq -c '{name, urlKey, userCount, createdIssueCount, subscription}' | tee "$tmp/workspace.json"
jq -e --slurpfile who "$tmp/whoami.json" '.urlKey == $who[0].organization.urlKey and (.userCount | type) == "number"' "$tmp/workspace.json" >/dev/null
echo
echo "smoke: muster workspace update --project-update-reminders-hour <the hour it has> (a change to the same value: the path is proven and nothing moves)"
hour=$("$bin" workspace get | jq -r .projectUpdateRemindersHour)
"$bin" workspace update --project-update-reminders-hour "$hour" | jq -c '{urlKey, projectUpdateRemindersHour, updatedAt}' | tee "$tmp/workspace-update.json"
jq -e --argjson hour "$hour" '.projectUpdateRemindersHour == $hour' "$tmp/workspace-update.json" >/dev/null
echo
echo "smoke: muster user get me, then user update me with a status that clears at once (ADR 0014)"
"$bin" user get me | jq -c '{displayName, admin, owner, guest, active, timezone}' | tee "$tmp/user.json"
jq -e --slurpfile who "$tmp/whoami.json" '.displayName == $who[0].viewer.displayName and .active == true' "$tmp/user.json" >/dev/null
"$bin" user update me --status-emoji "🧪" --status-label "Smoke test at $stamp" --status-until "$stamp" | jq -c '{displayName, statusEmoji, statusLabel, statusUntilAt}' | tee "$tmp/user-update.json"
jq -e --arg stamp "Smoke test at $stamp" '.statusLabel == $stamp' "$tmp/user-update.json" >/dev/null
echo
echo "smoke: muster team create --name 'Smoke tests' --key SMK (exit 6 after the first run: the duplicate guard; ADR 0014)"
set +e
"$bin" team create --name "Smoke tests" --key SMK --description "The smoke test's own team (ADR 0014): where its cycles, members and lifecycle are tested, so that APP is never touched." --timezone Etc/UTC > "$tmp/team.json" 2> "$tmp/team.err"
code=$?
set -e
case "$code" in
  0) jq -c '{key, name, cyclesEnabled, timezone}' "$tmp/team.json" ;;
  6) cat "$tmp/team.err"; jq -e '.error.code == "refused" and (.error.message | test("a team named"))' "$tmp/team.err" >/dev/null ;;
  *) echo "smoke: team create exited $code" >&2; cat "$tmp/team.err" >&2; exit 1 ;;
esac
echo
echo "smoke: muster team get SMK"
"$bin" team get SMK | jq -c '{key, name, cyclesEnabled, retiredAt, archivedAt, issueCount}' | tee "$tmp/team-get.json"
jq -e '.key == "SMK" and .archivedAt == null' "$tmp/team-get.json" >/dev/null
echo
echo "smoke: muster team update SMK --cycles-enabled true --cycle-enabled-start-date $stamp (the smoke team's cycles; ADR 0014)"
"$bin" team update SMK --cycles-enabled true --cycle-enabled-start-date "$stamp" --cycle-duration 1 --upcoming-cycle-count 2 --description "The smoke test's own team (ADR 0014): its cycles, members and lifecycle are tested here, so that the house's team is never touched." | jq -c '{key, cyclesEnabled, cycleDuration, upcomingCycleCount, activeCycle}' | tee "$tmp/team-update.json"
jq -e '.key == "SMK" and .cyclesEnabled == true and .cycleDuration == 1' "$tmp/team-update.json" >/dev/null
echo
echo "smoke: muster member list --team SMK, then remove me and add me back (ADR 0014)"
"$bin" --team SMK member list | jq -c '{user: .user.displayName, owner}' | tee "$tmp/members.jsonl"
jq -s -e 'length >= 1' "$tmp/members.jsonl" >/dev/null
"$bin" --team SMK member remove --user me | tee "$tmp/member-remove.json"
jq -e '.success == true' "$tmp/member-remove.json" >/dev/null
"$bin" --team SMK member add --user me | jq -c '{user: .user.displayName, team: .team.key, owner}' | tee "$tmp/member-add.json"
jq -e '.team == "SMK"' "$tmp/member-add.json" >/dev/null
echo
echo "smoke: muster team retire SMK, then unretire"
"$bin" team retire SMK | jq -c '{key, retiredAt}' | tee "$tmp/retire.json"
jq -e '.retiredAt != null' "$tmp/retire.json" >/dev/null
"$bin" team unretire SMK | jq -c '{key, retiredAt}' | tee "$tmp/unretire.json"
jq -e '.retiredAt == null' "$tmp/unretire.json" >/dev/null
echo
echo "smoke: muster team delete SMK, then unarchive (the only team the lifecycle is tested on; ADR 0014)"
"$bin" team delete SMK | tee "$tmp/delete.json"
jq -e '.success == true and (.entityId | type) == "string"' "$tmp/delete.json" >/dev/null
"$bin" team unarchive SMK | jq -c '{key, archivedAt}' | tee "$tmp/unarchive.json"
jq -e '.key == "SMK" and .archivedAt == null' "$tmp/unarchive.json" >/dev/null
"$bin" team list | jq -c 'select(.key == "SMK") | {key, retiredAt}' | tee "$tmp/team-list.json"
jq -e '.key == "SMK"' "$tmp/team-list.json" >/dev/null
echo
echo "smoke: muster project create --name 'Smoke tests' (exit 6 after the first run: the duplicate guard)"
set +e
"$bin" project create --name "Smoke tests" --description "Where the smoke test's issues go; each is canceled by the run that made it." > "$tmp/project.json" 2> "$tmp/project.err"
code=$?
set -e
case "$code" in
  0) cat "$tmp/project.json" ;;
  6) cat "$tmp/project.err"; jq -e '.error.code == "refused" and (.error.message | test("a project named"))' "$tmp/project.err" >/dev/null ;;
  *) echo "smoke: project create exited $code" >&2; cat "$tmp/project.err" >&2; exit 1 ;;
esac
echo
echo "smoke: muster state create --name 'Smoke <stamp>' --type backlog, update, archive, in team $MUSTER_TEAM (ADR 0014)"
"$bin" state create --name "Smoke $stamp" --type backlog --color "#bec2c8" --description "Made by scripts/smoke.sh at $stamp and archived by the same run." | jq -c '{name, type, position, team: .team.key}' | tee "$tmp/state.json"
jq -e --arg n "Smoke $stamp" '.name == $n and .type == "backlog"' "$tmp/state.json" >/dev/null
"$bin" state update "Smoke $stamp" --name "Smoky $stamp" | jq -c '{name, type}' | tee "$tmp/state-update.json"
jq -e --arg n "Smoky $stamp" '.name == $n' "$tmp/state-update.json" >/dev/null
"$bin" state archive "Smoky $stamp" | jq -c '{name, type}' | tee "$tmp/state-archive.json"
jq -e --arg n "Smoky $stamp" '.name == $n' "$tmp/state-archive.json" >/dev/null
"$bin" state list | jq -c 'select(.name == "Smoky '"$stamp"'")' | tee "$tmp/state-gone.jsonl"
[ ! -s "$tmp/state-gone.jsonl" ] || { echo "smoke: the archived state is still listed" >&2; exit 1; }
echo
echo "smoke: muster label create --name 'Smoke <stamp>', update, retire, restore, delete, in team $MUSTER_TEAM (ADR 0014)"
"$bin" label create --name "Smoke $stamp" --color "#26b5ce" --description "Made by scripts/smoke.sh at $stamp and deleted by the same run." | jq -c '{name, color, team: .team.key, retiredAt}' | tee "$tmp/label.json"
jq -e --arg n "Smoke $stamp" '.name == $n and .retiredAt == null' "$tmp/label.json" >/dev/null
"$bin" label update "Smoke $stamp" --name "Smoky $stamp" | jq -c '{name}' | tee "$tmp/label-update.json"
jq -e --arg n "Smoky $stamp" '.name == $n' "$tmp/label-update.json" >/dev/null
"$bin" label retire "Smoky $stamp" | jq -c '{name, retiredAt}' | tee "$tmp/label-retire.json"
jq -e '.retiredAt != null' "$tmp/label-retire.json" >/dev/null
"$bin" label restore "Smoky $stamp" | jq -c '{name, retiredAt}' | tee "$tmp/label-restore.json"
jq -e '.retiredAt == null' "$tmp/label-restore.json" >/dev/null
"$bin" label delete "Smoky $stamp" | tee "$tmp/label-delete.json"
jq -e '.success == true' "$tmp/label-delete.json" >/dev/null
echo
echo "smoke: muster project-status list, then a status created, updated, archived and unarchived (ADR 0014)"
"$bin" project-status list | jq -c '{name, type, position}' | tee "$tmp/statuses.jsonl"
jq -s -e 'length >= 1' "$tmp/statuses.jsonl" >/dev/null
# A project status's name is at most 25 characters (Linear's validation, read on
# 2026-11-03), so the status is named after the run's time of day.
short="${stamp:11:8}"
"$bin" project-status create --name "Smoke $short" --type planned --color "#f2c94c" --position 99 --description "Made by scripts/smoke.sh at $stamp and archived by the same run; a second planned status, since Linear keeps the last of a type." | jq -c '{name, type, position, indefinite}' | tee "$tmp/status-create.json"
jq -e --arg n "Smoke $short" '.name == $n and .type == "planned"' "$tmp/status-create.json" >/dev/null
"$bin" project-status update "Smoke $short" --name "Smoky $short" --indefinite true | jq -c '{name, indefinite}' | tee "$tmp/status-update.json"
jq -e --arg n "Smoky $short" '.name == $n' "$tmp/status-update.json" >/dev/null
"$bin" project-status archive "Smoky $short" | jq -c '{name, type}' | tee "$tmp/status-archive.json"
jq -e --arg n "Smoky $short" '.name == $n' "$tmp/status-archive.json" >/dev/null
"$bin" project-status unarchive "Smoky $short" | jq -c '{name}' | tee "$tmp/status-unarchive.json"
jq -e --arg n "Smoky $short" '.name == $n' "$tmp/status-unarchive.json" >/dev/null
"$bin" project-status archive "Smoky $short" | jq -c '{name}' >/dev/null
echo
echo "smoke: muster project get 'Smoke tests', then a throwaway project updated, deleted, unarchived and deleted again (ADR 0014)"
"$bin" project get "Smoke tests" | jq -c '{name, status: .status.name, members: (.members.nodes | length), milestones: (.projectMilestones.nodes | length), trashed}' | tee "$tmp/project-get.json"
jq -e '.name == "Smoke tests"' "$tmp/project-get.json" >/dev/null
"$bin" project create --name "Smoke project $stamp" --description "A throwaway project made by scripts/smoke.sh at $stamp; it ends in the trash." | jq -c '{name}' >/dev/null
"$bin" project update "Smoke project $stamp" --status "In Progress" --lead me --start-date "${stamp%%T*}" --priority 3 --add-member me | jq -c '{name, status: .status.name, lead: .lead.displayName, startDate, priority}' | tee "$tmp/project-update.json"
jq -e '.status == "In Progress" and .priority == 3' "$tmp/project-update.json" >/dev/null
"$bin" project get "Smoke project $stamp" | jq -c '{name, members: [.members.nodes[].displayName]}' | tee "$tmp/project-members.json"
jq -e '(.members | length) == 1' "$tmp/project-members.json" >/dev/null
"$bin" project delete "Smoke project $stamp" | jq -c '{name, trashed}' >/dev/null
"$bin" project list | jq -c 'select(.name == "Smoke project '"$stamp"'")' | tee "$tmp/project-gone.jsonl"
[ ! -s "$tmp/project-gone.jsonl" ] || { echo "smoke: the trashed project is still listed" >&2; exit 1; }
"$bin" project unarchive "Smoke project $stamp" | jq -c '{name}' | tee "$tmp/project-back.json"
jq -e --arg n "Smoke project $stamp" '.name == $n' "$tmp/project-back.json" >/dev/null
"$bin" project delete "Smoke project $stamp" | jq -c '{name}' >/dev/null
echo
echo "smoke: muster issue create --project 'Smoke tests'"
"$bin" issue create --title "Smoke test $stamp" --project "Smoke tests" --description "Created by scripts/smoke.sh at $stamp. It is canceled by the same run." | tee "$tmp/create.json"
jq -e '.project.name == "Smoke tests"' "$tmp/create.json" >/dev/null
id=$(jq -r .identifier "$tmp/create.json")
echo
echo "smoke: muster issue comment $id"
"$bin" issue comment "$id" --body "Read back by the smoke test." | tee "$tmp/comment.json"
echo
echo "smoke: muster issue get $id"
"$bin" issue get "$id" | jq '{identifier, title, state: .state.name, comments: [.comments.nodes[].body]}'
echo
echo "smoke: muster issue create --parent $id"
"$bin" issue create --title "Smoke test sub-issue $stamp" --parent "$id" --description "A sub-issue of $id, canceled by the same run." | tee "$tmp/child.json"
child=$(jq -r .identifier "$tmp/child.json")
jq -e '.parent.identifier == "'"$id"'"' "$tmp/child.json" >/dev/null
echo
echo "smoke: muster issue get $id (children)"
"$bin" issue get "$id" | jq -c '{identifier, children: [.children.nodes[] | {identifier, state: .state.name}]}' | tee "$tmp/children.json"
jq -e '.children[0].identifier == "'"$child"'"' "$tmp/children.json" >/dev/null
echo
echo "smoke: muster issue attach $id"
"$bin" issue attach "$id" --url "https://github.com/appshapes/muster/blob/master/scripts/smoke.sh" --title "The smoke script" --subtitle "attached at $stamp" | tee "$tmp/attach.json"
jq -e '.issue.identifier == "'"$id"'" and .title == "The smoke script"' "$tmp/attach.json" >/dev/null
echo
echo "smoke: muster issue attach $id, the same URL again"
"$bin" issue attach "$id" --url "https://github.com/appshapes/muster/blob/master/scripts/smoke.sh" --title "The smoke script, renamed" | jq -c '{id, title, subtitle}' | tee "$tmp/attach2.json"
jq -e --slurpfile first "$tmp/attach.json" '.id == $first[0].id and .title == "The smoke script, renamed" and .subtitle == $first[0].subtitle' "$tmp/attach2.json" >/dev/null
echo
echo "smoke: muster issue get $id (attachments)"
"$bin" issue get "$id" | jq -c '{identifier, attachments: [.attachments.nodes[] | {title, url}]}' | tee "$tmp/attachments.json"
jq -e '(.attachments | length) == 1 and .attachments[0].title == "The smoke script, renamed"' "$tmp/attachments.json" >/dev/null
echo
echo "smoke: muster issue list --project 'Smoke tests'"
"$bin" issue list --project "Smoke tests" | jq -c '{identifier, project: .project.name}' | tee "$tmp/in-project.jsonl"
jq -s -e --arg id "$id" 'map(.identifier) | index($id) != null' "$tmp/in-project.jsonl" >/dev/null
echo
echo "smoke: muster issue list --parent $id"
"$bin" issue list --parent "$id" | jq -c '{identifier, parent: .parent.identifier}'
echo
echo "smoke: muster issue update $child --parent none"
"$bin" issue update "$child" --parent none | jq -c '{identifier, parent}' | tee "$tmp/orphan.json"
jq -e '.parent == null' "$tmp/orphan.json" >/dev/null
echo
echo "smoke: muster issue close $child --cancel"
"$bin" issue close "$child" --cancel --comment "Smoke test sub-issue done at $stamp; canceled so that it never counts as work." | jq -c 'if .identifier then {identifier, state: .state.name} else {comment: .body} end'
echo
echo "smoke: muster issue close $id --cancel"
"$bin" issue close "$id" --cancel --comment "Smoke test done at $stamp; canceled so that it never counts as work." | tee "$tmp/close.jsonl"
jq -s -e '.[1].state.type == "canceled"' "$tmp/close.jsonl" >/dev/null
echo
echo "smoke: ok ($id and $child canceled)"
